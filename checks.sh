#!/usr/bin/env sh

set -em

cargo fmt
cargo fix --allow-dirty --allow-staged
cargo clippy
cargo fmt
