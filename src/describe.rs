//! Auto-describe and suggest mode: sniff input format, infer schema,
//! and generate example fk programs.

use std::io::{self, BufRead, BufReader, Read};

/// Detected input format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Csv,
    Tsv,
    Json,
    Space,
    Parquet,
}

impl Format {
    fn flag(&self) -> &str {
        match self {
            Format::Csv => "-i csv",
            Format::Tsv => "-i tsv",
            Format::Json => "-i json",
            Format::Parquet => "-i parquet",
            Format::Space => "",
        }
    }

    fn name(&self) -> &str {
        match self {
            Format::Csv => "csv",
            Format::Tsv => "tsv",
            Format::Json => "json-lines",
            Format::Parquet => "parquet",
            Format::Space => "whitespace-delimited",
        }
    }
}

/// Inferred column type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColType {
    Int,
    Float,
    String,
}

impl ColType {
    fn label(&self) -> &str {
        match self {
            ColType::Int => "int",
            ColType::Float => "float",
            ColType::String => "string",
        }
    }
}

/// Schema inferred from the input.
#[derive(Debug)]
pub struct Schema {
    pub format: Format,
    pub has_header: bool,
    pub columns: Vec<String>,
    pub types: Vec<ColType>,
    pub total_rows: usize,
    pub sample_rows: Vec<Vec<String>>,
}

/// Read up to `limit` lines from a reader.
fn read_lines(reader: &mut dyn BufRead, limit: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut buf = String::new();
    for _ in 0..limit {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => break,
            Ok(_) => {
                let line = buf.trim_end_matches('\n').trim_end_matches('\r').to_string();
                if !line.is_empty() {
                    lines.push(line);
                }
            }
            Err(_) => break,
        }
    }
    lines
}

/// Detect the input format from sample lines.
fn detect_format(lines: &[String]) -> Format {
    if lines.is_empty() {
        return Format::Space;
    }

    // JSON: first non-empty line starts with { or [
    let first = lines[0].trim_start();
    if first.starts_with('{') || first.starts_with('[') {
        let json_count = lines.iter()
            .filter(|l| l.trim_start().starts_with('{'))
            .count();
        if json_count > lines.len() / 2 {
            return Format::Json;
        }
    }

    // Count tabs vs commas in first few lines
    let sample = &lines[..lines.len().min(10)];
    let avg_tabs: f64 = sample.iter().map(|l| l.matches('\t').count() as f64).sum::<f64>() / sample.len() as f64;
    let avg_commas: f64 = sample.iter().map(|l| l.matches(',').count() as f64).sum::<f64>() / sample.len() as f64;

    // Consistent tab count across lines → TSV
    if avg_tabs >= 1.0 {
        let tab_counts: Vec<usize> = sample.iter().map(|l| l.matches('\t').count()).collect();
        let consistent = tab_counts.iter().all(|&c| c == tab_counts[0]);
        if consistent && avg_tabs >= avg_commas {
            return Format::Tsv;
        }
    }

    // Consistent comma count, or quoted fields → CSV
    if avg_commas >= 1.0 {
        let has_quotes = sample.iter().any(|l| l.contains('"'));
        let comma_counts: Vec<usize> = sample.iter().map(|l| count_csv_fields(l)).collect();
        let consistent = comma_counts.iter().all(|&c| c == comma_counts[0]);
        if consistent || has_quotes {
            return Format::Csv;
        }
    }

    Format::Space
}

/// Count CSV fields (respecting quoted fields).
fn count_csv_fields(line: &str) -> usize {
    let mut count = 1;
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => count += 1,
            _ => {}
        }
    }
    count
}

/// Split a line according to the detected format.
fn split_line(line: &str, format: Format) -> Vec<String> {
    match format {
        Format::Tsv => line.split('\t').map(|s| s.to_string()).collect(),
        Format::Csv => split_csv(line),
        Format::Space | Format::Parquet => line.split_whitespace().map(|s| s.to_string()).collect(),
        Format::Json => parse_json_keys(line),
    }
}

