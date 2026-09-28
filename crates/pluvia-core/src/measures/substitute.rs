/// Parsing and application of Rainmeter `Substitute` and `RegExpSubstitute` directives.

/// Upper bound on a substituted string. Rainmeter skins legitimately produce large
/// values, but a runaway substitution (e.g. an empty `from` key, which matches at every
/// character boundary) would otherwise grow without limit and exhaust memory.
const MAX_SUBSTITUTE_LEN: usize = 1 << 20; // 1 MiB

pub fn parse_substitute_pairs(raw: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let text = raw.trim();
    if text.is_empty() {
        return pairs;
    }

    // Try standard quoted matching first: "key":"val"
    if let Ok(re_quoted) = regex::Regex::new(r#""([^"]*)":"([^"]*)""#) {
        for cap in re_quoted.captures_iter(text) {
            pairs.push((cap[1].to_string(), cap[2].to_string()));
        }

        if !pairs.is_empty() {
            return pairs;
        }

        // Handle case where outer quotes were stripped by parser, e.g. `0":"ffffff","1":"333333`
        let wrapped = format!("\"{}\"", text);
        for cap in re_quoted.captures_iter(&wrapped) {
            pairs.push((cap[1].to_string(), cap[2].to_string()));
        }
        if !pairs.is_empty() {
            return pairs;
        }
    }

    // Fallback: comma separated key:val
    for item in text.split(',') {
        if let Some((k, v)) = item.split_once(':') {
            pairs.push((
                k.trim().trim_matches('"').to_string(),
                v.trim().trim_matches('"').to_string(),
            ));
        }
    }

    pairs
}

pub fn apply_substitute(input: &str, substitute_raw: &str, is_regex: bool) -> String {
    let pairs = parse_substitute_pairs(substitute_raw);
    if pairs.is_empty() {
        return input.to_string();
    }

    // Guard against unbounded growth before doing any work.
    if input.len() > MAX_SUBSTITUTE_LEN {
        return input.to_string();
    }

    let mut result = input.to_string();
    for (from, to) in pairs {
        if is_regex {
            if let Ok(re) = regex::Regex::new(&from) {
                let replaced = re.replace_all(&result, to.as_str());
                if replaced.len() > MAX_SUBSTITUTE_LEN {
                    return result;
                }
                result = replaced.into_owned();
            }
        } else {
            // An empty `from` matches at every character boundary, so replacing it
            // would insert `to` between every character and grow the string without
            // bound (e.g. `Substitute="":"#Color#"`). Skip the substitution instead.
            if from.is_empty() {
                continue;
            }
            if to.contains(from.as_str()) && from.len() < to.len() {
                // Replacing would re-introduce the key, expanding on every pass.
                continue;
            }
            if result.len() + to.len() > MAX_SUBSTITUTE_LEN {
                return result;
            }
            result = result.replace(&from, &to);
        }

        if result.len() > MAX_SUBSTITUTE_LEN {
            return result;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_substitute() {
        let sub = "\"0\":\"ffffff\",\"1\":\"333333\"";
        assert_eq!(apply_substitute("0", sub, false), "ffffff");
        assert_eq!(apply_substitute("1", sub, false), "333333");
        assert_eq!(apply_substitute("2", sub, false), "2");
    }

    #[test]
    fn test_unquoted_outer_substitute() {
        let sub = "0\":\"ffffff\",\"1\":\"333333";
        assert_eq!(apply_substitute("0", sub, false), "ffffff");
        assert_eq!(apply_substitute("1", sub, false), "333333");
    }

    #[test]
    fn test_regex_substitute() {
        let sub = "\"0.[4-9].*\":\"0\",\"0.[0-3].*\":\"1\"";
        assert_eq!(apply_substitute("0.75", sub, true), "0");
        assert_eq!(apply_substitute("0.2", sub, true), "1");
    }

    #[test]
    fn test_empty_key_does_not_explode() {
        // `Substitute="":"#Color#"` is used by real skins (e.g. monstercat Chameleon).
        // An empty key must not insert the value between every character.
        let sub = "\"\":\"255,255,255,255\"";
        let input = "255,255,255,255";
        let out = apply_substitute(input, sub, false);
        assert_eq!(out, input);
        // Must be stable under repeated application.
        assert_eq!(apply_substitute(&out, sub, false), input);
    }

    #[test]
    fn test_expanding_substitution_is_bounded() {
        // A key that appears inside its own replacement would grow on every pass.
        let sub = "\"a\":\"ab\"";
        let out = apply_substitute("a", sub, false);
        assert!(out.len() <= MAX_SUBSTITUTE_LEN);
    }

    #[test]
    fn test_repeated_application_is_stable() {
        let sub = "\"\":\"#Color#\"";
        let mut value = "initial".to_string();
        for _ in 0..64 {
            value = apply_substitute(&value, sub, false);
        }
        assert!(value.len() < 1024, "value grew unbounded: {}", value.len());
    }
}
