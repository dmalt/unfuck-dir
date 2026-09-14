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

# List every folder and move before doing them
unfk --verbose

# Reverse the most recent run
unfk --undo

# Show what --undo would do, without touching anything
unfk --undo --dry
```

## Architecture

### Module Structure

- **`src/main.rs`**: CLI parsing (clap), the `Plan` / `RunOutcome` types, the
  `undo()` orchestration, and all user-facing output. Nothing here is a library
  concern; `main.rs` *is* the presentation layer.
- **`src/lib.rs`**: the domain — planning, executing and reversing moves. Never
  prints; errors implement `Display` and `main` decides how to render them.
- **`src/group.rs`**: grouping files by type or by modification date.
- **`src/history.rs`**: where the undo record lives on disk and how it is read,
  written and deleted.
- **`src/temp_env.rs`**: `#[cfg(test)]` only. A hand-rolled replacement for the
  `temp-env` crate, written deliberately as a learning exercise — do not propose
  swapping it back for the crate.

### Key Design Patterns

**File Type Mapping**:

- `TYPE_TO_EXTS` constant defines categories → extensions mapping
- Inverted to `EXT_TO_TYPE` HashMap using `LazyLock` (initialized once on first access)
- Unknown extensions categorized as "Other"

**Grouping Functions**:

- `group::by_type()`: Groups files by extension using the static mapping
- `group::by_date()`: Groups files by modification date (format: `dd-mm-yyyy`)
- Both return `HashMap<String, Vec<PathBuf>>` where key is folder name

**Plan / execute split** (`main.rs`):

- `Plan::make()` computes what *would* happen: `lib::plan_folders` + `lib::plan_moves`
- `Plan::execute(self)` takes `self` by value, so a plan can only be run once
- `Plan::report()` is the `--dry` summary; `Display for Plan` is the `--verbose` listing
- `RunOutcome` holds `Vec<Result<..>>` for both folders and moves;
  `RunOutcome::report()` formats them, `RunOutcome::pending_undo()` derives the
  undo record
- Effectful code returns data; formatting is a separate, pure step

**Move types** — the distinction matters, don't merge them:

- `Move { src, dst, category }` — an *intention*, before execution
- `CompletedMove { mv, identity }` — a move that *happened*; `FileIdentity`
  (size + mtime) only exists post-execution
- `PendingUndo { moves, folders }` — see below

**Undo** (`--undo`):

- `PendingUndo` is a **to-do list of what remains to be reversed**, not a log of
  what happened. After an undo pass it is rewritten with only the entries that
  were *not* reversed; when empty it is deleted (`history::clear`).
- Only *successful* moves and *successfully created* folders are recorded.
- Reversal order: move files back first, **then** remove folders — otherwise you
  delete the directories holding the files you are about to restore.
- `fs::remove_dir`, never `remove_dir_all`: refusing on a non-empty directory is
  the safety feature.
- Each record stores size + mtime read from `dst` after the rename, because
  destination folders are shared space. At undo time: skip on a *positive*
  mismatch, but **proceed when verification is impossible** (no identity
  recorded, or no mtime on the platform). Refusing would strand files forever,
  a worse and commoner failure than touching a stranger's file.
- `check_undo()` is the verdict; `undo_move()` is `check_undo()` plus the rename.
  `--undo --dry` calls only the former, so it cannot mutate anything.
- `--undo --dry` cannot predict folder removal: the files have not moved back
  yet, so every category folder still looks non-empty.

### Important Notes

- Directories are skipped during grouping (only files are processed)
- `--dry` mode shows what would happen without actually moving files
- File extension lookup is case-sensitive (extensions stored as lowercase)
- Duplicate destinations are auto-renamed (`file__1.txt`) by `make_nonexistent_dst`
- `main()` and `undo()` return `ExitCode` rather than calling `process::exit`, so
  destructors run; `group_files` is the one remaining place that aborts directly

## Planned Features

- Duplicate file handling: choice of skip/rename/overwrite (only rename exists today)
- `--forget` to drop an undo record that can never be completed
- `--format json` for machine-readable output
- Atomic writes for the undo record (temp file + rename)
- Date range filtering
