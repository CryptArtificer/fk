use std::io::{self, BufRead};

use super::{Record, RecordReader};

const MAX_FIELD_CONTINUATION_LINES: usize = 50;

/// RFC 4180 compliant CSV record reader.
///
/// Handles:
/// - Comma-separated fields (configurable delimiter)
/// - Double-quoted fields (embedded commas, newlines, escaped `""`)
/// - CRLF and LF line endings
///
/// Uses a single-pass parser: fields are parsed character by character from
/// the input. When a quoted field spans multiple lines, additional lines are
/// read on demand (up to `MAX_FIELD_CONTINUATION_LINES` to guard against
/// malformed input with unclosed quotes).
pub struct CsvReader {
    delimiter: u8,
}

impl CsvReader {
    pub fn new(delimiter: u8) -> Self {
        CsvReader { delimiter }
    }

    /// Convenience constructor for standard CSV (comma-delimited).
    pub fn comma() -> Self {
        Self::new(b',')
    }

    /// Convenience constructor for TSV (tab-delimited).
    pub fn tab() -> Self {
        Self::new(b'\t')
    }
}

impl RecordReader for CsvReader {
    fn next_record(&mut self, reader: &mut dyn BufRead) -> io::Result<Option<Record>> {
        let mut raw = String::new();
        let bytes = reader.read_line(&mut raw)?;
        if bytes == 0 {
            return Ok(None);
        }

        let delim = self.delimiter as char;
        let mut fields: Vec<String> = Vec::new();
        let mut field = String::new();
        let mut chars: Vec<char> = raw.chars().collect();
        let mut pos = 0;
        let mut at_field_start = true;

        loop {
            if pos >= chars.len() {
                break;
            }
            let ch = chars[pos];

            if ch == '"' && at_field_start {
                pos += 1;
                let mut continuations = 0;
                loop {
                    if pos >= chars.len() {
                        if continuations >= MAX_FIELD_CONTINUATION_LINES {
                            break;
                        }
                        let mut cont = String::new();
                        let n = reader.read_line(&mut cont)?;
                        if n == 0 {
                            break;
                        }
                        raw.push_str(&cont);
                        let ext: Vec<char> = cont.chars().collect();
                        chars.extend(ext);
                        continuations += 1;
                        continue;
                    }
                    if chars[pos] == '"' {
                        if pos + 1 < chars.len() && chars[pos + 1] == '"' {
                            field.push('"');
                            pos += 2;
                            continue;
                        }
                        pos += 1; // closing quote
                        break;
                    }
                    field.push(chars[pos]);
                    pos += 1;
                }
                at_field_start = false;
                continue;
            }

            if ch == delim {
                fields.push(std::mem::take(&mut field));
                at_field_start = true;
                pos += 1;
                continue;
            }

            if ch == '\n' || ch == '\r' {
                break;
            }

            field.push(ch);
            at_field_start = false;
            pos += 1;
        }

        fields.push(field);

        strip_trailing_newline(&mut raw);

        Ok(Some(Record {
            text: raw,
            fields: Some(fields),
        }))
    }
}

fn strip_trailing_newline(s: &mut String) {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
}

