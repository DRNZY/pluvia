use crate::formulas::eval_formula;
use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use crate::variables::VariableMap;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum ActionItem {
    Wait(u64),
    Repeat {
        action_name: String,
        interval_ms: u64,
        count: u32,
    },
    Named(String),
}

#[derive(Debug, Clone)]
struct ActiveRepeat {
    action_name: String,
    interval_ms: u64,
    remaining_count: u32,
    wait_remaining_ms: u64,
}

#[derive(Debug, Clone)]
struct ExecutionState {
    steps: Vec<ActionItem>,
    current_step: usize,
    wait_remaining_ms: u64,
    active_repeat: Option<ActiveRepeat>,
}

/// ActionTimer plugin emulating Rainmeter's animation and timer sequence framework.
#[derive(Debug, Clone)]
pub struct ActionTimerPlugin {
    action_lists: HashMap<u32, Vec<ActionItem>>,
    actions: HashMap<String, String>,
    variables: VariableMap,
    running_lists: HashMap<u32, ExecutionState>,
    last_update: Option<Instant>,
    current_value: MeasureValue,
}

impl ActionTimerPlugin {
    /// Creates a new, empty ActionTimer plugin instance.
    pub fn new() -> Self {
        Self {
            action_lists: HashMap::new(),
            actions: HashMap::new(),
            variables: VariableMap::new(),
            running_lists: HashMap::new(),
            last_update: None,
            current_value: MeasureValue::Number(0.0),
        }
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let mut plugin = Self::new();

        for (k, v) in &config.properties {
            let key = k.to_ascii_lowercase();
            if key.starts_with("actionlist") {
                if let Ok(idx) = key["actionlist".len()..].parse::<u32>() {
                    plugin.add_action_list(idx, v);
                }
            } else if !["measure", "plugin"].contains(&key.as_str()) {
                plugin.add_action(k, v);
            }
        }

        plugin
    }