/// Simple CSV field splitter (handles quoted fields).
fn split_csv(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' if !in_quotes => in_quotes = true,
            ',' if !in_quotes => {
                fields.push(field.clone());
                field.clear();
            }
            _ => field.push(ch),
        }
    }
    fields.push(field);
    fields
}

/// Extract top-level keys from a JSON object line.
fn parse_json_keys(line: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return keys;
    }
    // Simple key extraction: find "key": patterns
    let mut i = 1;
    let chars: Vec<char> = trimmed.chars().collect();
    while i < chars.len() {
        if chars[i] == '"' {
            let start = i + 1;
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' { i += 1; }
                i += 1;
            }
            let end = i;
            i += 1;
            // Skip whitespace
            while i < chars.len() && chars[i].is_whitespace() { i += 1; }
            if i < chars.len() && chars[i] == ':' {
                let key: String = chars[start..end].iter().collect();
                keys.push(key);
            }
        }
        i += 1;
    }
    keys
}

/// Extract a JSON value for a key (simple string/number extraction).
fn json_value_for_key(line: &str, key: &str) -> String {
    let pattern = format!("\"{}\"", key);
    if let Some(pos) = line.find(&pattern) {
        let after = &line[pos + pattern.len()..];
        let after = after.trim_start().strip_prefix(':').unwrap_or(after).trim_start();
        if let Some(stripped) = after.strip_prefix('"') {
            // String value
            let end = stripped.find('"').unwrap_or(stripped.len());
            stripped[..end].to_string()
        } else {
            // Number or other
            let end = after.find(|c: char| c == ',' || c == '}' || c.is_whitespace())
                .unwrap_or(after.len());
            after[..end].to_string()
        }
    } else {
        String::new()
    }
}

/// Detect whether the first row looks like a header.
fn detect_header(rows: &[Vec<String>], format: Format) -> bool {
    if rows.len() < 2 {
        return false;
    }
    // JSON doesn't have headers — keys are the headers
    if format == Format::Json {
        return false;
    }
    let first = &rows[0];
    let rest = &rows[1..];

    // If first row is all non-numeric and subsequent rows have numerics → header
    let first_all_non_numeric = first.iter().all(|s| s.parse::<f64>().is_err());
    let rest_has_numeric = rest.iter().any(|row| {
        row.iter().any(|s| s.parse::<f64>().is_ok())
    });

    if first_all_non_numeric && rest_has_numeric {
        return true;
    }

    // If first row values are all unique and "label-like" (no numbers, short)
    if first_all_non_numeric && first.iter().all(|s| s.len() < 40) {
        let mut seen = std::collections::HashSet::new();
        if first.iter().all(|s| seen.insert(s.to_lowercase())) {
            return true;
        }
    }

    false
}

/// Infer the type of a column from sample values.
fn infer_type(values: &[String]) -> ColType {
    if values.is_empty() {
        return ColType::String;
    }
    let mut all_int = true;
    let mut all_float = true;
    for v in values {
        if v.is_empty() {
            continue;
        }
        if v.parse::<i64>().is_err() {
            all_int = false;
        }
        if v.parse::<f64>().is_err() {
            all_float = false;
        }
    }
    if all_int { ColType::Int }
    else if all_float { ColType::Float }
    else { ColType::String }
}

/// Sniff input and produce a Schema.
pub fn sniff(reader: &mut dyn BufRead) -> Schema {
    let lines = read_lines(reader, 100);
    let format = detect_format(&lines);

    if format == Format::Json {
        return sniff_json(&lines);
    }

    let rows: Vec<Vec<String>> = lines.iter().map(|l| split_line(l, format)).collect();
    let has_header = detect_header(&rows, format);

    let (columns, data_rows) = if has_header && !rows.is_empty() {
        (rows[0].clone(), &rows[1..])
    } else {
        let ncols = rows.first().map_or(0, |r| r.len());
        let cols: Vec<String> = (1..=ncols).map(|i| format!("${}", i)).collect();
        (cols, &rows[..])
    };

    let ncols = columns.len();
    let mut types = Vec::with_capacity(ncols);
    for col_idx in 0..ncols {
        let values: Vec<String> = data_rows.iter()
            .filter_map(|row| row.get(col_idx).cloned())
            .collect();
        types.push(infer_type(&values));
    }

    let sample_rows: Vec<Vec<String>> = data_rows.iter().take(5).cloned().collect();

    Schema {
        format,
        has_header,
        columns,
        types,
        total_rows: if has_header { lines.len() - 1 } else { lines.len() },
        sample_rows,
    }
}

