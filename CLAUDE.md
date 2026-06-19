# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## Important: Learning Project

**This is a learning project for studying Rust.** When working on this codebase:

- Act as a teacher and instructor, not just a code writer
- Explain concepts and trade-offs rather than implementing solutions directly
- Guide the user to write code themselves
- Provide hints, relevant functions/crates, and examples when asked
- Focus on educational value - help the user understand *why* and *how*, not
just *what*

The user is proficient in Python and is learning Rust through hands-on practice.

## Project Overview

A CLI tool for organizing files in a directory by grouping them into folders
based on either file type (extension) or modification date. The project name is
`unfk` but displays as "downloads-sorter" in help text.

## Build & Run Commands

```bash
# Build
cargo build

# Run in dev
cargo run -- [OPTIONS]

# Install locally
cargo install --path .

# Run after install
unfk [OPTIONS]
```

## CLI Usage Examples

```bash
# Organize ~/Downloads by type (default)
unfk

# Organize specific folder by date
unfk --path ~/Desktop --by date

# Preview changes without moving files
unfk --dry

# Show available file type categories
unfk --show-categories
```

## Architecture

### Module Structure

- **`src/main.rs`**: Entry point, CLI argument parsing (clap), orchestrates file organization
- **`src/group.rs`**: File grouping logic - contains functions to group files by type or date

### Key Design Patterns

**File Type Mapping**:

- `TYPE_TO_EXTS` constant defines categories → extensions mapping
- Inverted to `EXT_TO_TYPE` HashMap using `LazyLock` (initialized once on first access)
- Unknown extensions categorized as "Other"

**Grouping Functions**:

- `group::by_type()`: Groups files by extension using the static mapping
- `group::by_date()`: Groups files by modification date (format: `dd-mm-yyyy`)
- Both return `HashMap<String, Vec<PathBuf>>` where key is folder name

**File Operations**:

- `expand_tilde()`: Converts `~` to HOME directory path
- `move_files()`: Creates category folders and moves/renames files into them
- `format_mv()`: Formats move operations for dry-run display (replaces HOME with `~`)

### Important Notes

- Directories are skipped during grouping (only files are processed)
- Folders named "Folders" are skipped during move operations
- `--dry` mode shows what would happen without actually moving files
- File extension lookup is case-sensitive (extensions stored as lowercase)

## Planned Features

- Undo functionality (reverse last organization)
- Duplicate file handling (skip/rename/overwrite options)
- Metadata preservation (file modification times)
- Statistics/summary output
- Date range filtering
