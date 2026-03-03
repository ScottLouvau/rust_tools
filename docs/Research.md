# Episode Renamer - Research & Analysis

## Overview

`episode-renamer` is a Rust CLI tool for batch-renaming media files (e.g. TV episode files) from one naming convention to another. It uses a TSV (tab-separated values) file as a lookup table of metadata, combined with user-defined format strings containing variable placeholders, to generate "from" and "to" filenames. It then walks a directory tree, matches existing files against the "from" names, and renames them to the "to" names.

The motivating use case (visible in the usage example) is renaming files like `Bluey_t105` to `Bluey (2018) S01 E05 Sticky Gecko` using a spreadsheet-exported TSV mapping.

## Project Structure

```
episode-renamer/
├── Cargo.toml              # Package manifest (edition 2024, only dep: anyhow)
├── Cargo.lock              # Lock file (anyhow 1.0.100, no transitive deps)
└── src/
    ├── main.rs             # CLI entry point, orchestration, file renaming logic
    ├── tsv.rs              # TSV parser module
    └── format_string.rs    # Format string parser and formatter module
```

There are no configuration files, no README, no `.gitignore`, no CLAUDE.md, and no additional tooling beyond Cargo.

## Dependencies

| Crate   | Version | Purpose |
|---------|---------|---------|
| `anyhow`| 1.0.100 | Ergonomic error handling with `Result`, `Context`, `bail!` |

This is a minimal dependency footprint. The project previously used `regex` (visible in debug build artifacts under `target/debug/.fingerprint/regex-*` and `memchr-*`) but has since removed it in favor of a hand-rolled format string parser.

## Module Details

### `tsv.rs` - TSV Parser

**Struct:** `Tsv { headers: HashMap<String, usize>, rows: Vec<Vec<String>> }`

- `headers` maps column name -> column index (0-based)
- `rows` is a vector of rows, each row a vector of trimmed string values

**Parsing behavior:**
- First line is treated as the header row
- Splits on `\t` (tab character)
- All values are `.trim()`'d
- Validates that every data row has the same number of columns as the header; bails with a clear error message (including row number, expected count, actual count) if not
- `from_file()` reads a file path to string, then delegates to `from_text()`
- Empty files produce an error ("TSV file is empty")

**Tests:** One test covering a 3-column, 2-row TSV parsed from a string literal.

### `format_string.rs` - Template Engine

**Structs:**
- `FormatStringPart` enum: `Literal(String)` | `Variable(usize)`
- `FormatString { parts: Vec<FormatStringPart> }`

This is a simple template language where `{VariableName}` placeholders are interspersed with literal text. Variable names must exactly match TSV column headers.

**Parsing (`parse`):**
- Character-by-character state machine with an `in_variable` flag
- `{` opens a variable reference, `}` closes it
- Variable names are resolved to column indices via the provided `HashMap<String, usize>` (the TSV headers)
- Error cases detected:
  - Nested `{` (variable within variable)
  - Unmatched `}` (closing brace without opening)
  - Unclosed variable (EOF while inside `{...`)
  - Unknown variable name (not in the TSV headers) -- includes all known names in the error message

**Formatting (`format`):**
- Takes a `&[String]` (a TSV row) and concatenates literal parts with looked-up variable values by index
- Errors if a row doesn't have enough columns for a referenced index

**`to_string`:**
- Reconstructs a debug representation using numeric indices instead of names (e.g. `{0} S{1} E{2}`)

**Tests:** Two test functions covering successful parsing/formatting and the three error conditions.

### `main.rs` - CLI & Orchestration

**CLI Interface:**
```
episode-renamer <mappings-tsv-path> <within-folder-path> <from-name-format> <to-name-format> [--dry-run]
```

Arguments are positional (no clap or structopt):
1. Path to the TSV mappings file
2. Root folder containing media files to rename
3. "From" format string (pattern matching current filenames)
4. "To" format string (pattern for desired filenames)
5. Optional `--dry-run` flag (must be the 5th argument, i.e. `args[5]`)

**Core Flow (`run` function):**

1. **Parse TSV** from the file path
2. **Parse both format strings**, validating variable names against TSV headers
3. **Build two HashMaps:**
   - `renaming_map`: from-name -> to-name (for files that need renaming)
   - `backwards_map`: to-name -> from-name (for detecting already-renamed files)
