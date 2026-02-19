/// Lightweight JSON path access — "jq light".
///
/// `jpath(json_string, path)` navigates into a JSON value using a dot/bracket
/// path and returns the result as a string.
///
/// Path syntax:
///   .key        — object key
///   [N]         — array index (0-based)
///   .key.sub    — chained access
///   .key[0]     — mixed object + array
///   .           — root (identity, returns whole value)
///
/// Scalars are returned as their bare value (no quotes around strings).
/// Objects and arrays are returned as compact JSON text.
///
/// Minimal JSON value representation.
#[derive(Debug, Clone)]
enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

// ── public entry points ─────────────────────────────────────────

/// `jpath(json, path)` → string.
///
/// If the path resolves to a single value, returns that value.
/// If it resolves to multiple values (via `[]` or implicit iteration),
/// returns them joined by newlines.
pub fn call(args: &[String]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let json_str = &args[0];
    let path = if args.len() > 1 { &args[1] } else { "." };

    let val = match parse_value(&mut json_str.trim().chars().peekable()) {
        Some(v) => v,
        None => return String::new(),
    };

    let steps = parse_path(path);
    let results = navigate_multi(&val, &steps);

    if results.is_empty() {
        String::new()
    } else if results.len() == 1 {
        value_to_string(results[0])
    } else {
        results
            .iter()
            .map(|v| value_to_string(v))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Extract values at `path` into a list of (key, value) pairs for populating
/// an awk array.
///
/// Multi-value results (from `[]` or implicit iteration):
///   → `[("1", val1), ("2", val2), ...]`
///
/// Single value:
///   - Array  → `[("1", elem1), ("2", elem2), ...]`
///   - Object → `[("key1", val1), ("key2", val2), ...]`
///   - Scalar → `[("0", scalar)]`
pub fn extract(json_str: &str, path: &str) -> Vec<(String, String)> {
    let val = match parse_value(&mut json_str.trim().chars().peekable()) {
        Some(v) => v,
        None => return Vec::new(),
    };

    let steps = parse_path(path);
    let results = navigate_multi(&val, &steps);

    if results.is_empty() {
        return Vec::new();
    }

    // Multiple results from iteration: number them 1..N
    if results.len() > 1 {
        return results
            .iter()
            .enumerate()
            .map(|(i, v)| ((i + 1).to_string(), value_to_string(v)))
            .collect();
    }

    // Single result: expand arrays/objects
    match results[0] {
        Value::Array(arr) => arr
            .iter()
            .enumerate()
            .map(|(i, v)| ((i + 1).to_string(), value_to_string(v)))
            .collect(),
        Value::Object(pairs) => pairs
            .iter()
            .map(|(k, v)| (k.clone(), value_to_string(v)))
            .collect(),
        other => vec![("0".to_string(), value_to_string(other))],
    }
}

// ── path parsing ────────────────────────────────────────────────

#[derive(Debug)]
enum Step {
    Key(String),
    Index(usize),
    Iterate, // [] — expand all elements of an array or object
}

fn parse_path(path: &str) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut chars = path.chars().peekable();

    // Skip leading dot (identity / root)
    if chars.peek() == Some(&'.') {
        chars.next();
    }

    while chars.peek().is_some() {
        if chars.peek() == Some(&'[') {
            chars.next(); // skip '['
            let mut content = String::new();
            while let Some(&ch) = chars.peek() {
                if ch == ']' {
                    chars.next();
                    break;
                }
                content.push(ch);
                chars.next();
            }
            let trimmed = content.trim();
            if trimmed.is_empty() {
                steps.push(Step::Iterate);
            } else if let Ok(idx) = trimmed.parse::<usize>() {
                steps.push(Step::Index(idx));
            }
        } else if chars.peek() == Some(&'.') {
            chars.next(); // skip '.'
        } else {
            let mut key = String::new();
            while let Some(&ch) = chars.peek() {
                if ch == '.' || ch == '[' {
                    break;
                }
                key.push(ch);
                chars.next();
            }
            if !key.is_empty() {
                steps.push(Step::Key(key));
            }
        }
    }
    steps
}

/// Navigate a JSON value following path steps.
///
/// Supports multi-value expansion:
/// - `[]` (Iterate) expands all array elements or object values.
/// - `.key` applied to an array implicitly iterates and projects the key
///   from each element (like jq's `.items.name` ≈ `.items[].name`).
fn navigate_multi<'a>(val: &'a Value, steps: &[Step]) -> Vec<&'a Value> {
    let mut current = vec![val];
    for step in steps {
        let mut next = Vec::new();
        for v in &current {
            match (step, *v) {
                (Step::Key(k), Value::Object(pairs)) => {
                    if let Some((_, val)) = pairs.iter().find(|(key, _)| key == k) {
                        next.push(val);
                    }
                }
                // Implicit iteration: .key on an array projects into each element
                (Step::Key(k), Value::Array(arr)) => {
                    for elem in arr {
                        if let Value::Object(pairs) = elem
                            && let Some((_, val)) = pairs.iter().find(|(key, _)| key == k)
                        {
                            next.push(val);
                        }
                    }
                }
                (Step::Index(i), Value::Array(arr)) => {
                    if let Some(val) = arr.get(*i) {
                        next.push(val);
                    }
                }
                (Step::Iterate, Value::Array(arr)) => {
                    next.extend(arr.iter());
                }
                (Step::Iterate, Value::Object(pairs)) => {
                    next.extend(pairs.iter().map(|(_, v)| v));
                }
                _ => {}
            }
        }
        current = next;
    }
    current
}