fn sniff_json(lines: &[String]) -> Schema {
    if lines.is_empty() {
        return Schema {
            format: Format::Json,
            has_header: false,
            columns: vec![],
            types: vec![],
            total_rows: 0,
            sample_rows: vec![],
        };
    }

    let columns = parse_json_keys(&lines[0]);
    let ncols = columns.len();

    let mut types = Vec::with_capacity(ncols);
    for col in &columns {
        let values: Vec<String> = lines.iter()
            .map(|l| json_value_for_key(l, col))
            .filter(|v| !v.is_empty())
            .collect();
        types.push(infer_type(&values));
    }

    let sample_rows: Vec<Vec<String>> = lines.iter().take(5)
        .map(|l| columns.iter().map(|c| json_value_for_key(l, c)).collect())
        .collect();

    Schema {
        format: Format::Json,
        has_header: false,
        columns,
        types,
        total_rows: lines.len(),
        sample_rows,
    }
}

/// Format a column reference: bare `$name` for valid idents, `$"name"` otherwise.
fn col_ref(name: &str) -> String {
    if is_auto_header(name) {
        // Column is auto-generated like $1, use as-is
        name.to_string()
    } else if is_valid_ident(name) {
        format!("${}", name)
    } else {
        format!("$\"{}\"", name)
    }
}

fn is_auto_header(name: &str) -> bool {
    name.starts_with('$') && name[1..].parse::<usize>().is_ok()
}

fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Print the schema description to stderr.
pub fn print_description(schema: &Schema, row_count: Option<usize>) {
    let rows = row_count.unwrap_or(schema.total_rows);

    eprintln!();
    eprint!("  \x1b[1mformat:\x1b[0m {}", schema.format.name());
    eprint!("  \x1b[1mcolumns:\x1b[0m {}", schema.columns.len());
    eprint!("  \x1b[1mrows:\x1b[0m {}+", rows);
    if schema.has_header {
        eprint!("  \x1b[1mheader:\x1b[0m yes");
    }
    eprintln!();
    eprintln!();

    // Column table
    let max_name_len = schema.columns.iter().map(|c| c.len()).max().unwrap_or(4).max(6);
    eprintln!("  \x1b[90m{:<4}  {:<width$}  {:<6}  sample\x1b[0m",
        "#", "column", "type", width = max_name_len);
    eprintln!("  \x1b[90m{}  {}  {}  {}\x1b[0m",
        "─".repeat(4), "─".repeat(max_name_len), "─".repeat(6), "─".repeat(30));

    for (i, col) in schema.columns.iter().enumerate() {
        let typ = schema.types.get(i).unwrap_or(&ColType::String);
        let sample: String = schema.sample_rows.iter()
            .filter_map(|row| row.get(i))
            .take(3)
            .map(|v| truncate(v, 20))
            .collect::<Vec<_>>()
            .join(", ");

        let type_color = match typ {
            ColType::Int | ColType::Float => "\x1b[33m",
            ColType::String => "\x1b[36m",
        };

        eprintln!("  {:<4}  {:<width$}  {}{:<6}\x1b[0m  \x1b[90m{}\x1b[0m",
            i + 1, col, type_color, typ.label(), sample, width = max_name_len);
    }
    eprintln!();
}

