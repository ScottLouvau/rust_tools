use std::io;

pub struct Tsv {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Tsv {
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self { headers, rows }
    }

    pub fn from_file(path: &str) -> io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_text(&content)
    }

    pub fn from_text(text: &str) -> io::Result<Self> {
        let mut lines = text.lines();

        let headers_line = lines.next().ok_or_else(|| { 
            io::Error::new(io::ErrorKind::InvalidData, "TSV file is empty") })?;

        let headers: Vec<String> = headers_line
            .split('\t')
            .map(|s| s.trim().to_string())
            .collect();

        let mut rows: Vec<Vec<String>> = Vec::new();
        for line in lines {
            let row: Vec<String> = line
                .split('\t')
                .map(|s| s.trim().to_string())
                .collect();

            if row.len() != headers.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Row {} has {} columns but expected {} columns",
                        rows.len() + 1,
                        row.len(),
                        headers.len())));
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
        assert_eq!(tsv.headers, vec!["name", "age", "city"]);
        assert_eq!(
            tsv.rows,
            vec![
                vec!["Alice", "30", "New York"],
                vec!["Bob", "25", "Los Angeles"]
            ]
        );
    }
}