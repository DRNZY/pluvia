use crate::encoding::decode_ini_bytes;
use crate::formulas::{eval_formula, FormulaError};
use crate::variables::VariableMap;
use crate::vfs::VfsResolver;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Encoding error: {0}")]
    Encoding(#[from] crate::encoding::EncodingError),
    #[error("Include file not found: {0}")]
    IncludeNotFound(String),
    #[error("Circular include detected: {0}")]
    CircularInclude(String),
    #[error("Invalid syntax at line {line}: {msg}")]
    SyntaxError { line: usize, msg: String },
    #[error("Formula evaluation error: {0}")]
    Formula(#[from] FormulaError),
}

/// Representation of a parsed Rainmeter meter (`Meter=...`).
#[derive(Debug, Clone, PartialEq)]
pub struct MeterConfig {
    pub name: String,
    pub meter_type: String,
    pub measure_name: Option<String>,
    pub measure_names: Vec<String>,
    pub font_face: Option<String>,
    pub font_size: Option<f64>,
    pub font_color: Option<String>,
    pub text: Option<String>,
    pub anti_alias: bool,
    pub x: Option<String>,
    pub y: Option<String>,
    pub w: Option<f64>,
    pub h: Option<f64>,
    pub solid_color: Option<String>,
    pub hidden: bool,
    pub dynamic_variables: bool,
    pub meter_style: Option<String>,
    pub properties: HashMap<String, String>,
}

impl MeterConfig {
    /// Case-insensitive property lookup.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties.get(&key.to_ascii_lowercase()).map(|s| s.as_str())
    }
}

/// Representation of a parsed Rainmeter measure (`Measure=...`).
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureConfig {
    pub name: String,
    pub measure_type: String,
    pub plugin: Option<String>,
    pub format: Option<String>,
    pub formula: Option<String>,
    pub update_divider: u32,
    pub disabled: bool,
    pub dynamic_variables: bool,
    pub properties: HashMap<String, String>,
}

impl MeasureConfig {
    /// Case-insensitive property lookup.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties.get(&key.to_ascii_lowercase()).map(|s| s.as_str())
    }
}

/// Complete parsed configuration of a Rainmeter skin.
#[derive(Debug, Clone)]
pub struct SkinConfig {
    pub update_rate_ms: u64,
    pub accurate_text: bool,
    pub dynamic_window_size: bool,
    pub variables: VariableMap,
    pub measures: HashMap<String, MeasureConfig>,
    pub meters: HashMap<String, MeterConfig>,
    pub measure_order: Vec<String>,
    pub meter_order: Vec<String>,
    pub raw_sections: HashMap<String, HashMap<String, String>>,
    pub skin_dir: PathBuf,
}

impl SkinConfig {
    /// Case-insensitive meter lookup.
    pub fn get_meter(&self, name: &str) -> Option<&MeterConfig> {
        self.meters.get(&name.to_ascii_lowercase())
    }

    /// Case-insensitive measure lookup.
    pub fn get_measure(&self, name: &str) -> Option<&MeasureConfig> {
        self.measures.get(&name.to_ascii_lowercase())
    }

    /// Case-insensitive variable lookup.
    pub fn get_variable(&self, name: &str) -> Option<&str> {
        self.variables.get(name)
    }
}

struct ParserState<'a> {
    skin_dir: &'a Path,
    vfs: VfsResolver,
    include_stack: Vec<PathBuf>,
    variables: VariableMap,
    // (original_name, properties) keyed by lowercase section name
    raw_sections: HashMap<String, (String, HashMap<String, String>)>,
    section_order: Vec<String>,
}

