#!/usr/bin/env sh


git clone --branch 3.12 https://github.com/python/cpython.git crates/ruff_linter/resources/test/cpython --filter=blob:none --depth=1
