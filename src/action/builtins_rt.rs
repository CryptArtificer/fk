use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::builtins::{self, string_replace};
use crate::parser::Expr;
use crate::runtime::Value;

use super::{percentile_sorted, Executor};

impl<'a> Executor<'a> {
    /// Extract a regex pattern string from an expression that may be a bare
    /// `/regex/` literal (which the parser turns into `$0 ~ "pattern"`).
    fn extract_regex_or_eval(&mut self, expr: &Expr) -> String {
        if let Expr::Match(lhs, rhs) = expr
            && let Expr::Field(idx) = lhs.as_ref()
            && let Expr::NumberLit(n) = idx.as_ref()
            && *n == 0.0
            && let Expr::StringLit(pat) = rhs.as_ref()
        {
            return pat.clone();
        }
        self.eval_string(expr)
    }

    /// sub/gsub: these need runtime access to modify lvalues.
    pub(crate) fn builtin_sub(&mut self, args: &[Expr], global: bool) -> Value {
        if args.len() < 2 {
            eprintln!("fk: sub/gsub requires at least 2 arguments");
            return Value::from_number(0.0);
        }
        let pattern = self.extract_regex_or_eval(&args[0]);
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

    /// match(string, regex [, arr]) — find regex in string, optionally capture groups.
    pub(crate) fn builtin_match(&mut self, args: &[Expr]) -> Value {
        if args.len() < 2 {
            eprintln!("fk: match requires at least 2 arguments");
            return Value::from_number(0.0);
        }
        let s = self.eval_string(&args[0]);
        let pattern = self.extract_regex_or_eval(&args[1]);

        let capture_arr = if args.len() >= 3 {
            match &args[2] {
                Expr::Var(name) => Some(name.clone()),
                _ => None,
            }
        } else {
            None
        };

        if !self.ensure_regex(&pattern) {
            self.rt.set_value("RSTART", Value::from_number(0.0));
            self.rt.set_value("RLENGTH", Value::from_number(-1.0));
            return Value::from_number(0.0);
        }

        let re = &self.regex_cache[&pattern];
        if let Some(caps) = re.captures(&s) {
            let full = caps.get(0).unwrap();
            let rstart = s[..full.start()].chars().count() as f64 + 1.0;
            let rlength = full.as_str().chars().count() as f64;
            self.rt.set_value("RSTART", Value::from_number(rstart));
            self.rt.set_value("RLENGTH", Value::from_number(rlength));

            if let Some(arr_name) = capture_arr {
                self.rt.delete_array_all(&arr_name);
                self.rt.set_array(&arr_name, "0", full.as_str());
                for i in 1..caps.len() {
                    if let Some(m) = caps.get(i) {
                        self.rt.set_array(&arr_name, &i.to_string(), m.as_str());
                    } else {
                        self.rt.set_array(&arr_name, &i.to_string(), "");
                    }
                }
            }

            Value::from_number(rstart)
        } else {
            self.rt.set_value("RSTART", Value::from_number(0.0));
            self.rt.set_value("RLENGTH", Value::from_number(-1.0));
            if let Some(arr_name) = capture_arr {
                self.rt.delete_array_all(&arr_name);
            }
            Value::from_number(0.0)
        }
    }

    /// jpath(json, path, array) — extract JSON value into an awk array.
    pub(crate) fn builtin_jpath_extract(&mut self, args: &[Expr]) -> Value {
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
        self.rt.delete_array_all(&array_name);
        for (key, val) in &pairs {
            self.rt.set_array(&array_name, key, val);
        }
        Value::from_number(pairs.len() as f64)
    }

    /// split(string, array [, separator]) — returns element count.
    pub(crate) fn builtin_split(&mut self, args: &[Expr]) -> Value {
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
            self.extract_regex_or_eval(&args[2])
        } else {
            self.rt.get_var("FS")
        };

        let parts = crate::field::split(&s, &fs);
        self.rt.delete_array_all(&array_name);
        for (i, part) in parts.iter().enumerate() {
            self.rt.set_array(&array_name, &format!("{}", i + 1), part);
        }
        Value::from_number(parts.len() as f64)
    }

    /// fflush([file]) — flush stdout or a named output file.
    pub(crate) fn builtin_fflush(&mut self, args: &[Expr]) -> Value {
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
    pub(crate) fn builtin_system(&mut self, args: &[Expr]) -> Value {
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

    /// close(name) — close a file or pipe (output or input) by name.
    pub(crate) fn builtin_close(&mut self, args: &[Expr]) -> Value {
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
        if self.input_files.remove(&name).is_some() {
            return Value::from_number(0.0);
        }
        if self.input_pipe_readers.remove(&name).is_some() {
            if let Some(mut child) = self.input_pipe_children.remove(&name) {
                let _ = child.wait();
            }
            return Value::from_number(0.0);
        }
        Value::from_number(-1.0)
    }

    /// gensub(regex, replacement, how [, target]) — like gsub but returns result.
    pub(crate) fn builtin_gensub(&mut self, args: &[Expr]) -> Value {
        if args.len() < 3 {
            eprintln!("fk: gensub requires at least 3 arguments");
            return Value::default();
        }
        let pattern = self.extract_regex_or_eval(&args[0]);
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

    /// join(arr [, sep]) — join array values into a string. Defaults to OFS.
    pub(crate) fn builtin_join(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            eprintln!("fk: join requires at least 1 argument (array [, separator])");
            return Value::default();
        }
        let array_name = match &args[0] {
            Expr::Var(name) => name.clone(),
            _ => {
                eprintln!("fk: join: first argument must be an array name");
                return Value::default();
            }
        };
        let sep = if args.len() >= 2 {
            self.eval_string(&args[1])
        } else {
            self.rt.ofs().to_owned()
        };
        let mut keys: Vec<String> = self.rt.array_keys(&array_name);
        keys.sort_by(|a, b| {
            a.parse::<f64>().unwrap_or(f64::MAX)
                .partial_cmp(&b.parse::<f64>().unwrap_or(f64::MAX))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(b))
        });
        let parts: Vec<String> = keys.iter()
            .map(|k| self.rt.get_array(&array_name, k))
            .collect();
        Value::from_string(parts.join(&sep))
    }

    /// keys(arr) — return sorted keys joined by ORS.
    pub(crate) fn builtin_keys(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            eprintln!("fk: keys requires 1 argument (array)");
            return Value::default();
        }
        let array_name = match &args[0] {
            Expr::Var(name) => name.clone(),
            _ => {
                eprintln!("fk: keys: argument must be an array name");
                return Value::default();
            }
        };
        let mut keys = self.rt.array_keys(&array_name);
        smart_sort_keys(&mut keys);
        let sep = self.rt.ors().to_owned();
        Value::from_string(keys.join(&sep))
    }

