#!/usr/bin/env bash


set -eu

stdlib_path="./crates/ty_vendored/vendor/typeshed/stdlib"

for python_version in 3.15 3.14 3.13 3.12 3.11 3.10
do
  PYTHONUTF8=1 uv run \
    --locked \
    --only-group=typeshed-docstrings \
    --python="$python_version" \
    add-docstrings \
    --stdlib-path="${stdlib_path}"
done
