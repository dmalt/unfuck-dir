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
cargo test
cargo run -- --dry
```
