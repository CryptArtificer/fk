use std::collections::HashMap;

use crate::field;

#[derive(Debug)]
pub struct Runtime {
    pub variables: HashMap<String, String>,
    pub arrays: HashMap<String, HashMap<String, String>>,
    pub fields: Vec<String>,
    nr: u64,
    nf: usize,
    fs: String,
    ofs: String,
    rs: String,
    ors: String,
}

/// Names that are stored as dedicated fields rather than in the HashMap.
const INTERNED_NAMES: &[&str] = &["NR", "NF", "FS", "OFS", "RS", "ORS"];

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            variables: HashMap::new(),
            arrays: HashMap::new(),
            fields: Vec::new(),
            nr: 0,
            nf: 0,
            fs: " ".to_string(),
            ofs: " ".to_string(),
            rs: "\n".to_string(),
            ors: "\n".to_string(),
        }
    }

    pub fn get_var(&self, name: &str) -> String {
        match name {
            "NR" => self.nr.to_string(),
            "NF" => self.nf.to_string(),
            "FS" => self.fs.clone(),
            "OFS" => self.ofs.clone(),
            "RS" => self.rs.clone(),
            "ORS" => self.ors.clone(),
            _ => self.variables.get(name).cloned().unwrap_or_default(),
        }
    }

    pub fn set_var(&mut self, name: &str, value: &str) {
        match name {
            "NR" => self.nr = value.parse().unwrap_or(0),
            "NF" => self.nf = value.parse().unwrap_or(0),
            "FS" => self.fs = value.to_string(),
            "OFS" => self.ofs = value.to_string(),
            "RS" => self.rs = value.to_string(),
            "ORS" => self.ors = value.to_string(),
            _ => { self.variables.insert(name.to_string(), value.to_string()); }
        }
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
            "FS" => self.fs = " ".to_string(),
            "OFS" => self.ofs = " ".to_string(),
            "RS" => self.rs = "\n".to_string(),
            "ORS" => self.ors = "\n".to_string(),
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

    pub fn get_field(&self, idx: usize) -> String {
        if idx == 0 {
            return self.fields.join(&self.ofs);
        }
        self.fields
            .get(idx - 1)
            .cloned()
            .unwrap_or_default()
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
        self.fields = field::split(line, &self.fs);
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

    // --- array operations ---

    pub fn get_array(&self, name: &str, key: &str) -> String {
        self.arrays
            .get(name)
            .and_then(|a| a.get(key))
            .cloned()
            .unwrap_or_default()
    }

    pub fn set_array(&mut self, name: &str, key: &str, value: &str) {
        self.arrays
            .entry(name.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
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
            .map_or(false, |a| a.contains_key(key))
    }

    pub fn array_keys(&self, name: &str) -> Vec<String> {
        self.arrays
            .get(name)
            .map(|a| a.keys().cloned().collect())
            .unwrap_or_default()
    }
}
