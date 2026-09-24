use std::path::PathBuf;

/// AST representation of Rainmeter Bang commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bang {
    /// `!SetVariable <VarName> <Value> [Config]`
    SetVariable {
        name: String,
        value: String,
        config: Option<String>,
    },
    /// `!SetOption <Section> <Key> <Value> [Config]`
    SetOption {
        section: String,
        key: String,
        value: String,
        config: Option<String>,
    },
    /// `!WriteKeyValue <Section> <Key> <Value> [FilePath]`
    WriteKeyValue {
        section: String,
        key: String,
        value: String,
        file: Option<PathBuf>,
    },
    /// `!Update [Config]`
    Update { config: Option<String> },
    /// `!UpdateMeter <MeterName> [Config]` (or `*` for all meters)
    UpdateMeter {
        name: String,
        config: Option<String>,
    },
    /// `!UpdateMeasure <MeasureName> [Config]` (or `*` for all measures)
    UpdateMeasure {
        name: String,
        config: Option<String>,
    },
    /// `!Redraw [Config]`
    Redraw { config: Option<String> },
    /// `!ShowMeter <MeterName> [Config]`
    ShowMeter {
        name: String,
        config: Option<String>,
    },
    /// `!HideMeter <MeterName> [Config]`
    HideMeter {
        name: String,
        config: Option<String>,
    },
    /// `!ToggleMeter <MeterName> [Config]`
    ToggleMeter {
        name: String,
        config: Option<String>,
    },
    /// `!EnableMeasure <MeasureName> [Config]`
    EnableMeasure {
        name: String,
        config: Option<String>,
    },
    /// `!DisableMeasure <MeasureName> [Config]`
    DisableMeasure {
        name: String,
        config: Option<String>,
    },
    /// `!ToggleMeasure <MeasureName> [Config]`
    ToggleMeasure {
        name: String,
        config: Option<String>,
    },
    /// `!ActivateConfig <Config> [File]`
    ActivateConfig {
        config: String,
        file: Option<String>,
    },
    /// `!DeactivateConfig [Config]`
    DeactivateConfig { config: Option<String> },
    /// `!ToggleConfig <Config> [File]`
    ToggleConfig {
        config: String,
        file: Option<String>,
    },
    /// `!Refresh [Config]`
    Refresh { config: Option<String> },
    /// `!RefreshApp`
    RefreshApp,
    /// `!CommandMeasure <MeasureName> <Command> [Config]`
    CommandMeasure {
        measure: String,
        command: String,
        config: Option<String>,
    },
    /// `!PauseMeasure <MeasureName> [Config]`
    PauseMeasure {
        name: String,
        config: Option<String>,
    },
    /// `!UnpauseMeasure <MeasureName> [Config]`
    UnpauseMeasure {
        name: String,
        config: Option<String>,
    },
    /// `!TogglePauseMeasure <MeasureName> [Config]`
    TogglePauseMeasure {
        name: String,
        config: Option<String>,
    },
    /// `!UpdateMeterGroup <Group> [Config]`
    UpdateMeterGroup {
        group: String,
        config: Option<String>,
    },
    /// `!UpdateMeasureGroup <Group> [Config]`
    UpdateMeasureGroup {
        group: String,
        config: Option<String>,
    },
    /// `!RedrawGroup <Group> [Config]`
    RedrawGroup {
        group: String,
        config: Option<String>,
    },
    /// `!ShowMeterGroup <Group> [Config]`
    ShowMeterGroup {
        group: String,
        config: Option<String>,
    },
    /// `!HideMeterGroup <Group> [Config]`
    HideMeterGroup {
        group: String,
        config: Option<String>,
    },
    /// `!ToggleMeterGroup <Group> [Config]`
    ToggleMeterGroup {
        group: String,
        config: Option<String>,
    },
    /// `!ShowFade / !ShowFadeGroup`
    ShowFade {
        name: String,
        config: Option<String>,
    },
    /// `!HideFade / !HideFadeGroup`
    HideFade {
        name: String,
        config: Option<String>,
    },
    /// `!ToggleFade / !ToggleFadeGroup`
    ToggleFade {
        name: String,
        config: Option<String>,
    },
    /// `!RefreshGroup <Group>`
    RefreshGroup {
        group: String,
    },
    /// `!Log <Message>`
    Log {
        message: String,
    },
    /// `!NoOp` (Graceful fallback for aesthetic/unsupported bangs)
    NoOp,
    /// External command / shell execution (e.g. `["https://..."]`, `["xdg-open /path"]`)
    Execute(String),
}