/// Parse a single (already-assembled) CSV/TSV line into fields.
#[cfg(test)]
fn parse_fields(line: &str, delimiter: u8) -> Vec<String> {
    let delim = delimiter as char;
    let mut fields = Vec::new();
    let mut field = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut at_field_start = true;

    while i < chars.len() {
        let ch = chars[i];

        if ch == '"' && at_field_start {
            i += 1;
            while i < chars.len() {
                if chars[i] == '"' {
                    if i + 1 < chars.len() && chars[i + 1] == '"' {
                        field.push('"');
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    field.push(chars[i]);
                    i += 1;
                }
            }
            at_field_start = false;
            continue;
        }

        if ch == delim {
            fields.push(std::mem::take(&mut field));
            at_field_start = true;
            i += 1;
            continue;
        }

        field.push(ch);
        at_field_start = false;
        i += 1;
    }

    fields.push(field);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn simple_csv() {
        let fields = parse_fields("a,b,c", b',');
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn quoted_field_with_comma() {
        let fields = parse_fields("a,\"b,c\",d", b',');
        assert_eq!(fields, vec!["a", "b,c", "d"]);
    }

    #[test]
    fn escaped_quotes() {
        let fields = parse_fields("a,\"he said \"\"hi\"\"\",c", b',');
        assert_eq!(fields, vec!["a", "he said \"hi\"", "c"]);
    }

    #[test]
    fn tab_delimited() {
        let fields = parse_fields("x\ty\tz", b'\t');
        assert_eq!(fields, vec!["x", "y", "z"]);
    }

    #[test]
    fn empty_fields_csv() {
        let fields = parse_fields("a,,c", b',');
        assert_eq!(fields, vec!["a", "", "c"]);
    }

    #[test]
    fn multiline_quoted_field() {
        let input = "a,\"hello\nworld\",c\n";
        let mut cursor = Cursor::new(input.as_bytes());
        let mut reader = CsvReader::comma();
        let rec = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(rec.fields.unwrap(), vec!["a", "hello\nworld", "c"]);
    }

    #[test]
    fn unclosed_quote_limits_damage() {
        // The unclosed quote on line 1 reads one continuation line because
        // it contains a `"` that closes the quoted field. Lines after that
        // are parsed normally — the old two-pass parser swallowed everything.
        let input = "1,bad,\"unclosed\n2,ok,\"real quote\"\n3,fine,end\n";
        let mut cursor = Cursor::new(input.as_bytes());
        let mut reader = CsvReader::comma();

        let r1 = reader.next_record(&mut cursor).unwrap().unwrap();
        let f1 = r1.fields.unwrap();
        assert_eq!(f1[0], "1");
        assert_eq!(f1[1], "bad");
        assert!(f1[2].starts_with("unclosed\n2,ok,"));

        // Row 3 is still readable
        let r2 = reader.next_record(&mut cursor).unwrap().unwrap();
        let f2 = r2.fields.unwrap();
        assert_eq!(f2[0], "3");
        assert_eq!(f2[1], "fine");
        assert_eq!(f2[2], "end");
    }

    #[test]
    fn unclosed_quote_at_eof() {
        // No matching quote anywhere → reads to EOF, produces one record.
        let input = "1,x,\"no close\n2,y,z\n";
        let mut cursor = Cursor::new(input.as_bytes());
        let mut reader = CsvReader::comma();

        let r1 = reader.next_record(&mut cursor).unwrap().unwrap();
        let f1 = r1.fields.unwrap();
        assert_eq!(f1[0], "1");
        assert_eq!(f1[1], "x");
        assert!(f1[2].contains("no close"));

        assert!(reader.next_record(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn embedded_double_quotes() {
        let fields = parse_fields("a,\"say \"\"hi\"\" ok\",c", b',');
        assert_eq!(fields, vec!["a", "say \"hi\" ok", "c"]);
    }

    #[test]
    fn empty_quoted_field() {
        let fields = parse_fields("a,\"\",c", b',');
        assert_eq!(fields, vec!["a", "", "c"]);
    }

    #[test]
    fn crlf_line_ending() {
        let input = "a,b,c\r\nd,e,f\r\n";
        let mut cursor = Cursor::new(input.as_bytes());
        let mut reader = CsvReader::comma();

        let r1 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r1.fields.unwrap(), vec!["a", "b", "c"]);

        let r2 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r2.fields.unwrap(), vec!["d", "e", "f"]);
    }

    #[test]
    fn whitespace_preserved_in_fields() {
        let fields = parse_fields(" a , b , c ", b',');
        assert_eq!(fields, vec![" a ", " b ", " c "]);
    }

    // ── RFC 4180 compliance ─────────────────────────────────────────

    #[test]
    fn rfc4180_last_record_no_trailing_newline() {
        let input = "a,b\nc,d";
        let mut cursor = Cursor::new(input.as_bytes());
        let mut reader = CsvReader::comma();

        let r1 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r1.fields.unwrap(), vec!["a", "b"]);

        let r2 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r2.fields.unwrap(), vec!["c", "d"]);

        assert!(reader.next_record(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn rfc4180_escaped_quote_pairs() {
        // """x""" → field value is "x"
        let fields = parse_fields("\"\"\"x\"\"\"", b',');
        assert_eq!(fields, vec!["\"x\""]);
    }

    #[test]
    fn rfc4180_only_escaped_quotes() {
        // """" → field value is a single "
        let fields = parse_fields("\"\"\"\"", b',');
        assert_eq!(fields, vec!["\""]);
    }

    #[test]
    fn rfc4180_quoted_empty_is_empty() {
        let fields = parse_fields("\"\",\"\",\"\"", b',');
        assert_eq!(fields, vec!["", "", ""]);
    }

    #[test]
    fn rfc4180_crlf_inside_quoted_field() {
        let input = "a,\"line1\r\nline2\",c\n";
        let mut cursor = Cursor::new(input.as_bytes());
        let mut reader = CsvReader::comma();
        let rec = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(rec.fields.unwrap(), vec!["a", "line1\r\nline2", "c"]);
    }

    #[test]
    fn rfc4180_comma_inside_quoted_field() {
        let fields = parse_fields("\"a,b\",c", b',');
        assert_eq!(fields, vec!["a,b", "c"]);
    }

    #[test]
    fn rfc4180_quoted_field_with_only_delimiter() {
        let fields = parse_fields("\",\",x", b',');
        assert_eq!(fields, vec![",", "x"]);
    }

    #[test]
    fn rfc4180_mixed_quoted_unquoted() {
        let fields = parse_fields("a,\"b\",c,\"d\"", b',');
        assert_eq!(fields, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn rfc4180_crlf_line_ending_no_bom() {
        let input = "a,b\r\nc,d\r\n";
        let mut cursor = Cursor::new(input.as_bytes());
        let mut reader = CsvReader::comma();

        let r1 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r1.fields.unwrap(), vec!["a", "b"]);

        let r2 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r2.fields.unwrap(), vec!["c", "d"]);

        assert!(reader.next_record(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn bare_cr_line_ending() {
        let input = "a,b\rc,d\r";
        let mut cursor = Cursor::new(input.as_bytes());
        let mut reader = CsvReader::comma();

        let r1 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r1.fields.unwrap(), vec!["a", "b"]);
    }

    #[test]
    fn single_field_record() {
        let input = "hello\n";
        let mut cursor = Cursor::new(input.as_bytes());
        let mut reader = CsvReader::comma();
        let rec = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(rec.fields.unwrap(), vec!["hello"]);
    }

    #[test]
    fn all_empty_fields() {
        let fields = parse_fields(",,", b',');
        assert_eq!(fields, vec!["", "", ""]);
    }

    #[test]
    fn trailing_delimiter_produces_empty_field() {
        let fields = parse_fields("a,b,", b',');
        assert_eq!(fields, vec!["a", "b", ""]);
    }

    #[test]
    fn leading_delimiter_produces_empty_field() {
        let fields = parse_fields(",a,b", b',');
        assert_eq!(fields, vec!["", "a", "b"]);
    }

    #[test]
    fn text_after_closing_quote_is_lenient() {
        // RFC 4180 says this is malformed, but we handle it gracefully
        let fields = parse_fields("\"hello\"world,b", b',');
        assert_eq!(fields, vec!["helloworld", "b"]);
    }

    #[test]
    fn multiline_field_three_lines() {
        let input = "a,\"line1\nline2\nline3\",b\n";
        let mut cursor = Cursor::new(input.as_bytes());
        let mut reader = CsvReader::comma();
        let rec = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(rec.fields.unwrap(), vec!["a", "line1\nline2\nline3", "b"]);
    }

    #[test]
    fn multiline_field_with_embedded_quotes() {
        let input = "a,\"say \"\"hi\"\"\nacross lines\",b\n";
        let mut cursor = Cursor::new(input.as_bytes());
        let mut reader = CsvReader::comma();
        let rec = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(
            rec.fields.unwrap(),
            vec!["a", "say \"hi\"\nacross lines", "b"]
        );
    }

    #[test]
    fn tsv_with_quoted_tab() {
        let fields = parse_fields("a\t\"b\tc\"\td", b'\t');
        assert_eq!(fields, vec!["a", "b\tc", "d"]);
    }

    // ── Full edge-case CSV (same content as tests/data/edge_cases.csv) ────

    fn read_all_records(csv: &str) -> Vec<Vec<String>> {
        let mut cursor = Cursor::new(csv.as_bytes());
        let mut reader = CsvReader::comma();
        let mut records = Vec::new();
        while let Some(rec) = reader.next_record(&mut cursor).unwrap() {
            records.push(rec.fields.unwrap());
        }
        records
    }

    const EDGE_CASE_CSV: &str = "\
id,name,comment\n\
1,John Doe,\"Simple entry\"\n\
2,\"Jane, A.\",\"Contains comma in name\"\n\
3,Bob Smith,\"Unclosed quote example\n\
4,Alice,\"Embedded \"\"double quotes\"\" inside\"\n\
5, Charlie ,\" Leading and trailing spaces \"\n\
6,,Missing name field\n\
7,Élodie,\"UTF-8 accented character\"\n\
8,=SUM(A1:A3),\"Potential formula injection\"\n\
9,\"Multi\n\
Line\",\"Comment with\n\
embedded newline\"\n\
10,\"Quoted, Name\",Unquoted comment with, extra comma\n\
11,\"Escaped \"\"quote\"\" test\",\"All good here\"\n\
12,\"Mixed line ending test\",\"CRLF follows\"\n\
13,\"Tab\tInside\",\"Contains\ttab character\"\n\
14,\" \",\"Whitespace-only name\"\n\
15,NULL,\"Literal NULL string\"\n";

    #[test]
    fn edge_csv_header_row() {
        let recs = read_all_records(EDGE_CASE_CSV);
        assert_eq!(recs[0], vec!["id", "name", "comment"]);
    }

    #[test]
    fn edge_csv_simple_quoted() {
        let recs = read_all_records(EDGE_CASE_CSV);
        assert_eq!(recs[1], vec!["1", "John Doe", "Simple entry"]);
    }

    #[test]
    fn edge_csv_comma_in_name() {
        let recs = read_all_records(EDGE_CASE_CSV);
        assert_eq!(recs[2], vec!["2", "Jane, A.", "Contains comma in name"]);
    }

    #[test]
    fn edge_csv_unclosed_quote_consumes_one_neighbor() {
        // Row 3 has an unclosed quote. The parser reads row 4 as a
        // continuation (because it contains a `"` that closes the field).
        // Row 4 is consumed into the same record; rows 5+ are unaffected.
        let recs = read_all_records(EDGE_CASE_CSV);
        assert_eq!(recs[3][0], "3");
        assert_eq!(recs[3][1], "Bob Smith");
        assert!(recs[3][2].starts_with("Unclosed quote example\n4,Alice,"));
    }

    #[test]
    fn edge_csv_leading_trailing_spaces() {
        let recs = read_all_records(EDGE_CASE_CSV);
        let row = recs.iter().find(|r| r[0] == "5").unwrap();
        assert_eq!(row[1], " Charlie ");
        assert_eq!(row[2], " Leading and trailing spaces ");
    }

    #[test]
    fn edge_csv_empty_field() {
        let recs = read_all_records(EDGE_CASE_CSV);
        let row = recs.iter().find(|r| r[0] == "6").unwrap();
        assert_eq!(row[1], "");
        assert_eq!(row[2], "Missing name field");
    }

    #[test]
    fn edge_csv_utf8_accented() {
        let recs = read_all_records(EDGE_CASE_CSV);
        let row = recs.iter().find(|r| r[0] == "7").unwrap();
        assert_eq!(row[1], "Élodie");
        assert_eq!(row[2], "UTF-8 accented character");
    }

    #[test]
    fn edge_csv_formula_injection_passthrough() {
        let recs = read_all_records(EDGE_CASE_CSV);
        let row = recs.iter().find(|r| r[0] == "8").unwrap();
        assert_eq!(row[1], "=SUM(A1:A3)");
    }

    #[test]
    fn edge_csv_multiline_field() {
        let recs = read_all_records(EDGE_CASE_CSV);
        let row = recs.iter().find(|r| r[0] == "9").unwrap();
        assert_eq!(row[1], "Multi\nLine");
        assert_eq!(row[2], "Comment with\nembedded newline");
    }

    #[test]
    fn edge_csv_extra_comma_unquoted() {
        // Row 10: unquoted field with extra comma produces 4 fields
        let recs = read_all_records(EDGE_CASE_CSV);
        let row = recs.iter().find(|r| r[0] == "10").unwrap();
        assert_eq!(row[1], "Quoted, Name");
        assert_eq!(row[2], "Unquoted comment with");
        assert_eq!(row[3], " extra comma");
    }

    #[test]
    fn edge_csv_escaped_quotes_in_field() {
        let recs = read_all_records(EDGE_CASE_CSV);
        let row = recs.iter().find(|r| r[0] == "11").unwrap();
        assert_eq!(row[1], "Escaped \"quote\" test");
        assert_eq!(row[2], "All good here");
    }

    #[test]
    fn edge_csv_tab_inside_quoted() {
        let recs = read_all_records(EDGE_CASE_CSV);
        let row = recs.iter().find(|r| r[0] == "13").unwrap();
        assert_eq!(row[1], "Tab\tInside");
        assert_eq!(row[2], "Contains\ttab character");
    }

    #[test]
    fn edge_csv_whitespace_only_name() {
        let recs = read_all_records(EDGE_CASE_CSV);
        let row = recs.iter().find(|r| r[0] == "14").unwrap();
        assert_eq!(row[1], " ");
        assert_eq!(row[2], "Whitespace-only name");
    }

    #[test]
    fn edge_csv_literal_null_string() {
        let recs = read_all_records(EDGE_CASE_CSV);
        let row = recs.iter().find(|r| r[0] == "15").unwrap();
        assert_eq!(row[1], "NULL");
        assert_eq!(row[2], "Literal NULL string");
    }

    #[test]
    fn edge_csv_record_count() {
        let recs = read_all_records(EDGE_CASE_CSV);
        // 1 header + 15 data rows, but row 3's unclosed quote consumes
        // row 4, so we get 15 records total (header + 14 parseable records)
        assert_eq!(recs.len(), 15);
    }

    #[test]
    fn edge_csv_surviving_rows_after_malformed() {
        // After the unclosed quote merges rows 3+4, rows 5-15 must all
        // parse independently — none are corrupted by the malformed row.
        let recs = read_all_records(EDGE_CASE_CSV);
        let ids: Vec<&str> = recs.iter().map(|r| r[0].as_str()).collect();
        for expected in &["5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15"] {
            assert!(
                ids.contains(expected),
                "Row with id={} missing from parsed output",
                expected
            );
        }
    }
}