    /// Add an action list definition by index (e.g. 1 for `ActionList1`).
    pub fn add_action_list(&mut self, index: u32, actions_str: &str) {
        let mut steps = Vec::new();
        for part in actions_str.split('|') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }

            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("wait") {
                let rest = trimmed["wait".len()..].trim();
                let ms = rest.parse::<u64>().unwrap_or(0);
                steps.push(ActionItem::Wait(ms));
            } else if lower.starts_with("repeat") {
                let rest = trimmed["repeat".len()..].trim();
                let parts: Vec<&str> = rest.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 3 {
                    let action_name = parts[0].to_string();
                    let interval_ms = parts[1].parse::<u64>().unwrap_or(0);
                    let count = parts[2].parse::<u32>().unwrap_or(1);
                    steps.push(ActionItem::Repeat {
                        action_name,
                        interval_ms,
                        count,
                    });
                }
            } else {
                steps.push(ActionItem::Named(trimmed.to_string()));
            }
        }
        self.action_lists.insert(index, steps);
    }

    /// Register a named action string (e.g. "MoveDown", "[!SetVariable MyY (#MyY#+1)]").
    pub fn add_action(&mut self, name: &str, action_def: &str) {
        self.actions
            .insert(name.to_ascii_lowercase(), action_def.to_string());
    }

    /// Execute the specified action list index.
    pub fn execute(&mut self, list_idx: u32) {
        if let Some(steps) = self.action_lists.get(&list_idx) {
            if !steps.is_empty() {
                self.running_lists.insert(
                    list_idx,
                    ExecutionState {
                        steps: steps.clone(),
                        current_step: 0,
                        wait_remaining_ms: 0,
                        active_repeat: None,
                    },
                );
            }
        }
    }

    /// Stop execution of a specific action list, or all running action lists.
    pub fn stop(&mut self, list_idx: Option<u32>) {
        if let Some(idx) = list_idx {
            self.running_lists.remove(&idx);
        } else {
            self.running_lists.clear();
        }
    }

    /// Returns true if any action list is currently executing.
    pub fn is_running(&self) -> bool {
        !self.running_lists.is_empty()
    }

    /// Retrieve a variable value.
    pub fn get_variable(&self, name: &str) -> Option<&str> {
        self.variables.get(name)
    }

    /// Set a variable value.
    pub fn set_variable(&mut self, name: &str, val: &str) {
        self.variables.set(name, val);
    }

    /// Handle Rainmeter bang commands (e.g. `"Execute 1"`, `"Stop 1"`).
    pub fn command(&mut self, cmd: &str) {
        let trimmed = cmd.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("execute") {
            let rest = trimmed["execute".len()..].trim();
            if let Ok(idx) = rest.parse::<u32>() {
                self.execute(idx);
            }
        } else if lower.starts_with("stop") {
            let rest = trimmed["stop".len()..].trim();
            if rest.is_empty() {
                self.stop(None);
            } else if let Ok(idx) = rest.parse::<u32>() {
                self.stop(Some(idx));
            }
        }
    }

    /// Advances active timers by `delta_ms` milliseconds and executes ready actions.
    pub fn step(&mut self, delta_ms: u64) {
        let mut completed_lists = Vec::new();
        let running_keys: Vec<u32> = self.running_lists.keys().copied().collect();

        for key in running_keys {
            let mut state = self.running_lists.remove(&key).unwrap();
            let mut time_left = delta_ms;

            loop {
                // If in an active repeat
                if let Some(mut rep) = state.active_repeat.take() {
                    if rep.wait_remaining_ms > time_left {
                        rep.wait_remaining_ms -= time_left;
                        state.active_repeat = Some(rep);
                        break;
                    } else {
                        time_left -= rep.wait_remaining_ms;
                        // Execute action
                        self.execute_action_str(&rep.action_name.clone());
                        rep.remaining_count -= 1;
                        if rep.remaining_count > 0 {
                            rep.wait_remaining_ms = rep.interval_ms;
                            state.active_repeat = Some(rep);
                            continue;
                        } else {
                            // Repeat completed, advance to next step
                            state.current_step += 1;
                        }
                    }
                }

                // If waiting
                if state.wait_remaining_ms > time_left {
                    state.wait_remaining_ms -= time_left;
                    break;
                } else if state.wait_remaining_ms > 0 {
                    time_left -= state.wait_remaining_ms;
                    state.wait_remaining_ms = 0;
                    state.current_step += 1;
                }

                if state.current_step >= state.steps.len() {
                    completed_lists.push(key);
                    break;
                }

                match &state.steps[state.current_step] {
                    ActionItem::Wait(ms) => {
                        let ms = *ms;
                        if ms > time_left {
                            state.wait_remaining_ms = ms - time_left;
                            break;
                        } else {
                            time_left -= ms;
                            state.current_step += 1;
                        }
                    }
                    ActionItem::Named(name) => {
                        let name_clone = name.clone();
                        self.execute_action_str(&name_clone);
                        state.current_step += 1;
                    }
                    ActionItem::Repeat {
                        action_name,
                        interval_ms,
                        count,
                    } => {
                        if *count > 0 {
                            let action_name_clone = action_name.clone();
                            let interval = *interval_ms;
                            let count_val = *count;

                            // Execute initial iteration
                            self.execute_action_str(&action_name_clone);
                            if count_val > 1 {
                                state.active_repeat = Some(ActiveRepeat {
                                    action_name: action_name_clone,
                                    interval_ms: interval,
                                    remaining_count: count_val - 1,
                                    wait_remaining_ms: interval,
                                });
                            } else {
                                state.current_step += 1;
                            }
                        } else {
                            state.current_step += 1;
                        }
                    }
                }
            }

            if !completed_lists.contains(&key) {
                self.running_lists.insert(key, state);
            }
        }
    }

    fn execute_action_str(&mut self, action_str: &str) {
        let content = if let Some(resolved) = self.actions.get(&action_str.to_ascii_lowercase()) {
            resolved.clone()
        } else {
            action_str.to_string()
        };

        // Parse bangs in content: e.g. [!SetVariable VarName VarValue]
        let mut idx = 0;
        while let Some(start) = content[idx..].find('[') {
            let actual_start = idx + start;
            if let Some(end) = content[actual_start..].find(']') {
                let actual_end = actual_start + end;
                let bang = &content[actual_start + 1..actual_end].trim();
                self.process_bang(bang);
                idx = actual_end + 1;
            } else {
                break;
            }
        }
    }

    fn process_bang(&mut self, bang: &str) {
        let trimmed = bang.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("!setvariable") {
            let rest = trimmed["!setvariable".len()..].trim();
            // Split variable name and value
            if let Some((var, val)) = rest.split_once(char::is_whitespace) {
                let var_name = var.trim();
                let var_val = val.trim();

                let expanded = self.variables.expand(var_val);

                // Strip quotes if wrapped in quotes: "(#MyY#+1)"
                let cleaned = if expanded.starts_with('"') && expanded.ends_with('"') && expanded.len() >= 2 {
                    &expanded[1..expanded.len() - 1]
                } else {
                    &expanded
                };

                let cleaned = cleaned.trim();
                // If it's a formula in parentheses or mathematical formula, evaluate
                let formula_str = if cleaned.starts_with('(') && cleaned.ends_with(')') && cleaned.len() >= 2 {
                    &cleaned[1..cleaned.len() - 1]
                } else {
                    cleaned
                };

                let evaluated = match eval_formula(formula_str, &self.variables) {
                    Ok(num) => {
                        if num.fract() == 0.0 && !num.is_infinite() && !num.is_nan() && num.abs() < 1e16 {
                            format!("{:.0}", num)
                        } else {
                            format!("{}", num)
                        }
                    }
                    Err(_) => cleaned.to_string(),
                };

                self.variables.set(var_name, evaluated);
            }
        }
    }
}

impl Default for ActionTimerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Measure for ActionTimerPlugin {
    fn update(&mut self) -> MeasureValue {
        let now = Instant::now();
        if let Some(prev) = self.last_update {
            let elapsed_ms = now.duration_since(prev).as_millis() as u64;
            if elapsed_ms > 0 {
                self.step(elapsed_ms);
            }
        }
        self.last_update = Some(now);

        let is_run = if self.is_running() { 1.0 } else { 0.0 };
        self.current_value = MeasureValue::Number(is_run);
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }

    fn command(&mut self, cmd: &str) {
        self.command(cmd);
    }
}
