use std::collections::HashMap;
use std::path::Path;

/// Case-insensitive variable map supporting `#VarName#` expansion and built-in macros.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableMap {
    vars: HashMap<String, String>,
}

impl VariableMap {
    /// Creates a new `VariableMap` initialized with default built-in variables (`#@#`, `#SCREENAREAWIDTH#`, `#SCREENAREAHEIGHT#`, etc.).
    pub fn new() -> Self {
        let mut map = Self {
            vars: HashMap::new(),
        };
        // Built-in Rainmeter macro: #@# expands to @Resources/
        map.set("@", "@Resources/");
        map.set("screenareawidth", "1920");
        map.set("screenareaheight", "1080");
        map.set("workareawidth", "1920");
        map.set("workareaheight", "1080");
        map.set("screenareax", "0");
        map.set("screenareay", "0");
        map.set("workareax", "0");
        map.set("workareay", "0");
        map.set("programpath", "/usr/local/bin/");
        map.set("settingspath", "~/.config/pluvia/");
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
        map.set("rootconfigpath", &formatted);
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
        self.expand_with_full_context(input, current_section, measures, None)
    }

    /// Expands variables with full context including dynamic meter bounds (`[Meter:W]`, `[Meter:H]`, `[Meter:X]`, `[Meter:Y]`).
    pub fn expand_with_full_context(
        &self,
        input: &str,
        current_section: Option<&str>,
        measures: Option<&HashMap<String, crate::measures::MeasureValue>>,
        meter_bounds: Option<&HashMap<String, crate::render::Rect>>,
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

        // Expand dynamic section variables if measures or meter_bounds provided
        if measures.is_some() || meter_bounds.is_some() {
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
                            if let Some(colon_pos) = inner.find(':') {
                                let name = &inner[..colon_pos];
                                let prop = &inner[colon_pos + 1..];
                                let clean_name = name.strip_prefix('&').unwrap_or(name).to_ascii_lowercase();
                                let clean_prop = prop.to_ascii_lowercase();

                                if let Some(bounds) = meter_bounds {
                                    let rect_opt = bounds.get(&clean_name).or_else(|| bounds.iter().find(|(k, _)| k.eq_ignore_ascii_case(&clean_name)).map(|(_, v)| v));
                                if let Some(rect) = rect_opt {
                                        let val = match clean_prop.as_str() {
                                            "w" | "width" => Some(rect.width),
                                            "h" | "height" => Some(rect.height),
                                            "x" => Some(rect.x),
                                            "y" => Some(rect.y),
                                            _ => None,
                                        };
                                        if let Some(v) = val {
                                            if (v - v.round()).abs() < 1e-6 {
                                                result.push_str(&format!("{:.0}", v));
                                            } else {
                                                result.push_str(&v.to_string());
                                            }
                                            i += close + 1;
                                            continue;
                                        }
                                    }
                                }

                                if prop.is_empty() {
                                    if let Some(m_map) = measures {
                                        if let Some(val) = m_map.get(&clean_name) {
                                            result.push_str(&val.to_number_val().to_string());
                                            i += close + 1;
                                            continue;
                                        }
                                    }
                                }
                            } else {
                                let clean_name = inner.strip_prefix('&').unwrap_or(inner).to_ascii_lowercase();
                                if let Some(m_map) = measures {
                                    if let Some(val) = m_map.get(&clean_name) {
                                        result.push_str(&val.to_string_val());
                                        i += close + 1;
                                        continue;
                                    }
                                }
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
