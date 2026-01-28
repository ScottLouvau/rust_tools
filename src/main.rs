use std::{collections::HashMap, fs::DirEntry, io};

pub mod format_string;
pub mod tsv;

const USAGE: &str = "Rename show episodes using a TSV to map old names to new names.
  Usage: episode-renamer <using-mappings-csv-path> <within-folder-path> <from-name-format> <to-name-format>
   Ex: episode-renamer \"./Bluey-Mappings.csv\" \"./Shows/Bluey (2018)\" \"{SourceTitle}_t{TitleNumber}\" \"{SeriesTitle} S{SeasonNumber} E{EpisodeNumber} {EpisodeTitle}\"

  In the format strings, names in {} must match column names in the TSV.
  All other characters are interpreted as literals.

  For all files under within-folder-path recursively,
  if the filename (without extension) matches the from-name-format for any TSV row,
  rename the file to the to-name-format using values from the same TSV row.
";

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 4 {
        eprintln!("{}", USAGE);
        std::process::exit(1);
    }

    let mappings_tsv_path = &args[1];
    let within_folder_path = &args[2];
    let from_name_format = &args[3];
    let to_name_format = &args[4];

    let mappings_tsv = match tsv::Tsv::from_file(mappings_tsv_path) {
        Ok(tsv) => tsv,
        Err(e) => {
            eprintln!("Error reading TSV file: {}", e);
            std::process::exit(1);
        }
    };

    let from_name_formatter = match format_string::FormatString::parse(from_name_format) {
        Ok(formatter) => formatter,
        Err(e) => {
            eprintln!("Error in from-name-format: {}", e);
            std::process::exit(1);
        }
    };

    let to_name_formatter = match format_string::FormatString::parse(to_name_format) {
        Ok(formatter) => formatter,
        Err(e) => {
            eprintln!("Error in to-name-format: {}", e);
            std::process::exit(1);
        }
    };

    let mut renaming_map = HashMap::new();
    for row in mappings_tsv.rows.iter() {
        let mut row_map = HashMap::new();
        for (value, header) in row.iter().zip(mappings_tsv.headers.iter()) {
            row_map.insert(header.as_str(), value.as_str());
        }

        let from_name = match from_name_formatter.format(&row_map) {
            Ok(name) => name,
            Err(e) => {
                eprintln!("Error formatting from-name: {}", e);
                std::process::exit(1);
            }
        };

        let to_name = match to_name_formatter.format(&row_map) {
            Ok(name) => name,
            Err(e) => {
                eprintln!("Error formatting to-name: {}", e);
                std::process::exit(1);
            }
        };

        renaming_map.insert(from_name, to_name);
    }

    let result = rename_media_files(within_folder_path, &renaming_map);
    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}



fn get_files_recursive(root: &str) -> io::Result<Vec<DirEntry>> {
    let mut files: Vec<DirEntry> = Vec::new();

    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(get_files_recursive(&path.to_string_lossy())?);
        } else if path.is_file() {
            files.push(entry);
        }
    }

    Ok(files)
}

fn rename_media_files(root_folder: &str, renaming_map: &HashMap<String, String>) -> io::Result<()> {
    let files = get_files_recursive(root_folder)?;
    for file in files {
        let path = file.path();
        let old_name = path.file_name().unwrap_or_default().to_string_lossy();

        if let Some(new_name) = renaming_map.get(&old_name.to_string()) {
            let new_path = path.with_file_name(new_name);
            println!("{} -> {}", old_name, new_name);
            std::fs::rename(path, new_path)?;
        } else {
            println!("{} UNMATCHED", old_name);
        }
    }

    Ok(())
}
