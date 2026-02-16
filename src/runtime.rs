use std::collections::HashMap;

use crate::field;

#[derive(Debug)]
pub struct Runtime {
    pub variables: HashMap<String, String>,
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

    pub fn set_record(&mut self, line: &str) {
        let fs = self.get_var("FS");
        self.fields = field::split(line, &fs);
        let nf = self.fields.len();
        self.set_var("NF", &nf.to_string());
    }

    pub fn increment_nr(&mut self) {
        let nr: u64 = self.get_var("NR").parse().unwrap_or(0) + 1;
        self.set_var("NR", &nr.to_string());
    }

    pub fn nr(&self) -> u64 {
        self.get_var("NR").parse().unwrap_or(0)
    }

    pub fn nf(&self) -> usize {
        self.get_var("NF").parse().unwrap_or(0)
    }
}
