use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write, BufRead};
use std::process::{Child, Command, Stdio};

use regex::Regex;

use crate::builtins::{self, format_printf, string_replace};
use crate::input::Record;
use crate::parser::{BinOp, Block, Expr, FuncDef, Pattern, Program, Redirect, Statement};
use crate::runtime::{Runtime, Value};

/// Signal used to propagate control flow out of blocks.
enum Signal {
    Return(Value),
    Break,
    Continue,
    Exit(i32),
}

const MAX_CALL_DEPTH: usize = 200;

pub struct Executor<'a> {
    program: &'a Program,
    rt: &'a mut Runtime,
    functions: HashMap<String, FuncDef>,
    range_active: Vec<bool>,
    output_files: HashMap<String, File>,
    output_pipes: HashMap<String, Child>,
    stdout: BufWriter<io::Stdout>,
    call_depth: usize,
    next_record: bool,
    next_file: bool,
    exit_code: Option<i32>,
    regex_cache: HashMap<String, Regex>,
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
    fn ensure_regex(&mut self, pattern: &str) -> bool {
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
    fn regex_is_match(&mut self, pattern: &str, text: &str) -> bool {
        self.ensure_regex(pattern) && self.regex_cache[pattern].is_match(text)
    }

    /// Find first match of a cached regex in text.
    fn regex_find(&mut self, pattern: &str, text: &str) -> Option<(usize, usize)> {
        if !self.ensure_regex(pattern) { return None; }
        self.regex_cache[pattern].find(text).map(|m| (m.start(), m.end()))
    }

    /// Populate the HDR array from a header record (used with `-H`).
    pub fn set_header(&mut self, fields: &[String]) {
        for (i, name) in fields.iter().enumerate() {
            let key = (i + 1).to_string();
            self.rt.set_array("HDR", &key, name);
            self.rt.set_array("HDR", name, &key);
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
        let rules = &self.program.rules;
        let pattern = &rules[rule_idx].pattern;
        match pattern {
            None => true,
            Some(Pattern::Regex(pat)) => {
                self.regex_is_match(pat, line)
            }
            Some(Pattern::Expression(expr)) => {
                let expr = expr.clone();
                self.eval_expr(&expr).is_truthy()
            }
            Some(Pattern::Range(start, end)) => {
                let start = start.as_ref().clone();
                let end = end.as_ref().clone();
                if self.range_active[rule_idx] {
                    if self.match_single_pattern(&end, line) {
                        self.range_active[rule_idx] = false;
                    }
                    true
                } else if self.match_single_pattern(&start, line) {
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
                let expr = expr.clone();
                self.eval_expr(&expr).is_truthy()
            }
            Pattern::Range(_, _) => false,
        }
    }

    fn exec_block(&mut self, block: &Block) -> Option<Signal> {
        for stmt in block {
            if let Some(signal) = self.exec_stmt(stmt) {
                return Some(signal);
            }
        }
        None
    }

    fn exec_stmt(&mut self, stmt: &Statement) -> Option<Signal> {
        match stmt {
            Statement::Print(exprs, redir) => {
                if redir.is_none() {
                    let ofs = self.rt.ofs().to_owned();
                    let ors = self.rt.ors().to_owned();
                    for (i, e) in exprs.iter().enumerate() {
                        if i > 0 {
                            let _ = self.stdout.write_all(ofs.as_bytes());
                        }
                        let val = self.eval_expr(e);
                        val.write_to(&mut self.stdout);
                    }
                    let _ = self.stdout.write_all(ors.as_bytes());
                } else {
                    let ofs = self.rt.ofs().to_owned();
                    let ors = self.rt.ors().to_owned();
                    let parts: Vec<String> = exprs.iter().map(|e| self.eval_string(e)).collect();
                    let mut output = parts.join(&ofs);
                    output.push_str(&ors);
                    self.write_output(&output, redir);
                }
            }
            Statement::Printf(exprs, redir) => {
                if exprs.is_empty() {
                    return None;
                }
                let args: Vec<String> = exprs.iter().map(|e| self.eval_string(e)).collect();
                let output = format_printf(&args[0], &args[1..]);
                if redir.is_none() {
                    let _ = self.stdout.write_all(output.as_bytes());
                } else {
                    self.write_output(&output, redir);
                }
            }
            Statement::If(cond, then_block, else_block) => {
                let val = self.eval_expr(cond);
                if val.is_truthy() {
                    if let Some(signal) = self.exec_block(then_block) {
                        return Some(signal);
                    }
                } else if let Some(eb) = else_block
                    && let Some(signal) = self.exec_block(eb) {
                        return Some(signal);
                    }
            }
            Statement::While(cond, body) => {
                loop {
                    if !self.eval_expr(cond).is_truthy() {
                        break;
                    }
                    match self.exec_block(body) {
                        Some(Signal::Break) => break,
                        Some(Signal::Continue) => continue,
                        Some(signal) => return Some(signal),
                        None => {}
                    }
                }
            }
            Statement::DoWhile(body, cond) => {
                loop {
                    match self.exec_block(body) {
                        Some(Signal::Break) => break,
                        Some(Signal::Continue) => {}
                        Some(signal) => return Some(signal),
                        None => {}
                    }
                    if !self.eval_expr(cond).is_truthy() {
                        break;
                    }
                }
            }
            Statement::For(init, cond, update, body) => {
                if let Some(init_stmt) = init
                    && let Some(signal) = self.exec_stmt(init_stmt) {
                        match signal {
                            Signal::Return(_) | Signal::Exit(_) => return Some(signal),
                            _ => {}
                        }
                    }
                loop {
                    if let Some(cond_expr) = cond
                        && !self.eval_expr(cond_expr).is_truthy() {
                            break;
                    }
                    match self.exec_block(body) {
                        Some(Signal::Break) => break,
                        Some(Signal::Continue) => {}
                        Some(signal) => return Some(signal),
                        None => {}
                    }
                    if let Some(update_stmt) = update
                        && let Some(signal) = self.exec_stmt(update_stmt) {
                            match signal {
                                Signal::Return(_) | Signal::Exit(_) => return Some(signal),
                                _ => {}
                            }
                        }
                }
            }
            Statement::ForIn(var, array, body) => {
                let keys = self.rt.array_keys(array);
                for key in keys {
                    self.rt.set_var(var, &key);
                    match self.exec_block(body) {
                        Some(Signal::Break) => break,
                        Some(Signal::Continue) => continue,
                        Some(signal) => return Some(signal),
                        None => {}
                    }
                }
            }
            Statement::Delete(array, key_expr) => {
                let key = self.eval_string(key_expr);
                self.rt.delete_array(array, &key);
            }
            Statement::DeleteAll(array) => {
                self.rt.delete_array_all(array);
            }
            Statement::Next => {
                self.next_record = true;
                return None;
            }
            Statement::Nextfile => {
                self.next_file = true;
                return None;
            }
            Statement::Break => return Some(Signal::Break),
            Statement::Continue => return Some(Signal::Continue),
            Statement::Exit(expr) => {
                let code = match expr {
                    Some(e) => self.eval_expr(e).to_number() as i32,
                    None => 0,
                };
                return Some(Signal::Exit(code));
            }
            Statement::Return(expr) => {
                let val = match expr {
                    Some(e) => self.eval_expr(e),
                    None => Value::default(),
                };
                return Some(Signal::Return(val));
            }
            Statement::Block(block) => {
                if let Some(signal) = self.exec_block(block) {
                    return Some(signal);
                }
            }
            Statement::Expression(expr) => {
                self.eval_expr(expr);
            }
        }
        None
    }

    fn eval_expr(&mut self, expr: &Expr) -> Value {
        match expr {
            Expr::NumberLit(n) => Value::from_number(*n),
            Expr::StringLit(s) => Value::from_string(s.clone()),
            Expr::Var(name) => self.rt.get_value(name),
            Expr::Field(idx_expr) => {
                let n = if let Expr::NumberLit(n) = idx_expr.as_ref() {
                    *n
                } else {
                    self.eval_expr(idx_expr).to_number()
                };
                Value::from_string(self.rt.get_field(self.resolve_field_idx(n)))
            }
            Expr::ArrayRef(name, key_expr) => {
                let key = self.eval_expr(key_expr).into_string();
                self.rt.get_array_value(name, &key)
            }
            Expr::ArrayIn(key, array) => {
                bool_val(self.rt.array_has_key(array, key))
            }
            Expr::BinOp(left, op, right) => {
                let l = self.eval_expr(left);
                let r = self.eval_expr(right);
                eval_binop(l, op, r)
            }
            Expr::LogicalAnd(left, right) => {
                let l = self.eval_expr(left);
                if !l.is_truthy() {
                    return Value::from_number(0.0);
                }
                let r = self.eval_expr(right);
                bool_val(r.is_truthy())
            }
            Expr::LogicalOr(left, right) => {
                let l = self.eval_expr(left);
                if l.is_truthy() {
                    return Value::from_number(1.0);
                }
                let r = self.eval_expr(right);
                bool_val(r.is_truthy())
            }
            Expr::LogicalNot(inner) => {
                let val = self.eval_expr(inner);
                bool_val(!val.is_truthy())
            }
            Expr::Match(expr, pat_expr) => {
                let val = self.eval_expr(expr).into_string();
                let pat = self.eval_expr(pat_expr).into_string();
                bool_val(self.regex_is_match(&pat, &val))
            }
            Expr::NotMatch(expr, pat_expr) => {
                let val = self.eval_expr(expr).into_string();
                let pat = self.eval_expr(pat_expr).into_string();
                bool_val(!self.regex_is_match(&pat, &val))
            }
            Expr::Concat(left, right) => {
                let mut l = self.eval_expr(left).into_string();
                let r = self.eval_expr(right);
                r.write_to_string(&mut l);
                Value::from_string(l)
            }
            Expr::Assign(target, value) => {
                let val = self.eval_expr(value);
                self.assign_to(target, val.clone());
                val
            }
            Expr::CompoundAssign(target, op, value) => {
                let current = self.eval_lvalue(target);
                let rhs = self.eval_expr(value);
                let result = eval_binop(current, op, rhs);
                self.assign_to(target, result.clone());
                result
            }
            Expr::Increment(target, pre) => {
                let current = self.eval_lvalue(target);
                let n = current.to_number();
                let new_val = Value::from_number(n + 1.0);
                self.assign_to(target, new_val.clone());
                if *pre { new_val } else { Value::from_number(n) }
            }
            Expr::Decrement(target, pre) => {
                let current = self.eval_lvalue(target);
                let n = current.to_number();
                let new_val = Value::from_number(n - 1.0);
                self.assign_to(target, new_val.clone());
                if *pre { new_val } else { Value::from_number(n) }
            }
            Expr::UnaryMinus(inner) => {
                let val = self.eval_expr(inner);
                Value::from_number(-val.to_number())
            }
            Expr::Ternary(cond, then_expr, else_expr) => {
                let val = self.eval_expr(cond);
                if val.is_truthy() {
                    self.eval_expr(then_expr)
                } else {
                    self.eval_expr(else_expr)
                }
            }
            Expr::Sprintf(args) => {
                let evaled: Vec<String> = args.iter().map(|e| self.eval_string(e)).collect();
                if evaled.is_empty() {
                    Value::default()
                } else {
                    Value::from_string(format_printf(&evaled[0], &evaled[1..]))
                }
            }
            Expr::FuncCall(name, args) => {
                match name.as_str() {
                    "sub" => return self.builtin_sub(args, false),
                    "gsub" => return self.builtin_sub(args, true),
                    "match" => return self.builtin_match(args),
                    "split" => return self.builtin_split(args),
                    "jpath" if args.len() >= 3 => return self.builtin_jpath_extract(args),
                    "length" if args.len() == 1 => {
                        if let Expr::Var(var_name) = &args[0]
                            && self.rt.arrays.contains_key(var_name.as_str()) {
                                return Value::from_number(self.rt.array_len(var_name) as f64);
                            }
                    }
                    "close" => return self.builtin_close(args),
                    "gensub" => return self.builtin_gensub(args),
                    "fflush" => return self.builtin_fflush(args),
                    "system" => return self.builtin_system(args),
                    _ => {}
                }
                let evaled: Vec<String> = args.iter().map(|e| self.eval_string(e)).collect();
                if let Some(func) = self.functions.get(name).cloned() {
                    self.call_user_func(&func, &evaled)
                } else {
                    Value::from_string(builtins::call_builtin(name, &evaled))
                }
            }
            Expr::Getline(var, source) => {
                self.exec_getline(var.as_deref(), source.as_deref())
            }
            Expr::GetlinePipe(cmd_expr, var) => {
                let cmd = self.eval_string(cmd_expr);
                self.exec_getline_pipe(&cmd, var.as_deref())
            }
        }
    }

    /// Evaluate an expression and return its string representation.
    fn eval_string(&mut self, expr: &Expr) -> String {
        self.eval_expr(expr).into_string()
    }

    /// Resolve a field index, supporting negative values ($-1 = last field).
    fn resolve_field_idx(&self, n: f64) -> usize {
        let i = n as isize;
        if i >= 0 {
            i as usize
        } else {
            let nf = self.rt.fields.len() as isize;
            let resolved = nf + 1 + i; // -1 → nf, -2 → nf-1, etc.
            if resolved < 0 { 0 } else { resolved as usize }
        }
    }

    fn eval_lvalue(&mut self, expr: &Expr) -> Value {
        match expr {
            Expr::Var(name) => self.rt.get_value(name),
            Expr::ArrayRef(name, key_expr) => {
                let key = self.eval_string(key_expr);
                self.rt.get_array_value(name, &key)
            }
            Expr::Field(idx_expr) => {
                let n = if let Expr::NumberLit(n) = idx_expr.as_ref() {
                    *n
                } else {
                    self.eval_expr(idx_expr).to_number()
                };
                Value::from_string(self.rt.get_field(self.resolve_field_idx(n)))
            }
            _ => Value::default(),
        }
    }

    fn assign_to(&mut self, target: &Expr, value: Value) {
        match target {
            Expr::Var(name) => self.rt.set_value(name, value),
            Expr::ArrayRef(name, key_expr) => {
                let key = self.eval_string(key_expr);
                self.rt.set_array_value(name, &key, value);
            }
            Expr::Field(idx_expr) => {
                let n = if let Expr::NumberLit(n) = idx_expr.as_ref() {
                    *n
                } else {
                    self.eval_expr(idx_expr).to_number()
                };
                self.rt.set_field(self.resolve_field_idx(n), &value.into_string());
            }
            _ => {}
        }
    }

    fn write_output(&mut self, text: &str, redir: &Option<Redirect>) {
        match redir {
            None => {
                let _ = self.stdout.write_all(text.as_bytes());
            }
            Some(Redirect::Overwrite(target_expr)) | Some(Redirect::Append(target_expr)) => {
                let path = self.eval_string(target_expr);
                if path == "/dev/stderr" {
                    eprint!("{}", text);
                    return;
                }
                if path == "/dev/stdout" {
                    let _ = self.stdout.write_all(text.as_bytes());
                    return;
                }
                let is_append = matches!(redir, Some(Redirect::Append(_)));
                let file = self.output_files.entry(path.clone()).or_insert_with(|| {
                    if is_append {
                        OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&path)
                            .unwrap_or_else(|e| {
                                eprintln!("fk: cannot open '{}': {}", path, e);
                                File::create("/dev/null").unwrap()
                            })
                    } else {
                        File::create(&path).unwrap_or_else(|e| {
                            eprintln!("fk: cannot open '{}': {}", path, e);
                            File::create("/dev/null").unwrap()
                        })
                    }
                });
                let _ = file.write_all(text.as_bytes());
            }
            Some(Redirect::Pipe(cmd_expr)) => {
                let cmd = self.eval_string(cmd_expr);
                let child = self.output_pipes.entry(cmd.clone()).or_insert_with(|| {
                    Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .stdin(Stdio::piped())
                        .spawn()
                        .unwrap_or_else(|e| {
                            eprintln!("fk: cannot run '{}': {}", cmd, e);
                            Command::new("cat").stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap()
                        })
                });
                if let Some(ref mut stdin) = child.stdin {
                    let _ = stdin.write_all(text.as_bytes());
                }
            }
        }
    }

    fn call_user_func(&mut self, func: &FuncDef, args: &[String]) -> Value {
        if self.call_depth >= MAX_CALL_DEPTH {
            eprintln!("fk: maximum call depth ({}) exceeded", MAX_CALL_DEPTH);
            return Value::default();
        }
        self.call_depth += 1;

        let mut saved: Vec<(String, bool, Value)> = Vec::new();
        for param in &func.params {
            let existed = self.rt.has_var(param);
            let old = self.rt.get_value(param);
            saved.push((param.clone(), existed, old));
        }

        for (i, param) in func.params.iter().enumerate() {
            let val = args.get(i).map(|s| s.as_str()).unwrap_or("");
            self.rt.set_var(param, val);
        }

        let result = match self.exec_block(&func.body) {
            Some(Signal::Return(v)) => v,
            Some(Signal::Exit(code)) => {
                self.exit_code = Some(code);
                Value::default()
            }
            _ => Value::default(),
        };

        for (name, existed, old_val) in saved {
            if existed {
                self.rt.set_value(&name, old_val);
            } else {
                self.rt.remove_var(&name);
            }
        }

        self.call_depth -= 1;
        result
    }

    /// sub/gsub: these need runtime access to modify lvalues.
    fn builtin_sub(&mut self, args: &[Expr], global: bool) -> Value {
        if args.len() < 2 {
            eprintln!("fk: sub/gsub requires at least 2 arguments");
            return Value::from_number(0.0);
        }
        let pattern = self.eval_string(&args[0]);
        let replacement = self.eval_string(&args[1]);

        let target_expr = if args.len() >= 3 {
            args[2].clone()
        } else {
            Expr::Field(Box::new(Expr::NumberLit(0.0)))
        };

        let target_val = self.eval_lvalue(&target_expr).into_string();
        let (new_val, count) = string_replace(&target_val, &pattern, &replacement, global);
        self.assign_to(&target_expr, Value::from_string(new_val));

        Value::from_number(count as f64)
    }

    /// match(string, regex) — sets RSTART and RLENGTH.
    fn builtin_match(&mut self, args: &[Expr]) -> Value {
        if args.len() < 2 {
            eprintln!("fk: match requires 2 arguments");
            return Value::from_number(0.0);
        }
        let s = self.eval_string(&args[0]);
        let pattern = self.eval_string(&args[1]);

        let matched = self.regex_find(&pattern, &s);
        if let Some((start, end)) = matched {
            let rstart = s[..start].chars().count() as f64 + 1.0;
            let rlength = s[start..end].chars().count() as f64;
            self.rt.set_value("RSTART", Value::from_number(rstart));
            self.rt.set_value("RLENGTH", Value::from_number(rlength));
            Value::from_number(rstart)
        } else {
            self.rt.set_value("RSTART", Value::from_number(0.0));
            self.rt.set_value("RLENGTH", Value::from_number(-1.0));
            Value::from_number(0.0)
        }
    }

    /// jpath(json, path, array) — extract JSON value into an awk array.
    fn builtin_jpath_extract(&mut self, args: &[Expr]) -> Value {
        let json_str = self.eval_string(&args[0]);
        let path = self.eval_string(&args[1]);
        let array_name = match &args[2] {
            Expr::Var(name) => name.clone(),
            _ => {
                eprintln!("fk: jpath: third argument must be an array name");
                return Value::from_number(0.0);
            }
        };
        let pairs = builtins::json::extract(&json_str, &path);
        self.rt.arrays.remove(&array_name);
        for (key, val) in &pairs {
            self.rt.set_array(&array_name, key, val);
        }
        Value::from_number(pairs.len() as f64)
    }

    /// split(string, array [, separator]) — returns element count.
    fn builtin_split(&mut self, args: &[Expr]) -> Value {
        if args.len() < 2 {
            eprintln!("fk: split requires at least 2 arguments");
            return Value::from_number(0.0);
        }
        let s = self.eval_string(&args[0]);
        let array_name = match &args[1] {
            Expr::Var(name) => name.clone(),
            _ => {
                eprintln!("fk: split: second argument must be an array name");
                return Value::from_number(0.0);
            }
        };
        let fs = if args.len() >= 3 {
            self.eval_string(&args[2])
        } else {
            self.rt.get_var("FS")
        };

        let parts = crate::field::split(&s, &fs);
        self.rt.arrays.remove(&array_name);
        for (i, part) in parts.iter().enumerate() {
            self.rt.set_array(&array_name, &format!("{}", i + 1), part);
        }
        Value::from_number(parts.len() as f64)
    }

    /// fflush([file]) — flush stdout or a named output file. Returns 0 on success.
    fn builtin_fflush(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            let _ = self.stdout.flush();
        } else {
            let path = self.eval_string(&args[0]);
            if path.is_empty() {
                let _ = self.stdout.flush();
            } else if let Some(file) = self.output_files.get_mut(&path) {
                let _ = file.flush();
            }
        }
        Value::from_number(0.0)
    }

    /// system(cmd) — run a shell command, return its exit status.
    fn builtin_system(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            return Value::from_number(-1.0);
        }
        let _ = self.stdout.flush();
        let cmd = self.eval_string(&args[0]);
        match Command::new("sh").arg("-c").arg(&cmd).status() {
            Ok(status) => Value::from_number(status.code().unwrap_or(-1) as f64),
            Err(_) => Value::from_number(-1.0),
        }
    }

    /// close(name) — close an output file or pipe by name.
    fn builtin_close(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            return Value::from_number(-1.0);
        }
        let name = self.eval_string(&args[0]);
        if let Some(file) = self.output_files.remove(&name) {
            drop(file);
            return Value::from_number(0.0);
        }
        if let Some(mut child) = self.output_pipes.remove(&name) {
            drop(child.stdin.take());
            let _ = child.wait();
            return Value::from_number(0.0);
        }
        Value::from_number(-1.0)
    }

    /// gensub(regex, replacement, how [, target]) — like gsub but returns result.
    fn builtin_gensub(&mut self, args: &[Expr]) -> Value {
        if args.len() < 3 {
            eprintln!("fk: gensub requires at least 3 arguments");
            return Value::default();
        }
        let pattern = self.eval_string(&args[0]);
        let replacement = self.eval_string(&args[1]);
        let how = self.eval_string(&args[2]);

        let target = if args.len() >= 4 {
            self.eval_string(&args[3])
        } else {
            self.rt.get_field(0)
        };

        if !self.ensure_regex(&pattern) {
            return Value::from_string(target);
        }

        let global = how.starts_with('g') || how.starts_with('G');
        let re = &self.regex_cache[&pattern];
        if global {
            Value::from_string(re.replace_all(&target, replacement.as_str()).to_string())
        } else {
            let n: usize = how.parse().unwrap_or(1);
            if n == 0 {
                return Value::from_string(target);
            }
            let mut count = 0usize;
            for m in re.find_iter(&target) {
                count += 1;
                if count == n {
                    let mut result = String::with_capacity(target.len());
                    result.push_str(&target[..m.start()]);
                    result.push_str(&replacement);
                    result.push_str(&target[m.end()..]);
                    return Value::from_string(result);
                }
            }
            Value::from_string(target)
        }
    }

    fn exec_getline(&mut self, var: Option<&str>, source: Option<&Expr>) -> Value {
        let line = if let Some(src_expr) = source {
            let path = self.eval_string(src_expr);
            match std::fs::File::open(&path) {
                Ok(file) => {
                    let mut reader = std::io::BufReader::new(file);
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => return Value::from_number(0.0),
                        Ok(_) => {
                            if line.ends_with('\n') { line.pop(); }
                            if line.ends_with('\r') { line.pop(); }
                            line
                        }
                        Err(_) => return Value::from_number(-1.0),
                    }
                }
                Err(_) => return Value::from_number(-1.0),
            }
        } else {
            let mut line = String::new();
            match std::io::stdin().read_line(&mut line) {
                Ok(0) => return Value::from_number(0.0),
                Ok(_) => {
                    if line.ends_with('\n') { line.pop(); }
                    if line.ends_with('\r') { line.pop(); }
                    line
                }
                Err(_) => return Value::from_number(-1.0),
            }
        };

        match var {
            Some(name) => {
                self.rt.set_var(name, &line);
            }
            None => {
                self.rt.set_record(&line);
            }
        }
        self.rt.increment_nr();
        Value::from_number(1.0)
    }

    fn exec_getline_pipe(&mut self, cmd: &str, var: Option<&str>) -> Value {
        match Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => {
                let output = child.wait_with_output();
                match output {
                    Ok(out) => {
                        let text = String::from_utf8_lossy(&out.stdout);
                        let line = text.lines().next().unwrap_or("").to_string();
                        match var {
                            Some(name) => self.rt.set_var(name, &line),
                            None => self.rt.set_record(&line),
                        }
                        self.rt.increment_nr();
                        Value::from_number(1.0)
                    }
                    Err(_) => Value::from_number(-1.0),
                }
            }
            Err(_) => Value::from_number(-1.0),
        }
    }
}