4. **Duplicate detection:** If either map encounters a duplicate key on insert (`.insert()` returns `Some`), it bails with an error identifying the conflicting row number. This prevents:
   - Multiple rows mapping to the same target name
   - Multiple rows mapping from the same source name
5. **Rename files** via `rename_media_files()`

**File Discovery (`get_files_recursive`):**
- Recursively walks the directory tree
- Collects all files (not directories) as `DirEntry` objects
- Uses `std::fs::read_dir` directly (no `walkdir` crate)

**Renaming Logic (`rename_media_files`):**

For each file found recursively:
- Extract the filename stem (without extension)
- Check against three cases:
  1. **Match in `renaming_map`**: File needs renaming. Prints aligned `"old" -> "new" RENAME` line. If not dry-run, performs `std::fs::rename()`. Extension is preserved.
  2. **Match in `backwards_map`**: File already has the target name. Prints `"old" -> "current" SKIP` line. Counts as skipped.
  3. **No match**: File is unrelated. Prints `"name" UNMATCHED` line. Path is saved for the summary.

**Output formatting:**
- Names are padded to align columns using the `pad()` helper (right-pads with spaces to the longest name length)
- Summary prints: count renamed out of total files, skipped count, unmatched files list
- If zero files matched but unmatched files exist, prints a diagnostic showing 5 example expected names vs 5 example actual names -- very helpful for debugging format string mistakes

**Dry-run behavior:**
- The `--dry-run` flag must be exactly `args[5]` (the 6th argument)
- When dry-run is active, `really_do` is `false`, and `std::fs::rename()` is skipped
- Output says "Would rename" instead of "Renamed"
- Note: if fewer than 6 args are provided, `really_do` defaults to `true` (renames happen)

## Design Decisions & Observations

### Strengths
- **Minimal dependencies** -- only `anyhow` for error handling
- **Clear error messages** -- unknown variable names list all known names; duplicate mappings identify the conflicting row; TSV column count mismatches show expected vs actual
- **Safety features** -- duplicate detection prevents ambiguous renames; backwards-map detects already-renamed files; dry-run mode
- **Helpful diagnostics** -- when zero files match, shows example expected vs actual names side-by-side
- **Extension preservation** -- renames only the stem, keeping the original file extension

### Potential Issues / Edge Cases
- **Argument parsing:** No argument validation beyond count. If you pass 4 args (no `--dry-run`), it will rename for real. The `--dry-run` flag detection (`args.len() < 6 || &args[5] != "--dry-run"`) means any 6th argument that isn't exactly `--dry-run` also results in real renames.
- **No confirmation prompt** in non-dry-run mode -- files are renamed immediately
- **File conflicts:** If a target filename already exists on disk (from a different file), `std::fs::rename` will silently overwrite on Unix. No pre-check for this.
- **Case sensitivity:** Filename matching is exact (case-sensitive). Files with slightly different casing won't match.
- **Unicode:** Uses `to_string_lossy()` for path conversion, which replaces invalid UTF-8 with `U+FFFD`. Tab-split parsing assumes well-formed text.
- **Symlinks:** `path.is_dir()` and `path.is_file()` follow symlinks, so symlinked directories would be traversed and symlinked files would be renamed.
- **Empty rows:** A TSV with blank trailing lines would produce rows of a single empty string, which would fail the column-count validation (1 != expected count), which is correct behavior.

### Architecture
The code follows a straightforward pipeline pattern:
```
TSV File  -->  Parse TSV  -->  Parse Format Strings  -->  Build Rename Map  -->  Walk Directory  -->  Match & Rename
```

All error handling flows through `anyhow::Result` with contextual messages. The program exits with code 1 on any error, printing the debug-formatted error chain to stderr.

## Test Coverage

The project has unit tests in both library modules:

- **`tsv::tests`**: 1 test -- basic 3-column TSV parsing
- **`format_string::tests`**: 2 tests -- successful parse/format and 3 error conditions

No integration tests. No tests for the renaming logic itself (`main.rs` functions are not tested).

## Build Configuration

- **Rust Edition:** 2024 (latest stable edition)
- **Build artifacts** exist for both debug and release profiles
- **Release binary** located at `target/release/episode-renamer`
- The `target/` directory also contains `flycheck*/` directories, indicating the author uses Emacs with `flycheck-rust` or a similar LSP-based checker