/// Print suggested fk programs.
pub fn print_suggestions(schema: &Schema, file_hint: &str) {
    let flags = build_flags(schema);
    let file_part = if file_hint.is_empty() { String::new() } else { format!(" {}", file_hint) };

    // Find interesting columns: prefer floats for aggregation, last numeric for
    // "measure" heuristic (first numeric is often an ID).
    let first_str = schema.columns.iter().enumerate()
        .find(|(i, _)| schema.types.get(*i) == Some(&ColType::String))
        .map(|(_, c)| c.as_str());
    let first_float = schema.columns.iter().enumerate()
        .find(|(i, _)| schema.types.get(*i) == Some(&ColType::Float))
        .map(|(_, c)| c.as_str());
    let last_num = schema.columns.iter().enumerate()
        .rev()
        .find(|(i, _)| matches!(schema.types.get(*i), Some(ColType::Int | ColType::Float)))
        .map(|(_, c)| c.as_str());
    let first_num = first_float.or(last_num);
    let second_str = schema.columns.iter().enumerate()
        .filter(|(i, _)| schema.types.get(*i) == Some(&ColType::String))
        .nth(1)
        .map(|(_, c)| c.as_str());

    eprintln!("  \x1b[1mexamples:\x1b[0m");
    eprintln!();

    // 1. Select columns
    if schema.columns.len() >= 2 {
        let c1 = col_ref(&schema.columns[0]);
        let c2 = col_ref(&schema.columns[1]);
        suggestion(&flags, &format!("{{ print {}, {} }}", c1, c2), "select columns", &file_part);
    }

    // 2. Filter rows
    if let Some(s) = first_str {
        let cr = col_ref(s);
        suggestion(&flags, &format!("{} ~ /pattern/", cr), "filter rows (regex)", &file_part);
    }

    // 3. Count rows
    suggestion(&flags, "{ n++ } END { print n }", "count rows", &file_part);

    // 4. Sum a numeric column
    if let Some(n) = first_num {
        let cr = col_ref(n);
        suggestion(&flags, &format!("{{ s += {} }} END {{ print s }}", cr), &format!("sum {}", n), &file_part);
    }

    // 5. Group by
    if let (Some(s), Some(n)) = (first_str, first_num) {
        let sr = col_ref(s);
        let nr = col_ref(n);
        suggestion(&flags,
            &format!("{{ a[{}] += {} }} END {{ for (k in a) print k, a[k] }}", sr, nr),
            &format!("group by {}", s), &file_part);
    }

    // 6. Statistics
    if let Some(n) = first_num {
        let nr = col_ref(n);
        suggestion(&flags,
            &format!("{{ a[NR] = {} }} END {{ printf \"mean=%.2f median=%.2f p95=%.2f\\n\", mean(a), median(a), p(a,95) }}", nr),
            &format!("stats on {}", n), &file_part);
    }

    // 7. Unique values
    if let Some(s) = first_str {
        let sr = col_ref(s);
        suggestion(&flags, &format!("{{ a[{}] }} END {{ for (k in a) print k }}", sr),
            &format!("unique {}", s), &file_part);
    }

    // 8. Top N by frequency
    if let Some(s) = second_str.or(first_str) {
        let sr = col_ref(s);
        suggestion(&flags,
            &format!("{{ a[{}]++ }} END {{ for (k in a) print a[k], k }}", sr),
            &format!("frequency of {} (pipe to sort -rn | head)", s), &file_part);
    }

    eprintln!();
}

fn build_flags(schema: &Schema) -> String {
    let mut parts = Vec::new();
    if schema.has_header {
        parts.push("-H".to_string());
    }
    let mode_flag = schema.format.flag();
    if !mode_flag.is_empty() {
        parts.push(mode_flag.to_string());
    }
    if parts.is_empty() {
        String::new()
    } else {
        parts.join(" ")
    }
}

fn suggestion(flags: &str, program: &str, comment: &str, file_part: &str) {
    let flag_part = if flags.is_empty() { String::new() } else { format!(" {}", flags) };
    eprintln!("  \x1b[32mfk{} '{}'{}\x1b[0m", flag_part, program, file_part);
    eprintln!("  \x1b[90m# {}\x1b[0m", comment);
    eprintln!();
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

/// Open a file, decompressing transparently if needed.
/// Returns a boxed reader and whether decompression was used.
pub fn open_maybe_compressed(path: &str) -> io::Result<Box<dyn Read + Send>> {
    let (cmd, args): (&str, &[&str]) = if path.ends_with(".gz") || path.ends_with(".tgz") {
        ("gzip", &["-dc", path])
    } else if path.ends_with(".zst") || path.ends_with(".zstd") {
        ("zstd", &["-dc", path])
    } else if path.ends_with(".bz2") {
        ("bzip2", &["-dc", path])
    } else if path.ends_with(".xz") {
        ("xz", &["-dc", path])
    } else if path.ends_with(".lz4") {
        ("lz4", &["-dc", path])
    } else {
        return Ok(Box::new(std::fs::File::open(path)?));
    };

    let child = std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| io::Error::new(e.kind(),
            format!("fk: cannot run '{}' to decompress '{}': {}", cmd, path, e)))?;

    Ok(Box::new(child.stdout.unwrap()))
}

