use super::to_number;

/// Parsed format flags from a printf conversion specifier.
struct FmtFlags {
    left_align: bool,
    zero_pad: bool,
    force_sign: bool,
    space_sign: bool,
    width: usize,
    precision: Option<usize>,
}

/// Parse flags, width, and precision from the characters between '%' and the
/// conversion letter.  Handles flags: `-`, `0`, `+`, ` ` (space).
fn parse_flags(spec: &str) -> FmtFlags {
    let bytes = spec.as_bytes();
    let mut i = 0;
    let mut left_align = false;
    let mut zero_pad = false;
    let mut force_sign = false;
    let mut space_sign = false;

    while i < bytes.len() {
        match bytes[i] {
            b'-' => left_align = true,
            b'0' if i == 0 || !bytes[..i].iter().any(|b| b.is_ascii_digit() && *b != b'0') => {
                zero_pad = true;
            }
            b'+' => force_sign = true,
            b' ' => space_sign = true,
            _ => break,
        }
        i += 1;
    }

    let rest = &spec[i..];
    let (width, precision) = if let Some(dot_pos) = rest.find('.') {
        let w: usize = rest[..dot_pos].parse().unwrap_or(0);
        let p: usize = rest[dot_pos + 1..].parse().unwrap_or(6);
        (w, Some(p))
    } else {
        let w: usize = rest.parse().unwrap_or(0);
        (w, None)
    };

    if left_align {
        zero_pad = false;
    }

    FmtFlags { left_align, zero_pad, force_sign, space_sign, width, precision }
}

/// Apply width / alignment / padding to an already-formatted string.
fn apply_width(s: &str, flags: &FmtFlags, pad: char) -> String {
    if flags.width == 0 || s.len() >= flags.width {
        return s.to_string();
    }
    if flags.left_align {
        format!("{:<width$}", s, width = flags.width)
    } else {
        let fill = String::from(pad).repeat(flags.width - s.len());
        format!("{}{}", fill, s)
    }
}

/// printf implementation supporting %d, %i, %f, %g, %e, %s, %c, %x, %o, %%.
/// Flags: `-` (left-align), `0` (zero-pad), `+` (force sign), ` ` (space sign).
pub fn format_printf(fmt: &str, args: &[String]) -> String {
    let mut result = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    let mut arg_idx = 0;

    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            i += 1;
            let mut spec = String::new();
            while i < chars.len() && !chars[i].is_ascii_alphabetic() && chars[i] != '%' {
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
            let flags = parse_flags(&spec);
            match conv {
                '%' => result.push('%'),
                'd' | 'i' => {
                    let val = args.get(arg_idx).map(|s| to_number(s)).unwrap_or(0.0) as i64;
                    arg_idx += 1;
                    let prefix = if val < 0 { "" } else if flags.force_sign { "+" } else if flags.space_sign { " " } else { "" };
                    let s = format!("{}{}", prefix, val);
                    let pad = if flags.zero_pad { '0' } else { ' ' };
                    if flags.zero_pad && (val < 0 || flags.force_sign || flags.space_sign) {
                        let sign = &s[..1];
                        let digits = &s[1..];
                        if flags.width > s.len() {
                            let zeros = "0".repeat(flags.width - s.len());
                            result.push_str(&format!("{}{}{}", sign, zeros, digits));
                        } else {
                            result.push_str(&s);
                        }
                    } else {
                        result.push_str(&apply_width(&s, &flags, pad));
                    }
                }
                'f' | 'e' => {
                    let val = args.get(arg_idx).map(|s| to_number(s)).unwrap_or(0.0);
                    arg_idx += 1;
                    let prec = flags.precision.unwrap_or(6);
                    let prefix = if val < 0.0 || val.is_sign_negative() { "" } else if flags.force_sign { "+" } else if flags.space_sign { " " } else { "" };
                    let s = if conv == 'e' {
                        format!("{}{:.*e}", prefix, prec, val)
                    } else {
                        format!("{}{:.*}", prefix, prec, val)
                    };
                    let pad = if flags.zero_pad { '0' } else { ' ' };
                    if flags.zero_pad && s.len() < flags.width && !s.is_empty() {
                        let first = s.as_bytes()[0];
                        if first == b'-' || first == b'+' || first == b' ' {
                            let sign = &s[..1];
                            let rest = &s[1..];
                            let zeros = "0".repeat(flags.width - s.len());
                            result.push_str(&format!("{}{}{}", sign, zeros, rest));
                        } else {
                            result.push_str(&apply_width(&s, &flags, '0'));
                        }
                    } else {
                        result.push_str(&apply_width(&s, &flags, pad));
                    }
                }
                'g' => {
                    let val = args.get(arg_idx).map(|s| to_number(s)).unwrap_or(0.0);
                    arg_idx += 1;
                    let prec = flags.precision.unwrap_or(6);
                    let prefix = if val < 0.0 || val.is_sign_negative() { "" } else if flags.force_sign { "+" } else if flags.space_sign { " " } else { "" };
                    let s_f = format!("{:.*}", prec, val);
                    let s_e = format!("{:.*e}", prec, val);
                    let formatted = if s_f.len() <= s_e.len() { s_f } else { s_e };
                    let trimmed = formatted.trim_end_matches('0');
                    let trimmed = trimmed.trim_end_matches('.');
                    let s = format!("{}{}", prefix, trimmed);
                    let pad = if flags.zero_pad { '0' } else { ' ' };
                    result.push_str(&apply_width(&s, &flags, pad));
                }
                'x' => {
                    let val = args.get(arg_idx).map(|s| to_number(s)).unwrap_or(0.0) as i64;
                    arg_idx += 1;
                    let s = format!("{:x}", val);
                    let pad = if flags.zero_pad { '0' } else { ' ' };
                    result.push_str(&apply_width(&s, &flags, pad));
                }
                'o' => {
                    let val = args.get(arg_idx).map(|s| to_number(s)).unwrap_or(0.0) as i64;
                    arg_idx += 1;
                    let s = format!("{:o}", val);
                    let pad = if flags.zero_pad { '0' } else { ' ' };
                    result.push_str(&apply_width(&s, &flags, pad));
                }
                's' => {
                    let val = args.get(arg_idx).map(|s| s.as_str()).unwrap_or("");
                    arg_idx += 1;
                    let val = if let Some(prec) = flags.precision {
                        &val[..val.len().min(prec)]
                    } else {
                        val
                    };
                    result.push_str(&apply_width(val, &flags, ' '));
                }
                'c' => {
                    if let Some(s) = args.get(arg_idx)
                        && let Some(ch) = s.chars().next() {
                            result.push(ch);
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
