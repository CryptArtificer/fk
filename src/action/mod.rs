mod builtins_rt;
mod eval;
mod stmt;

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::process::Child;

use regex::Regex;

use crate::input::Record;
use crate::parser::{FuncDef, Pattern, Program};
use crate::runtime::{Runtime, Value};

/// Signal used to propagate control flow out of blocks.
pub(crate) enum Signal {
    Return(Value),
    Break,
    Continue,
    Exit(i32),
}

pub(crate) const MAX_CALL_DEPTH: usize = 200;

/// Compute the p-th percentile from a *sorted* slice using linear interpolation.
pub(crate) fn percentile_sorted(sorted: &[f64], pct: f64) -> f64 {
    let n = sorted.len();
    if n == 0 { return 0.0; }
    if n == 1 { return sorted[0]; }
    let pct = pct.clamp(0.0, 100.0);
    let rank = (pct / 100.0) * (n as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi || hi >= n {
        sorted[lo.min(n - 1)]
    } else {
        let frac = rank - lo as f64;
        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
    }
}

pub struct Executor<'a> {
    pub(crate) program: &'a Program,
    pub(crate) rt: &'a mut Runtime,
    pub(crate) functions: HashMap<String, FuncDef>,
    pub(crate) range_active: Vec<bool>,
    pub(crate) output_files: HashMap<String, File>,
    pub(crate) output_pipes: HashMap<String, Child>,
    pub(crate) stdout: BufWriter<io::Stdout>,
    pub(crate) call_depth: usize,
    pub(crate) next_record: bool,
    pub(crate) next_file: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) regex_cache: HashMap<String, Regex>,
}

impl<'a> Executor<'a> {
    pub fn new(program: &'a Program, rt: &'a mut Runtime) -> Self {
        let mut functions = HashMap::new();
        for f in &program.functions {
            functions.insert(f.name.clone(), f.clone());
        }
        let range_active = vec![false; program.rules.len()];
        Executor {
            program, rt, functions, range_active,
            output_files: HashMap::new(),
            output_pipes: HashMap::new(),
            stdout: BufWriter::new(io::stdout()),
            call_depth: 0,
            next_record: false,
            next_file: false,
            exit_code: None,
            regex_cache: HashMap::new(),
        }
    }

    /// Ensure a regex is compiled and cached. Returns false if invalid.
    pub(crate) fn ensure_regex(&mut self, pattern: &str) -> bool {
        if self.regex_cache.contains_key(pattern) {
            return true;
        }
        match Regex::new(pattern) {
            Ok(re) => { self.regex_cache.insert(pattern.to_string(), re); true }
            Err(_) => {
                eprintln!("fk: invalid regex: {}", pattern);
                false
            }
        }
    }

    /// Test if text matches a cached regex pattern.
    pub(crate) fn regex_is_match(&mut self, pattern: &str, text: &str) -> bool {
        self.ensure_regex(pattern) && self.regex_cache[pattern].is_match(text)
    }

    /// Populate the HDR array from a header record (used with `-H`).
    pub fn set_header(&mut self, fields: &[String]) {
        for (i, name) in fields.iter().enumerate() {
            let idx = i + 1;
            let key = idx.to_string();
            self.rt.set_array("HDR", &key, name);
            self.rt.set_array("HDR", name, &key);
            if is_valid_ident(name) && !is_builtin_var(name) {
                self.rt.set_value(name, Value::from_number(idx as f64));
            }
        }
        self.rt.increment_nr();
    }

    /// Populate header from raw text using FS-based splitting.
    pub fn set_header_from_text(&mut self, text: &str) {
        let fs = self.rt.get_var("FS");
        let fields = crate::field::split(text, &fs);
        self.set_header(&fields);
    }

    /// Read a runtime variable (e.g. RS after BEGIN has run).
    pub fn get_var(&self, name: &str) -> String {
        self.rt.get_var(name)
    }

