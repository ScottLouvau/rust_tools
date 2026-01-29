
#[derive(Debug, PartialEq)]
pub enum FormatStringPart {
    Literal(String),
    Variable(String),
}

#[derive(Debug, PartialEq)]
pub struct FormatString {
    parts: Vec<FormatStringPart>,
}

impl FormatString {
    pub fn parse(format: &str) -> Result<Self, String> {
        let mut parts = Vec::new();
        let mut current_literal = String::new();
        let mut in_variable = false;
        let mut variable_name = String::new();

        for (i, c) in format.chars().enumerate() {
            match c {
                '{' => {
                    if in_variable {
                        return Err(format!("Variable within variable at position {i} in format string."));
                    }

                    if !current_literal.is_empty() {
                        parts.push(FormatStringPart::Literal(current_literal.clone()));
                        current_literal.clear();
                    }

                    in_variable = true;
                }
                '}' => {
                    if !in_variable {
                        return Err(format!("Closing brace without opening brace at position {i} in format string."));
                    }

                    parts.push(FormatStringPart::Variable(variable_name.clone()));
                    variable_name.clear();
                    in_variable = false;
                }
                _ => {
                    if in_variable {
                        variable_name.push(c);
                    } else {
                        current_literal.push(c);
                    }
                }
            }
        }

        if !current_literal.is_empty() {
            parts.push(FormatStringPart::Literal(current_literal));
        }

        if in_variable {
            return Err("Unclosed variable at end of format string.".to_string());
        }

        Ok(FormatString { parts })
    }

    pub fn to_string(&self) -> String {
        let mut result = String::new();
        for part in &self.parts {
            match part {
                FormatStringPart::Literal(lit) => result.push_str(&lit),
                FormatStringPart::Variable(var) => {
                    result.push('{');
                    result.push_str(&var);
                    result.push('}');
                }
            }
        }
        result
    }

    pub fn format_map(&self, values: &std::collections::HashMap<&str, &str>) -> Result<String, String> {
        let mut result = String::new();
        for part in &self.parts {
            match part {
                FormatStringPart::Literal(lit) => result.push_str(&lit),
                FormatStringPart::Variable(var) => {
                    if let Some(value) = values.get(var.as_str()) {
                        result.push_str(value);
                    } else {
                        return Err(format!("Missing value for variable '{}'", var));
                    }
                }
            }
        }
        Ok(result)
    }

    pub fn format_names_and_values(&self, names: &std::collections::HashMap<String, usize>, values: &[String]) -> Result<String, String> {
        let mut result = String::new();
        for part in &self.parts {
            match part {
                FormatStringPart::Literal(lit) => result.push_str(&lit),
                FormatStringPart::Variable(var) => {
                    let index = names.get(var).ok_or_else(|| format!("Unknown variable name '{}'", var))?;
                    let value = values.get(*index).ok_or_else(|| format!("No value for variable '{}'", var))?;
                    result.push_str(value);
                }
            }
        }
        Ok(result)
    }
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_parse_format_string() {
        let format = "{SeriesTitle} S{SeasonNumber} E{EpisodeNumber}";
        
        let parsed = FormatString::parse(format);
        assert!(parsed.is_ok());

        let parsed = parsed.unwrap();
        assert_eq!(parsed.to_string(), format);

        assert_eq!(parsed.parts.len(), 5);
        assert_eq!(parsed.parts[0], FormatStringPart::Variable("SeriesTitle".to_string()));
        assert_eq!(parsed.parts[1], FormatStringPart::Literal(" S".to_string()));
        assert_eq!(parsed.parts[2], FormatStringPart::Variable("SeasonNumber".to_string()));
        assert_eq!(parsed.parts[3], FormatStringPart::Literal(" E".to_string()));
        assert_eq!(parsed.parts[4], FormatStringPart::Variable("EpisodeNumber".to_string()));

        let mut map: HashMap<&str, &str> = HashMap::new();
        map.insert(&"SeriesTitle", &"MyShow");
        map.insert(&"SeasonNumber", &"01");
        map.insert(&"EpisodeNumber", &"02");

        let resolved = parsed.format_map(&map);

        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap(), "MyShow S01 E02");

        map.remove("SeriesTitle");
        let unresolved = parsed.format_map(&map);
        assert!(unresolved.is_err());
        assert_eq!(unresolved.unwrap_err(), "Missing value for variable 'SeriesTitle'");
    }

    #[test]
    fn test_parse_format_string_errors() {
        assert_eq!(
            FormatString::parse("{SeriesTitle S{SeasonNumber} E{EpisodeNumber}").unwrap_err(),
            "Variable within variable at position 14 in format string."
        );

        assert_eq!(
            FormatString::parse("SeriesTitle} S{SeasonNumber E{EpisodeNumber}").unwrap_err(),
            "Closing brace without opening brace at position 11 in format string."
        );

        assert_eq!(
            FormatString::parse("{SeriesTitle} S{SeasonNumber} E{EpisodeNumber").unwrap_err(),
            "Unclosed variable at end of format string."
        );
    }
}
