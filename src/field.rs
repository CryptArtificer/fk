/// Split a record into fields based on the field separator.
///
/// Follows awk semantics:
/// - If FS is a single space, split on runs of whitespace and trim leading/trailing.
/// - If FS is a single character, split on that literal character.
/// - Otherwise treat FS as a literal string separator.
pub fn split(record: &str, fs: &str) -> Vec<String> {
    if fs == " " {
        record
            .split_whitespace()
            .map(String::from)
            .collect()
    } else if fs.len() == 1 || (fs.len() > 1 && fs.chars().count() == 1) {
        record
            .split(fs.chars().next().unwrap())
            .map(String::from)
            .collect()
    } else {
        record
            .split(fs)
            .map(String::from)
            .collect()
    }
}

/// Split into an existing Vec, reusing String allocations from previous records.
pub fn split_into(fields: &mut Vec<String>, record: &str, fs: &str) {
    let mut i = 0;
    if fs == " " {
        for part in record.split_whitespace() {
            set_field(fields, i, part);
            i += 1;
        }
    } else if fs.len() == 1 || (fs.len() > 1 && fs.chars().count() == 1) {
        let ch = fs.chars().next().unwrap();
        for part in record.split(ch) {
            set_field(fields, i, part);
            i += 1;
        }
    } else {
        for part in record.split(fs) {
            set_field(fields, i, part);
            i += 1;
        }
    }
    fields.truncate(i);
}

#[inline]
fn set_field(fields: &mut Vec<String>, i: usize, val: &str) {
    if i < fields.len() {
        fields[i].clear();
        fields[i].push_str(val);
    } else {
        fields.push(val.to_string());
    }
}

/// Split at most `limit` fields, discarding the remainder of the record.
/// Used when the program only accesses $1…$N and doesn't need NF.
pub fn split_into_limit(fields: &mut Vec<String>, record: &str, fs: &str, limit: usize) {
    let mut i = 0;
    if fs == " " {
        for part in record.split_whitespace() {
            if i >= limit { break; }
            set_field(fields, i, part);
            i += 1;
        }
    } else if fs.len() == 1 || (fs.len() > 1 && fs.chars().count() == 1) {
        let ch = fs.chars().next().unwrap();
        for part in record.splitn(limit + 1, ch) {
            if i >= limit { break; }
            set_field(fields, i, part);
            i += 1;
        }
    } else {
        for part in record.splitn(limit + 1, fs) {
            if i >= limit { break; }
            set_field(fields, i, part);
            i += 1;
        }
    }
    fields.truncate(i);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_default() {
        let fields = split("  hello   world  ", " ");
        assert_eq!(fields, vec!["hello", "world"]);
    }

    #[test]
    fn comma_separator() {
        let fields = split("a,b,c", ",");
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn tab_separator() {
        let fields = split("x\ty\tz", "\t");
        assert_eq!(fields, vec!["x", "y", "z"]);
    }

    #[test]
    fn empty_fields() {
        let fields = split("a::b", ":");
        assert_eq!(fields, vec!["a", "", "b"]);
    }

    #[test]
    fn limit_comma() {
        let mut fields = Vec::new();
        split_into_limit(&mut fields, "a,b,c,d,e", ",", 2);
        assert_eq!(fields, vec!["a", "b"]);
    }

    #[test]
    fn limit_whitespace() {
        let mut fields = Vec::new();
        split_into_limit(&mut fields, "  one two three four  ", " ", 2);
        assert_eq!(fields, vec!["one", "two"]);
    }

    #[test]
    fn limit_exceeds_fields() {
        let mut fields = Vec::new();
        split_into_limit(&mut fields, "x,y", ",", 10);
        assert_eq!(fields, vec!["x", "y"]);
    }
}
