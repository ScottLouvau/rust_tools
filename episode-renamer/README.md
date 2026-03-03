## Purpose
episode-renamer automates renaming a set of files from one pattern to another, using a TSV with mappings.

I had a large number of TV show episodes with file names like "Show (Year) S01 Exx.mkv" and I wanted to add the titles to them.
Rather than doing all of the renames manually (and risking having to do it again someday), I wanted to make one file with the mapping between Season/Episode numbers and Titles and then automate the renames.

## Installation
[Install Rust](https://rust-lang.org/tools/install/) if you haven't already, then run `cargo install episode-renamer`.

## Example Use
With this tool, make a TSV file `Mappings.tsv` like:

| SeasonNumber | EpisodeNumber | Title         |
| ------------ | ------------- | ------------- |
| 1            | 01            | Pilot         |
| 1            | 02            | The Adventure |

The current file names look like:
`"Show (Year) S0{SeasonNumber} E{EpisodeNumber}"`

but I want them to be like:
`"Show (Year) S0{SeasonNumber} E{EpisodeNumber} - {Title}"`

So, I can run the tool, passing the TSV path, the folder with the episode files, and those two format strings:
`episode-renamer Mappings.tsv "./Shows/Show (Year)" "Show (Year) S0{SeasonNumber} E{EpisodeNumber}" "Show (Year) S0{SeasonNumber} E{EpisodeNumber} - {Title}" --dry-run`

This will find all files under `./Shows/Show (Year)`, and propose a rename for each file that matches a row in Mappings.TSV. `--dry-run` allows you to see what the tool will do without the renames actually happening. 


## Clear Output

I've designed episode-renamer to provide clear, helpful error messages for as many mistakes as possible.

You'll get clear messages if:
- The TSV can't be parsed
- A TSV row doesn't have the right number of columns
- A Format String can't be parsed
- A Format string has variable names that don't match the TSV column names
- Multiple TSV rows would rename **from** the same file name
- Multiple TSV rows would rename **to** the same file name
- Files which didn't have any matching row in the TSV
- Format strings which don't match any of your files

These are all mistakes I made when iterating on episode renaming.

When errors occur, the tool will tell you where the error was, and list valid values that would've worked. When no files match your format strings, it gives examples of file names that would've matched, to help you identify the problem with the format string.
