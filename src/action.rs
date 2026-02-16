use crate::parser::{BinOp, Block, Expr, Pattern, Program, Statement};
use crate::runtime::Runtime;

/// Execute a parsed program against the runtime.
pub struct Executor<'a> {
    program: &'a Program,
    rt: &'a mut Runtime,
}

impl<'a> Executor<'a> {
    pub fn new(program: &'a Program, rt: &'a mut Runtime) -> Self {
        Executor { program, rt }
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
    }

    pub fn run_record(&mut self, line: &str) {
        self.rt.increment_nr();
        self.rt.set_record(line);

        for rule in &self.program.rules {
            if self.match_pattern(&rule.pattern, line) {
                self.exec_block(&rule.action);
            }
        }
    }

    fn match_pattern(&mut self, pattern: &Option<Pattern>, line: &str) -> bool {
        match pattern {
            None => true,
            Some(Pattern::Regex(pat)) => {
                line.contains(pat.as_str())
            }
            Some(Pattern::Expression(expr)) => {
                let val = self.eval_expr(expr);
                is_truthy(&val)
            }
        }
    }

    fn exec_block(&mut self, block: &Block) {
        for stmt in block {
            self.exec_stmt(stmt);
        }
    }

    fn exec_stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Print(exprs) => {
                let ofs = self.rt.get_var("OFS");
                let ors = self.rt.get_var("ORS");
                let parts: Vec<String> = exprs.iter().map(|e| self.eval_expr(e)).collect();
                print!("{}{}", parts.join(&ofs), ors);
            }
            Statement::Printf(exprs) => {
                if exprs.is_empty() {
                    return;
                }
                let args: Vec<String> = exprs.iter().map(|e| self.eval_expr(e)).collect();
                let result = format_printf(&args[0], &args[1..]);
                print!("{}", result);
            }
            Statement::If(cond, then_block, else_block) => {
                let val = self.eval_expr(cond);
                if is_truthy(&val) {
                    self.exec_block(then_block);
                } else if let Some(eb) = else_block {
                    self.exec_block(eb);
                }
            }
            Statement::While(cond, body) => {
                loop {
                    let val = self.eval_expr(cond);
                    if !is_truthy(&val) {
                        break;
                    }
                    self.exec_block(body);
                }
            }
            Statement::For(init, cond, update, body) => {
                if let Some(init_stmt) = init {
                    self.exec_stmt(init_stmt);
                }
                loop {
                    if let Some(cond_expr) = cond {
                        let val = self.eval_expr(cond_expr);
                        if !is_truthy(&val) {
                            break;
                        }
                    }
                    self.exec_block(body);
                    if let Some(update_stmt) = update {
                        self.exec_stmt(update_stmt);
                    }
                }
            }
            Statement::ForIn(var, array, body) => {
                let keys = self.rt.array_keys(array);
                for key in keys {
                    self.rt.set_var(var, &key);
                    self.exec_block(body);
                }
            }
            Statement::Delete(array, key_expr) => {
                let key = self.eval_expr(key_expr);
                self.rt.delete_array(array, &key);
            }
            Statement::Block(block) => {
                self.exec_block(block);
            }
            Statement::Expression(expr) => {
                self.eval_expr(expr);
            }
        }
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
                let evaled: Vec<String> = args.iter().map(|e| self.eval_expr(e)).collect();
                call_builtin(name, &evaled)
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
}

// --- helper functions ---

fn to_number(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(0.0)
}

fn format_number(n: f64) -> String {
    if n == (n as i64) as f64 {
        format!("{}", n as i64)
    } else {
        format!("{:.6}", n)
    }
}

fn is_truthy(s: &str) -> bool {
    !s.is_empty() && s != "0"
}

fn bool_str(b: bool) -> String {
    if b { "1".to_string() } else { "0".to_string() }
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
        BinOp::Eq => bool_str(left == right || to_number(left) == to_number(right)),
        BinOp::Ne => bool_str(left != right && to_number(left) != to_number(right)),
        BinOp::Lt => bool_str(to_number(left) < to_number(right)),
        BinOp::Le => bool_str(to_number(left) <= to_number(right)),
        BinOp::Gt => bool_str(to_number(left) > to_number(right)),
        BinOp::Ge => bool_str(to_number(left) >= to_number(right)),
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
