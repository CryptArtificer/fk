use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::io::BufRead;
use std::process::{Child, Command, Stdio};

use crate::parser::{BinOp, Block, Expr, FuncDef, Pattern, Program, Redirect, Statement};
use crate::runtime::Runtime;

/// Sentinel used to propagate early return from user-defined functions.
struct ReturnValue(String);

/// Execute a parsed program against the runtime.
pub struct Executor<'a> {
    program: &'a Program,
    rt: &'a mut Runtime,
    functions: HashMap<String, FuncDef>,
    range_active: Vec<bool>,
    output_files: HashMap<String, File>,
    output_pipes: HashMap<String, Child>,
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
        }
    }

    pub fn run_begin(&mut self) {
        if let Some(ref block) = self.program.begin {
            self.exec_block(block);
        }
    }

    pub fn run_end(&mut self) {
        if let Some(ref block) = self.program.end {
            self.exec_block(block);
        }
        self.close_outputs();
    }

    /// Flush files and close pipe processes.
    fn close_outputs(&mut self) {
        for (_, file) in self.output_files.drain() {
            drop(file);
        }
        for (_, mut child) in self.output_pipes.drain() {
            // Drop stdin to signal EOF, then wait
            drop(child.stdin.take());
            let _ = child.wait();
        }
    }

    pub fn run_record(&mut self, line: &str) {
        self.rt.increment_nr();
        self.rt.set_record(line);

        for i in 0..self.program.rules.len() {
            let matched = self.match_rule(i, line);
            if matched {
                let action = &self.program.rules[i].action as *const Block;
                // Safety: we only read the action block, executor mutations are to rt
                self.exec_block(unsafe { &*action });
            }
        }
    }

    fn match_rule(&mut self, rule_idx: usize, line: &str) -> bool {
        let pattern = &self.program.rules[rule_idx].pattern;
        match pattern {
            None => true,
            Some(Pattern::Regex(pat)) => {
                line.contains(pat.as_str())
            }
            Some(Pattern::Expression(expr)) => {
                let expr = expr.clone();
                let val = self.eval_expr(&expr);
                is_truthy(&val)
            }
            Some(Pattern::Range(start, end)) => {
                if self.range_active[rule_idx] {
                    // Currently in range — check if end matches
                    if self.match_single_pattern(end, line) {
                        self.range_active[rule_idx] = false;
                    }
                    true
                } else {
                    // Not in range — check if start matches
                    if self.match_single_pattern(start, line) {
                        self.range_active[rule_idx] = true;
                        true
                    } else {
                        false
                    }
                }
            }
        }
    }

    fn match_single_pattern(&mut self, pattern: &Pattern, line: &str) -> bool {
        match pattern {
            Pattern::Regex(pat) => line.contains(pat.as_str()),
            Pattern::Expression(expr) => {
                let val = self.eval_expr(expr);
                is_truthy(&val)
            }
            Pattern::Range(_, _) => false, // nested ranges not supported
        }
    }

    fn exec_block(&mut self, block: &Block) -> Option<ReturnValue> {
        for stmt in block {
            if let Some(rv) = self.exec_stmt(stmt) {
                return Some(rv);
            }
        }
        None
    }

    fn exec_stmt(&mut self, stmt: &Statement) -> Option<ReturnValue> {
        match stmt {
            Statement::Print(exprs, redir) => {
                let ofs = self.rt.get_var("OFS");
                let ors = self.rt.get_var("ORS");
                let parts: Vec<String> = exprs.iter().map(|e| self.eval_expr(e)).collect();
                let output = format!("{}{}", parts.join(&ofs), ors);
                self.write_output(&output, redir);
            }
            Statement::Printf(exprs, redir) => {
                if exprs.is_empty() {
                    return None;
                }
                let args: Vec<String> = exprs.iter().map(|e| self.eval_expr(e)).collect();
                let output = format_printf(&args[0], &args[1..]);
                self.write_output(&output, redir);
            }
            Statement::If(cond, then_block, else_block) => {
                let val = self.eval_expr(cond);
                if is_truthy(&val) {
                    if let Some(rv) = self.exec_block(then_block) {
                        return Some(rv);
                    }
                } else if let Some(eb) = else_block {
                    if let Some(rv) = self.exec_block(eb) {
                        return Some(rv);
                    }
                }
            }
            Statement::While(cond, body) => {
                loop {
                    let val = self.eval_expr(cond);
                    if !is_truthy(&val) {
                        break;
                    }
                    if let Some(rv) = self.exec_block(body) {
                        return Some(rv);
                    }
                }
            }
            Statement::For(init, cond, update, body) => {
                if let Some(init_stmt) = init {
                    if let Some(rv) = self.exec_stmt(init_stmt) {
                        return Some(rv);
                    }
                }
                loop {
                    if let Some(cond_expr) = cond {
                        let val = self.eval_expr(cond_expr);
                        if !is_truthy(&val) {
                            break;
                        }
                    }
                    if let Some(rv) = self.exec_block(body) {
                        return Some(rv);
                    }
                    if let Some(update_stmt) = update {
                        if let Some(rv) = self.exec_stmt(update_stmt) {
                            return Some(rv);
                        }
                    }
                }
            }
            Statement::ForIn(var, array, body) => {
                let keys = self.rt.array_keys(array);
                for key in keys {
                    self.rt.set_var(var, &key);
                    if let Some(rv) = self.exec_block(body) {
                        return Some(rv);
                    }
                }
            }
            Statement::Delete(array, key_expr) => {
                let key = self.eval_expr(key_expr);
                self.rt.delete_array(array, &key);
            }
            Statement::Return(expr) => {
                let val = match expr {
                    Some(e) => self.eval_expr(e),
                    None => String::new(),
                };
                return Some(ReturnValue(val));
            }
            Statement::Block(block) => {
                if let Some(rv) = self.exec_block(block) {
                    return Some(rv);
                }
            }
            Statement::Expression(expr) => {
                self.eval_expr(expr);
            }
        }
        None
    }

    fn eval_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::NumberLit(n) => format_number(*n),
            Expr::StringLit(s) => s.clone(),
            Expr::Var(name) => self.rt.get_var(name),
            Expr::Field(idx_expr) => {
                let idx_str = self.eval_expr(idx_expr);
                let idx: usize = idx_str.parse::<f64>().unwrap_or(0.0) as usize;
                self.rt.get_field(idx)
            }
            Expr::ArrayRef(name, key_expr) => {
                let key = self.eval_expr(key_expr);
                self.rt.get_array(name, &key)
            }
            Expr::ArrayIn(key, array) => {
                let has = self.rt.array_has_key(array, key);
                bool_str(has)
            }
            Expr::BinOp(left, op, right) => {
                let l = self.eval_expr(left);
                let r = self.eval_expr(right);
                eval_binop(&l, op, &r)
            }
            Expr::LogicalAnd(left, right) => {
                let l = self.eval_expr(left);
                if !is_truthy(&l) {
                    return "0".to_string();
                }
                let r = self.eval_expr(right);
                bool_str(is_truthy(&r))
            }
            Expr::LogicalOr(left, right) => {
                let l = self.eval_expr(left);
                if is_truthy(&l) {
                    return "1".to_string();
                }
                let r = self.eval_expr(right);
                bool_str(is_truthy(&r))
            }
            Expr::LogicalNot(inner) => {
                let val = self.eval_expr(inner);
                bool_str(!is_truthy(&val))
            }
            Expr::Match(expr, pat) => {
                let val = self.eval_expr(expr);
                bool_str(val.contains(pat.as_str()))
            }
            Expr::NotMatch(expr, pat) => {
                let val = self.eval_expr(expr);
                bool_str(!val.contains(pat.as_str()))
            }
            Expr::Concat(left, right) => {
                let l = self.eval_expr(left);
                let r = self.eval_expr(right);
                format!("{}{}", l, r)
            }
            Expr::Assign(target, value) => {
                let val = self.eval_expr(value);
                self.assign_to(target, &val);
                val
            }
            Expr::CompoundAssign(target, op, value) => {
                let current = self.eval_lvalue(target);
                let rhs = self.eval_expr(value);
                let result = eval_binop(&current, op, &rhs);
                self.assign_to(target, &result);
                result
            }
            Expr::Increment(target, pre) => {
                let current = self.eval_lvalue(target);
                let n = to_number(&current);
                let new_val = format_number(n + 1.0);
                self.assign_to(target, &new_val);
                if *pre { new_val } else { format_number(n) }
            }
            Expr::Decrement(target, pre) => {
                let current = self.eval_lvalue(target);
                let n = to_number(&current);
                let new_val = format_number(n - 1.0);
                self.assign_to(target, &new_val);
                if *pre { new_val } else { format_number(n) }
            }
            Expr::UnaryMinus(inner) => {
                let val = self.eval_expr(inner);
                let n = to_number(&val);
                format_number(-n)
            }
            Expr::Ternary(cond, then_expr, else_expr) => {
                let val = self.eval_expr(cond);
                if is_truthy(&val) {
                    self.eval_expr(then_expr)
                } else {
                    self.eval_expr(else_expr)
                }
            }
            Expr::Sprintf(args) => {
                let evaled: Vec<String> = args.iter().map(|e| self.eval_expr(e)).collect();
                if evaled.is_empty() {
                    String::new()
                } else {
                    format_printf(&evaled[0], &evaled[1..])
                }
            }
            Expr::FuncCall(name, args) => {
                // Builtins that need runtime access (they modify vars/arrays)
                match name.as_str() {
                    "sub" => return self.builtin_sub(args, false),
                    "gsub" => return self.builtin_sub(args, true),
                    "match" => return self.builtin_match(args),
                    "split" => return self.builtin_split(args),
                    _ => {}
                }
                let evaled: Vec<String> = args.iter().map(|e| self.eval_expr(e)).collect();
                if let Some(func) = self.functions.get(name).cloned() {
                    self.call_user_func(&func, &evaled)
                } else {
                    call_builtin(name, &evaled)
                }
            }
            Expr::Getline(var, source) => {
                self.exec_getline(var.as_deref(), source.as_deref())
            }
            Expr::GetlinePipe(cmd_expr, var) => {
                let cmd = self.eval_expr(cmd_expr);
                self.exec_getline_pipe(&cmd, var.as_deref())
            }
        }
    }

    fn eval_lvalue(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Var(name) => self.rt.get_var(name),
            Expr::ArrayRef(name, key_expr) => {
                let key = self.eval_expr(key_expr);
                self.rt.get_array(name, &key)
            }
            Expr::Field(idx_expr) => {
                let idx_str = self.eval_expr(idx_expr);
                let idx: usize = idx_str.parse::<f64>().unwrap_or(0.0) as usize;
                self.rt.get_field(idx)
            }
            _ => String::new(),
        }
    }

    fn assign_to(&mut self, target: &Expr, value: &str) {
        match target {
            Expr::Var(name) => self.rt.set_var(name, value),
            Expr::ArrayRef(name, key_expr) => {
                let key = self.eval_expr(key_expr);
                self.rt.set_array(name, &key, value);
            }
            Expr::Field(idx_expr) => {
                let idx_str = self.eval_expr(idx_expr);
                let idx: usize = idx_str.parse::<f64>().unwrap_or(0.0) as usize;
                self.rt.set_field(idx, value);
            }
            _ => {}
        }
    }

    fn write_output(&mut self, text: &str, redir: &Option<Redirect>) {
        match redir {
            None => {
                print!("{}", text);
            }
            Some(Redirect::Overwrite(target_expr)) => {
                let path = self.eval_expr(target_expr);
                // Open file (truncate on first use, then append for same path)
                let file = self.output_files.entry(path.clone()).or_insert_with(|| {
                    File::create(&path).unwrap_or_else(|e| {
                        eprintln!("fk: cannot open '{}': {}", path, e);
                        File::create("/dev/null").unwrap()
                    })
                });
                let _ = file.write_all(text.as_bytes());
            }
            Some(Redirect::Append(target_expr)) => {
                let path = self.eval_expr(target_expr);
                let file = self.output_files.entry(path.clone()).or_insert_with(|| {
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&path)
                        .unwrap_or_else(|e| {
                            eprintln!("fk: cannot open '{}': {}", path, e);
                            File::create("/dev/null").unwrap()
                        })
                });
                let _ = file.write_all(text.as_bytes());
            }
            Some(Redirect::Pipe(cmd_expr)) => {
                let cmd = self.eval_expr(cmd_expr);
                let child = self.output_pipes.entry(cmd.clone()).or_insert_with(|| {
                    Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .stdin(Stdio::piped())
                        .spawn()
                        .unwrap_or_else(|e| {
                            eprintln!("fk: cannot run '{}': {}", cmd, e);
                            // Fallback: spawn cat to /dev/null
                            Command::new("cat").stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap()
                        })
                });
                if let Some(ref mut stdin) = child.stdin {
                    let _ = stdin.write_all(text.as_bytes());
                }
            }
        }
    }

    /// Call a user-defined function: save caller's locals, bind params, run body,
    /// restore caller's locals.
    fn call_user_func(&mut self, func: &FuncDef, args: &[String]) -> String {
        // Save current values of param names so we can restore them after the call
        let mut saved: Vec<(String, Option<String>)> = Vec::new();
        for param in &func.params {
            let old = self.rt.variables.get(param).cloned();
            saved.push((param.clone(), old));
        }

        // Bind arguments to parameter names; extra params get ""
        for (i, param) in func.params.iter().enumerate() {
            let val = args.get(i).map(|s| s.as_str()).unwrap_or("");
            self.rt.set_var(param, val);
        }

        // Execute function body
        let result = match self.exec_block(&func.body) {
            Some(ReturnValue(v)) => v,
            None => String::new(),
        };

        // Restore caller's variables
        for (name, old_val) in saved {
            match old_val {
                Some(v) => self.rt.set_var(&name, &v),
                None => { self.rt.variables.remove(&name); }
            }
        }

        result
    }

    /// sub(regex, replacement [, target]) / gsub(regex, replacement [, target])
    /// Replaces first (sub) or all (gsub) occurrences of pattern in target.
    /// If target is omitted, uses $0. Returns number of replacements made.
    fn builtin_sub(&mut self, args: &[Expr], global: bool) -> String {
        if args.len() < 2 {
            eprintln!("fk: sub/gsub requires at least 2 arguments");
            return "0".to_string();
        }
        let pattern = self.eval_expr(&args[0]);
        let replacement = self.eval_expr(&args[1]);

        // Determine target: if 3rd arg given it must be an lvalue, otherwise $0
        let target_expr = if args.len() >= 3 {
            args[2].clone()
        } else {
            Expr::Field(Box::new(Expr::NumberLit(0.0)))
        };

        let target_val = self.eval_lvalue(&target_expr);
        let (new_val, count) = string_replace(&target_val, &pattern, &replacement, global);
        self.assign_to(&target_expr, &new_val);

        format_number(count as f64)
    }

    /// match(string, regex) — sets RSTART and RLENGTH, returns RSTART.
    fn builtin_match(&mut self, args: &[Expr]) -> String {
        if args.len() < 2 {
            eprintln!("fk: match requires 2 arguments");
            return "0".to_string();
        }
        let s = self.eval_expr(&args[0]);
        let pattern = self.eval_expr(&args[1]);

        if let Some(pos) = s.find(&pattern) {
            let rstart = (pos + 1) as f64; // awk is 1-indexed
            let rlength = pattern.len() as f64;
            self.rt.set_var("RSTART", &format_number(rstart));
            self.rt.set_var("RLENGTH", &format_number(rlength));
            format_number(rstart)
        } else {
            self.rt.set_var("RSTART", "0");
            self.rt.set_var("RLENGTH", "-1");
            "0".to_string()
        }
    }

    /// split(string, array [, separator]) — splits string into array, returns count.
    fn builtin_split(&mut self, args: &[Expr]) -> String {
        if args.len() < 2 {
            eprintln!("fk: split requires at least 2 arguments");
            return "0".to_string();
        }
        let s = self.eval_expr(&args[0]);
        let array_name = match &args[1] {
            Expr::Var(name) => name.clone(),
            _ => {
                eprintln!("fk: split: second argument must be an array name");
                return "0".to_string();
            }
        };
        let fs = if args.len() >= 3 {
            self.eval_expr(&args[2])
        } else {
            self.rt.get_var("FS")
        };

        let parts = crate::field::split(&s, &fs);
        // Clear existing array
        self.rt.arrays.remove(&array_name);
        for (i, part) in parts.iter().enumerate() {
            self.rt.set_array(&array_name, &format!("{}", i + 1), part);
        }
        format_number(parts.len() as f64)
    }

    /// getline [var] [< file]
    /// No source: reads next line from stdin.
    /// With source: reads from file.
    /// Returns "1" on success, "0" on EOF, "-1" on error.
    fn exec_getline(&mut self, var: Option<&str>, source: Option<&Expr>) -> String {
        let line = if let Some(src_expr) = source {
            let path = self.eval_expr(src_expr);
            match std::fs::File::open(&path) {
                Ok(file) => {
                    // Read one line — for repeated getline from same file,
                    // full caching would be needed. Simple version for now.
                    let mut reader = std::io::BufReader::new(file);
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => return "0".to_string(),
                        Ok(_) => {
                            if line.ends_with('\n') { line.pop(); }
                            if line.ends_with('\r') { line.pop(); }
                            line
                        }
                        Err(_) => return "-1".to_string(),
                    }
                }
                Err(_) => return "-1".to_string(),
            }
        } else {
            // Read from stdin
            let mut line = String::new();
            match std::io::stdin().read_line(&mut line) {
                Ok(0) => return "0".to_string(),
                Ok(_) => {
                    if line.ends_with('\n') { line.pop(); }
                    if line.ends_with('\r') { line.pop(); }
                    line
                }
                Err(_) => return "-1".to_string(),
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
        "1".to_string()
    }

    /// "cmd" | getline [var]
    fn exec_getline_pipe(&mut self, cmd: &str, var: Option<&str>) -> String {
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
                        "1".to_string()
                    }
                    Err(_) => "-1".to_string(),
                }
            }
            Err(_) => "-1".to_string(),
        }
    }
}