/// Tokenizes a single command line into arguments, respecting quotes.
pub fn tokenize_args(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '"';

    for c in input.chars() {
        match c {
            '"' | '\'' => {
                if in_quotes && c == quote_char {
                    in_quotes = false;
                } else if !in_quotes {
                    in_quotes = true;
                    quote_char = c;
                } else {
                    current.push(c);
                }
            }
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current);
                    current = String::new();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Parses a string containing one or multiple Rainmeter bangs.
///
/// Supports chained bracket syntax: `[!SetVariable Var 1][!UpdateMeter *][!Redraw]`
/// as well as unbracketed single bangs: `!SetVariable Var 1` or `["xdg-open url"]`.
pub fn parse_bangs(input: &str) -> Vec<Bang> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // Check for bracketed syntax `[...]`
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let mut bangs = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut quote_char = '"';
        let mut depth: usize = 0;

        for c in trimmed.chars() {
            match c {
                '"' | '\'' => {
                    if in_quotes && c == quote_char {
                        in_quotes = false;
                    } else if !in_quotes {
                        in_quotes = true;
                        quote_char = c;
                    }
                    if depth > 0 {
                        current.push(c);
                    }
                }
                '[' if !in_quotes => {
                    if depth == 0 {
                        current.clear();
                    } else {
                        current.push('[');
                    }
                    depth += 1;
                }
                ']' if !in_quotes => {
                    if depth > 0 {
                        depth -= 1;
                        if depth == 0 {
                            let inner = current.trim();
                            if !inner.is_empty() {
                                if let Some(bang) = parse_single_bang(inner) {
                                    bangs.push(bang);
                                }
                            }
                            current.clear();
                        } else {
                            current.push(']');
                        }
                    }
                }
                _ => {
                    if depth > 0 {
                        current.push(c);
                    }
                }
            }
        }
        bangs
    } else {
        parse_single_bang(trimmed).into_iter().collect()
    }
}

/// Parses a single bang command string.
fn parse_single_bang(input: &str) -> Option<Bang> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Handle external URL or executable in quotes `["..."]`
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        let stripped = &trimmed[1..trimmed.len() - 1].trim();
        return Some(Bang::Execute(stripped.to_string()));
    }

    let tokens = tokenize_args(trimmed);
    if tokens.is_empty() {
        return None;
    }

    let is_explicit_bang = tokens[0].starts_with('!');
    let mut bang_name = tokens[0].as_str();
    if let Some(stripped) = bang_name.strip_prefix('!') {
        bang_name = stripped;
    }

    let lower_name = bang_name.to_ascii_lowercase();
    let args = &tokens[1..];

    match lower_name.as_str() {
        "setvariable" => {
            if args.is_empty() {
                return None;
            }
            let name = args[0].clone();
            let value = args.get(1).cloned().unwrap_or_default();
            let config = args.get(2).cloned();
            Some(Bang::SetVariable { name, value, config })
        }
        "setoption" => {
            if args.len() < 2 {
                return None;
            }
            let section = args[0].clone();
            let key = args[1].clone();
            let value = args.get(2).cloned().unwrap_or_default();
            let config = args.get(3).cloned();
            Some(Bang::SetOption { section, key, value, config })
        }
        "writekeyvalue" => {
            if args.len() < 2 {
                return None;
            }
            let section = args[0].clone();
            let key = args[1].clone();
            let value = args.get(2).cloned().unwrap_or_default();
            let file = args.get(3).map(PathBuf::from);
            Some(Bang::WriteKeyValue { section, key, value, file })
        }
        "update" => {
            let config = args.first().cloned();
            Some(Bang::Update { config })
        }
        "updatemeter" => {
            let name = args.first().cloned().unwrap_or_else(|| "*".to_string());
            let config = args.get(1).cloned();
            Some(Bang::UpdateMeter { name, config })
        }
        "updatemeasure" => {
            let name = args.first().cloned().unwrap_or_else(|| "*".to_string());
            let config = args.get(1).cloned();
            Some(Bang::UpdateMeasure { name, config })
        }
        "redraw" => {
            let config = args.first().cloned();
            Some(Bang::Redraw { config })
        }
        "showmeter" => {
            let name = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::ShowMeter { name, config })
        }
        "hidemeter" => {
            let name = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::HideMeter { name, config })
        }
        "togglemeter" => {
            let name = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::ToggleMeter { name, config })
        }
        "updatemetergroup" => {
            let group = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::UpdateMeterGroup { group, config })
        }
        "updatemeasuregroup" => {
            let group = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::UpdateMeasureGroup { group, config })
        }
        "redrawgroup" => {
            let group = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::RedrawGroup { group, config })
        }
        "showmetergroup" => {
            let group = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::ShowMeterGroup { group, config })
        }
        "hidemetergroup" => {
            let group = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::HideMeterGroup { group, config })
        }
        "togglemetergroup" => {
            let group = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::ToggleMeterGroup { group, config })
        }
        "showfade" | "showfadegroup" => {
            let name = args.first().cloned().unwrap_or_default();
            let config = args.get(1).cloned();
            Some(Bang::ShowFade { name, config })
        }
        "hidefade" | "hidefadegroup" => {
            let name = args.first().cloned().unwrap_or_default();
            let config = args.get(1).cloned();
            Some(Bang::HideFade { name, config })
        }
        "togglefade" | "togglefadegroup" => {
            let name = args.first().cloned().unwrap_or_default();
            let config = args.get(1).cloned();
            Some(Bang::ToggleFade { name, config })
        }
        "enablemeasure" => {
            let name = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::EnableMeasure { name, config })
        }
        "disablemeasure" => {
            let name = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::DisableMeasure { name, config })
        }
        "togglemeasure" => {
            let name = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::ToggleMeasure { name, config })
        }
        "pausemeasure" => {
            let name = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::PauseMeasure { name, config })
        }
        "unpausemeasure" => {
            let name = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::UnpauseMeasure { name, config })
        }
        "togglepausemeasure" => {
            let name = args.first().cloned()?;
            let config = args.get(1).cloned();
            Some(Bang::TogglePauseMeasure { name, config })
        }
        "activateconfig" => {
            let config = args.first().cloned()?;
            let file = args.get(1).cloned();
            Some(Bang::ActivateConfig { config, file })
        }
        "deactivateconfig" => {
            let config = args.first().cloned();
            Some(Bang::DeactivateConfig { config })
        }
        "toggleconfig" => {
            let config = args.first().cloned()?;
            let file = args.get(1).cloned();
            Some(Bang::ToggleConfig { config, file })
        }
        "refresh" => {
            let config = args.first().cloned();
            Some(Bang::Refresh { config })
        }
        "refreshgroup" => {
            let group = args.first().cloned().unwrap_or_default();
            Some(Bang::RefreshGroup { group })
        }
        "refreshapp" => Some(Bang::RefreshApp),
        "log" | "logmessage" => {
            let message = args.join(" ");
            Some(Bang::Log { message })
        }
        "commandmeasure" => {
            if args.len() < 2 {
                return None;
            }
            let measure = args[0].clone();
            let command = args[1].clone();
            let config = args.get(2).cloned();
            Some(Bang::CommandMeasure { measure, command, config })
        }
        "execute" => {
            let cmd = args.join(" ");
            Some(Bang::Execute(cmd))
        }
        _ => {
            if is_explicit_bang {
                Some(Bang::NoOp)
            } else {
                Some(Bang::Execute(trimmed.to_string()))
            }
        }
    }
}

