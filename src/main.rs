use std::{collections::HashMap, fs::DirEntry, io};

pub mod format_string;
pub mod tsv;

const USAGE: &str = "Rename show episodes using a TSV to map old names to new names.
  Usage: episode-renamer <using-mappings-csv-path> <within-folder-path> <from-name-format> <to-name-format> <--dry-run?>
   Ex: episode-renamer \"./Bluey-Mappings.csv\" \"./Shows/Bluey (2018)\" \"{SourceTitle}_t{TitleNumber}\" \"{SeriesTitle} S{SeasonNumber} E{EpisodeNumber} {EpisodeTitle}\"
   Pass --dry-run after arguments to show actions without doing renames.

  In the format strings, names in {} must match column names in the TSV.
  All other characters are interpreted as literals.

  For all files under within-folder-path recursively,
  if the filename (without extension) matches the from-name-format for any TSV row,
  rename the file to the to-name-format using values from the same TSV row.
";

// TODO:
//   Check I/O errors.
//   Better design bindings between Tsv and FormatString. ColumnIndex option, and map variables to ColumnIndex before main loop?
//.  'Do' option to allow dry-run verification?
//.  Terser code for wrapping error message, outputting, stopping?

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
    let really_do = args.len() < 6 || &args[5] != "--dry-run";

    let mappings_tsv = match tsv::Tsv::from_file(mappings_tsv_path) {
        Ok(tsv) => tsv,
        Err(e) => {
            eprintln!("Error reading TSV file '{}': {}", mappings_tsv_path, e);
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
    let mut backwards_map = HashMap::new();
    for row in mappings_tsv.rows.iter() {
        let from_name = match from_name_formatter.format_names_and_values(&mappings_tsv.headers, &row) {
            Ok(name) => name,
            Err(e) => {
                eprintln!("Error formatting from-name: {}", e);
                std::process::exit(1);
            }
        };

        let to_name = match to_name_formatter.format_names_and_values(&mappings_tsv.headers, &row) {
            Ok(name) => name,
            Err(e) => {
                eprintln!("Error formatting to-name: {}", e);
                std::process::exit(1);
            }
        };

        if backwards_map.insert(to_name.clone(), from_name.clone()).is_some() {
            eprintln!("Error: Duplicate to-name '{}' generated from multiple TSV rows.", to_name);
            std::process::exit(1);
        }

        if renaming_map.insert(from_name, to_name).is_some() {
            eprintln!("Error: Duplicate from-name generated from multiple TSV rows.");
            std::process::exit(1);
        }
    }

    let result = rename_media_files(within_folder_path, &renaming_map, &backwards_map, really_do);
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

fn rename_media_files(root_folder: &str, renaming_map: &HashMap<String, String>, backwards_map: &HashMap<String, String>, really_do: bool) -> io::Result<()> {
    let mut renamed_count = 0;
    let mut skipped_count = 0;
    let mut unmatched = Vec::new();

    let longest_name = renaming_map.iter().map(|(from, to)| from.len().max(to.len())).max().unwrap_or(0);

    let files = get_files_recursive(root_folder)?;
    for file in files.iter() {
        let path = file.path();
        let extension = path.extension().unwrap_or_default();
        let current_name = path.file_stem().unwrap_or_default().to_string_lossy();

        if let Some(new_name) = renaming_map.get(&current_name.to_string()) {
            println!("{}{}  ->  {}{} RENAME", current_name, pad(&current_name, longest_name), new_name, pad(&new_name, longest_name));

            let new_path = path.with_file_name(new_name).with_extension(extension);
            if really_do {
                std::fs::rename(path, new_path)?;
            }
            renamed_count += 1;

        } else if let Some(old_name) = backwards_map.get(&current_name.to_string()) {
            println!("{}{}  ->  {}{}  SKIP", old_name, pad(&old_name, longest_name), current_name, pad(&current_name, longest_name));
            skipped_count += 1;

        } else {
            println!("{}{} UNMATCHED", current_name, pad(&current_name, 2 * longest_name + 7));
            unmatched.push(current_name.to_string());
        }
    }

    println!("");
    let action = if really_do { "Renamed" } else { "Would rename" };
    println!("{} {} of {} files under \"{}\".", action, renamed_count, files.len(), root_folder);

    if skipped_count > 0 {
        println!("Skipped {} files already in the desired pattern.", skipped_count);
    }

    if unmatched.len() > 0 {
        println!("");
        println!("Unable to match {} files:", unmatched.len());
        for name in unmatched.iter() {
            println!("  {}", name);
        }
    }

    Ok(())
}

fn pad(s: &str, padded_length: usize) -> String {
    if s.len() < padded_length {
        let added_length = padded_length - s.len();
        " ".repeat(added_length)
    } else {
        String::new()
    }
}