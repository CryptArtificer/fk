use super::to_number;

/// Minimal printf implementation supporting %d, %s, %f, %%, \n, \t.
pub fn format_printf(fmt: &str, args: &[String]) -> String {
    let mut result = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    let mut arg_idx = 0;

    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            i += 1;
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
                        result.push_str(&format_with_spec_d(val, &spec));
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

fn format_with_spec_d(val: i64, spec: &str) -> String {
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
