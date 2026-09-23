use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use regex::Regex;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Linux native WebParser plugin executing HTTP queries, regex extractions, and file downloads.
#[derive(Debug, Clone)]
pub struct WebParserPlugin {
    url: Option<String>,
    regex_pattern: Option<String>,
    regex: Option<Regex>,
    string_index: usize,
    mock_data: Option<String>,
    download: bool,
    download_file: Option<String>,
    raw_content: String,
    cached_captures: Vec<String>,
    current_value: MeasureValue,
}

impl WebParserPlugin {
    /// Create a new WebParser plugin instance.
    pub fn new() -> Self {
        Self {
            url: None,
            regex_pattern: None,
            regex: None,
            string_index: 0,
            mock_data: None,
            download: false,
            download_file: None,
            raw_content: String::new(),
            cached_captures: Vec::new(),
            current_value: MeasureValue::String(String::new()),
        }
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let mut plugin = Self::new();

        if let Some(url) = config.get("url") {
            plugin = plugin.with_url(url);
        }

        if let Some(regex) = config.get("regex") {
            plugin = plugin.with_regex(regex);
        }

        if let Some(idx) = config
            .get("stringindex")
            .and_then(|s| s.parse::<usize>().ok())
        {
            plugin = plugin.with_string_index(idx);
        }

        if let Some(dl) = config.get("download") {
            plugin.download = dl == "1" || dl.eq_ignore_ascii_case("true");
        }

        if let Some(dl_file) = config.get("downloadfile") {
            plugin.download_file = Some(dl_file.to_string());
        }

        plugin
    }

    /// Set URL (http, https, file, or local path).
    pub fn with_url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }

    /// Set regular expression pattern for parsing.
    pub fn with_regex(mut self, pattern: &str) -> Self {
        self.regex_pattern = Some(pattern.to_string());
        self.regex = Regex::new(pattern).ok();
        self
    }

    /// Set StringIndex (1-based capture group index, or 0 for entire match).
    pub fn with_string_index(mut self, index: usize) -> Self {
        self.string_index = index;
        self
    }

    /// Set mock data for deterministic testing.
    pub fn with_mock_data(mut self, data: &str) -> Self {
        self.mock_data = Some(data.to_string());
        self.raw_content = data.to_string();
        self.extract_captures();
        self
    }

    /// Dynamically configure mock data.
    pub fn set_mock_data(&mut self, data: Option<String>) {
        if let Some(ref d) = data {
            self.raw_content = d.clone();
            self.mock_data = data;
            self.extract_captures();
        } else {
            self.mock_data = None;
        }
    }

    /// Directly supply data to parse.
    pub fn set_data(&mut self, data: &str) {
        self.raw_content = data.to_string();
        self.extract_captures();
    }

    /// Retrieve capture group match by index.
    pub fn get_match(&self, index: usize) -> Option<&str> {
        self.cached_captures.get(index).map(|s| s.as_str())
    }

    /// Download or cache content to the given file path.
    pub fn download_to_file(&self, target_path: &Path) -> Result<PathBuf, io::Error> {
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = File::create(target_path)?;
        let content = if !self.raw_content.is_empty() {
            self.raw_content.clone()
        } else if let Some(ref mock) = self.mock_data {
            mock.clone()
        } else if let Some(ref url_str) = self.url {
            self.fetch_url(url_str)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
        } else {
            String::new()
        };

        file.write_all(content.as_bytes())?;
        Ok(target_path.to_path_buf())
    }

    fn fetch_url(&self, url_str: &str) -> Result<String, String> {
        if url_str.starts_with("http://") || url_str.starts_with("https://") {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| e.to_string())?;

            let resp = client.get(url_str).send().map_err(|e| e.to_string())?;
            resp.text().map_err(|e| e.to_string())
        } else if let Some(stripped) = url_str.strip_prefix("file://") {
            fs::read_to_string(stripped).map_err(|e| e.to_string())
        } else if Path::new(url_str).exists() {
            fs::read_to_string(url_str).map_err(|e| e.to_string())
        } else {
            Err(format!("Unsupported URL scheme: {}", url_str))
        }
    }

    fn extract_captures(&mut self) {
        self.cached_captures.clear();
        if let Some(ref re) = self.regex {
            if let Some(caps) = re.captures(&self.raw_content) {
                for i in 0..caps.len() {
                    let text = caps.get(i).map(|m| m.as_str()).unwrap_or("");
                    self.cached_captures.push(text.to_string());
                }
            }
        }
    }
}

impl Default for WebParserPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Measure for WebParserPlugin {
    fn update(&mut self) -> MeasureValue {
        if let Some(ref mock) = self.mock_data {
            self.raw_content = mock.clone();
        } else if let Some(ref url) = self.url {
            if let Ok(fetched) = self.fetch_url(url) {
                self.raw_content = fetched;
            }
        }

        self.extract_captures();

        let val_str = if self.string_index < self.cached_captures.len() {
            self.cached_captures[self.string_index].clone()
        } else if self.string_index == 0 && !self.raw_content.is_empty() {
            self.raw_content.clone()
        } else {
            String::new()
        };

        self.current_value = MeasureValue::String(val_str);
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }

    fn command(&mut self, _cmd: &str) {
        self.update();
    }
}
