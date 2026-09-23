# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## Important: Learning Project

**This is a learning project for studying Rust.** When working on this codebase:

- Act as a teacher and instructor, not just a code writer
- Explain concepts and trade-offs rather than implementing solutions directly
- Guide the user to write code themselves
- Provide hints, relevant functions/crates, and examples when asked
- Focus on educational value - help the user understand _why_ and _how_, not
  just _what_

The user is proficient in Python and is learning Rust through hands-on practice.

## Project Overview

A CLI tool for organizing files in a directory by grouping them into folders
based on either file type (extension) or modification date. Three names are in
play: the crate is `unfuck-dir` on crates.io, the installed command is `unfuck`,
and the library is `unfk` (so `use unfk::…` in `main.rs` keeps working).

## Build & Run Commands

```bash
# Build
cargo build

# Run in dev
cargo run -- [OPTIONS]

# Install locally
cargo install --path .

# Run after install
unfuck [FOLDER] [OPTIONS]
```

## CLI Usage Examples

```bash
# Organize ~/Downloads by type (the folder defaults to ~/Downloads)
unfuck

# The folder is a positional argument
unfuck ~/Desktop --by date

# Preview changes without moving files
unfuck --dry

# Show available file type categories
unfuck --show-categories

# List every folder and move before doing them
unfuck --verbose

# Reverse the most recent run
unfuck --undo

# Show what --undo would do, without touching anything
unfuck --undo --dry
```

## Architecture

### Module Structure

- **`src/main.rs`**: CLI parsing (clap), the `undo()` orchestration, and all
  user-facing output. Nothing here is a library concern; `main.rs` _is_ the
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

|                     | forward                                | reverse                                     |
| ------------------- | -------------------------------------- | ------------------------------------------- |
| plan                | `RunPlan::make()`                      | `PendingUndo` (loaded from disk)            |
| execute             | `RunPlan::execute(self) -> RunOutcome` | `PendingUndo::execute(self) -> UndoOutcome` |
| summary             | `RunOutcome::report()`                 | `UndoOutcome::report()`                     |
| `--dry` summary     | `RunPlan::report()`                    | `PendingUndo::report()`                     |
| `--verbose` listing | `Display` impls                        | `Display` impls                             |

- `execute(self)` takes `self` by value, so a plan can only be run once
- Effectful code returns data; formatting is a separate, pure step
- `RunOutcome::capture()` → `PendingUndo` derives the undo record, keeping only
  the successes

**Path display**: every user-facing path goes through `lib::display_path`, which
shortens `$HOME` to `~` and quotes the result. Do not print a `PathBuf` with
`{:?}` or `.display()` directly — the output then disagrees with every other line.

**Move types** — the distinction matters, don't merge them:

- `Move { src, dst, category }` — an _intention_, before execution
- `ReverseMove { mv, identity }` — a completed move, ready to be undone. Its `mv`
  is **already flipped**, so no call site can get the rename arguments backwards.
  `FileIdentity` (size + mtime) only exists post-execution.
- `PendingUndo { moves, folders }` — see below

**Undo** (`--undo`):

- `PendingUndo` is a **to-do list of what remains to be reversed**, not a log of
  what happened. After an undo pass it is rewritten with only the entries that
  were _not_ reversed (`UndoOutcome::failed()`); when empty it is deleted
  (`undo::clear`).
- Only _successful_ moves and _successfully created_ folders are recorded.
- Reversal order: move files back first, **then** remove folders — otherwise you
  delete the directories holding the files you are about to restore. The
  `Display` impls list them in that same order, so the log matches reality.
- `fs::remove_dir`, never `remove_dir_all`: refusing on a non-empty directory is
  the safety feature. A directory that is _already gone_ counts as success
  (`Removal::AlreadyGone`) and leaves the record.
- Each record stores size + mtime read from the destination after the rename,
  because destination folders are shared space. At undo time: skip on a
  _positive_ mismatch, but **proceed when verification is impossible** (no
  identity recorded, or no mtime on the platform). Refusing would strand files
  forever, a worse and commoner failure than touching a stranger's file.
- The guards live inside `ReverseMove::execute` — checking is part of moving and
  is deliberately not separable. **`--undo --dry` therefore reports the stored
  plan, not a prediction**: it never touches the filesystem and cannot foresee a
  skip.

### Exit Codes

| Code | Meaning                                                                        |
| ---- | ------------------------------------------------------------------------------ |
| `0`  | Success, including "nothing to undo"                                           |
| `1`  | Could not proceed: no state directory, damaged record, failed to save or clear |
| `2`  | Reserved — clap returns it for argument errors                                 |
| `3`  | Partial undo: some entries remain queued for the next `--undo`                 |

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
- Persist per-run failures beside the undo record, inspectable with a command
  (e.g. `--errors`). This is what lets the report _aggregate_ failures by reason
  instead of listing every one: today the detail exists only in `--verbose`
  output, and re-running with `--verbose` cannot reproduce it because the run
  already happened. Design notes: keep it in its own file, not inside
  `PendingUndo` — the record is a shrinking to-do list, a failure log is
  replaced wholesale each run; `io::Error` is not `Serialize`, so the persisted
  shape needs a mirror type holding `kind` and `message` as strings; and it must
  record which operation produced it, since both sorting and undoing generate
  failures into the same last-run-wins slot.
- `--format json` for machine-readable output
- Atomic writes for the undo record (temp file + rename)
- Date range filtering
