use std::collections::HashMap;

use crate::builtins;
use crate::field;

// --- Value type: dual string/number representation with lazy conversion ---

const STR_VALID: u8 = 1;
const NUM_VALID: u8 = 2;

#[derive(Clone, Debug)]
#[must_use]
pub struct Value {
    s: String,
    n: f64,
    flags: u8,
}

impl Default for Value {
    fn default() -> Self {
        Value { s: String::new(), n: 0.0, flags: STR_VALID | NUM_VALID }
    }
}

impl Value {
    pub fn from_string(s: String) -> Self {
        Value { s, n: 0.0, flags: STR_VALID }
    }

    pub fn from_str_ref(s: &str) -> Self {
        Value { s: s.to_string(), n: 0.0, flags: STR_VALID }
    }

    pub fn from_number(n: f64) -> Self {
        Value { s: String::new(), n, flags: NUM_VALID }
    }

    /// Get numeric value (fast path if number is already cached).
    pub fn to_number(&self) -> f64 {
        if self.flags & NUM_VALID != 0 {
            self.n
        } else {
            builtins::to_number(&self.s)
        }
    }

    /// Consume the value and return its string representation.
    pub fn into_string(self) -> String {
        if self.flags & STR_VALID != 0 {
            self.s
        } else {
            builtins::format_number(self.n)
        }
    }

    /// Clone the string representation (allocates if number-only).
    pub fn to_string_val(&self) -> String {
        if self.flags & STR_VALID != 0 {
            self.s.clone()
        } else {
            builtins::format_number(self.n)
        }
    }

    /// Write string representation directly to a writer.
    pub fn write_to(&self, w: &mut impl std::io::Write) {
        if self.flags & STR_VALID != 0 {
            let _ = w.write_all(self.s.as_bytes());
        } else {
            let s = builtins::format_number(self.n);
            let _ = w.write_all(s.as_bytes());
        }
    }

    /// Append string representation to an existing String.
    pub fn write_to_string(&self, buf: &mut String) {
        if self.flags & STR_VALID != 0 {
            buf.push_str(&self.s);
        } else {
            buf.push_str(&builtins::format_number(self.n));
        }
    }

    pub fn is_truthy(&self) -> bool {
        if self.flags & NUM_VALID != 0 {
            self.n != 0.0
        } else {
            !self.s.is_empty() && self.s != "0"
        }
    }

    pub fn is_numeric(&self) -> bool {
        self.flags & NUM_VALID != 0
    }

    /// Check if this value looks numeric (for comparison semantics).
    pub fn looks_numeric(&self) -> bool {
        if self.flags & NUM_VALID != 0 { return true; }
        if self.flags & STR_VALID != 0 {
            let s = self.s.trim();
            !s.is_empty() && s.parse::<f64>().is_ok()
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub struct Runtime {
    variables: HashMap<String, Value>,
    arrays: HashMap<String, HashMap<String, Value>>,
    pub(crate) fields: Vec<String>,
    nr: u64,
    nf: usize,
    fnr: u64,
    fs: String,
    ofs: String,
    rs: String,
    ors: String,
    subsep: String,
    ofmt: String,
    filename: String,
}

/// Names that are stored as dedicated fields rather than in the HashMap.
const INTERNED_NAMES: &[&str] = &[
    "FILENAME", "FNR", "FS", "NF", "NR", "OFS", "OFMT", "ORS", "RS", "SUBSEP",
];

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            variables: HashMap::new(),
            arrays: HashMap::new(),
            fields: Vec::new(),
            nr: 0,
            nf: 0,
            fnr: 0,
            fs: " ".to_string(),
            ofs: " ".to_string(),
            rs: "\n".to_string(),
            ors: "\n".to_string(),
            subsep: "\x1c".to_string(),
            ofmt: "%.6g".to_string(),
            filename: String::new(),
        }
    }

    /// Get a variable as a Value (fast path — no string formatting for numbers).
    pub fn get_value(&self, name: &str) -> Value {
        match name {
            "NR" => Value::from_number(self.nr as f64),
            "NF" => Value::from_number(self.nf as f64),
            "FNR" => Value::from_number(self.fnr as f64),
            "FS" => Value::from_str_ref(&self.fs),
            "OFS" => Value::from_str_ref(&self.ofs),
            "RS" => Value::from_str_ref(&self.rs),
            "ORS" => Value::from_str_ref(&self.ors),
            "SUBSEP" => Value::from_str_ref(&self.subsep),
            "OFMT" => Value::from_str_ref(&self.ofmt),
            "FILENAME" => Value::from_str_ref(&self.filename),
            _ => self.variables.get(name).cloned().unwrap_or_default(),
        }
    }

    /// Set a variable from a Value (fast path — no string parsing for numbers).
    pub fn set_value(&mut self, name: &str, val: Value) {
        match name {
            "NR" => self.nr = val.to_number() as u64,
            "NF" => self.nf = val.to_number() as usize,
            "FNR" => self.fnr = val.to_number() as u64,
            "FS" => self.fs = val.into_string(),
            "OFS" => self.ofs = val.into_string(),
            "RS" => self.rs = val.into_string(),
            "ORS" => self.ors = val.into_string(),
            "SUBSEP" => self.subsep = val.into_string(),
            "OFMT" => self.ofmt = val.into_string(),
            "FILENAME" => self.filename = val.into_string(),
            _ => { self.variables.insert(name.to_string(), val); }
        }
    }

