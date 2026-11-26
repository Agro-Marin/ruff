#!/usr/bin/env sh


cargo build --release && hyperfine --warmup 10 \
  "./target/release/ruff check ./crates/ruff_linter/resources/test/cpython/ --no-cache --exit-zero"
