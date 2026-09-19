use std::collections::HashMap;
use std::path::Path;

/// Case-insensitive variable map supporting `#VarName#` expansion and built-in macros.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableMap {
    vars: HashMap<String, String>,
}

impl VariableMap {
    /// Creates a new `VariableMap` initialized with default built-in variables (`#@#`).
    pub fn new() -> Self {
        let mut map = Self {
            vars: HashMap::new(),
        };
        // Built-in Rainmeter macro: #@# expands to @Resources/
        map.set("@", "@Resources/");
        map
    }

    /// Creates a `VariableMap` initialized with skin directory paths and built-ins.
    pub fn with_skin_dir<P: AsRef<Path>>(skin_dir: P) -> Self {
        let mut map = Self::new();
        let path = skin_dir.as_ref();
        let path_str = path.to_string_lossy();
        let formatted = if path_str.ends_with('/') {
            path_str.to_string()
        } else {
            format!("{}/", path_str)
        };
        map.set("currentpath", &formatted);
        map.set("skinspath", &formatted);
        map
    }

    /// Sets a variable with case-insensitive key.
    pub fn set<K: AsRef<str>, V: AsRef<str>>(&mut self, key: K, val: V) {
        self.vars
            .insert(key.as_ref().to_ascii_lowercase(), val.as_ref().to_string());
    }

    /// Retrieves a variable by case-insensitive key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars
            .get(&key.to_ascii_lowercase())
            .map(|s| s.as_str())
    }

    /// Retrieves a variable parsed as an `f64`.
    pub fn get_num(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|s| s.trim().parse::<f64>().ok())
    }

    /// Checks if a variable is defined (case-insensitive).
    pub fn contains_key(&self, key: &str) -> bool {
        self.vars.contains_key(&key.to_ascii_lowercase())
    }

    /// Removes a variable by case-insensitive key.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.vars.remove(&key.to_ascii_lowercase())
    }

    /// Number of variables stored.
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// Iterates over all (lowercase key, value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.vars.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Expands all `#VarName#` occurrences in `input`.
    ///
    /// Supports nested/chained variable expansion up to 16 passes.
    /// Unrecognized `#...#` patterns are left unmodified.
    pub fn expand(&self, input: &str) -> String {
        let mut current = input.to_string();

        for _ in 0..16 {
            let next = self.expand_single_pass(&current);
            if next == current {
                break;
            }
            current = next;
        }

        current
    }

    fn expand_single_pass(&self, input: &str) -> String {
        let mut output = String::with_capacity(input.len());
        let mut chars = input.char_indices().peekable();

        while let Some((i, c)) = chars.next() {
            if c == '#' {
                // Find closing '#'
                let rest = &input[i + 1..];
                if let Some(close_rel) = rest.find('#') {
                    let var_name = &rest[..close_rel];
                    // Verify var_name is non-empty and contains valid characters
                    if !var_name.is_empty() && var_name.chars().all(is_valid_var_char) {
                        if let Some(val) = self.get(var_name) {
                            output.push_str(val);
                            // Advance iterator past closing '#'
                            let target_idx = i + 1 + close_rel;
                            while let Some(&(next_idx, _)) = chars.peek() {
                                if next_idx <= target_idx {
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            continue;
                        }
                    }
                }
            }
            output.push(c);
        }

        output
    }

    /// Expands variables with optional context: `#CURRENTSECTION#`, `#VarName#`, and `[MeasureName]` / `[MeasureName:]` dynamic section variables.
    pub fn expand_with_context(
        &self,
        input: &str,
        current_section: Option<&str>,
        measures: Option<&HashMap<String, crate::measures::MeasureValue>>,
    ) -> String {
        let mut text = input.to_string();
        if let Some(sec) = current_section {
            let sec_upper = "#CURRENTSECTION#";
            while let Some(pos) = text.to_ascii_uppercase().find(sec_upper) {
                text.replace_range(pos..pos + sec_upper.len(), sec);
            }
        }

        // Expand standard variables
        text = self.expand(&text);

        // Expand dynamic section variables if measures provided
        if let Some(m_map) = measures {
            let mut result = String::with_capacity(text.len());
            let mut i = 0;
            let bytes = text.as_bytes();
            while i < bytes.len() {
                if bytes[i] == b'[' {
                    if let Some(close) = text[i..].find(']') {
                        let inner = &text[i + 1..i + close];
                        if !inner.starts_with('!')
                            && !inner.starts_with('"')
                            && !inner.starts_with('\'')
                            && !inner.contains(' ')
                            && !inner.is_empty()
                        {
                            let (name, is_num) = if inner.ends_with(':') {
                                (&inner[..inner.len() - 1], true)
                            } else {
                                (inner, false)
                            };
                            let clean_name = name.strip_prefix('&').unwrap_or(name);
                            if let Some(val) = m_map.get(&clean_name.to_ascii_lowercase()) {
                                if is_num {
                                    result.push_str(&val.to_number_val().to_string());
                                } else {
                                    result.push_str(&val.to_string_val());
                                }
                                i += close + 1;
                                continue;
                            }
                        }
                    }
                }
                result.push(bytes[i] as char);
                i += 1;
            }
            text = result;
        }

        text
    }
}

fn is_valid_var_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '@' || c == ':'
}

impl Default for VariableMap {
    fn default() -> Self {
        Self::new()
    }
}