// ── value → string ──────────────────────────────────────────────

fn value_to_string(val: &Value) -> String {
    match val {
        Value::Null => String::new(),
        Value::Bool(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Value::Number(n) => super::format_number(*n),
        Value::Str(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => to_json(val),
    }
}

fn to_json(val: &Value) -> String {
    match val {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => super::format_number(*n),
        Value::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(to_json).collect();
            format!("[{}]", items.join(","))
        }
        Value::Object(pairs) => {
            let items: Vec<String> = pairs
                .iter()
                .map(|(k, v)| {
                    format!(
                        "\"{}\":{}",
                        k.replace('\\', "\\\\").replace('"', "\\\""),
                        to_json(v)
                    )
                })
                .collect();
            format!("{{{}}}", items.join(","))
        }
    }
}

// ── minimal JSON parser ─────────────────────────────────────────

use std::iter::Peekable;
use std::str::Chars;

fn parse_value(chars: &mut Peekable<Chars>) -> Option<Value> {
    skip_ws(chars);
    match chars.peek()? {
        '"' => parse_string(chars).map(Value::Str),
        '{' => parse_object(chars),
        '[' => parse_array(chars),
        't' | 'f' => parse_bool(chars),
        'n' => parse_null(chars),
        _ => parse_number(chars),
    }
}

fn parse_string(chars: &mut Peekable<Chars>) -> Option<String> {
    if chars.next()? != '"' {
        return None;
    }
    let mut s = String::new();
    loop {
        match chars.next()? {
            '"' => return Some(s),
            '\\' => match chars.next()? {
                '"' => s.push('"'),
                '\\' => s.push('\\'),
                '/' => s.push('/'),
                'n' => s.push('\n'),
                't' => s.push('\t'),
                'r' => s.push('\r'),
                'u' => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        hex.push(chars.next()?);
                    }
                    if let Ok(cp) = u32::from_str_radix(&hex, 16)
                        && let Some(ch) = char::from_u32(cp)
                    {
                        s.push(ch);
                    }
                }
                other => {
                    s.push('\\');
                    s.push(other);
                }
            },
            ch => s.push(ch),
        }
    }
}

