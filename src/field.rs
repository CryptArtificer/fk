/// Split a record into fields based on the field separator.
///
/// Follows awk semantics:
/// - If FS is a single space, split on runs of whitespace and trim leading/trailing.
/// - If FS is a single character, split on that literal character.
/// - Otherwise treat FS as a regex pattern (TODO: regex FS in later phase).
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
}