// --- helper functions ---

/// Replace first or all occurrences of a pattern in a string.
/// Returns (new_string, replacement_count).
fn string_replace(s: &str, pattern: &str, replacement: &str, global: bool) -> (String, usize) {
    if pattern.is_empty() {
        return (s.to_string(), 0);
    }
    if global {
        let count = s.matches(pattern).count();
        (s.replace(pattern, replacement), count)
    } else {
        if let Some(pos) = s.find(pattern) {
            let mut result = String::with_capacity(s.len());
            result.push_str(&s[..pos]);
            result.push_str(replacement);
            result.push_str(&s[pos + pattern.len()..]);
            (result, 1)
        } else {
            (s.to_string(), 0)
        }
    }
}

/// Coerce a string to a number (awk semantics: leading numeric prefix is parsed,
/// non-numeric strings become 0).
fn to_number(s: &str) -> f64 {
    let s = s.trim();
    if s.is_empty() {
        return 0.0;
    }
    // Try full parse first
    if let Ok(n) = s.parse::<f64>() {
        return n;
    }
    // Try leading numeric prefix (awk parses "123abc" as 123)
    let mut end = 0;
    let bytes = s.as_bytes();
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    let mut has_digit = false;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
        has_digit = true;
    }
    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
            has_digit = true;
        }
    }
    if has_digit {
        s[..end].parse::<f64>().unwrap_or(0.0)
    } else {
        0.0
    }
}

