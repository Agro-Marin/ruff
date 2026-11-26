#!/usr/bin/env sh


TARGET_DIR=${1}


hyperfine --ignore-failure \
  --prepare "./target/release/ruff format ${TARGET_DIR}" \
  "./target/release/ruff format ${TARGET_DIR}" \
  --prepare "black --no-cache ${TARGET_DIR} --safe" \
  "black --no-cache ${TARGET_DIR} --safe" \
  --prepare "black --no-cache ${TARGET_DIR} --fast" \
  "black --no-cache ${TARGET_DIR} --fast" \
  --prepare "black --no-cache ${TARGET_DIR} --safe --preview" \
  "black --no-cache ${TARGET_DIR} --safe --preview" \
  --prepare "black --no-cache ${TARGET_DIR} --fast --preview" \
  "black --no-cache ${TARGET_DIR} --fast --preview" \
  --prepare "autopep8 ${TARGET_DIR} --recursive --in-place" \
  "autopep8 ${TARGET_DIR} --recursive --in-place" \
  --prepare "yapf ${TARGET_DIR} --parallel --recursive --in-place" \
  "yapf ${TARGET_DIR} --parallel --recursive --in-place"

hyperfine --ignore-failure --env RAYON_NUM_THREADS=1 \
  --prepare "./target/release/ruff format ${TARGET_DIR}" \
  "./target/release/ruff format ${TARGET_DIR}" \
  --prepare "black --no-cache ${TARGET_DIR} --safe" \
  "black --no-cache ${TARGET_DIR} --workers=1 --safe" \
  --prepare "black --no-cache ${TARGET_DIR} --fast" \
  "black --no-cache ${TARGET_DIR} --workers=1 --fast" \
  --prepare "black --no-cache ${TARGET_DIR} --safe --preview" \
  "black --no-cache ${TARGET_DIR} --workers=1 --safe --preview" \
  --prepare "black --no-cache ${TARGET_DIR} --fast --preview" \
  "black --no-cache ${TARGET_DIR} --workers=1 --fast --preview" \
  --prepare "autopep8 ${TARGET_DIR} --recursive --in-place" \
  "autopep8 ${TARGET_DIR} --in-place --recursive --jobs=1" \
  --prepare "yapf ${TARGET_DIR} --parallel --recursive --in-place" \
  "yapf ${TARGET_DIR} --recursive --in-place"

hyperfine --ignore-failure \
  --prepare "./target/release/ruff format ${TARGET_DIR}" \
  "./target/release/ruff format ${TARGET_DIR} --check" \
  --prepare "black --no-cache ${TARGET_DIR} --safe" \
  "black --no-cache ${TARGET_DIR} --check --safe" \
  --prepare "black --no-cache ${TARGET_DIR} --fast" \
  "black --no-cache ${TARGET_DIR} --check --fast" \
  --prepare "black --no-cache ${TARGET_DIR} --safe --preview" \
  "black --no-cache ${TARGET_DIR} --check --safe --preview" \
  --prepare "black --no-cache ${TARGET_DIR} --fast --preview" \
  "black --no-cache ${TARGET_DIR} --check --fast --preview" \
  --prepare "autopep8 ${TARGET_DIR} --recursive --in-place" \
  "autopep8 ${TARGET_DIR} --recursive --diff" \
  --prepare "yapf ${TARGET_DIR} --parallel --recursive --in-place" \
  "yapf ${TARGET_DIR} --parallel --recursive --quiet"