/// Check if a path looks like a compressed file.
pub fn is_compressed(path: &str) -> bool {
    path.ends_with(".gz") || path.ends_with(".tgz")
        || path.ends_with(".zst") || path.ends_with(".zstd")
        || path.ends_with(".bz2")
        || path.ends_with(".xz")
        || path.ends_with(".lz4")
}

/// Detect format from file extension (before compression suffix).
pub fn format_from_extension(path: &str) -> Option<Format> {
    let base = path.trim_end_matches(".gz")
        .trim_end_matches(".zst").trim_end_matches(".zstd")
        .trim_end_matches(".bz2").trim_end_matches(".xz")
        .trim_end_matches(".lz4");
    if base.ends_with(".csv") { Some(Format::Csv) }
    else if base.ends_with(".tsv") || base.ends_with(".tab") { Some(Format::Tsv) }
    else if base.ends_with(".json") || base.ends_with(".jsonl") || base.ends_with(".ndjson") { Some(Format::Json) }
    else if base.ends_with(".parquet") { Some(Format::Parquet) }
    else { None }
}

/// Helper: get sample values for a column by name.
fn sample_values<'a>(schema: &'a Schema, col_name: &str) -> Vec<&'a str> {
    let idx = schema.columns.iter().position(|c| c == col_name);
    match idx {
        Some(i) => schema.sample_rows.iter()
            .filter_map(|row| row.get(i).map(|s| s.as_str()))
            .filter(|s| !s.is_empty())
            .collect(),
        None => vec![],
    }
}

/// Helper: pick a representative string sample (first non-empty value).
fn sample_str<'a>(schema: &'a Schema, col_name: &str) -> &'a str {
    sample_values(schema, col_name).into_iter().next().unwrap_or("example")
}

/// Helper: extract a regex-friendly substring from a sample value.
/// Picks a short, interesting fragment (word, prefix, etc.).
fn sample_regex(schema: &Schema, col_name: &str) -> String {
    let vals = sample_values(schema, col_name);
    if vals.is_empty() {
        return "pattern".to_string();
    }
    let v = vals[0];
    // If it contains a space or separator, use the first word
    if let Some(word) = v.split([' ', '-', '_']).next()
        && word.len() >= 2 && word.len() <= 15 {
            return regex_escape(word);
    }
    // Otherwise use up to the first 8 chars
    let fragment = if v.len() > 8 { &v[..8] } else { v };
    regex_escape(fragment)
}

/// Escape regex metacharacters in a literal string.
fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\.*+?[](){}|^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Helper: compute a numeric threshold from sample data (approximate median).
fn sample_threshold(schema: &Schema, col_name: &str) -> String {
    let vals = sample_values(schema, col_name);
    let mut nums: Vec<f64> = vals.iter()
        .filter_map(|v| v.parse::<f64>().ok())
        .collect();
    if nums.is_empty() {
        return "100".to_string();
    }
    nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = nums[nums.len() / 2];
    // Format nicely: integer if whole, otherwise 1 decimal
    if median == median.floor() {
        format!("{}", median as i64)
    } else {
        format!("{:.1}", median)
    }
}

/// Helper: detect a common delimiter in sample string values.
fn sample_delimiter(schema: &Schema, col_name: &str) -> &'static str {
    let vals = sample_values(schema, col_name);
    let counts = |ch: char| -> usize { vals.iter().filter(|v| v.contains(ch)).count() };
    if counts('-') > vals.len() / 2 { return "-"; }
    if counts('_') > vals.len() / 2 { return "_"; }
    if counts('.') > vals.len() / 2 { return "."; }
    if counts('/') > vals.len() / 2 { return "/"; }
    if counts(':') > vals.len() / 2 { return ":"; }
    if counts(' ') > vals.len() / 2 { return " "; }
    "-"
}

