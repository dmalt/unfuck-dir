# Unfuck-dir

```
    /\          .   *
   /**\       * UN#@%!            .   *   .
  /____\     /  .     *        .              .
  ( o_o)    /              ?                     [[[]]]
  /|   |___o          @   [~]  {}                {{{}}}
 / |___|             _.-[_]-(@)-._  %            ((()))
   /   \        __.-~[#]~{}~@~[_]~-.__           @@@###

A magic spell for the darkest corners of the filesystem.
```

Organizes 'Downloads' and other file graveyards:

```console
$ ls ~/Downloads
holiday.jpg  notes.txt  report.pdf  setup.dmg  song.mp3

$ unfuck
Moved 5 file(s):
  Documents              2
  Apps                   1
  Images                 1
  Music                  1

$ tree ~/Downloads
~/Downloads
├── Apps
│   └── setup.dmg
├── Documents
│   ├── notes.txt
│   └── report.pdf
├── Images
│   └── holiday.jpg
└── Music
    └── song.mp3
```

It's safe to point at a folder you care about:

- `--dry` shows the plan without touching anything
- `--undo` puts files back where they were ([details](#undo))
- nothing is overwritten: name collisions get a suffix instead (`a__1.pdf`)

## Installation

### MacOS

```sh
mkdir -p ~/.local/bin
curl -fsSL https://github.com/dmalt/unfuck-dir/releases/latest/download/unfuck-macos-universal.tar.gz \
    | tar -xz -C ~/.local/bin unfuck
```

`~/.local/bin` is not in the `PATH` on MacOS by defalut, so you might want to add
it, e.g.

```sh
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

### Linux

```sh
mkdir -p ~/.local/bin
curl -fsSL https://github.com/dmalt/unfuck-dir/releases/latest/download/unfuck-linux-x86_64.tar.gz \
    | tar -xz -C ~/.local/bin unfuck
```

### Build from source

```sh
cargo install unfuck-dir
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

### Releasing

There's no fancy CI build system in place for two reasons:

- Setting up CI results in a disproportionate amount of code to maintain for something very simple
- I'd rather not use 3rd party tools where I can avoid them, especially since
  this project is mostly for fun and learning.

Instead, the release is generated manually:

1. Bump the version in `Cargo.toml`, run `cargo check` to update the lockfile
2. Commit all the changes, e.g. `git add -u && git commit -m "chore(release): 0.2.2"`
3. Tag it, e.g. `git tag -a v0.2.2 -m "Release notes"`. The
   annotation message will appear in the release notes.
4. Push the tag with `git push --follow-tags`
5. Build and publish the artefacts with

   ```sh
   ./release.sh
   ```

#### Build setup

Run once to add the targets:

```sh
rustup target add x86_64-apple-darwin
rustup target add x86_64-unknown-linux-musl
```

`CLAUDE.md` documents the internal design and the reasoning behind it.