    /// vals(arr) — return values joined by ORS, sorted by key.
    pub(crate) fn builtin_vals(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            eprintln!("fk: vals requires 1 argument (array)");
            return Value::default();
        }
        let array_name = match &args[0] {
            Expr::Var(name) => name.clone(),
            _ => {
                eprintln!("fk: vals: argument must be an array name");
                return Value::default();
            }
        };
        let mut keys = self.rt.array_keys(&array_name);
        smart_sort_keys(&mut keys);
        let vals: Vec<String> = keys.iter()
            .map(|k| self.rt.get_array(&array_name, k))
            .collect();
        let sep = self.rt.ors().to_owned();
        Value::from_string(vals.join(&sep))
    }

    /// Print array contents directly to stdout (used by `print arr`).
    /// Sequential arrays (1..N) print values; associative arrays print keys.
    pub(crate) fn print_array(&mut self, name: &str) {
        let mut keys = self.rt.array_keys(name);
        if keys.is_empty() { return; }
        smart_sort_keys(&mut keys);
        let sequential = is_sequential(&keys);
        let ors = self.rt.ors().to_owned();
        for (i, k) in keys.iter().enumerate() {
            if i > 0 {
                let _ = self.stdout.write_all(ors.as_bytes());
            }
            if sequential {
                let v = self.rt.get_array(name, k);
                let _ = self.stdout.write_all(v.as_bytes());
            } else {
                let _ = self.stdout.write_all(k.as_bytes());
            }
        }
        let _ = self.stdout.write_all(ors.as_bytes());
    }

    /// uniq(arr) — deduplicate values, re-key 1..N. Returns new count.
    pub(crate) fn builtin_uniq(&mut self, args: &[Expr]) -> Value {
        let array_name = match extract_array_name(args) {
            Some(n) => n,
            None => { eprintln!("fk: uniq: argument must be an array name"); return Value::from_number(0.0); }
        };
        let mut keys = self.rt.array_keys(&array_name);
        smart_sort_keys(&mut keys);
        let mut seen = std::collections::HashSet::new();
        let mut unique: Vec<String> = Vec::new();
        for k in &keys {
            let v = self.rt.get_array(&array_name, k);
            if seen.insert(v.clone()) {
                unique.push(v);
            }
        }
        let count = unique.len();
        self.rt.delete_array_all(&array_name);
        for (i, v) in unique.into_iter().enumerate() {
            self.rt.set_array(&array_name, &(i + 1).to_string(), &v);
        }
        Value::from_number(count as f64)
    }

    /// inv(arr) — swap keys and values in place. Returns count.
    pub(crate) fn builtin_invert(&mut self, args: &[Expr]) -> Value {
        let array_name = match extract_array_name(args) {
            Some(n) => n,
            None => { eprintln!("fk: inv: argument must be an array name"); return Value::from_number(0.0); }
        };
        let keys = self.rt.array_keys(&array_name);
        let pairs: Vec<(String, String)> = keys.iter()
            .map(|k| (k.clone(), self.rt.get_array(&array_name, k)))
            .collect();
        let count = pairs.len();
        self.rt.delete_array_all(&array_name);
        for (k, v) in pairs {
            self.rt.set_array(&array_name, &v, &k);
        }
        Value::from_number(count as f64)
    }

    /// tidy(arr) — remove entries with empty or zero values. Returns remaining count.
    pub(crate) fn builtin_compact(&mut self, args: &[Expr]) -> Value {
        let array_name = match extract_array_name(args) {
            Some(n) => n,
            None => { eprintln!("fk: tidy: argument must be an array name"); return Value::from_number(0.0); }
        };
        let keys = self.rt.array_keys(&array_name);
        let mut to_remove = Vec::new();
        for k in &keys {
            let v = self.rt.get_array_value(&array_name, k);
            if !v.is_truthy() {
                to_remove.push(k.clone());
            }
        }
        for k in &to_remove {
            self.rt.delete_array(&array_name, k);
        }
        Value::from_number(self.rt.array_len(&array_name) as f64)
    }

    /// shuf(arr) — randomize order, re-key 1..N. Returns count.
    pub(crate) fn builtin_shuffle(&mut self, args: &[Expr]) -> Value {
        let array_name = match extract_array_name(args) {
            Some(n) => n,
            None => { eprintln!("fk: shuf: argument must be an array name"); return Value::from_number(0.0); }
        };
        let keys = self.rt.array_keys(&array_name);
        let mut vals: Vec<String> = keys.iter()
            .map(|k| self.rt.get_array(&array_name, k))
            .collect();
        for i in (1..vals.len()).rev() {
            let j = (builtins::math::rng_next() * (i + 1) as f64) as usize;
            vals.swap(i, j.min(i));
        }
        let count = vals.len();
        self.rt.delete_array_all(&array_name);
        for (i, v) in vals.into_iter().enumerate() {
            self.rt.set_array(&array_name, &(i + 1).to_string(), &v);
        }
        Value::from_number(count as f64)
    }

    /// diff(a, b) — remove from a any key present in b. Returns remaining count.
    pub(crate) fn builtin_set_op(&mut self, op: &str, args: &[Expr]) -> Value {
        if args.len() < 2 {
            eprintln!("fk: {} requires 2 arguments (array, array)", op);
            return Value::from_number(0.0);
        }
        let name_a = match &args[0] {
            Expr::Var(n) => n.clone(),
            _ => { eprintln!("fk: {}: arguments must be array names", op); return Value::from_number(0.0); }
        };
        let name_b = match &args[1] {
            Expr::Var(n) => n.clone(),
            _ => { eprintln!("fk: {}: arguments must be array names", op); return Value::from_number(0.0); }
        };
        let keys_a: std::collections::HashSet<String> = self.rt.array_keys(&name_a).into_iter().collect();
        let keys_b: std::collections::HashSet<String> = self.rt.array_keys(&name_b).into_iter().collect();

        match op {
            "diff" => {
                for k in keys_a.intersection(&keys_b) {
                    self.rt.delete_array(&name_a, k);
                }
            }
            "inter" => {
                for k in keys_a.difference(&keys_b) {
                    self.rt.delete_array(&name_a, &k.clone());
                }
            }
            "union" => {
                for k in &keys_b {
                    if !keys_a.contains(k) {
                        let v = self.rt.get_array(&name_b, k);
                        self.rt.set_array(&name_a, k, &v);
                    }
                }
            }
            _ => {}
        }
        Value::from_number(self.rt.array_len(&name_a) as f64)
    }

    /// seq(arr, from, to) — fill arr with from..to, keyed 1..N. Returns count.
    pub(crate) fn builtin_seq(&mut self, args: &[Expr]) -> Value {
        if args.len() < 3 {
            eprintln!("fk: seq requires 3 arguments (array, from, to)");
            return Value::from_number(0.0);
        }
        let array_name = match &args[0] {
            Expr::Var(n) => n.clone(),
            _ => { eprintln!("fk: seq: first argument must be an array name"); return Value::from_number(0.0); }
        };
        let from = self.eval_expr(&args[1]).to_number() as i64;
        let to = self.eval_expr(&args[2]).to_number() as i64;
        self.rt.delete_array_all(&array_name);
        let step: i64 = if from <= to { 1 } else { -1 };
        let mut i = from;
        let mut idx = 1;
        loop {
            self.rt.set_array(&array_name, &idx.to_string(), &i.to_string());
            if i == to { break; }
            i += step;
            idx += 1;
            if idx > 1_000_000 { break; }
        }
        Value::from_number(idx as f64)
    }

    /// samp(arr, n) — keep n random elements, re-key 1..n. Returns n.
    pub(crate) fn builtin_sample(&mut self, args: &[Expr]) -> Value {
        if args.len() < 2 {
            eprintln!("fk: samp requires 2 arguments (array, n)");
            return Value::from_number(0.0);
        }
        let array_name = match &args[0] {
            Expr::Var(n) => n.clone(),
            _ => { eprintln!("fk: samp: first argument must be an array name"); return Value::from_number(0.0); }
        };
        let n = self.eval_expr(&args[1]).to_number() as usize;
        let keys = self.rt.array_keys(&array_name);
        let mut vals: Vec<String> = keys.iter()
            .map(|k| self.rt.get_array(&array_name, k))
            .collect();
        // Fisher-Yates partial shuffle for first n elements
        let take = n.min(vals.len());
        for i in 0..take {
            let j = i + (builtins::math::rng_next() * (vals.len() - i) as f64) as usize;
            let j = j.min(vals.len() - 1);
            vals.swap(i, j);
        }
        vals.truncate(take);
        self.rt.delete_array_all(&array_name);
        for (i, v) in vals.into_iter().enumerate() {
            self.rt.set_array(&array_name, &(i + 1).to_string(), &v);
        }
        Value::from_number(take as f64)
    }

    /// slurp(file [, arr]) — read file into string, or into arr lines. Returns string or line count.
    pub(crate) fn builtin_slurp(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            eprintln!("fk: slurp requires at least 1 argument (filename)");
            return Value::default();
        }
        let filename = self.eval_string(&args[0]);
        let contents = match std::fs::read_to_string(&filename) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("fk: slurp: {}: {}", filename, e);
                return if args.len() >= 2 { Value::from_number(0.0) } else { Value::default() };
            }
        };
        if args.len() >= 2 {
            let array_name = match &args[1] {
                Expr::Var(n) => n.clone(),
                _ => { eprintln!("fk: slurp: second argument must be an array name"); return Value::from_number(0.0); }
            };
            self.rt.delete_array_all(&array_name);
            let mut count = 0;
            for line in contents.lines() {
                count += 1;
                self.rt.set_array(&array_name, &count.to_string(), line);
            }
            Value::from_number(count as f64)
        } else {
            Value::from_string(contents)
        }
    }

    /// typeof(x) — return type name of a variable.
    pub(crate) fn builtin_typeof(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            return Value::from_string("uninitialized".to_string());
        }
        let ty = match &args[0] {
            Expr::Var(name) => {
                if self.rt.has_array(name) {
                    "array"
                } else if !self.rt.has_var(name) {
                    "uninitialized"
                } else {
                    Self::value_type_name(&self.rt.get_value(name))
                }
            }
            Expr::ArrayRef(name, key_expr) => {
                let key = self.eval_string(key_expr);
                if !self.rt.array_has_key(name, &key) {
                    "uninitialized"
                } else {
                    Self::value_type_name(&self.rt.get_array_value(name, &key))
                }
            }
            Expr::NumberLit(_) => "number",
            Expr::StringLit(_) => "string",
            _ => Self::value_type_name(&self.eval_expr(&args[0])),
        };
        Value::from_string(ty.to_string())
    }

    fn value_type_name(val: &Value) -> &'static str {
        if val.is_numeric() { "number" } else { "string" }
    }

    /// Bitwise operations: and, or, xor, lshift, rshift, compl.
    pub(crate) fn builtin_bitwise(&mut self, name: &str, args: &[Expr]) -> Value {
        if name == "compl" {
            let n = if args.is_empty() { 0i64 } else { self.eval_expr(&args[0]).to_number() as i64 };
            return Value::from_number(!n as f64);
        }
        if args.len() < 2 {
            eprintln!("fk: {} requires 2 arguments", name);
            return Value::from_number(0.0);
        }
        let a = self.eval_expr(&args[0]).to_number() as i64;
        let b = self.eval_expr(&args[1]).to_number() as i64;
        let result = match name {
            "and" => a & b,
            "or" => a | b,
            "xor" => a ^ b,
            "lshift" => a << (b as u32 & 63),
            "rshift" => ((a as u64) >> (b as u32 & 63)) as i64,
            _ => 0,
        };
        Value::from_number(result as f64)
    }

    /// asort(arr) — sort array by values, re-key with 1..N.
    /// asorti(arr) — sort array by keys, store sorted keys as values with 1..N.
    pub(crate) fn builtin_asort(&mut self, args: &[Expr], by_index: bool) -> Value {
        if args.is_empty() {
            eprintln!("fk: asort/asorti requires at least 1 argument");
            return Value::from_number(0.0);
        }
        let array_name = match &args[0] {
            Expr::Var(name) => name.clone(),
            _ => {
                eprintln!("fk: asort/asorti: argument must be an array name");
                return Value::from_number(0.0);
            }
        };

        let mut items: Vec<(String, String)> = self.rt.array_keys(&array_name)
            .into_iter()
            .map(|k| {
                let v = self.rt.get_array(&array_name, &k);
                (k, v)
            })
            .collect();

        if by_index {
            items.sort_by(|a, b| a.0.cmp(&b.0));
        } else {
            items.sort_by(|a, b| {
                let na = crate::builtins::to_number(&a.1);
                let nb = crate::builtins::to_number(&b.1);
                if na != 0.0 || a.1.is_empty() || nb != 0.0 || b.1.is_empty() {
                    na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    a.1.cmp(&b.1)
                }
            });
        }

        let count = items.len();
        self.rt.delete_array_all(&array_name);
        for (i, (key, val)) in items.into_iter().enumerate() {
            let new_key = (i + 1).to_string();
            if by_index {
                self.rt.set_array(&array_name, &new_key, &key);
            } else {
                self.rt.set_array(&array_name, &new_key, &val);
            }
        }
        Value::from_number(count as f64)
    }

    /// Extract sorted numeric values from an array.
    fn array_sorted_values(&self, array_name: &str) -> Vec<f64> {
        let mut vals: Vec<f64> = self.rt.array_keys(array_name)
            .into_iter()
            .map(|k| {
                let v = self.rt.get_array(array_name, &k);
                builtins::to_number(&v)
            })
            .collect();
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        vals
    }

    /// Statistical functions that operate on arrays.
    pub(crate) fn builtin_stats(&mut self, name: &str, args: &[Expr]) -> Value {
        if args.is_empty() {
            eprintln!("fk: {}() requires an array argument", name);
            return Value::from_number(0.0);
        }
        let array_name = match &args[0] {
            Expr::Var(v) => v.clone(),
            _ => {
                eprintln!("fk: {}(): first argument must be an array name", name);
                return Value::from_number(0.0);
            }
        };
        if !self.rt.has_array(&array_name) {
            return Value::from_number(0.0);
        }

        if matches!(name, "sum" | "mean" | "variance" | "stddev" | "min" | "max") {
            let Some(values) = self.rt.array_values(&array_name) else {
                return Value::from_number(0.0);
            };
            let mut count: f64 = 0.0;
            let mut mean: f64 = 0.0;
            let mut m2: f64 = 0.0;
            let mut sum: f64 = 0.0;
            let mut min: f64 = f64::INFINITY;
            let mut max: f64 = f64::NEG_INFINITY;

            for v in values {
                let v = v.to_number();
                count += 1.0;
                sum += v;
                if v < min { min = v; }
                if v > max { max = v; }
                let delta = v - mean;
                mean += delta / count;
                let delta2 = v - mean;
                m2 += delta * delta2;
            }

            if count == 0.0 {
                return Value::from_number(0.0);
            }
            return match name {
                "sum" => Value::from_number(sum),
                "mean" => Value::from_number(sum / count),
                "variance" => Value::from_number(m2 / count),
                "stddev" => Value::from_number((m2 / count).sqrt()),
                "min" => Value::from_number(min),
                "max" => Value::from_number(max),
                _ => Value::from_number(0.0),
            };
        }

        let vals = self.array_sorted_values(&array_name);
        if vals.is_empty() {
            return Value::from_number(0.0);
        }
        let n = vals.len();

        match name {
            "median" => {
                Value::from_number(percentile_sorted(&vals, 50.0))
            }
            "percentile" | "p" => {
                let pct = if args.len() >= 2 {
                    builtins::to_number(&self.eval_string(&args[1]))
                } else {
                    50.0
                };
                Value::from_number(percentile_sorted(&vals, pct))
            }
            "quantile" => {
                let q = if args.len() >= 2 {
                    builtins::to_number(&self.eval_string(&args[1]))
                } else {
                    0.5
                };
                Value::from_number(percentile_sorted(&vals, q * 100.0))
            }
            "iqm" => {
                let q1_idx = ((n as f64) * 0.25).ceil() as usize;
                let q3_idx = ((n as f64) * 0.75).floor() as usize;
                let q1 = q1_idx.max(1) - 1;
                let q3 = q3_idx.min(n);
                if q3 <= q1 {
                    Value::from_number(vals.iter().sum::<f64>() / n as f64)
                } else {
                    let slice = &vals[q1..q3];
                    let s: f64 = slice.iter().sum();
                    Value::from_number(s / slice.len() as f64)
                }
            }
            _ => Value::from_number(0.0),
        }
    }

    /// hist(arr, bins [, out [, min [, max]]]) — histogram of numeric values.
    /// Writes counts into `out` (or `arr` if omitted) with keys 1..bins.
    /// Also stores metadata keys: _min, _max, _width. Returns bin count.
    pub(crate) fn builtin_hist(&mut self, args: &[Expr]) -> Value {
        if args.len() < 2 {
            eprintln!("fk: hist() requires an array and bin count");
            return Value::from_number(0.0);
        }
        let array_name = match &args[0] {
            Expr::Var(v) => v.clone(),
            _ => {
                eprintln!("fk: hist(): first argument must be an array name");
                return Value::from_number(0.0);
            }
        };
        let bins_raw = builtins::to_number(&self.eval_string(&args[1])).round() as i64;
        if bins_raw <= 0 {
            eprintln!("fk: hist(): bins must be > 0");
            return Value::from_number(0.0);
        }
        let bins = bins_raw as usize;

        let out_name = if let Some(expr) = args.get(2) {
            match expr {
                Expr::Var(v) => v.clone(),
                _ => {
                    eprintln!("fk: hist(): output must be an array name");
                    return Value::from_number(0.0);
                }
            }
        } else {
            array_name.clone()
        };

        let Some(values) = self.rt.array_values(&array_name) else {
            return Value::from_number(0.0);
        };
        let mut vals: Vec<f64> = values.into_iter().map(|v| v.to_number()).collect();
        if vals.is_empty() {
            self.rt.delete_array_all(&out_name);
            return Value::from_number(0.0);
        }

        let mut min = vals[0];
        let mut max = vals[0];
        for v in &vals[1..] {
            if *v < min { min = *v; }
            if *v > max { max = *v; }
        }
        if let Some(expr) = args.get(3) {
            min = builtins::to_number(&self.eval_string(expr));
        }
        if let Some(expr) = args.get(4) {
            max = builtins::to_number(&self.eval_string(expr));
        }
        if min > max {
            std::mem::swap(&mut min, &mut max);
        }

        let mut width = (max - min) / bins as f64;
        if width == 0.0 || !width.is_finite() {
            width = 1.0;
        }

        let mut counts = vec![0usize; bins];
        for v in vals.drain(..) {
            let mut idx = ((v - min) / width).floor() as i64;
            if idx < 0 { idx = 0; }
            if idx >= bins as i64 { idx = bins as i64 - 1; }
            counts[idx as usize] += 1;
        }

        self.rt.delete_array_all(&out_name);
        for (i, count) in counts.into_iter().enumerate() {
            let key = (i + 1).to_string();
            self.rt.set_array(&out_name, &key, &count.to_string());
        }
        self.rt.set_array(&out_name, "_min", &builtins::format_number(min));
        self.rt.set_array(&out_name, "_max", &builtins::format_number(max));
        self.rt.set_array(&out_name, "_width", &builtins::format_number(width));

        Value::from_number(bins as f64)
    }

    /// plot(arr [, width [, char [, precision [, color]]]]) — render a simple horizontal bar chart.
    /// Uses numeric keys in ascending order if present (ignores keys starting with '_').
    /// If histogram metadata keys (_min/_max/_width) exist, labels bins by range.
    pub(crate) fn builtin_plot(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            eprintln!("fk: plot() requires an array argument");
            return Value::from_string(String::new());
        }
        let array_name = match &args[0] {
            Expr::Var(v) => v.clone(),
            _ => {
                eprintln!("fk: plot(): first argument must be an array name");
                return Value::from_string(String::new());
            }
        };
        if !self.rt.has_array(&array_name) {
            return Value::from_string(String::new());
        }

        let width_raw = if let Some(expr) = args.get(1) {
            builtins::to_number(&self.eval_string(expr)).round() as i64
        } else {
            40
        };
        let width = if width_raw <= 0 { 40 } else { width_raw as usize };
        let ch = if let Some(expr) = args.get(2) {
            self.eval_string(expr).chars().next().unwrap_or('#')
        } else {
            '#'
        };
        let precision_raw = if let Some(expr) = args.get(3) {
            builtins::to_number(&self.eval_string(expr)).round() as i64
        } else {
            -1
        };
        let precision = if precision_raw < 0 { None } else { Some(precision_raw as usize) };
        let color_name = args.get(4).map(|expr| self.eval_string(expr)).unwrap_or_default();
        let color_code = match color_name.as_str() {
            "" | "none" => "",
            "red" => "\x1b[31m",
            "green" => "\x1b[32m",
            "yellow" => "\x1b[33m",
            "blue" => "\x1b[34m",
            "magenta" => "\x1b[35m",
            "cyan" => "\x1b[36m",
            "gray" | "grey" => "\x1b[90m",
            _ => "",
        };
        let color_reset = if color_code.is_empty() { "" } else { "\x1b[0m" };

        let mut numeric_keys: Vec<(i64, String)> = Vec::new();
        let mut other_keys: Vec<String> = Vec::new();
        for k in self.rt.array_keys(&array_name) {
            if k.starts_with('_') {
                continue;
            }
            if let Ok(n) = k.parse::<i64>() {
                numeric_keys.push((n, k));
            } else {
                other_keys.push(k);
            }
        }
        if !numeric_keys.is_empty() {
            numeric_keys.sort_by_key(|(n, _)| *n);
        } else {
            other_keys.sort();
        }

        let mut entries: Vec<(String, f64)> = Vec::new();
        if !numeric_keys.is_empty() {
            for (_n, k) in &numeric_keys {
                let v = self.rt.get_array(&array_name, k);
                entries.push((k.clone(), builtins::to_number(&v)));
            }
        } else {
            for k in &other_keys {
                let v = self.rt.get_array(&array_name, k);
                entries.push((k.clone(), builtins::to_number(&v)));
            }
        }

        let mut max_count = 0.0;
        for (_k, v) in &entries {
            if *v > max_count {
                max_count = *v;
            }
        }

        let mut label_with_range = false;
        let min = builtins::to_number(&self.rt.get_array(&array_name, "_min"));
        let max = builtins::to_number(&self.rt.get_array(&array_name, "_max"));
        let bin_width = builtins::to_number(&self.rt.get_array(&array_name, "_width"));
        if bin_width.is_finite() && bin_width > 0.0 && min.is_finite() && max.is_finite() {
            label_with_range = !numeric_keys.is_empty();
        }

        let range_decimals = if let Some(p) = precision {
            p
        } else if !bin_width.is_finite() || bin_width == 0.0 {
            0
        } else {
            let w = bin_width.abs();
            if w >= 1.0 { 0 }
            else if w >= 0.1 { 1 }
            else if w >= 0.01 { 2 }
            else if w >= 0.001 { 3 }
            else { 4 }
        };

        let mut labels: Vec<String> = Vec::new();
        for (idx, (key, _count)) in entries.iter().enumerate() {
            let label = if label_with_range {
                let lo = min + (idx as f64) * bin_width;
                let hi = if idx + 1 == entries.len() {
                    max
                } else {
                    lo + bin_width
                };
                format!("{:.*}..{:.*}", range_decimals, lo, range_decimals, hi)
            } else {
                key.clone()
            };
            labels.push(label);
        }
        let label_width = labels.iter().map(|l| l.len()).max().unwrap_or(0);
        let count_width = entries.iter()
            .map(|(_k, v)| builtins::format_number(*v).len())
            .max()
            .unwrap_or(0);

        let mut lines: Vec<String> = Vec::new();
        for (idx, (_key, count)) in entries.iter().enumerate() {
            let mut bar_len = if max_count > 0.0 {
                ((count / max_count) * width as f64).round() as usize
            } else {
                0
            };
            if *count > 0.0 && bar_len == 0 {
                bar_len = 1;
            }
            let mut bar = ch.to_string().repeat(bar_len);
            if bar_len < width {
                bar.push_str(&" ".repeat(width - bar_len));
            }
            let count_str = builtins::format_number(*count);
            let colored_bar = if color_code.is_empty() {
                bar
            } else {
                format!("{}{}{}", color_code, bar, color_reset)
            };
            lines.push(format!(
                "{:label_w$} | {} {:count_w$}",
                labels[idx],
                colored_bar,
                count_str,
                label_w = label_width,
                count_w = count_width,
            ));
        }

        Value::from_string(lines.join("\n"))
    }

    /// plotbox(arr [, width [, char [, precision [, title [, xlabel [, color]]]]]])
    /// Render a boxed horizontal bar chart.
    pub(crate) fn builtin_plotbox(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            eprintln!("fk: plotbox() requires an array argument");
            return Value::from_string(String::new());
        }
        let array_name = match &args[0] {
            Expr::Var(v) => v.clone(),
            _ => {
                eprintln!("fk: plotbox(): first argument must be an array name");
                return Value::from_string(String::new());
            }
        };
        if !self.rt.has_array(&array_name) {
            return Value::from_string(String::new());
        }

        let width_raw = if let Some(expr) = args.get(1) {
            builtins::to_number(&self.eval_string(expr)).round() as i64
        } else {
            40
        };
        let width = if width_raw <= 0 { 40 } else { width_raw as usize };
        let ch = if let Some(expr) = args.get(2) {
            self.eval_string(expr).chars().next().unwrap_or('▇')
        } else {
            '▇'
        };
        let precision_raw = if let Some(expr) = args.get(3) {
            builtins::to_number(&self.eval_string(expr)).round() as i64
        } else {
            -1
        };
        let precision = if precision_raw < 0 { None } else { Some(precision_raw as usize) };
        let title = args.get(4).map(|expr| self.eval_string(expr)).unwrap_or_default();
        let xlabel = args.get(5).map(|expr| self.eval_string(expr)).unwrap_or_default();
        let color_name = args.get(6).map(|expr| self.eval_string(expr)).unwrap_or_default();
        let color_code = match color_name.as_str() {
            "" | "none" => "",
            "red" => "\x1b[31m",
            "green" => "\x1b[32m",
            "yellow" => "\x1b[33m",
            "blue" => "\x1b[34m",
            "magenta" => "\x1b[35m",
            "cyan" => "\x1b[36m",
            "gray" | "grey" => "\x1b[90m",
            _ => "",
        };
        let color_reset = if color_code.is_empty() { "" } else { "\x1b[0m" };

        let mut numeric_keys: Vec<(i64, String)> = Vec::new();
        let mut other_keys: Vec<String> = Vec::new();
        for k in self.rt.array_keys(&array_name) {
            if k.starts_with('_') {
                continue;
            }
            if let Ok(n) = k.parse::<i64>() {
                numeric_keys.push((n, k));
            } else {
                other_keys.push(k);
            }
        }
        if !numeric_keys.is_empty() {
            numeric_keys.sort_by_key(|(n, _)| *n);
        } else {
            other_keys.sort();
        }

        let mut entries: Vec<(String, f64)> = Vec::new();
        if !numeric_keys.is_empty() {
            for (_n, k) in &numeric_keys {
                let v = self.rt.get_array(&array_name, k);
                entries.push((k.clone(), builtins::to_number(&v)));
            }
        } else {
            for k in &other_keys {
                let v = self.rt.get_array(&array_name, k);
                entries.push((k.clone(), builtins::to_number(&v)));
            }
        }

        let mut max_count = 0.0;
        for (_k, v) in &entries {
            if *v > max_count {
                max_count = *v;
            }
        }

        let min = builtins::to_number(&self.rt.get_array(&array_name, "_min"));
        let max = builtins::to_number(&self.rt.get_array(&array_name, "_max"));
        let bin_width = builtins::to_number(&self.rt.get_array(&array_name, "_width"));
        let label_with_range = bin_width.is_finite() && bin_width > 0.0 && min.is_finite() && max.is_finite() && !numeric_keys.is_empty();
        let range_decimals = if let Some(p) = precision {
            p
        } else if !bin_width.is_finite() || bin_width == 0.0 {
            0
        } else {
            let w = bin_width.abs();
            if w >= 1.0 { 0 }
            else if w >= 0.1 { 1 }
            else if w >= 0.01 { 2 }
            else if w >= 0.001 { 3 }
            else { 4 }
        };

        let mut labels: Vec<String> = Vec::new();
        if label_with_range {
            let mut bounds: Vec<(String, String)> = Vec::new();
            let mut num_width = 0usize;
            for (idx, _entry) in entries.iter().enumerate() {
                let lo = min + (idx as f64) * bin_width;
                let hi = if idx + 1 == entries.len() {
                    max
                } else {
                    lo + bin_width
                };
                let lo_s = format!("{:.*}", range_decimals, lo);
                let hi_s = format!("{:.*}", range_decimals, hi);
                num_width = num_width.max(lo_s.len()).max(hi_s.len());
                bounds.push((lo_s, hi_s));
            }
            for (lo_s, hi_s) in bounds {
                labels.push(format!("[{:>w$}, {:>w$})", lo_s, hi_s, w = num_width));
            }
        } else {
            for (key, _count) in &entries {
                labels.push(key.clone());
            }
        }
        let label_width = labels.iter().map(|l| l.len()).max().unwrap_or(0);
        let count_width = entries.iter()
            .map(|(_k, v)| builtins::format_number(*v).len())
            .max()
            .unwrap_or(0);

        let box_width = width + count_width + 1; // bar + space + count
        let mut lines: Vec<String> = Vec::new();
        if !title.is_empty() {
            let total = label_width + 3 + box_width + 1;
            let title_pad = if total > title.len() { (total - title.len()) / 2 } else { 0 };
            lines.push(format!("{:pad$}{}", "", title, pad = title_pad));
        }
        lines.push(format!(
            "{:>label_w$} ┌{}┐",
            "",
            " ".repeat(box_width),
            label_w = label_width,
        ));

        for (idx, (_key, count)) in entries.iter().enumerate() {
            let mut bar_len = if max_count > 0.0 {
                ((count / max_count) * width as f64).round() as usize
            } else {
                0
            };
            if *count > 0.0 && bar_len == 0 {
                bar_len = 1;
            }
            let mut bar = ch.to_string().repeat(bar_len);
            if bar_len < width {
                bar.push_str(&" ".repeat(width - bar_len));
            }
            let count_str = builtins::format_number(*count);
            let colored_bar = if color_code.is_empty() {
                bar
            } else {
                format!("{}{}{}", color_code, bar, color_reset)
            };
            lines.push(format!(
                "{:>label_w$} ┤{} {:count_w$}",
                labels[idx],
                colored_bar,
                count_str,
                label_w = label_width,
                count_w = count_width,
            ));
        }

        lines.push(format!(
            "{:>label_w$} └{}┘",
            "",
            " ".repeat(box_width),
            label_w = label_width,
        ));
        if !xlabel.is_empty() {
            let total = label_width + 3 + box_width + 1;
            let pad = if total > xlabel.len() { (total - xlabel.len()) / 2 } else { 0 };
            lines.push(format!("{:pad$}{}", "", xlabel, pad = pad));
        }

        Value::from_string(lines.join("\n"))
    }

    pub(crate) fn exec_getline(&mut self, var: Option<&str>, source: Option<&Expr>) -> Value {
        if let Some(src_expr) = source {
            let path = self.eval_string(src_expr);
            return self.getline_from_file(&path, var);
        }

        // No source: read the next record from the current input stream
        let record = {
            if let Some(ref mut inp) = self.input {
                match inp.next_record() {
                    Ok(Some(r)) => Some(r),
                    Ok(None) => return Value::from_number(0.0),
                    Err(_) => return Value::from_number(-1.0),
                }
            } else {
                None
            }
        };

        if let Some(rec) = record {
            match var {
                Some(name) => self.rt.set_var(name, &rec.text),
                None => {
                    match &rec.fields {
                        Some(fields) => self.rt.set_record_fields(&rec.text, fields.clone()),
                        None => self.rt.set_record(&rec.text),
                    }
                }
            }
            self.rt.increment_nr();
            return Value::from_number(1.0);
        }

        // Fallback: no attached input (e.g. in BEGIN), read raw stdin
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) => Value::from_number(0.0),
            Ok(_) => {
                if line.ends_with('\n') { line.pop(); }
                if line.ends_with('\r') { line.pop(); }
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

    fn getline_from_file(&mut self, path: &str, var: Option<&str>) -> Value {
        if !self.input_files.contains_key(path) {
            match std::fs::File::open(path) {
                Ok(file) => {
                    self.input_files.insert(
                        path.to_string(),
                        std::io::BufReader::new(file),
                    );
                }
                Err(_) => return Value::from_number(-1.0),
            }
        }
        let reader = self.input_files.get_mut(path).unwrap();
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => Value::from_number(0.0),
            Ok(_) => {
                if line.ends_with('\n') { line.pop(); }
                if line.ends_with('\r') { line.pop(); }
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

    pub(crate) fn exec_getline_pipe(&mut self, cmd: &str, var: Option<&str>) -> Value {
        if !self.input_pipe_readers.contains_key(cmd) {
            match Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .stdout(Stdio::piped())
                .spawn()
            {
                Ok(mut child) => {
                    let stdout = child.stdout.take().unwrap();
                    self.input_pipe_readers.insert(
                        cmd.to_string(),
                        std::io::BufReader::new(stdout),
                    );
                    self.input_pipe_children.insert(cmd.to_string(), child);
                }
                Err(_) => return Value::from_number(-1.0),
            }
        }
        let reader = self.input_pipe_readers.get_mut(cmd).unwrap();
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => Value::from_number(0.0),
            Ok(_) => {
                if line.ends_with('\n') { line.pop(); }
                if line.ends_with('\r') { line.pop(); }
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
    // ── Diagnostics ─────────────────────────────────────────────────

    /// dump(x [, file]) — write detailed variable/array info to stderr or file.
    pub(crate) fn builtin_dump(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            eprintln!("fk: dump requires at least 1 argument");
            return Value::from_number(0.0);
        }

        let mut buf = String::new();
        let label = match &args[0] {
            Expr::Var(name) => name.clone(),
            Expr::ArrayRef(name, _) => name.clone(),
            Expr::Field(idx) => format!("${}", self.eval_field_idx(idx)),
            _ => "expr".to_string(),
        };

        if let Expr::Var(name) = &args[0]
            && self.rt.has_array(name)
        {
            let mut keys = self.rt.array_keys(name);
            smart_sort_keys(&mut keys);
            let sequential = is_sequential(&keys);
            buf.push_str(&format!("dump: {} = array ({} elements{})\n",
                name, keys.len(),
                if sequential { ", sequential" } else { "" }));
            for k in &keys {
                let v = self.rt.get_array(name, k);
                buf.push_str(&format!("  [{}] = \"{}\"\n", k, v));
            }
            return self.dump_output(&buf, args);
        }

        let val = self.eval_expr(&args[0]);
        let type_str = if val.is_numeric() { "number" } else { "string" };
        let num = val.to_number();
        let s = val.to_string_val();
        buf.push_str(&format!("dump: {} = ({}) \"{}\"\n", label, type_str, s));
        if !val.is_numeric() && num != 0.0 {
            buf.push_str(&format!("  numeric coercion = {}\n", num));
        }
        buf.push_str(&format!("  length = {}\n", s.chars().count()));

        self.dump_output(&buf, args)
    }

    fn dump_output(&mut self, buf: &str, args: &[Expr]) -> Value {
        if args.len() >= 2 {
            let filename = self.eval_string(&args[1]);
            match std::fs::OpenOptions::new().create(true).append(true).open(&filename) {
                Ok(mut f) => { let _ = f.write_all(buf.as_bytes()); }
                Err(e) => eprintln!("fk: dump: {}: {}", filename, e),
            }
        } else {
            eprint!("{}", buf);
        }
        Value::from_number(1.0)
    }

    /// clock() — monotonic seconds since program start.
    pub(crate) fn builtin_clock(&self) -> Value {
        let elapsed = self.epoch.elapsed();
        Value::from_number(elapsed.as_secs_f64())
    }

    /// start([id]) — start or restart a named timer. Returns 0.
    pub(crate) fn builtin_start(&mut self, args: &[Expr]) -> Value {
        let id = if args.is_empty() {
            String::new()
        } else {
            self.eval_string(&args[0])
        };
        self.timers.insert(id, Instant::now());
        Value::from_number(0.0)
    }

    /// elapsed([id]) — seconds since start(id). Falls back to program start.
    pub(crate) fn builtin_elapsed(&mut self, args: &[Expr]) -> Value {
        let id = if args.is_empty() {
            String::new()
        } else {
            self.eval_string(&args[0])
        };
        let t = self.timers.get(&id).unwrap_or(&self.epoch);
        Value::from_number(t.elapsed().as_secs_f64())
    }
}

fn extract_array_name(args: &[Expr]) -> Option<String> {
    if args.is_empty() { return None; }
    match &args[0] {
        Expr::Var(name) => Some(name.clone()),
        _ => None,
    }
}

/// Sort keys numeric-first (1, 2, 10 before "apple", "banana").
fn smart_sort_keys(keys: &mut [String]) {
    keys.sort_by(|a, b| {
        let na = a.parse::<f64>();
        let nb = b.parse::<f64>();
        match (na, nb) {
            (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Err(_), Err(_)) => a.cmp(b),
        }
    });
}

/// Check if sorted keys are sequential 1..N.
fn is_sequential(sorted_keys: &[String]) -> bool {
    sorted_keys.iter().enumerate().all(|(i, k)| {
        k.parse::<usize>().is_ok_and(|n| n == i + 1)
    })
}
