use std::collections::HashMap;

use crate::field;

#[derive(Debug)]
pub struct Runtime {
    pub variables: HashMap<String, String>,
    pub arrays: HashMap<String, HashMap<String, String>>,
    pub fields: Vec<String>,
}

impl Runtime {
    pub fn new() -> Self {
        let mut variables = HashMap::new();
        variables.insert("FS".to_string(), " ".to_string());
        variables.insert("OFS".to_string(), " ".to_string());
        variables.insert("RS".to_string(), "\n".to_string());
        variables.insert("ORS".to_string(), "\n".to_string());
        variables.insert("NR".to_string(), "0".to_string());
        variables.insert("NF".to_string(), "0".to_string());

        Runtime {
            variables,
            arrays: HashMap::new(),
            fields: Vec::new(),
        }
    }

    pub fn get_var(&self, name: &str) -> String {
        self.variables
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    pub fn set_var(&mut self, name: &str, value: &str) {
        self.variables.insert(name.to_string(), value.to_string());
    }

    pub fn get_field(&self, idx: usize) -> String {
        if idx == 0 {
            return self.fields.join(&self.get_var("OFS"));
        }
        self.fields
            .get(idx - 1)
            .cloned()
            .unwrap_or_default()
    }

    pub fn set_field(&mut self, idx: usize, value: &str) {
        if idx == 0 {
            let fs = self.get_var("FS");
            self.fields = field::split(value, &fs);
            let nf = self.fields.len();
            self.set_var("NF", &nf.to_string());
            return;
        }
        let idx = idx - 1;
        while self.fields.len() <= idx {
            self.fields.push(String::new());
        }
        self.fields[idx] = value.to_string();
        let nf = self.fields.len();
        self.set_var("NF", &nf.to_string());
    }

    pub fn set_record(&mut self, line: &str) {
        let fs = self.get_var("FS");
        self.fields = field::split(line, &fs);
        let nf = self.fields.len();
        self.set_var("NF", &nf.to_string());
    }

    /// Set the record with pre-split fields (used by CSV/TSV/JSON readers).
    pub fn set_record_fields(&mut self, text: &str, fields: Vec<String>) {
        let _ = text; // $0 is reconstructed from fields via OFS
        let nf = fields.len();
        self.fields = fields;
        self.set_var("NF", &nf.to_string());
    }

    pub fn increment_nr(&mut self) {
        let nr: u64 = self.get_var("NR").parse().unwrap_or(0) + 1;
        self.set_var("NR", &nr.to_string());
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
