use regex::{Captures, Regex};
use std::borrow::Cow;
use std::sync::LazyLock;

static CURLY_BRACES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\\N\{[^}]+})|([{}])").unwrap());

pub(super) fn curly_escape(text: &str) -> Cow<'_, str> {
    CURLY_BRACES.replace_all(text, |caps: &Captures| {
        if let Some(match_) = caps.get(1) {
            match_.as_str().to_string()
        } else {
            if &caps[2] == "{" {
                "{{".to_string()
            } else {
                "}}".to_string()
            }
        }
    })
}

static DOUBLE_CURLY_BRACES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"((\{\{)|(\}\}))").unwrap());

pub(super) fn curly_unescape(text: &str) -> Cow<'_, str> {
    DOUBLE_CURLY_BRACES.replace_all(text, |caps: &Captures| {
        if &caps[1] == "{{" {
            "{".to_string()
        } else {
            "}".to_string()
        }
    })
}