fn strip_quotes(s: &str) -> &str {
    let trimmed = s.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

fn parse_num_or_formula(val_str: &str, vars: &VariableMap) -> Option<f64> {
    let expanded = vars.expand(val_str);
    let trimmed = expanded.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(result) = eval_formula(trimmed, vars) {
        return Some(result);
    }
    trimmed.parse::<f64>().ok()
}

fn resolve_include_path(
    skin_dir: &Path,
    current_file_path: &Path,
    raw_path: &str,
    vfs: &VfsResolver,
) -> Result<PathBuf, ParseError> {
    let norm = VfsResolver::normalize_rel_path(raw_path);
    let current_dir = current_file_path
        .parent()
        .unwrap_or_else(|| Path::new("."));

    // 1. Try VFS resolve relative to current file's directory
    if let Some(path) = vfs.resolve(current_dir, &norm) {
        return Ok(path);
    }

    // 2. Direct relative check
    let direct_curr = current_dir.join(&norm);
    if direct_curr.exists() {
        return Ok(direct_curr);
    }

    // 3. Try VFS resolve and direct relative on skin_dir and all ancestors
    let mut check_dir = Some(skin_dir);
    while let Some(dir) = check_dir {
        if let Some(path) = vfs.resolve(dir, &norm) {
            return Ok(path);
        }
        let direct = dir.join(&norm);
        if direct.exists() {
            return Ok(direct);
        }
        check_dir = dir.parent();
    }

    // 4. Also walk up from current_dir in case it is in a different sub-tree
    let mut curr_ancestor = current_dir.parent();
    while let Some(dir) = curr_ancestor {
        if let Some(path) = vfs.resolve(dir, &norm) {
            return Ok(path);
        }
        let direct = dir.join(&norm);
        if direct.exists() {
            return Ok(direct);
        }
        curr_ancestor = dir.parent();
    }

    // 5. Absolute path check
    let p = Path::new(raw_path);
    if p.is_absolute() && p.exists() {
        return Ok(p.to_path_buf());
    }

    Err(ParseError::IncludeNotFound(raw_path.to_string()))
}

fn parse_content_recursive(
    content: &str,
    current_file_path: &Path,
    state: &mut ParserState,
    mut current_section: Option<String>,
) -> Result<(), ParseError> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }

        // Section header: [SectionName]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let title = trimmed[1..trimmed.len() - 1].trim();
            let lower_title = title.to_ascii_lowercase();
            if !state.raw_sections.contains_key(&lower_title) {
                state
                    .raw_sections
                    .insert(lower_title.clone(), (title.to_string(), HashMap::new()));
                state.section_order.push(lower_title.clone());
            }
            current_section = Some(lower_title);
            continue;
        }

        // Key=Value
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim();
            let val = v.trim();
            let lower_key = key.to_ascii_lowercase();

            // Check for @Include directive
            if lower_key.starts_with("@include") {
                let inc_path_str = state.variables.expand(val);
                if let Ok(inc_path) = resolve_include_path(
                    state.skin_dir,
                    current_file_path,
                    &inc_path_str,
                    &state.vfs,
                ) {
                    let canonical = inc_path
                        .canonicalize()
                        .unwrap_or_else(|_| inc_path.clone());

                    // Check active call stack for circular dependencies (A -> B -> A)
                    if state.include_stack.contains(&canonical) {
                        return Err(ParseError::CircularInclude(inc_path.display().to_string()));
                    }
                    state.include_stack.push(canonical);

                    if let Ok(bytes) = fs::read(&inc_path) {
                        if let Ok(decoded) = decode_ini_bytes(&bytes) {
                            parse_content_recursive(
                                &decoded,
                                &inc_path,
                                state,
                                current_section.clone(),
                            )?;
                        }
                    }
                    state.include_stack.pop();
                }
                continue;
            }

            if let Some(ref sec) = current_section {
                if sec == "variables" {
                    state.variables.set(key, val);
                }
                if let Some((_, props)) = state.raw_sections.get_mut(sec) {
                    props.insert(lower_key, val.to_string());
                }
            }
        }
    }

    Ok(())
}

fn apply_meter_styles(
    props: &mut HashMap<String, String>,
    raw_sections: &HashMap<String, (String, HashMap<String, String>)>,
) {
    if let Some(styles_str) = props.get("meterstyle").cloned() {
        // Rainmeter precedence: rightmost style supersedes earlier styles.
        // Reverse iteration with or_insert_with ensures:
        // 1. Meter's explicit options take highest precedence (already in props).
        // 2. The rightmost style inserts missing keys first.
        // 3. Earlier styles cannot overwrite keys from later styles.
        for style_name in styles_str.split('|').map(str::trim).rev() {
            let lower_style = style_name.to_ascii_lowercase();
            if let Some((_, style_props)) = raw_sections.get(&lower_style) {
                for (sk, sv) in style_props {
                    props.entry(sk.clone()).or_insert_with(|| sv.clone());
                }
            }
        }
    }
}