    /// Set a runtime variable from outside (e.g. FILENAME from main loop).
    pub fn set_var(&mut self, name: &str, value: &str) {
        self.rt.set_var(name, value);
    }

    pub fn increment_fnr(&mut self) {
        self.rt.increment_fnr();
    }

    pub fn reset_fnr(&mut self) {
        self.rt.reset_fnr();
    }

    pub fn run_begin(&mut self) {
        if let Some(ref block) = self.program.begin
            && let Some(Signal::Exit(code)) = self.exec_block(block) {
                self.exit_code = Some(code);
            }
    }

    pub fn run_end(&mut self) {
        if let Some(ref block) = self.program.end {
            self.exec_block(block);
        }
        let _ = self.stdout.flush();
        self.close_outputs();
    }

    /// Returns the exit code if `exit` was called, or None.
    pub fn should_exit(&self) -> Option<i32> {
        self.exit_code
    }

    fn close_outputs(&mut self) {
        for (_, file) in self.output_files.drain() {
            drop(file);
        }
        for (_, mut child) in self.output_pipes.drain() {
            drop(child.stdin.take());
            let _ = child.wait();
        }
    }

    /// Returns true if nextfile was requested during this record.
    pub fn take_next_file(&mut self) -> bool {
        let v = self.next_file;
        self.next_file = false;
        v
    }

    pub fn run_record(&mut self, record: &Record) {
        if self.exit_code.is_some() { return; }
        self.next_record = false;
        self.rt.increment_nr();
        match &record.fields {
            Some(fields) => self.rt.set_record_fields(&record.text, fields.clone()),
            None => self.rt.set_record(&record.text),
        }

        let program = self.program;
        for i in 0..program.rules.len() {
            if self.next_record || self.next_file || self.exit_code.is_some() { break; }
            let matched = self.match_rule(i, &record.text);
            if matched
                && let Some(Signal::Exit(code)) = self.exec_block(&program.rules[i].action) {
                    self.exit_code = Some(code);
                    break;
            }
        }
    }

    fn match_rule(&mut self, rule_idx: usize, line: &str) -> bool {
        // Copy the program reference to sever the borrow through `self`,
        // allowing `&mut self` methods (eval_expr, regex_is_match) to be
        // called while holding pattern references from the program.
        let program = self.program;
        let pattern = &program.rules[rule_idx].pattern;
        match pattern {
            None => true,
            Some(Pattern::Regex(pat)) => {
                self.regex_is_match(pat, line)
            }
            Some(Pattern::Expression(expr)) => {
                self.eval_expr(expr).is_truthy()
            }
            Some(Pattern::Range(start, end)) => {
                if self.range_active[rule_idx] {
                    if self.match_single_pattern(end, line) {
                        self.range_active[rule_idx] = false;
                    }
                    true
                } else if self.match_single_pattern(start, line) {
                    self.range_active[rule_idx] = true;
                    true
                } else {
                    false
                }
            }
        }
    }

    fn match_single_pattern(&mut self, pattern: &Pattern, line: &str) -> bool {
        match pattern {
            Pattern::Regex(pat) => {
                self.regex_is_match(pat, line)
            }
            Pattern::Expression(expr) => {
                self.eval_expr(expr).is_truthy()
            }
            Pattern::Range(_, _) => false,
        }
    }
}

pub(crate) fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(crate) fn is_builtin_var(name: &str) -> bool {
    matches!(name,
        "NR" | "NF" | "FNR" | "FS" | "OFS" | "RS" | "ORS" | "SUBSEP" |
        "OFMT" | "FILENAME" | "RSTART" | "RLENGTH" | "ARGC" | "ARGV" |
        "ENVIRON" | "BEGIN" | "END" | "HDR"
    )
}

pub(crate) fn bool_val(b: bool) -> Value {
    Value::from_number(if b { 1.0 } else { 0.0 })
}
