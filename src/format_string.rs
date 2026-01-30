use anyhow::{Result, bail};

/* FormatString is designed to contain literal parts and variable references, like "Series {Title} Season {SeasonNumber}".
    A HashMap of recognized variable names mapping to indices must be passed to parse().
    Variable Names are validated and converted to indices at parse time.
    Then, during format(), a slice of values is passed in and the output value is constructed from the literal parts and corresponding variable values.
 */

#[derive(Debug, PartialEq)]
pub enum FormatStringPart {
    Literal(String),
    Variable(usize)
}

#[derive(Debug, PartialEq)]
pub struct FormatString {
    parts: Vec<FormatStringPart>,
}

impl FormatString {
    pub fn parse(format: &str, variable_indices: &std::collections::HashMap<String, usize>) -> Result<Self> {
        let mut parts = Vec::new();
        let mut current_literal = String::new();
        let mut in_variable = false;
        let mut variable_name = String::new();

        for (i, c) in format.chars().enumerate() {
            match c {
                '{' => {
                    if in_variable {
                        bail!("\"{format}\" has variable within variable at position {i}.");
                    }

                    if !current_literal.is_empty() {
                        parts.push(FormatStringPart::Literal(current_literal.clone()));
                        current_literal.clear();
                    }

                    in_variable = true;
                }
                '}' => {
                    if !in_variable {
                        bail!("\"{format}\" has closing brace without opening brace at position {i}.");
                    }

                    let var_index = variable_indices.get(&variable_name).ok_or_else(|| anyhow::anyhow!("\"{format}\" refers to unknown variable name '{variable_name}'.\nKnown Names: {}", variable_indices.keys().cloned().collect::<Vec<_>>().join(", ")))?;
                    parts.push(FormatStringPart::Variable(*var_index));
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
            bail!("\"{format}\" has unclosed variable.");
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
                    result.push_str(&(*var.to_string()));
                    result.push('}');
                }
            }
        }
        result
    }

    pub fn format(&self, values: &[String]) -> Result<String> {
        let mut result = String::new();
        for part in &self.parts {
            match part {
                FormatStringPart::Literal(lit) => result.push_str(&lit),
                FormatStringPart::Variable(index) => {
                    let value = values.get(*index).ok_or_else(|| anyhow::anyhow!("A row didn't have enough columns"))?;
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
        let mut known_names = HashMap::from([
            ("SeriesTitle".to_string(), 0),
            ("SeasonNumber".to_string(), 1),
            ("EpisodeNumber".to_string(), 2),
        ]);

        let format = "{SeriesTitle} S{SeasonNumber} E{EpisodeNumber}";

        let parsed = FormatString::parse(format, &known_names);
        assert!(parsed.is_ok());

        let parsed = parsed.unwrap();
        assert_eq!(parsed.to_string(), "{0} S{1} E{2}");

        assert_eq!(parsed.parts.len(), 5);
        assert_eq!(parsed.parts[0], FormatStringPart::Variable(0));
        assert_eq!(parsed.parts[1], FormatStringPart::Literal(" S".to_string()));
        assert_eq!(parsed.parts[2], FormatStringPart::Variable(1));
        assert_eq!(parsed.parts[3], FormatStringPart::Literal(" E".to_string()));
        assert_eq!(parsed.parts[4], FormatStringPart::Variable(2));

        let values = vec!["MyShow".to_string(), "01".to_string(), "02".to_string()];
        let resolved = parsed.format(&values);

        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap(), "MyShow S01 E02");

        known_names.remove("SeriesTitle");
        let unresolved = FormatString::parse(format, &known_names);
        assert!(unresolved.is_err()); // "Missing value for variable 'SeriesTitle'"
    }

    #[test]
    fn test_parse_format_string_errors() {
        let known_names = HashMap::from([
            ("SeriesTitle".to_string(), 0),
            ("SeasonNumber".to_string(), 1),
            ("EpisodeNumber".to_string(), 2),
        ]);

        // "Variable within variable at position 14 in format string."
        assert!(FormatString::parse("{SeriesTitle S{SeasonNumber} E{EpisodeNumber}", &known_names).is_err());

        // "Closing brace without opening brace at position 11 in format string."
        assert!(FormatString::parse("SeriesTitle} S{SeasonNumber E{EpisodeNumber}", &known_names).is_err());

        // "Unclosed variable at end of format string."
        assert!(FormatString::parse("{SeriesTitle} S{SeasonNumber} E{EpisodeNumber", &known_names).is_err());
    }
}
