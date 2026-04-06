# UNFK

Organize files in a folder

## Installation

```sh
cargo install --path .
```

## Usage

```sh
unfk  # Organize ~/Downloads folder by type
unfk --path ~/Desktop --by date
unfk --dry  # Dry-run
unfk --show-categories
```

## Development

```sh
cargo build
cargo build --release  # build with optimizations for release; slower build, faster run
cargo test
cargo run -- --dry
cargo check  # see if the code compiles without building the binary; faster than build
```

## References

[the book](https://doc.rust-lang.org/book/ch01-03-hello-cargo.html)
