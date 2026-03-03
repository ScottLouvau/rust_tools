# Plan: Refactor main.rs for Testability

## Problem

All logic in `main.rs` is untested. The two key functions -- `run()` and `rename_media_files()` -- blend pure logic with filesystem I/O and printing, making them impossible to unit test without a real directory tree.

Specifically:
- **`run()` lines 52-65**: Map-building logic (format application + duplicate detection) is tangled with TSV file reading and the rename call.
- **`rename_media_files()`**: File classification (rename/skip/unmatched) is interleaved with `get_files_recursive()`, `println!`, and `std::fs::rename()`.

## Approach

Extract pure-logic functions that take simple types and return values, leaving the existing functions as thin wrappers that call them. No new files, no new dependencies, no structural changes to the module layout.

## Changes

### 1. Extract `build_renaming_maps`

**What:** Pull lines 52-65 of `run()` into a standalone function.

**Signature:**
```rust
fn build_renaming_maps(
    mappings_tsv: &tsv::Tsv,
    from_fmt: &FormatString,
    to_fmt: &FormatString,
) -> Result<(HashMap<String, String>, HashMap<String, String>)>
```

**Returns:** `(renaming_map, backwards_map)` or an error on duplicate from-names or duplicate to-names.

**In `run()`**, replace lines 52-67 with:
```rust
let (renaming_map, backwards_map) = build_renaming_maps(&mappings_tsv, &from_name_formatter, &to_name_formatter)?;
rename_media_files(within_folder_path, &renaming_map, &backwards_map, really_do)
```

**Tests to add:**
- Basic: 3 rows, unique mappings, verify both maps have correct entries.
- Duplicate from-name: two rows producing the same from-name -> error.
- Duplicate to-name: two rows producing the same to-name -> error.

---

### 2. Introduce `FileAction` enum and `classify_file` function

**What:** Make the per-file rename/skip/unmatched decision an explicit, testable return value instead of inline branching with side effects.

**New types:**
```rust
#[derive(Debug, PartialEq)]
enum FileAction {
    Rename { to_name: String },
    Skip,
    Unmatched,
}
```

**New function:**
```rust
fn classify_file(
    file_name_without_extension: &str,
    renaming_map: &HashMap<String, String>,
    backwards_map: &HashMap<String, String>,
) -> FileAction
```

Logic is the same three-branch check already in `rename_media_files()` lines 104-120, just returning data instead of printing/renaming.

**In `rename_media_files()`**, replace the `if let / else if let / else` chain with a call to `classify_file`, then `match` on the result to handle printing and renaming as before.

**Tests to add:**
- File stem found in renaming_map -> `Rename` with correct to_name.
- File stem found in backwards_map -> `Skip`.
- File stem in neither -> `Unmatched`.

---

### 3. Change `get_files_recursive` to return `io::Result<Vec<PathBuf>>`

**What:** Currently returns `Vec<DirEntry>`, which is opaque and ties callers to the filesystem API. Changing to `Vec<PathBuf>` makes the return value a simple, ownable type and decouples downstream code from `DirEntry`.

**Signature change:**
```rust
fn get_files_recursive(root: &str) -> io::Result<Vec<PathBuf>>
```

The body changes minimally: push `entry.path()` instead of `entry`, since `entry.path()` already returns `PathBuf`.

**Test fixture:** Add a `tst/sample-structure/` directory committed to the repo with a few fake media files across subdirectories:
```
tst/sample-structure/
├── show_s01e01.mkv
├── show_s01e02.mkv
├── season2/
│   ├── show_s02e01.mkv
│   └── extras/
│       └── behind_the_scenes.mkv
└── metadata.nfo
```

**Tests to add:**
- `get_files_recursive` on `tst/sample-structure/` returns all 5 files (including those in subdirectories) but no directories.
- Returned paths are sorted or collected into a `HashSet` for order-independent comparison.

---

### 4. Make `rename_media_files` take a `Vec<PathBuf>`

**What:** Currently `rename_media_files` calls `get_files_recursive` internally, coupling it to filesystem discovery. Instead, have `run()` call `get_files_recursive` and pass the result in. This lets tests supply a fake file list without touching the filesystem.

**Signature change:**
```rust
fn rename_media_files(
    files: &Vec<PathBuf>,
    root_folder: &str,
    renaming_map: &HashMap<String, String>,
    backwards_map: &HashMap<String, String>,
    really_do: bool,
) -> Result<()>
```

**In `run()`**, add the file-discovery call before `rename_media_files`:
```rust
let files = get_files_recursive(within_folder_path)?;
rename_media_files(files, within_folder_path, &renaming_map, &backwards_map, really_do)
```

The body of `rename_media_files` changes only in that it uses the passed-in `files` instead of calling `get_files_recursive` itself. The loop iterates `&PathBuf` instead of `&DirEntry`, replacing `file.path()` with the path directly.

**No new tests for this step** -- it's a mechanical parameter change. The testability payoff is that `rename_media_files` can now be tested with constructed `PathBuf` vectors in the future if needed, without a real directory.

---

## What Does NOT Change

- `main()`, `USAGE`, argument parsing -- untouched.
- `file_name_without_extension()`, `pad()` -- untouched.
- `rename_media_files()` still exists and still does printing + `fs::rename()`, it just delegates classification to the new functions.
- `tsv.rs` and `format_string.rs` -- untouched.
- No new files. All new functions and tests go in `main.rs`.
- No new dependencies.

## Edit Summary

| Location | Edit |
|----------|------|
| `main.rs` top | Add `use std::path::PathBuf;`, add `FileAction` enum |
| `main.rs` after `run()` | Add `build_renaming_maps()` function (logic moved from `run()` lines 52-65) |
| `main.rs` after `file_name_without_extension()` | Add `classify_file()` function |
| `main.rs` `run()` | Replace map-building loop with `build_renaming_maps()` call; add `get_files_recursive()` call and pass result to `rename_media_files()` |
| `main.rs` `get_files_recursive()` | Change return type from `Vec<DirEntry>` to `Vec<PathBuf>` |
| `main.rs` `rename_media_files()` | Add `files: Vec<PathBuf>` parameter; replace inline classification with `classify_file()` calls |
| `main.rs` bottom | Add `#[cfg(test)] mod tests` with tests for `build_renaming_maps`, `classify_file`, and `get_files_recursive` |
| `tst/sample-structure/` | Add test fixture directory with fake media files in subdirectories |
