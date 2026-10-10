#!/usr/bin/env bash
set -euo pipefail  # stop the script at first fail

tag="v$(cargo pkgid | sed -E 's/[^@]+@//')"

if [ -n "$(git status --porcelain)" ]; then
    echo "The git state is dirty. Commit the changes before releasing!"
    exit 1;
fi;

if ! git rev-parse --quiet --verify "refs/tags/$tag" > /dev/null; then
    echo "Tag $tag is missing! Run \`git tag -a $tag\` before continuing."
    exit 1;
fi

if ! git tag --points-at HEAD | grep -qx "$tag"; then
    echo "Tag $tag is not on HEAD. Check out the tagged commit first."
    exit 1
fi


# ------------ MacOS --------------- #
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
lipo -create -output unfuck \
    target/aarch64-apple-darwin/release/unfuck \
    target/x86_64-apple-darwin/release/unfuck
tar -czf unfuck-macos-universal.tar.gz unfuck

# ------------ Linux --------------- #
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
    cargo build --release --target x86_64-unknown-linux-musl
tar -czf unfuck-linux-x86_64.tar.gz -C target/x86_64-unknown-linux-musl/release unfuck

# ------------ Cargo --------------- #
cargo publish

gh release create "$tag" ./*.tar.gz --title "$tag" --notes-from-tag

rm unfuck ./*.tar.gz

# https://github.com/dmalt/unfuck-dir/releases/latest/download/unfuck-macos-universal.tar.gz
#
# To test it the way a user would, run curl -L <that link> | tar xz in a scratch folder,
# then ./unfuck --version.