// --- value helpers (used by the executor, kept here since they're tightly coupled) ---

fn bool_val(b: bool) -> Value {
    Value::from_number(if b { 1.0 } else { 0.0 })
}

fn compare_values(left: &Value, right: &Value) -> std::cmp::Ordering {
    if left.is_numeric() && right.is_numeric() {
        return left.to_number().partial_cmp(&right.to_number())
            .unwrap_or(std::cmp::Ordering::Equal);
    }
    if left.looks_numeric() && right.looks_numeric() {
        left.to_number().partial_cmp(&right.to_number())
            .unwrap_or(std::cmp::Ordering::Equal)
    } else {
        left.to_string_val().cmp(&right.to_string_val())
    }
}

fn eval_binop(left: Value, op: &BinOp, right: Value) -> Value {
    match op {
        BinOp::Add => Value::from_number(left.to_number() + right.to_number()),
        BinOp::Sub => Value::from_number(left.to_number() - right.to_number()),
        BinOp::Mul => Value::from_number(left.to_number() * right.to_number()),
        BinOp::Pow => Value::from_number(left.to_number().powf(right.to_number())),
        BinOp::Div => {
            let r = right.to_number();
            if r == 0.0 {
                eprintln!("fk: division by zero");
                Value::from_number(0.0)
            } else {
                Value::from_number(left.to_number() / r)
            }
        }
        BinOp::Mod => {
            let r = right.to_number();
            if r == 0.0 {
                eprintln!("fk: division by zero");
                Value::from_number(0.0)
            } else {
                Value::from_number(left.to_number() % r)
            }
        }
        BinOp::Eq => {
            if left.looks_numeric() && right.looks_numeric() {
                bool_val(left.to_number() == right.to_number())
            } else {
                bool_val(left.to_string_val() == right.to_string_val())
            }
        }
        BinOp::Ne => {
            if left.looks_numeric() && right.looks_numeric() {
                bool_val(left.to_number() != right.to_number())
            } else {
                bool_val(left.to_string_val() != right.to_string_val())
            }
        }
        BinOp::Lt => bool_val(compare_values(&left, &right) == std::cmp::Ordering::Less),
        BinOp::Le => bool_val(compare_values(&left, &right) != std::cmp::Ordering::Greater),
        BinOp::Gt => bool_val(compare_values(&left, &right) == std::cmp::Ordering::Greater),
        BinOp::Ge => bool_val(compare_values(&left, &right) != std::cmp::Ordering::Less),
    }
}
