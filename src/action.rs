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

    fn match_pattern(&self, pattern: &Option<Pattern>, line: &str) -> bool {
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
            Statement::Expression(expr) => {
                self.eval_expr(expr);
            }
        }
    }

    fn eval_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::NumberLit(n) => format_number(*n),
            Expr::StringLit(s) => s.clone(),
            Expr::Var(name) => self.rt.get_var(name),
            Expr::Field(idx_expr) => {
                let idx_str = self.eval_expr(idx_expr);
                let idx: usize = idx_str.parse::<f64>().unwrap_or(0.0) as usize;
                self.rt.get_field(idx)
            }
            Expr::BinOp(left, op, right) => {
                let l = self.eval_expr(left);
                let r = self.eval_expr(right);
                self.eval_binop(&l, op, &r)
            }
            Expr::Assign(name, value) => {
                let val = self.eval_expr(value);
                // Safety: we need mutable access here. The borrow checker
                // is satisfied because we compute val before mutating.
                let rt = unsafe { &mut *(self.rt as *const Runtime as *mut Runtime) };
                rt.set_var(name, &val);
                val
            }
            Expr::UnaryMinus(inner) => {
                let val = self.eval_expr(inner);
                let n = to_number(&val);
                format_number(-n)
            }
        }
    }

    fn eval_binop(&self, left: &str, op: &BinOp, right: &str) -> String {
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
            BinOp::Concat => format!("{}{}", left, right),
        }
    }
}

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
