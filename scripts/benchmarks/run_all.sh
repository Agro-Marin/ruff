#!/usr/bin/env sh



hyperfine --ignore-failure --warmup 5 \
  "./target/release/ruff check ./crates/ruff_linter/resources/test/cpython/ --no-cache --silent --select ALL --exit-zero" \
  "flake8 crates/ruff_linter/resources/test/cpython -qq --exit-zero" \
  "pycodestyle crates/ruff_linter/resources/test/cpython -qq" \
  "pylint crates/ruff_linter/resources/test/cpython -j 0  --recursive=y --disable=E,W,C,R --exit-zero"
