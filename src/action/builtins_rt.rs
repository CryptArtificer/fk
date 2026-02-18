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

    /// close(name) — close an output file or pipe by name.
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
        match &args[0] {
            Expr::Var(name) => {
                if self.rt.has_array(name) {
                    Value::from_string("array".to_string())
                } else if !self.rt.has_var(name) {
                    Value::from_string("uninitialized".to_string())
                } else {
                    let val = self.rt.get_value(name);
                    if val.is_numeric() {
                        Value::from_string("number".to_string())
                    } else {
                        Value::from_string("string".to_string())
                    }
                }
            }
            Expr::NumberLit(_) => Value::from_string("number".to_string()),
            Expr::StringLit(_) => Value::from_string("string".to_string()),
            _ => {
                let val = self.eval_expr(&args[0]);
                if val.is_numeric() {
                    Value::from_string("number".to_string())
                } else {
                    Value::from_string("string".to_string())
                }
            }
        }
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

        let vals = self.array_sorted_values(&array_name);
        if vals.is_empty() {
            return Value::from_number(0.0);
        }
        let n = vals.len();

        match name {
            "sum" => {
                let s: f64 = vals.iter().sum();
                Value::from_number(s)
            }
            "mean" => {
                let s: f64 = vals.iter().sum();
                Value::from_number(s / n as f64)
            }
            "median" => {
                Value::from_number(percentile_sorted(&vals, 50.0))
            }
            "variance" => {
                let mean: f64 = vals.iter().sum::<f64>() / n as f64;
                let var: f64 = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
                Value::from_number(var)
            }
            "stddev" => {
                let mean: f64 = vals.iter().sum::<f64>() / n as f64;
                let var: f64 = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
                Value::from_number(var.sqrt())
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
            "min" => {
                Value::from_number(vals[0])
            }
            "max" => {
                Value::from_number(vals[n - 1])
            }
            _ => Value::from_number(0.0),
        }
    }

    pub(crate) fn exec_getline(&mut self, var: Option<&str>, source: Option<&Expr>) -> Value {
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

    pub(crate) fn exec_getline_pipe(&mut self, cmd: &str, var: Option<&str>) -> Value {
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
