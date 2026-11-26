#![cfg(not(target_family = "wasm"))]

use std::path::Path;
use std::process::Command;
use std::str;

use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};
const BIN_NAME: &str = "ruff";

#[cfg(not(target_os = "windows"))]
const TEST_FILTERS: &[(&str, &str)] = &[(".*/resources/test/fixtures/", "[BASEPATH]/")];
#[cfg(target_os = "windows")]
const TEST_FILTERS: &[(&str, &str)] = &[
    (r".*\\resources\\test\\fixtures\\", "[BASEPATH]\\"),
    (r"\\", "/"),
];

#[test]
fn check_project_include_defaults() {

    insta::with_settings!({
        filters => TEST_FILTERS.to_vec()
    }, {
        assert_cmd_snapshot!(Command::new(get_cargo_bin(BIN_NAME))
        .args(["check", "--show-files"]).current_dir(Path::new("./resources/test/fixtures/include-test")), @"
        success: true
        exit_code: 0
        ----- stdout -----
        [BASEPATH]/include-test/a.py
        [BASEPATH]/include-test/nested-project/e.py
        [BASEPATH]/include-test/nested-project/pyproject.toml
        [BASEPATH]/include-test/subdirectory/c.py

        ----- stderr -----
        warning: The top-level linter settings are deprecated in favour of their counterparts in the `lint` section. Please update the following options in `nested-project/pyproject.toml`:
          - 'select' -> 'lint.select'
        ");
    });
}

#[test]
fn check_project_respects_direct_paths() {

    insta::with_settings!({
        filters => TEST_FILTERS.to_vec()
    }, {
        assert_cmd_snapshot!(Command::new(get_cargo_bin(BIN_NAME))
        .args(["check", "--show-files", "b.py"]).current_dir(Path::new("./resources/test/fixtures/include-test")), @"
        success: true
        exit_code: 0
        ----- stdout -----
        [BASEPATH]/include-test/b.py

        ----- stderr -----
        ");
    });
}

#[test]
fn check_project_respects_subdirectory_includes() {

    insta::with_settings!({
        filters => TEST_FILTERS.to_vec()
    }, {
        assert_cmd_snapshot!(Command::new(get_cargo_bin(BIN_NAME))
        .args(["check", "--show-files", "subdirectory"]).current_dir(Path::new("./resources/test/fixtures/include-test")), @"
        success: true
        exit_code: 0
        ----- stdout -----
        [BASEPATH]/include-test/subdirectory/c.py

        ----- stderr -----
        ");
    });
}

#[test]
fn check_project_from_project_subdirectory_respects_includes() {

    insta::with_settings!({
        filters => TEST_FILTERS.to_vec()
    }, {
        assert_cmd_snapshot!(Command::new(get_cargo_bin(BIN_NAME))
        .args(["check", "--show-files"]).current_dir(Path::new("./resources/test/fixtures/include-test/subdirectory")), @"
        success: true
        exit_code: 0
        ----- stdout -----
        [BASEPATH]/include-test/subdirectory/c.py

        ----- stderr -----
        ");
    });
}