    /// Convenience: get variable as String (for backward-compatible callers).
    pub fn get_var(&self, name: &str) -> String {
        self.get_value(name).into_string()
    }

    /// Convenience: set variable from &str (for backward-compatible callers).
    pub fn set_var(&mut self, name: &str, value: &str) {
        self.set_value(name, Value::from_str_ref(value));
    }

    /// Check whether a variable exists (interned vars always exist).
    pub fn has_var(&self, name: &str) -> bool {
        INTERNED_NAMES.contains(&name) || self.variables.contains_key(name)
    }

    /// Remove a user variable. Interned vars are reset to their defaults.
    pub fn remove_var(&mut self, name: &str) {
        match name {
            "NR" => self.nr = 0,
            "NF" => self.nf = 0,
            "FNR" => self.fnr = 0,
            "FS" => self.fs = " ".to_string(),
            "OFS" => self.ofs = " ".to_string(),
            "RS" => self.rs = "\n".to_string(),
            "ORS" => self.ors = "\n".to_string(),
            "SUBSEP" => self.subsep = "\x1c".to_string(),
            "OFMT" => self.ofmt = "%.6g".to_string(),
            "FILENAME" => self.filename = String::new(),
            _ => { self.variables.remove(name); }
        }
    }

    /// Iterate all variable names (interned + user-defined).
    pub fn all_var_names(&self) -> Vec<String> {
        let mut names: Vec<String> = INTERNED_NAMES.iter().map(|s| s.to_string()).collect();
        for k in self.variables.keys() {
            names.push(k.clone());
        }
        names.sort();
        names
    }

    /// Borrow OFS directly (avoids clone in hot print path).
    pub fn ofs(&self) -> &str { &self.ofs }

    /// Borrow ORS directly (avoids clone in hot print path).
    pub fn ors(&self) -> &str { &self.ors }

    pub fn get_field(&self, idx: usize) -> String {
        if idx == 0 {
            return self.fields.join(&self.ofs);
        }
        self.fields
            .get(idx - 1)
            .cloned()
            .unwrap_or_default()
    }

    /// Write a field directly to a writer without cloning (zero-copy print).
    pub fn write_field_to(&self, idx: usize, w: &mut impl std::io::Write) {
        if idx == 0 {
            for (i, f) in self.fields.iter().enumerate() {
                if i > 0 { let _ = w.write_all(self.ofs.as_bytes()); }
                let _ = w.write_all(f.as_bytes());
            }
        } else if let Some(f) = self.fields.get(idx - 1) {
            let _ = w.write_all(f.as_bytes());
        }
    }

    pub fn set_field(&mut self, idx: usize, value: &str) {
        if idx == 0 {
            self.fields = field::split(value, &self.fs);
            self.nf = self.fields.len();
            return;
        }
        let idx = idx - 1;
        while self.fields.len() <= idx {
            self.fields.push(String::new());
        }
        self.fields[idx] = value.to_string();
        self.nf = self.fields.len();
    }

    pub fn set_record(&mut self, line: &str) {
        field::split_into(&mut self.fields, line, &self.fs);
        self.nf = self.fields.len();
    }

    /// Set the record with pre-split fields (used by CSV/TSV/JSON readers).
    pub fn set_record_fields(&mut self, text: &str, fields: Vec<String>) {
        let _ = text; // $0 is reconstructed from fields via OFS
        self.nf = fields.len();
        self.fields = fields;
    }

    pub fn increment_nr(&mut self) {
        self.nr += 1;
    }

    pub fn increment_fnr(&mut self) {
        self.fnr += 1;
    }

    pub fn reset_fnr(&mut self) {
        self.fnr = 0;
    }

    pub fn set_filename(&mut self, name: &str) {
        self.filename = name.to_string();
    }

    // --- array operations ---

    pub fn get_array_value(&self, name: &str, key: &str) -> Value {
        self.arrays
            .get(name)
            .and_then(|a| a.get(key))
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_array(&self, name: &str, key: &str) -> String {
        self.get_array_value(name, key).into_string()
    }

    pub fn get_array_opt(&self, name: &str, key: &str) -> Option<String> {
        self.arrays
            .get(name)
            .and_then(|a| a.get(key))
            .map(|v| v.clone().into_string())
    }

    pub fn set_array_value(&mut self, name: &str, key: &str, val: Value) {
        self.arrays
            .entry(name.to_string())
            .or_default()
            .insert(key.to_string(), val);
    }

    pub fn set_array(&mut self, name: &str, key: &str, value: &str) {
        self.set_array_value(name, key, Value::from_str_ref(value));
    }

    pub fn delete_array(&mut self, name: &str, key: &str) {
        if let Some(a) = self.arrays.get_mut(name) {
            a.remove(key);
        }
    }

    pub fn delete_array_all(&mut self, name: &str) {
        self.arrays.remove(name);
    }

    pub fn array_len(&self, name: &str) -> usize {
        self.arrays.get(name).map_or(0, |a| a.len())
    }

    pub fn array_has_key(&self, name: &str, key: &str) -> bool {
        self.arrays
            .get(name)
            .is_some_and(|a| a.contains_key(key))
    }

    /// Check if an array exists (may be empty).
    pub fn has_array(&self, name: &str) -> bool {
        self.arrays.contains_key(name)
    }

    pub fn array_keys(&self, name: &str) -> Vec<String> {
        self.arrays
            .get(name)
            .map(|a| a.keys().cloned().collect())
            .unwrap_or_default()
    }
}
