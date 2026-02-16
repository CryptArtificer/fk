use super::{to_number, format_number};

/// Dispatch math built-in functions.
pub fn call(name: &str, args: &[String]) -> String {
    let n = args.first().map(|s| to_number(s)).unwrap_or(0.0);
    match name {
        "int" => format_number(n.trunc()),
        "sin" => format!("{:.6}", n.sin()),
        "cos" => format!("{:.6}", n.cos()),
        "sqrt" => format_number(n.sqrt()),
        "log" => format!("{:.6}", n.ln()),
        "exp" => format!("{:.6}", n.exp()),
        _ => String::new(),
    }
}
