/// Parsing and application of Rainmeter `Substitute` and `RegExpSubstitute` directives.

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

    let mut result = input.to_string();
    for (from, to) in pairs {
        if is_regex {
            if let Ok(re) = regex::Regex::new(&from) {
                result = re.replace_all(&result, to.as_str()).to_string();
            }
        } else {
            result = result.replace(&from, &to);
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
}