/// Returns true if a string looks like a number (for comparison coercion).
fn looks_numeric(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    s.parse::<f64>().is_ok()
}

fn format_number(n: f64) -> String {
    if n.is_nan() {
        return "nan".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    if n == (n as i64) as f64 {
        format!("{}", n as i64)
    } else {
        // Use OFMT-style: up to 6 decimal places, trimming trailing zeros
        let s = format!("{:.6}", n);
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_string()
    }
}

fn is_truthy(s: &str) -> bool {
    !s.is_empty() && s != "0"
}

fn bool_str(b: bool) -> String {
    if b { "1".to_string() } else { "0".to_string() }
}

/// Compare two values using awk coercion rules:
/// - If both look numeric, compare as numbers.
/// - Otherwise compare as strings.
fn compare(left: &str, right: &str) -> std::cmp::Ordering {
    if looks_numeric(left) && looks_numeric(right) {
        let l = to_number(left);
        let r = to_number(right);
        l.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal)
    } else {
        left.cmp(right)
    }
}

fn eval_binop(left: &str, op: &BinOp, right: &str) -> String {
    match op {
        BinOp::Add => format_number(to_number(left) + to_number(right)),
        BinOp::Sub => format_number(to_number(left) - to_number(right)),
        BinOp::Mul => format_number(to_number(left) * to_number(right)),
        BinOp::Div => {
            let r = to_number(right);
            if r == 0.0 {
                eprintln!("fk: division by zero");
                format_number(0.0)
            } else {
                format_number(to_number(left) / r)
            }
        }
        BinOp::Mod => {
            let r = to_number(right);
            if r == 0.0 {
                eprintln!("fk: division by zero");
                format_number(0.0)
            } else {
                format_number(to_number(left) % r)
            }
        }
        BinOp::Eq => {
            if looks_numeric(left) && looks_numeric(right) {
                bool_str(to_number(left) == to_number(right))
            } else {
                bool_str(left == right)
            }
        }
        BinOp::Ne => {
            if looks_numeric(left) && looks_numeric(right) {
                bool_str(to_number(left) != to_number(right))
            } else {
                bool_str(left != right)
            }
        }
        BinOp::Lt => bool_str(compare(left, right) == std::cmp::Ordering::Less),
        BinOp::Le => bool_str(compare(left, right) != std::cmp::Ordering::Greater),
        BinOp::Gt => bool_str(compare(left, right) == std::cmp::Ordering::Greater),
        BinOp::Ge => bool_str(compare(left, right) != std::cmp::Ordering::Less),
    }
}

