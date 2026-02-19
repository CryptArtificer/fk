use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::{Command, Stdio};

use crate::builtins::format_printf;
use crate::parser::{Block, Expr, FuncDef, Redirect, Statement};
use crate::runtime::Value;

use super::{Executor, MAX_CALL_DEPTH, Signal};

impl<'a> Executor<'a> {
    pub(crate) fn exec_block(&mut self, block: &Block) -> Option<Signal> {
        for stmt in block {
            if let Some(signal) = self.exec_stmt(stmt) {
                return Some(signal);
            }
        }
        None
    }

    pub(crate) fn exec_stmt(&mut self, stmt: &Statement) -> Option<Signal> {
        match stmt {
            Statement::Print(exprs, redir) => {
                if redir.is_none() {
                    if exprs.len() == 1 {
                        if let Expr::Var(name) = &exprs[0]
                            && self.rt.has_array(name)
                        {
                            self.print_array(name);
                        } else {
                            self.print_expr_fast(&exprs[0]);
                            let _ = self.stdout.write_all(self.rt.ors().as_bytes());
                        }
                    } else {
                        let ofs = self.rt.ofs().to_owned();
                        for (i, e) in exprs.iter().enumerate() {
                            if i > 0 {
                                let _ = self.stdout.write_all(ofs.as_bytes());
                            }
                            self.print_expr_fast(e);
                        }
                        let _ = self.stdout.write_all(self.rt.ors().as_bytes());
                    }
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
                    && let Some(signal) = self.exec_block(eb)
                {
                    return Some(signal);
                }
            }
            Statement::While(cond, body) => loop {
                if !self.eval_expr(cond).is_truthy() {
                    break;
                }
                match self.exec_block(body) {
                    Some(Signal::Break) => break,
                    Some(Signal::Continue) => continue,
                    Some(signal) => return Some(signal),
                    None => {}
                }
            },
            Statement::DoWhile(body, cond) => loop {
                match self.exec_block(body) {
                    Some(Signal::Break) => break,
                    Some(Signal::Continue) => {}
                    Some(signal) => return Some(signal),
                    None => {}
                }
                if !self.eval_expr(cond).is_truthy() {
                    break;
                }
            },
            Statement::For(init, cond, update, body) => {
                if let Some(init_stmt) = init
                    && let Some(signal) = self.exec_stmt(init_stmt)
                {
                    match signal {
                        Signal::Return(_) | Signal::Exit(_) => return Some(signal),
                        _ => {}
                    }
                }
                loop {
                    if let Some(cond_expr) = cond
                        && !self.eval_expr(cond_expr).is_truthy()
                    {
                        break;
                    }
                    match self.exec_block(body) {
                        Some(Signal::Break) => break,
                        Some(Signal::Continue) => {}
                        Some(signal) => return Some(signal),
                        None => {}
                    }
                    if let Some(update_stmt) = update
                        && let Some(signal) = self.exec_stmt(update_stmt)
                    {
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
                let _ = self.eval_expr(expr);
            }
        }
        None
    }

    pub(crate) fn write_output(&mut self, text: &str, redir: &Option<Redirect>) {
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
                            Command::new("cat")
                                .stdin(Stdio::piped())
                                .stdout(Stdio::null())
                                .spawn()
                                .unwrap()
                        })
                });
                if let Some(ref mut stdin) = child.stdin {
                    let _ = stdin.write_all(text.as_bytes());
                }
            }
        }
    }

    pub(crate) fn call_user_func(&mut self, func: &FuncDef, args: &[String]) -> Value {
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
}
