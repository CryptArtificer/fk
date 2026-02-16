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