/// Minimal printf implementation supporting %d, %s, %f, %%, \n, \t.
fn format_printf(fmt: &str, args: &[String]) -> String {
    let mut result = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    let mut arg_idx = 0;

    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            i += 1;
            // Parse optional width/precision
            let mut spec = String::new();
            if chars[i] == '-' {
                spec.push('-');
                i += 1;
            }
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                spec.push(chars[i]);
                i += 1;
            }
            if i >= chars.len() {
                result.push('%');
                result.push_str(&spec);
                break;
            }
            let conv = chars[i];
            i += 1;
            match conv {
                '%' => result.push('%'),
                'd' | 'i' => {
                    let val = args.get(arg_idx).map(|s| to_number(s)).unwrap_or(0.0) as i64;
                    arg_idx += 1;
                    if spec.is_empty() {
                        result.push_str(&format!("{}", val));
                    } else {
                        result.push_str(&format_with_spec(val, &spec, 'd'));
                    }
                }
                'f' | 'g' | 'e' => {
                    let val = args.get(arg_idx).map(|s| to_number(s)).unwrap_or(0.0);
                    arg_idx += 1;
                    if spec.is_empty() {
                        result.push_str(&format!("{:.6}", val));
                    } else {
                        result.push_str(&format_with_spec_f(val, &spec));
                    }
                }
                's' => {
                    let val = args.get(arg_idx).map(|s| s.as_str()).unwrap_or("");
                    arg_idx += 1;
                    if spec.is_empty() {
                        result.push_str(val);
                    } else {
                        result.push_str(&format_with_spec_s(val, &spec));
                    }
                }
                'c' => {
                    if let Some(s) = args.get(arg_idx) {
                        if let Some(ch) = s.chars().next() {
                            result.push(ch);
                        }
                    }
                    arg_idx += 1;
                }
                _ => {
                    result.push('%');
                    result.push_str(&spec);
                    result.push(conv);
                }
            }
        } else if chars[i] == '\\' && i + 1 < chars.len() {
            i += 1;
            match chars[i] {
                'n' => result.push('\n'),
                't' => result.push('\t'),
                '\\' => result.push('\\'),
                _ => {
                    result.push('\\');
                    result.push(chars[i]);
                }
            }
            i += 1;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

fn format_with_spec(val: i64, spec: &str, _conv: char) -> String {
    let left_align = spec.starts_with('-');
    let width_str = spec.trim_start_matches('-');
    let width: usize = width_str.parse().unwrap_or(0);
    let s = format!("{}", val);
    if width == 0 {
        return s;
    }
    if left_align {
        format!("{:<width$}", s, width = width)
    } else {
        format!("{:>width$}", s, width = width)
    }
}

fn format_with_spec_f(val: f64, spec: &str) -> String {
    let left_align = spec.starts_with('-');
    let spec_inner = spec.trim_start_matches('-');
    let (width, prec) = if let Some(dot_pos) = spec_inner.find('.') {
        let w: usize = spec_inner[..dot_pos].parse().unwrap_or(0);
        let p: usize = spec_inner[dot_pos + 1..].parse().unwrap_or(6);
        (w, p)
    } else {
        let w: usize = spec_inner.parse().unwrap_or(0);
        (w, 6)
    };
    let s = format!("{:.prec$}", val, prec = prec);
    if width == 0 {
        return s;
    }
    if left_align {
        format!("{:<width$}", s, width = width)
    } else {
        format!("{:>width$}", s, width = width)
    }
}

fn format_with_spec_s(val: &str, spec: &str) -> String {
    let left_align = spec.starts_with('-');
    let width_str = spec.trim_start_matches('-');
    let width: usize = width_str.parse().unwrap_or(0);
    if width == 0 {
        return val.to_string();
    }
    if left_align {
        format!("{:<width$}", val, width = width)
    } else {
        format!("{:>width$}", val, width = width)
    }
}

/// Built-in functions: length, substr, index, tolower, toupper, int, split.
fn call_builtin(name: &str, args: &[String]) -> String {
    match name {
        "length" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            format_number(s.len() as f64)
        }
        "substr" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            let start = args.get(1).map(|s| to_number(s) as usize).unwrap_or(1);
            let start = if start > 0 { start - 1 } else { 0 }; // awk is 1-indexed
            if start >= s.len() {
                return String::new();
            }
            if let Some(len_str) = args.get(2) {
                let len = to_number(len_str) as usize;
                let end = (start + len).min(s.len());
                s[start..end].to_string()
            } else {
                s[start..].to_string()
            }
        }
        "index" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            let target = args.get(1).map(|s| s.as_str()).unwrap_or("");
            match s.find(target) {
                Some(pos) => format_number((pos + 1) as f64),
                None => "0".to_string(),
            }
        }
        "tolower" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            s.to_lowercase()
        }
        "toupper" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            s.to_uppercase()
        }
        "int" => {
            let n = args.first().map(|s| to_number(s)).unwrap_or(0.0);
            format_number(n.trunc())
        }
        "sin" => {
            let n = args.first().map(|s| to_number(s)).unwrap_or(0.0);
            format!("{:.6}", n.sin())
        }
        "cos" => {
            let n = args.first().map(|s| to_number(s)).unwrap_or(0.0);
            format!("{:.6}", n.cos())
        }
        "sqrt" => {
            let n = args.first().map(|s| to_number(s)).unwrap_or(0.0);
            format_number(n.sqrt())
        }
        "log" => {
            let n = args.first().map(|s| to_number(s)).unwrap_or(0.0);
            format!("{:.6}", n.ln())
        }
        "exp" => {
            let n = args.first().map(|s| to_number(s)).unwrap_or(0.0);
            format!("{:.6}", n.exp())
        }
        _ => {
            eprintln!("fk: unknown function: {}", name);
            String::new()
        }
    }
}