/// Print comprehensive suggestions covering fk's feature surface,
/// tailored to actual values in the sample data.
pub fn print_suggest(schema: &Schema, file_hint: &str) {
    let flags = build_flags(schema);
    let fp = if file_hint.is_empty() { String::new() } else { format!(" {}", file_hint) };

    let strs: Vec<&str> = schema.columns.iter().enumerate()
        .filter(|(i, _)| schema.types.get(*i) == Some(&ColType::String))
        .map(|(_, c)| c.as_str()).collect();
    let nums: Vec<&str> = schema.columns.iter().enumerate()
        .filter(|(i, _)| matches!(schema.types.get(*i), Some(ColType::Int | ColType::Float)))
        .map(|(_, c)| c.as_str()).collect();
    let floats: Vec<&str> = schema.columns.iter().enumerate()
        .filter(|(i, _)| schema.types.get(*i) == Some(&ColType::Float))
        .map(|(_, c)| c.as_str()).collect();

    let s1 = strs.first().copied().unwrap_or("$1");
    let s2 = strs.get(1).copied().unwrap_or(s1);
    let n1 = floats.first().copied()
        .or_else(|| nums.last().copied())
        .unwrap_or("$1");

    let sr1 = col_ref(s1);
    let sr2 = col_ref(s2);
    let nr1 = col_ref(n1);

    // Sample data for tailored examples
    let s1_val = sample_str(schema, s1);
    let s1_regex = sample_regex(schema, s1);
    let n1_thresh = sample_threshold(schema, n1);
    let s1_delim = sample_delimiter(schema, s1);

    // ── Selecting & Printing ──
    section("Selecting & Printing");

    if schema.columns.len() >= 2 {
        let c1 = col_ref(&schema.columns[0]);
        let c2 = col_ref(&schema.columns[1]);
        suggestion(&flags, &format!("{{ print {}, {} }}", c1, c2), "select columns", &fp);
    }
    suggestion(&flags, "{ print NR, $0 }", "print with line numbers", &fp);
    if schema.columns.len() >= 3 {
        let c1 = col_ref(&schema.columns[0]);
        let c3 = col_ref(&schema.columns[2]);
        suggestion(&flags,
            &format!("{{ printf \"%s -> %s\\n\", {}, {} }}", c1, c3),
            "formatted output", &fp);
    }
    if n1 != "$1" && s1 != "$1" {
        suggestion(&flags,
            &format!("{{ printf \"%-20s %10.2f\\n\", {}, {} }}", sr1, nr1),
            "aligned columns (right-justify numbers)", &fp);
    }
    suggestion(&flags, "BEGIN { OFS = \"\\t\" } { print $0 }",
        "change output separator to tab", &fp);

    // ── Filtering ──
    section("Filtering");

    suggestion(&flags,
        &format!("{} ~ /{}/", sr1, s1_regex),
        &format!("rows where {} matches \"{}\"", s1, s1_regex), &fp);
    suggestion(&flags,
        &format!("{} !~ /{}/", sr1, s1_regex),
        &format!("rows where {} does not match \"{}\"", s1, s1_regex), &fp);
    if !nums.is_empty() {
        suggestion(&flags,
            &format!("{} > {}", nr1, n1_thresh),
            &format!("{} above {} (sample median)", n1, n1_thresh), &fp);
    }
    if !strs.is_empty() && !nums.is_empty() {
        suggestion(&flags,
            &format!("{} == \"{}\" && {} > {}", sr1, s1_val, nr1, n1_thresh),
            "compound filter with real values", &fp);
    }
    suggestion(&flags, "NR >= 5 && NR <= 15", "select row range", &fp);

    // ── Counting & Aggregation ──
    section("Counting & Aggregation");

    suggestion(&flags, "END { print NR }", "count all rows", &fp);
    suggestion(&flags,
        &format!("{} ~ /{}/ {{ n++ }} END {{ print n }}", sr1, s1_regex),
        &format!("count rows matching \"{}\"", s1_regex), &fp);
    if !nums.is_empty() {
        suggestion(&flags,
            &format!("{{ s += {} }} END {{ print s }}", nr1),
            &format!("sum {}", n1), &fp);
        suggestion(&flags,
            &format!("{{ s += {}; n++ }} END {{ print s/n }}", nr1),
            &format!("average {}", n1), &fp);
        suggestion(&flags,
            &format!("{{ if ({0} > max) max = {0} }} END {{ print max }}", nr1),
            &format!("max {}", n1), &fp);
    }

    // ── Grouping ──
    section("Grouping");

    if s1 != "$1" && n1 != "$1" {
        suggestion(&flags,
            &format!("{{ a[{}] += {} }} END {{ for (k in a) print k, a[k] }}", sr1, nr1),
            &format!("sum {} by {}", n1, s1), &fp);
        suggestion(&flags,
            &format!("{{ a[{}]++; s[{}] += {} }} END {{ for (k in a) printf \"%s: n=%d sum=%.2f avg=%.2f\\n\", k, a[k], s[k], s[k]/a[k] }}", sr1, sr1, nr1),
            &format!("count + sum + avg of {} by {}", n1, s1), &fp);
    }
    if s1 != s2 && s1 != "$1" {
        suggestion(&flags,
            &format!("{{ a[{},{}]++ }} END {{ for (k in a) print k, a[k] }}", sr1, sr2),
            &format!("cross-tab {} x {} (uses SUBSEP)", s1, s2), &fp);
    }

    // ── Statistics (built-in) ──
    section("Statistics");

    if !nums.is_empty() {
        suggestion(&flags,
            &format!("{{ a[NR] = {} }} END {{ printf \"mean=%.2f stddev=%.2f\\n\", mean(a), stddev(a) }}", nr1),
            &format!("mean & standard deviation of {}", n1), &fp);
        suggestion(&flags,
            &format!("{{ a[NR] = {} }} END {{ printf \"min=%.2f p25=%.2f median=%.2f p75=%.2f max=%.2f\\n\", min(a), p(a,25), median(a), p(a,75), max(a) }}", nr1),
            &format!("five-number summary of {}", n1), &fp);
        suggestion(&flags,
            &format!("{{ a[NR] = {} }} END {{ printf \"p50=%.2f p90=%.2f p95=%.2f p99=%.2f\\n\", p(a,50), p(a,90), p(a,95), p(a,99) }}", nr1),
            &format!("percentile ladder for {}", n1), &fp);
        suggestion(&flags,
            &format!("{{ a[NR] = {} }} END {{ printf \"iqm=%.2f variance=%.4f\\n\", iqm(a), variance(a) }}", nr1),
            "interquartile mean & variance", &fp);
    }

    // ── Sorting ──
    section("Sorting");

    if s1 != "$1" {
        suggestion(&flags,
            &format!("{{ a[NR] = {} }} END {{ asort(a); for (i in a) print a[i] }}", sr1),
            &format!("sort {} values alphabetically", s1), &fp);
    }
    if !nums.is_empty() {
        suggestion(&flags,
            &format!("{{ a[NR] = {} }} END {{ asort(a); for (i=1; i<=length(a); i++) print a[i] }}", nr1),
            &format!("sort {} numerically", n1), &fp);
    }
    if s1 != "$1" && n1 != "$1" {
        suggestion(&flags,
            &format!("{{ a[{}] += {} }} END {{ for (k in a) print a[k], k }}",
                sr1, nr1),
            "group totals (pipe to: sort -rn | head -10)", &fp);
    }

    // ── String Operations ──
    section("String Operations");

    suggestion(&flags,
        &format!("{{ print toupper({}) }}", sr1),
        "uppercase", &fp);
    suggestion(&flags,
        &format!("{{ print substr({}, 1, 4) }}", sr1),
        &format!("first 4 chars (e.g. \"{}\" → \"{}\")", s1_val, &s1_val[..s1_val.len().min(4)]), &fp);
    suggestion(&flags,
        &format!("{{ gsub(/{0}/, \"NEW\", {1}); print }}", s1_regex, sr1),
        &format!("replace \"{}\" with \"NEW\"", s1_regex), &fp);
    suggestion(&flags,
        &format!("{{ n = split({}, parts, \"{}\"); print parts[1] }}", sr1, s1_delim),
        &format!("split on \"{}\"", s1_delim), &fp);
    suggestion(&flags,
        &format!("{{ if (match({}, /[0-9]+/)) print substr({}, RSTART, RLENGTH) }}", sr1, sr1),
        "extract first number from string", &fp);

    // ── Output Formatting ──
    section("Output Formatting");

    if !nums.is_empty() {
        suggestion(&flags,
            &format!("{{ printf \"%08d\\n\", {} }}", nr1),
            "zero-padded number", &fp);
        suggestion(&flags,
            &format!("{{ printf \"%+.2f\\n\", {} }}", nr1),
            "force sign on number", &fp);
    }
    suggestion(&flags,
        "BEGIN { OFS = \",\" } { $1 = $1; print }",
        "convert to CSV", &fp);
    if schema.columns.len() >= 2 {
        let c1 = col_ref(&schema.columns[0]);
        let c2 = col_ref(&schema.columns[1]);
        suggestion(&flags,
            &format!("BEGIN {{ OFS = \"\\t\" }} {{ print {}, {} }}", c1, c2),
            "convert to TSV", &fp);
    }

    // ── Control Flow ──
    section("Control Flow");

    suggestion(&flags, "NR <= 10", "first 10 rows (like head)", &fp);
    suggestion(&flags, "NR % 2 == 0", "every other row", &fp);
    if !nums.is_empty() {
        suggestion(&flags,
            &format!("{{ if ({0} > {1}) print \"high\", {0}; else print \"low\", {0} }}", nr1, n1_thresh),
            &format!("classify by {} threshold ({})", n1, n1_thresh), &fp);
    }
    if s1 != "$1" {
        suggestion(&flags,
            &format!("!seen[{}]++", sr1),
            &format!("deduplicate by {}", s1), &fp);
    }

    // ── Multi-file & System ──
    section("Multi-file & System");

    suggestion(&flags, "{ print FILENAME, NR, FNR, $0 }",
        "show filename and record numbers", &fp);
    suggestion(&flags, "FNR == 1 { print \"--- \" FILENAME \" ---\" }",
        "print header between files", &fp);
    suggestion(&flags, "BEGIN { while ((\"date\" | getline d) > 0) print d }",
        "capture command output with getline", &fp);

    // ── Pipelines ──
    section("Pipelines (combine with shell)");

    if s1 != "$1" && n1 != "$1" {
        suggestion(&flags,
            &format!("{{ a[{}] += {} }} END {{ for (k in a) print a[k], k }}", sr1, nr1),
            &format!("| sort -rn | head  → top {} by {}", s1, n1), &fp);
    }
    suggestion(&flags, "{ print $0 }",
        "| wc -l  → count (or just: END { print NR })", &fp);
    if !nums.is_empty() {
        suggestion(&flags,
            &format!("{{ print {} }}", nr1),
            &format!("| sort -n | uniq -c  → frequency distribution of {}", n1), &fp);
    }

    eprintln!();
}

/// Run describe mode: sniff the input and print schema + suggestions.
pub fn run_describe(files: &[String], suggest: bool) {
    if files.is_empty() {
        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        let schema = sniff(&mut reader);
        print_description(&schema, None);
        if suggest {
            print_suggest(&schema, "");
        } else {
            print_suggestions(&schema, "");
        }
    } else {
        for path in files {
            if files.len() > 1 {
                eprintln!("  \x1b[1m{}:\x1b[0m", path);
            }

            let file_reader: Box<dyn Read> = match open_maybe_compressed(path) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("fk: {}", e);
                    continue;
                }
            };
            let mut reader = BufReader::new(file_reader);
            let schema = sniff(&mut reader);
            print_description(&schema, None);
            if suggest {
                print_suggest(&schema, path);
            } else {
                print_suggestions(&schema, path);
            }
        }
    }
}

fn section(title: &str) {
    eprintln!("  \x1b[1;4m{}\x1b[0m", title);
    eprintln!();
}