/// Updates or inserts a Key=Value in a specific [Section] of an INI file.
pub fn write_key_value_to_file<P: AsRef<std::path::Path>>(
    path: P,
    target_section: &str,
    target_key: &str,
    new_value: &str,
) -> std::io::Result<()> {
    let p = path.as_ref();
    let content = if p.exists() {
        std::fs::read_to_string(p)?
    } else {
        String::new()
    };

    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut in_section = false;
    let mut key_found = false;
    let mut section_found = false;
    let mut insert_idx = lines.len();

    let target_sec_lower = target_section.to_ascii_lowercase();
    let target_k_lower = target_key.to_ascii_lowercase();

    for (i, line) in lines.iter_mut().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let sec_name = &trimmed[1..trimmed.len() - 1].trim().to_ascii_lowercase();
            if in_section {
                insert_idx = i;
                break;
            }
            if sec_name == &target_sec_lower {
                in_section = true;
                section_found = true;
            }
        } else if in_section && trimmed.contains('=') {
            if let Some((k, _)) = trimmed.split_once('=') {
                if k.trim().to_ascii_lowercase() == target_k_lower {
                    *line = format!("{}={}", target_key, new_value);
                    key_found = true;
                    break;
                }
            }
        }
    }

    if !section_found {
        if !lines.is_empty() && !lines.last().map(|s| s.is_empty()).unwrap_or(false) {
            lines.push(String::new());
        }
        lines.push(format!("[{}]", target_section));
        lines.push(format!("{}={}", target_key, new_value));
    } else if !key_found {
        lines.insert(insert_idx, format!("{}={}", target_key, new_value));
    }

    let mut new_content = lines.join("\n");
    new_content.push('\n');
    std::fs::write(p, new_content)
}

/// Dispatches an external shell command or opens a web URL safely.
pub fn execute_command(cmd: &str) -> std::io::Result<()> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        std::process::Command::new("xdg-open")
            .arg(trimmed)
            .spawn()?;
    } else {
        std::process::Command::new("sh")
            .args(["-c", trimmed])
            .spawn()?;
    }
    Ok(())
}
