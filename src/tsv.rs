use std::collections::HashMap;
use anyhow::{Context, Result, bail};

pub struct Tsv {
    pub headers: HashMap<String, usize>,
    pub rows: Vec<Vec<String>>,
}

impl Tsv {
    pub fn new(headers: HashMap<String, usize>, rows: Vec<Vec<String>>) -> Self {
        Self { headers, rows }
    }

    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path).with_context(|| format!("Error reading TSV \"{path}\""))?;
        Self::from_text(&content)
    }

    pub fn from_text(text: &str) -> Result<Self> {
        let mut lines = text.lines();

        let headers_line = lines.next().ok_or_else(|| { anyhow::anyhow!("TSV file is empty") })?;
        let headers: HashMap<String, usize> = headers_line
            .split('\t')
            .enumerate()
            .map(|(i, s)| (s.trim().to_string(), i))
        .collect::<HashMap<String, usize>>();

        let mut rows: Vec<Vec<String>> = Vec::new();
        for line in lines {
            let row: Vec<String> = line
                .split('\t')
                .map(|s| s.trim().to_string())
                .collect();

            if row.len() != headers.len() {
                bail!("Row {} must have {} columns, like header, but has {} columns.", rows.len() + 1, headers.len(), row.len());
            }

            rows.push(row);
        }

        Ok(Self::new(headers, rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tsv_from_file() {
        let tsv_content = "name\tage\tcity\nAlice\t30\tNew York\nBob\t25\tLos Angeles";
        let tsv = Tsv::from_text(&tsv_content);
        assert!(tsv.is_ok());
        
        let tsv = tsv.unwrap();
        assert_eq!(tsv.headers.len(), 3);
        assert_eq!(tsv.headers["name"], 0);
        assert_eq!(tsv.headers["age"], 1);
        assert_eq!(tsv.headers["city"], 2);
        
        assert_eq!(
            tsv.rows,
            vec![
                vec!["Alice", "30", "New York"],
                vec!["Bob", "25", "Los Angeles"]
            ]
        );
    }
}