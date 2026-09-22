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

- **`src/main.rs`**: CLI parsing (clap), the `undo()` orchestration, and all
  user-facing output. Nothing here is a library concern; `main.rs` *is* the
  presentation layer.
- **`src/lib.rs`**: the forward direction — planning and executing a sort
  (`RunPlan`, `RunOutcome`, `Move`), plus `display_path`. Never prints.
- **`src/undo.rs`**: everything about reversing a run — the `PendingUndo` record,
  its on-disk location (`state_dir` / `save` / `load` / `clear`), and the
  reversal itself. Never prints.
- **`src/group.rs`**: grouping files by type or by modification date.
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

**Plan / execute / report** — the same three-step shape in both directions:

| | forward | reverse |
|---|---|---|
| plan | `RunPlan::make()` | `PendingUndo` (loaded from disk) |
| execute | `RunPlan::execute(self) -> RunOutcome` | `PendingUndo::execute(self) -> UndoOutcome` |
| summary | `RunOutcome::report()` | `UndoOutcome::report()` |
| `--dry` summary | `RunPlan::report()` | `PendingUndo::report()` |
| `--verbose` listing | `Display` impls | `Display` impls |

- `execute(self)` takes `self` by value, so a plan can only be run once
- Effectful code returns data; formatting is a separate, pure step
- `RunOutcome::capture()` → `PendingUndo` derives the undo record, keeping only
  the successes

**Path display**: every user-facing path goes through `lib::display_path`, which
shortens `$HOME` to `~` and quotes the result. Do not print a `PathBuf` with
`{:?}` or `.display()` directly — the output then disagrees with every other line.

**Move types** — the distinction matters, don't merge them:

- `Move { src, dst, category }` — an *intention*, before execution
- `ReverseMove { mv, identity }` — a completed move, ready to be undone. Its `mv`
  is **already flipped**, so no call site can get the rename arguments backwards.
  `FileIdentity` (size + mtime) only exists post-execution.
- `PendingUndo { moves, folders }` — see below

**Undo** (`--undo`):

- `PendingUndo` is a **to-do list of what remains to be reversed**, not a log of
  what happened. After an undo pass it is rewritten with only the entries that
  were *not* reversed (`UndoOutcome::failed()`); when empty it is deleted
  (`undo::clear`).
- Only *successful* moves and *successfully created* folders are recorded.
- Reversal order: move files back first, **then** remove folders — otherwise you
  delete the directories holding the files you are about to restore. The
  `Display` impls list them in that same order, so the log matches reality.
- `fs::remove_dir`, never `remove_dir_all`: refusing on a non-empty directory is
  the safety feature. A directory that is *already gone* counts as success
  (`Removal::AlreadyGone`) and leaves the record.
- Each record stores size + mtime read from the destination after the rename,
  because destination folders are shared space. At undo time: skip on a
  *positive* mismatch, but **proceed when verification is impossible** (no
  identity recorded, or no mtime on the platform). Refusing would strand files
  forever, a worse and commoner failure than touching a stranger's file.
- The guards live inside `ReverseMove::execute` — checking is part of moving and
  is deliberately not separable. **`--undo --dry` therefore reports the stored
  plan, not a prediction**: it never touches the filesystem and cannot foresee a
  skip.

### Exit Codes

| Code | Meaning |
|---|---|
| `0` | Success, including "nothing to undo" |
| `1` | Could not proceed: no state directory, damaged record, failed to save or clear |
| `2` | Reserved — clap returns it for argument errors |
| `3` | Partial undo: some entries remain queued for the next `--undo` |

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
