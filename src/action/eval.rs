use std::io::Write;

use crate::builtins::{self, format_printf};
use crate::parser::{BinOp, Expr};
use crate::runtime::Value;

use super::{bool_val, Executor};

impl<'a> Executor<'a> {
    pub(crate) fn eval_expr(&mut self, expr: &Expr) -> Value {
        match expr {
            Expr::NumberLit(n) => Value::from_number(*n),
            Expr::StringLit(s) => Value::from_string(s.clone()),
            Expr::Var(name) => self.rt.get_value(name),
            Expr::Field(idx_expr) => {
                let idx = self.eval_field_idx(idx_expr);
                Value::from_string(self.rt.get_field(idx))
            }
            Expr::ArrayRef(name, key_expr) => {
                let key = self.eval_expr(key_expr).into_string();
                self.rt.get_array_value(name, &key)
            }
            Expr::ArrayIn(key_expr, array) => {
                let key = self.eval_expr(key_expr).into_string();
                bool_val(self.rt.array_has_key(array, &key))
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
                let l = self.eval_expr(left);
                let r = self.eval_expr(right);
                let convfmt = self.rt.convfmt();
                let mut s = if l.is_numeric_only() && convfmt != "%.6g" {
                    builtins::format_number_fmt(l.to_number(), convfmt)
                } else {
                    l.into_string()
                };
                if r.is_numeric_only() && convfmt != "%.6g" {
                    s.push_str(&builtins::format_number_fmt(r.to_number(), convfmt));
                } else {
                    r.write_to_string(&mut s);
                }
                Value::from_string(s)
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
                    "length" if args.is_empty() => {
                        let s = self.rt.get_field(0);
                        return Value::from_number(s.chars().count() as f64);
                    }
                    "length" if args.len() == 1 => {
                        if let Expr::Var(var_name) = &args[0]
                            && self.rt.has_array(var_name) {
                                return Value::from_number(self.rt.array_len(var_name) as f64);
                            }
                    }
                    "close" => return self.builtin_close(args),
                    "gensub" => return self.builtin_gensub(args),
                    "fflush" => return self.builtin_fflush(args),
                    "system" => return self.builtin_system(args),
                    "join" => return self.builtin_join(args),
                    "typeof" => return self.builtin_typeof(args),
                    "asort" => return self.builtin_asort(args, false),
                    "asorti" => return self.builtin_asort(args, true),
                    "keys" => return self.builtin_keys(args),
                    "vals" => return self.builtin_vals(args),
                    "uniq" => return self.builtin_uniq(args),
                    "inv" => return self.builtin_invert(args),
                    "tidy" => return self.builtin_compact(args),
                    "shuf" => return self.builtin_shuffle(args),
                    "diff" | "inter" | "union" => return self.builtin_set_op(name, args),
                    "seq" => return self.builtin_seq(args),
                    "samp" => return self.builtin_sample(args),
                    "slurp" => return self.builtin_slurp(args),
                    "dump" => return self.builtin_dump(args),
                    "clk" | "clock" => return self.builtin_clock(),
                    "tic" | "start" => return self.builtin_start(args),
                    "toc" | "elapsed" => return self.builtin_elapsed(args),
                    "and" | "or" | "xor" | "lshift" | "rshift" | "compl" => {
                        return self.builtin_bitwise(name, args);
                    }
                    "sum" | "mean" | "median" | "stddev" | "variance"
                    | "percentile" | "p" | "iqm" | "quantile" => {
                        return self.builtin_stats(name, args);
                    }
                    "min" if args.len() == 1 => {
                        if let Expr::Var(v) = &args[0]
                            && self.rt.has_array(v) {
                                return self.builtin_stats("min", args);
                        }
                    }
                    "max" if args.len() == 1 => {
                        if let Expr::Var(v) = &args[0]
                            && self.rt.has_array(v) {
                                return self.builtin_stats("max", args);
                        }
                    }
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
    pub(crate) fn eval_string(&mut self, expr: &Expr) -> String {
        self.eval_expr(expr).into_string()
    }

    /// Write an expression's value directly to stdout, bypassing Value
    /// construction when possible (zero-copy for field literals).
    #[inline]
    pub(crate) fn print_expr_fast(&mut self, expr: &Expr) {
        if let Expr::Field(idx_expr) = expr
            && let Expr::NumberLit(n) = idx_expr.as_ref() {
                let idx = self.resolve_field_idx(*n);
                self.rt.write_field_to(idx, &mut self.stdout);
                return;
        }
        let val = self.eval_expr(expr);
        if val.is_numeric_only() {
            let ofmt = self.rt.ofmt();
            if ofmt != "%.6g" {
                let s = builtins::format_number_fmt(val.to_number(), ofmt);
                let _ = self.stdout.write_all(s.as_bytes());
                return;
            }
        }
        val.write_to(&mut self.stdout);
    }

    /// Resolve a field index, supporting negative values ($-1 = last field).
    pub(crate) fn resolve_field_idx(&self, n: f64) -> usize {
        let i = n as isize;
        if i >= 0 {
            i as usize
        } else {
            let nf = self.rt.fields.len() as isize;
            let resolved = nf + 1 + i;
            if resolved < 0 { 0 } else { resolved as usize }
        }
    }

    /// Evaluate a field index expression: numeric -> direct index,
    /// non-numeric string -> HDR lookup (for named column access).
    pub(crate) fn eval_field_idx(&mut self, idx_expr: &Expr) -> usize {
        if let Expr::NumberLit(n) = idx_expr {
            return self.resolve_field_idx(*n);
        }
        let val = self.eval_expr(idx_expr);
        if val.is_numeric() {
            return self.resolve_field_idx(val.to_number());
        }
        let s = val.to_string_val();
        let n = crate::builtins::to_number(&s);
        if n != 0.0 || s == "0" {
            return self.resolve_field_idx(n);
        }
        if let Some(idx_val) = self.rt.get_array_opt("HDR", &s) {
            let idx = crate::builtins::to_number(&idx_val);
            if idx > 0.0 {
                return self.resolve_field_idx(idx);
            }
        }
        0
    }

    pub(crate) fn eval_lvalue(&mut self, expr: &Expr) -> Value {
        match expr {
            Expr::Var(name) => self.rt.get_value(name),
            Expr::ArrayRef(name, key_expr) => {
                let key = self.eval_string(key_expr);
                self.rt.get_array_value(name, &key)
            }
            Expr::Field(idx_expr) => {
                let idx = self.eval_field_idx(idx_expr);
                Value::from_string(self.rt.get_field(idx))
            }
            _ => Value::default(),
        }
    }

    pub(crate) fn assign_to(&mut self, target: &Expr, value: Value) {
        match target {
            Expr::Var(name) => self.rt.set_value(name, value),
            Expr::ArrayRef(name, key_expr) => {
                let key = self.eval_string(key_expr);
                self.rt.set_array_value(name, &key, value);
            }
            Expr::Field(idx_expr) => {
                let idx = self.eval_field_idx(idx_expr);
                self.rt.set_field(idx, &value.into_string());
            }
            _ => {}
        }
    }
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

pub(crate) fn eval_binop(left: Value, op: &BinOp, right: Value) -> Value {
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
