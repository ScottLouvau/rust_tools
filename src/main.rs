use std::{collections::HashMap, fs::DirEntry, io};
use anyhow::{Context, Result};

pub mod format_string;
pub mod tsv;

const USAGE: &str = "Rename episode files from one naming pattern to another, using a TSV with mappings.
  Usage: episode-renamer <mappings-tsv-path> <within-folder-path> <from-name-format> <to-name-format> <--dry-run?>
   Ex: episode-renamer \"./Bluey-Mappings.tsv\" \"./Shows/Bluey (2018)\" \"{SourceTitle}_t{TitleNumber}\" \"{SeriesTitle} S{SeasonNumber} E{EpisodeNumber} {EpisodeTitle}\"

   The format strings are applied to each row in the Mappings TSV to generate expected 'from' and 'to' file names.
   Then, for each file under <within-folder-path>, if the filename (without extension) matches a from-name,
   rename it to the corresponding to-name.
   
   Pass --dry-run after arguments to show actions without doing renames.

   In format strings, names in {} must exactly match TSV column names.
   All other characters are interpreted as literals.
   Ex: \"{SeriesTitle} S{SeasonNumber}E{EpisodeNumber} - SuperHQ\" -> \"Bluey (2018) S01E05 - SuperHQ\"
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
    let really_do = args.len() < 6 || &args[5] != "--dry-run";

    match run(mappings_tsv_path, within_folder_path, from_name_format, to_name_format, really_do) {
        Ok(_) => {}
        Err(e) => {            
            eprintln!("\n{:?}", e);
            std::process::exit(1);
        }
    }
}

fn run(mappings_tsv_path: &str, within_folder_path: &str, from_name_format: &str, to_name_format: &str, really_do: bool) -> Result<()> {
    // Parse TSV (error if file not found, not readable, or rows have different column count than header row)
    let mappings_tsv = tsv::Tsv::from_file(mappings_tsv_path)?;

    // Parse format strings, converting variable names to column indices (error if variable name not in TSV header)
    let from_name_formatter = format_string::FormatString::parse(from_name_format, &mappings_tsv.headers).context("From-Format Error")?;
    let to_name_formatter = format_string::FormatString::parse(to_name_format, &mappings_tsv.headers).context("To-Format Error")?;

    let mut renaming_map = HashMap::new();
    let mut backwards_map = HashMap::new();
    for (index, row) in mappings_tsv.rows.iter().enumerate() {
        let from_name = from_name_formatter.format(&row)?;
        let to_name = to_name_formatter.format(&row)?;

        if backwards_map.insert(to_name.clone(), from_name.clone()).is_some() {
            return Result::Err(anyhow::anyhow!("Multiple TSV rows would rename to \"{to_name}\", including from \"{from_name}\" on row {}.", index + 2));
        }

        if renaming_map.insert(from_name.clone(), to_name.clone()).is_some() {
            return Result::Err(anyhow::anyhow!("Multiple TSV rows would rename from \"{from_name}\", including to \"{to_name}\" on row {}.", index + 2));
        }
    }

    rename_media_files(within_folder_path, &renaming_map, &backwards_map, really_do)
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

fn file_name_without_extension(path: &std::path::Path) -> Option<String> {
    path.file_stem().map(|s| s.to_string_lossy().to_string())
}

fn rename_media_files(root_folder: &str, renaming_map: &HashMap<String, String>, backwards_map: &HashMap<String, String>, really_do: bool) -> Result<()> {
    let mut renamed_count = 0;
    let mut skipped_count = 0;
    let mut unmatched = Vec::new();

    let longest_name = renaming_map.iter().map(|(from, to)| from.len().max(to.len())).max().unwrap_or(0);

    let files = get_files_recursive(root_folder)?;
    for file in files.iter() {
        let path = file.path();
        let extension = path.extension().unwrap_or_default();
        let current_name = file_name_without_extension(&path).unwrap_or_default();

        if let Some(new_name) = renaming_map.get(&current_name.to_string()) {
            println!("\"{}\"{}  ->  \"{}\"{} RENAME", current_name, pad(&current_name, longest_name), new_name, pad(&new_name, longest_name));

            let new_path = path.with_file_name(new_name).with_extension(extension);
            if really_do {
                std::fs::rename(path, new_path)?;
            }
            renamed_count += 1;

        } else if let Some(old_name) = backwards_map.get(&current_name.to_string()) {
            println!("\"{}\"{}  ->  \"{}\"{}  SKIP", old_name, pad(&old_name, longest_name), current_name, pad(&current_name, longest_name));
            skipped_count += 1;

        } else {
            println!("\"{}\"{} UNMATCHED", current_name, pad(&current_name, 2 * longest_name + 7));
            unmatched.push(path.to_string_lossy().to_string());
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

    if unmatched.len() > 0 && renamed_count == 0 {
        eprintln!("\n\nNO FILES MATCHED\n  Expected names like:\n{}\n\n  Saw names like:\n{}", 
            renaming_map.keys().take(5).map(|f| format!("    \"{}\"", f)).collect::<Vec<String>>().join("\n"), 
            files.iter().take(5).map(|f| format!("    \"{}\"", file_name_without_extension(&f.path()).unwrap_or_default())).collect::<Vec<String>>().join("\n"));
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