/// Parses a Rainmeter skin configuration from an INI string.
pub fn parse_skin_ini<P: AsRef<Path>>(content: &str, skin_dir: P) -> Result<SkinConfig, ParseError> {
    let skin_dir_ref = skin_dir.as_ref();
    let dummy_main = skin_dir_ref.join("main.ini");
    let canon_main = dummy_main
        .canonicalize()
        .unwrap_or_else(|_| dummy_main.clone());

    let mut state = ParserState {
        skin_dir: skin_dir_ref,
        vfs: VfsResolver::new(),
        include_stack: vec![canon_main],
        variables: VariableMap::with_skin_dir(skin_dir_ref),
        raw_sections: HashMap::new(),
        section_order: Vec::new(),
    };

    parse_content_recursive(content, &dummy_main, &mut state, None)?;

    // Chained variable resolution in VariableMap
    for _ in 0..16 {
        let mut changed = false;
        let keys: Vec<String> = state.variables.iter().map(|(k, _)| k.to_string()).collect();
        for k in keys {
            if let Some(val) = state.variables.get(&k).map(|s| s.to_string()) {
                let expanded = state.variables.expand(&val);
                if expanded != val {
                    state.variables.set(&k, &expanded);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Extract [Rainmeter] section settings
    let mut update_rate_ms = 1000;
    let mut accurate_text = false;
    let mut dynamic_window_size = false;

    if let Some((_, props)) = state.raw_sections.get("rainmeter") {
        if let Some(up) = props.get("update") {
            let exp = state.variables.expand(up);
            if let Ok(num) = eval_formula(&exp, &state.variables) {
                update_rate_ms = num.max(0.0) as u64;
            } else if let Ok(num) = exp.trim().parse::<u64>() {
                update_rate_ms = num;
            }
        }
        if let Some(at) = props.get("accuratetext") {
            accurate_text = at == "1" || at.eq_ignore_ascii_case("true");
        }
        if let Some(dws) = props.get("dynamicwindowsize") {
            dynamic_window_size = dws == "1" || dws.eq_ignore_ascii_case("true");
        }
    }

    // Categorize sections into Measures and Meters
    let mut measures = HashMap::new();
    let mut meters = HashMap::new();
    let mut measure_order = Vec::new();
    let mut meter_order = Vec::new();
    let mut raw_output = HashMap::new();

    for sec_lower in &state.section_order {
        if let Some((orig_name, props)) = state.raw_sections.get(sec_lower) {
            let mut props = props.clone();
            raw_output.insert(sec_lower.clone(), props.clone());

            if sec_lower == "rainmeter" || sec_lower == "variables" {
                continue;
            }

            if props.contains_key("measure") {
                if let Some(parent_name) = props.get("parent") {
                    let lower_parent = parent_name.to_ascii_lowercase();
                    if let Some((_, parent_props)) = state.raw_sections.get(&lower_parent) {
                        for (pk, pv) in parent_props {
                            if !props.contains_key(pk) {
                                props.insert(pk.clone(), pv.clone());
                            }
                        }
                    }
                }

                let measure_type = props.get("measure").cloned().unwrap_or_default();
                let plugin = props.get("plugin").map(|s| state.variables.expand(s));
                let format = props.get("format").map(|s| state.variables.expand(s));
                let formula = props.get("formula").map(|s| state.variables.expand(s));
                let update_divider = props
                    .get("updatedivider")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1);
                let disabled = props.get("disabled").map(|s| s == "1").unwrap_or(false);
                let dynamic_variables = props
                    .get("dynamicvariables")
                    .map(|s| s == "1")
                    .unwrap_or(false);

                let mut expanded_props = HashMap::new();
                for (k, v) in &props {
                    let exp = state.variables.expand(v);
                    expanded_props.insert(k.clone(), strip_quotes(&exp).to_string());
                }

                let measure_config = MeasureConfig {
                    name: orig_name.clone(),
                    measure_type,
                    plugin,
                    format,
                    formula,
                    update_divider,
                    disabled,
                    dynamic_variables,
                    properties: expanded_props,
                };

                measures.insert(sec_lower.clone(), measure_config);
                measure_order.push(sec_lower.clone());
            } else if props.contains_key("meter") {
                apply_meter_styles(&mut props, &state.raw_sections);

                let meter_type = props.get("meter").cloned().unwrap_or_default();

                // Deterministic measure ordering by index:
                // measurename -> index 1
                // measurename<N> -> index N
                let mut indexed_measures: Vec<(usize, String)> = Vec::new();
                for (k, v) in &props {
                    if k == "measurename" {
                        indexed_measures.push((1, state.variables.expand(v)));
                    } else if let Some(suffix) = k.strip_prefix("measurename") {
                        if let Ok(idx) = suffix.parse::<usize>() {
                            indexed_measures.push((idx, state.variables.expand(v)));
                        }
                    }
                }
                indexed_measures.sort_by_key(|(idx, _)| *idx);
                let measure_names: Vec<String> = indexed_measures
                    .into_iter()
                    .map(|(_, val)| strip_quotes(&val).to_string())
                    .collect();
                let measure_name = measure_names.first().cloned();

                let font_face = props
                    .get("fontface")
                    .map(|s| strip_quotes(&state.variables.expand(s)).to_string());
                let font_size = props
                    .get("fontsize")
                    .and_then(|s| parse_num_or_formula(s, &state.variables));
                let font_color = props
                    .get("fontcolor")
                    .map(|s| strip_quotes(&state.variables.expand(s)).to_string());
                let text = props
                    .get("text")
                    .map(|s| strip_quotes(&state.variables.expand(s)).to_string());
                let anti_alias = props
                    .get("antialias")
                    .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);

                let x = props.get("x").map(|s| state.variables.expand(s));
                let y = props.get("y").map(|s| state.variables.expand(s));
                let w = props
                    .get("w")
                    .and_then(|s| parse_num_or_formula(s, &state.variables));
                let h = props
                    .get("h")
                    .and_then(|s| parse_num_or_formula(s, &state.variables));
                let solid_color = props
                    .get("solidcolor")
                    .map(|s| state.variables.expand(s));
                let hidden = props.get("hidden").map(|s| s == "1").unwrap_or(false);
                let dynamic_variables = props
                    .get("dynamicvariables")
                    .map(|s| s == "1")
                    .unwrap_or(false);
                let meter_style = props.get("meterstyle").cloned();

                let mut expanded_props = HashMap::new();
                for (k, v) in &props {
                    let exp = state.variables.expand(v);
                    expanded_props.insert(k.clone(), strip_quotes(&exp).to_string());
                }

                let meter_config = MeterConfig {
                    name: orig_name.clone(),
                    meter_type,
                    measure_name,
                    measure_names,
                    font_face,
                    font_size,
                    font_color,
                    text,
                    anti_alias,
                    x,
                    y,
                    w,
                    h,
                    solid_color,
                    hidden,
                    dynamic_variables,
                    meter_style,
                    properties: expanded_props,
                };

                meters.insert(sec_lower.clone(), meter_config);
                meter_order.push(sec_lower.clone());
            }
        }
    }

    Ok(SkinConfig {
        update_rate_ms,
        accurate_text,
        dynamic_window_size,
        variables: state.variables,
        measures,
        meters,
        measure_order,
        meter_order,
        raw_sections: raw_output,
        skin_dir: skin_dir_ref.to_path_buf(),
    })
}

/// Parses a Rainmeter skin file from disk, detecting encoding automatically.
pub fn parse_skin_file<P: AsRef<Path>>(path: P) -> Result<SkinConfig, ParseError> {
    let file_path = path.as_ref();
    let bytes = fs::read(file_path)?;
    let content = decode_ini_bytes(&bytes)?;
    let skin_dir = file_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    parse_skin_ini(&content, skin_dir)
}