fn parse_number(chars: &mut Peekable<Chars>) -> Option<Value> {
    let mut buf = String::new();
    if chars.peek() == Some(&'-') {
        buf.push(chars.next()?);
    }
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() || ch == '.' || ch == 'e' || ch == 'E' || ch == '+' || ch == '-' {
            if (ch == '+' || ch == '-') && !buf.ends_with('e') && !buf.ends_with('E') {
                break;
            }
            buf.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    buf.parse::<f64>().ok().map(Value::Number)
}

fn parse_object(chars: &mut Peekable<Chars>) -> Option<Value> {
    chars.next(); // skip '{'
    let mut pairs = Vec::new();
    skip_ws(chars);
    if chars.peek() == Some(&'}') {
        chars.next();
        return Some(Value::Object(pairs));
    }
    loop {
        skip_ws(chars);
        let key = parse_string(chars)?;
        skip_ws(chars);
        if chars.next()? != ':' {
            return None;
        }
        let val = parse_value(chars)?;
        pairs.push((key, val));
        skip_ws(chars);
        match chars.peek()? {
            ',' => {
                chars.next();
            }
            '}' => {
                chars.next();
                return Some(Value::Object(pairs));
            }
            _ => return None,
        }
    }
}

fn parse_array(chars: &mut Peekable<Chars>) -> Option<Value> {
    chars.next(); // skip '['
    let mut items = Vec::new();
    skip_ws(chars);
    if chars.peek() == Some(&']') {
        chars.next();
        return Some(Value::Array(items));
    }
    loop {
        let val = parse_value(chars)?;
        items.push(val);
        skip_ws(chars);
        match chars.peek()? {
            ',' => {
                chars.next();
            }
            ']' => {
                chars.next();
                return Some(Value::Array(items));
            }
            _ => return None,
        }
    }
}

fn parse_bool(chars: &mut Peekable<Chars>) -> Option<Value> {
    let mut word = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_alphabetic() {
            word.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    match word.as_str() {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        _ => None,
    }
}

fn parse_null(chars: &mut Peekable<Chars>) -> Option<Value> {
    for expected in ['n', 'u', 'l', 'l'] {
        if chars.next()? != expected {
            return None;
        }
    }
    Some(Value::Null)
}

fn skip_ws(chars: &mut Peekable<Chars>) {
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

// ── tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn jp(json: &str, path: &str) -> String {
        call(&[json.to_string(), path.to_string()])
    }

    #[test]
    fn flat_key() {
        assert_eq!(jp(r#"{"name":"Alice","age":30}"#, ".name"), "Alice");
        assert_eq!(jp(r#"{"name":"Alice","age":30}"#, ".age"), "30");
    }

    #[test]
    fn nested_key() {
        assert_eq!(
            jp(r#"{"user":{"name":"Bob","id":42}}"#, ".user.name"),
            "Bob"
        );
    }

    #[test]
    fn array_index() {
        assert_eq!(jp(r#"{"items":[10,20,30]}"#, ".items[1]"), "20");
    }

    #[test]
    fn deep_mixed_path() {
        let json = r#"{"users":[{"name":"Alice"},{"name":"Bob"}]}"#;
        assert_eq!(jp(json, ".users[1].name"), "Bob");
    }

    #[test]
    fn missing_key_returns_empty() {
        assert_eq!(jp(r#"{"a":1}"#, ".b"), "");
    }

    #[test]
    fn identity_returns_json() {
        let json = r#"{"a":1}"#;
        assert_eq!(jp(json, "."), r#"{"a":1}"#);
    }

    #[test]
    fn nested_object_returns_json() {
        let json = r#"{"a":{"b":2}}"#;
        assert_eq!(jp(json, ".a"), r#"{"b":2}"#);
    }

    #[test]
    fn bool_and_null() {
        assert_eq!(jp(r#"{"ok":true}"#, ".ok"), "1");
        assert_eq!(jp(r#"{"x":null}"#, ".x"), "");
    }

    #[test]
    fn extract_array() {
        let pairs = extract(r#"{"items":[10,20,30]}"#, ".items");
        assert_eq!(
            pairs,
            vec![
                ("1".to_string(), "10".to_string()),
                ("2".to_string(), "20".to_string()),
                ("3".to_string(), "30".to_string()),
            ]
        );
    }

    #[test]
    fn extract_object() {
        let pairs = extract(r#"{"a":"x","b":"y"}"#, ".");
        assert_eq!(
            pairs,
            vec![
                ("a".to_string(), "x".to_string()),
                ("b".to_string(), "y".to_string()),
            ]
        );
    }

    // ── iteration / projection ──────────────────────────────────

    #[test]
    fn iterate_array_explicit() {
        // .items[] → all elements
        let result = jp(r#"{"items":[10,20,30]}"#, ".items[]");
        assert_eq!(result, "10\n20\n30");
    }

    #[test]
    fn iterate_and_project_key() {
        // .users[].name → project "name" from each element
        let json = r#"{"users":[{"name":"Alice","id":1},{"name":"Bob","id":2}]}"#;
        assert_eq!(jp(json, ".users[].name"), "Alice\nBob");
    }

    #[test]
    fn implicit_iteration_key_on_array() {
        // .users.name without [] → same as .users[].name
        let json = r#"{"users":[{"name":"Alice"},{"name":"Bob"}]}"#;
        assert_eq!(jp(json, ".users.name"), "Alice\nBob");
    }

    #[test]
    fn extract_iterated_into_array() {
        let json = r#"{"users":[{"id":10},{"id":20},{"id":30}]}"#;
        let pairs = extract(json, ".users[].id");
        assert_eq!(
            pairs,
            vec![
                ("1".to_string(), "10".to_string()),
                ("2".to_string(), "20".to_string()),
                ("3".to_string(), "30".to_string()),
            ]
        );
    }

    #[test]
    fn implicit_iteration_extract() {
        let json = r#"{"users":[{"id":10},{"id":20}]}"#;
        let pairs = extract(json, ".users.id");
        assert_eq!(
            pairs,
            vec![
                ("1".to_string(), "10".to_string()),
                ("2".to_string(), "20".to_string()),
            ]
        );
    }

    #[test]
    fn iterate_object_values() {
        let json = r#"{"scores":{"alice":95,"bob":87}}"#;
        let result = jp(json, ".scores[]");
        assert_eq!(result, "95\n87");
    }
}
