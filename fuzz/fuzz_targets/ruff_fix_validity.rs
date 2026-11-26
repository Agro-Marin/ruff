//! Fuzzer harness which actively tries to find testcases that cause Ruff to introduce errors into
//! the resulting file.

#![no_main]

use libfuzzer_sys::{Corpus, fuzz_target};
use ruff_linter::settings::LinterSettings;
use std::sync::OnceLock;

static SETTINGS: OnceLock<LinterSettings> = OnceLock::new();

fn do_fuzz(case: &[u8]) -> Corpus {
    let Ok(code) = std::str::from_utf8(case) else {
        return Corpus::Reject;
    };

    let settings = SETTINGS.get_or_init(LinterSettings::default);
    ruff_linter::test::set_max_iterations(usize::MAX);

    let _ = ruff_linter::test::test_snippet(code, settings);

    Corpus::Keep
}

fuzz_target!(|case: &[u8]| -> Corpus { do_fuzz(case) });
