# Unfuck-dir

Organize 'Downloads' and other file graveyards.

```txt
~/Downloads/a.pdf     ->  ~/Downloads/Documents/a.pdf
~/Downloads/holiday.jpg  ->  ~/Downloads/Images/holiday.jpg
```

## Installation

```sh
cargo install --path .
```

## Usage

```sh
unfuck                            # organize ~/Downloads by file type
unfuck ~/Desktop --by date        # organize another folder, grouped by date
unfuck --dry                      # show what would happen, change nothing
unfuck --verbose                  # list every folder and move as it happens
unfuck --include-dotfiles         # don't skip dotfiles
unfuck --show-categories          # print the type -> extension table and exit
unfuck --undo                     # reverse the most recent run
unfuck --undo --dry               # show what --undo would do
```

Files that would collide at the destination are renamed rather than overwritten:
`a.pdf`, `a__1.pdf`, `a__2.pdf`. Directories are left alone — only files move.

## Undo

Each real run records what it did under the OS state directory
(`~/Library/Application Support/unfuck` on macOS, `$XDG_STATE_HOME/unfuck` or
`~/.local/state/unfuck` on Linux and the BSDs). `--undo` moves the files back and
removes the folders it created.

Only the most recent run is recorded, so sorting twice makes the first run
unrecoverable.

The record is a **to-do list of what remains to be reversed**, not a log of what
happened. An entry leaves the list only when it has actually been reversed, so a
file you cannot restore today — because something else now sits in its place, or
because it has changed since it was moved — stays queued for the next `--undo`
rather than being silently dropped. When the list empties, the record is deleted.

Before moving a file back, unfuck checks that it is still the file it moved (size
and modification time). If it is not, that entry is skipped and reported. After
we revert the files, we remove the empty category folders; a folder containing
anything unexpected is left in place.

## Exit codes

| Code | Meaning                                                                 |
| ---- | ----------------------------------------------------------------------- |
| `0`  | Success, including "nothing to undo"                                    |
| `1`  | Could not proceed: no state directory, damaged record, or write failure |
| `2`  | Invalid arguments                                                       |
| `3`  | Partial undo: some entries were not reverted and remain queued          |

## Development

```sh
cargo build
cargo build --release  # build with optimizations for release; slower build, faster run
cargo test
cargo clippy --all-targets
cargo fmt
cargo run -- --dry
cargo check  # see if the code compiles without building the binary; faster than build
```

`CLAUDE.md` documents the internal design and the reasoning behind it.

## References

[the book](https://doc.rust-lang.org/book/ch01-03-hello-cargo.html)
