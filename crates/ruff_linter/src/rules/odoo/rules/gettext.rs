use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;
use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, Operator};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::codes::Rule;
use crate::rules::odoo::helpers::{callee_name, is_test_path};
use crate::rules::odoo::strings::{JoinedValue, joined_values};

/// ## What it does
/// Checks for `_()` or `_lt()` called with something other than a string
/// literal.
///
/// ## Why is this bad?
/// The extractor reads the literal: a variable cannot be extracted into the
/// `.pot`, so the message is never translated.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct GettextVariable {
    name: String,
}

impl Violation for GettextVariable {
    #[derive_message_formats]
    fn message(&self) -> String {
        let GettextVariable { name } = self;
        format!("`{name}()` takes a literal; a variable cannot be extracted into the `.pot`")
    }
}

/// ## What it does
/// Checks for a translated message with two or more unnamed placeholders.
///
/// ## Why is this bad?
/// A translator cannot reorder `%s` placeholders: `%(name)s` lets each
/// language place each value where its grammar wants it.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Style)]
pub(crate) struct GettextPlaceholders;

impl Violation for GettextPlaceholders {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Translated message with multiple unnamed placeholders".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Use `%(name)s` rather than a second bare `%s`".to_string())
    }
}

/// ## What it does
/// Checks for `%r` in a translated message.
///
/// ## Why is this bad?
/// `%r` leaks Python syntax into a user-facing sentence.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Style)]
pub(crate) struct GettextRepr;

impl Violation for GettextRepr {
    #[derive_message_formats]
    fn message(&self) -> String {
        "`%r` in a translated message".to_string()
    }
}

/// ## What it does
/// Checks for a user-facing exception (`UserError`, `ValidationError`,
/// `AccessError`, `AccessDenied`, `MissingError`, `RedirectWarning`) raised
/// with a static message that is not translated.
///
/// ## Why is this bad?
/// The message reaches a reader in the UI: wrapped in `_()` it can be
/// translated.
///
/// A message built from a name, an attribute, a subscript or a call is not
/// reported, nor a concatenation or an f-string whose literal text carries no
/// word.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct MissingGettext {
    name: String,
}

impl Violation for MissingGettext {
    #[derive_message_formats]
    fn message(&self) -> String {
        let MissingGettext { name } = self;
        format!("Static string passed to `{name}` without a gettext call")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Wrap the message in `_()`".to_string())
    }
}

/// ## What it does
/// Checks for a builtin exception raised with a translated message.
///
/// ## Why is this bad?
/// A builtin exception reaches a reader as a traceback, not as UI, and
/// translating it books a developer diagnostic into the module catalogue.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct GettextDeveloperError {
    name: String,
}

impl Violation for GettextDeveloperError {
    #[derive_message_formats]
    fn message(&self) -> String {
        let GettextDeveloperError { name } = self;
        format!("`{name}` reaches a reader as a traceback, not as UI")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Drop the `_()` and use an f-string".to_string())
    }
}

const ERRORS_REQUIRING_GETTEXT: &[&str] = &[
    "UserError",
    "ValidationError",
    "AccessError",
    "AccessDenied",
    "MissingError",
    "RedirectWarning",
];

/// Every builtin exception class of Python 3.14.
const ERRORS_REFUSING_GETTEXT: &[&str] = &[
    "ArithmeticError",
    "AssertionError",
    "AttributeError",
    "BlockingIOError",
    "BrokenPipeError",
    "BufferError",
    "BytesWarning",
    "ChildProcessError",
    "ConnectionAbortedError",
    "ConnectionError",
    "ConnectionRefusedError",
    "ConnectionResetError",
    "DeprecationWarning",
    "EOFError",
    "EncodingWarning",
    "EnvironmentError",
    "Exception",
    "ExceptionGroup",
    "FileExistsError",
    "FileNotFoundError",
    "FloatingPointError",
    "FutureWarning",
    "IOError",
    "ImportError",
    "ImportWarning",
    "IndentationError",
    "IndexError",
    "InterruptedError",
    "IsADirectoryError",
    "KeyError",
    "LookupError",
    "MemoryError",
    "ModuleNotFoundError",
    "NameError",
    "NotADirectoryError",
    "NotImplementedError",
    "OSError",
    "OverflowError",
    "PendingDeprecationWarning",
    "PermissionError",
    "ProcessLookupError",
    "PythonFinalizationError",
    "RecursionError",
    "ReferenceError",
    "ResourceWarning",
    "RuntimeError",
    "RuntimeWarning",
    "StopAsyncIteration",
    "StopIteration",
    "SyntaxError",
    "SyntaxWarning",
    "SystemError",
    "TabError",
    "TimeoutError",
    "TypeError",
    "UnboundLocalError",
    "UnicodeDecodeError",
    "UnicodeEncodeError",
    "UnicodeError",
    "UnicodeTranslateError",
    "UnicodeWarning",
    "UserWarning",
    "ValueError",
    "Warning",
    "ZeroDivisionError",
    "_IncompleteInputError",
];

/// A `%` substitution Python's `%` operator accepts, after any run of escaped
/// `%%`. Anchored: [`matches`] tries it only where no `%` precedes, which is
/// the lookbehind `(?<!%)` the `regex` crate lacks.
static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A(?:%%)*%[#0\- +]*(?:\d+|\*)?(?:\.(?:\d+|\*))?[hlL]?[diouxXeEfFgGcrsa]")
        .expect("valid regex")
});
static REPR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A(?:%%)*%(?:\(\w+\))?r").expect("valid regex"));
static LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{L}").expect("valid regex"));

/// The non-overlapping matches of `(?<!%)` followed by `pattern`, scanned as
/// Python's `re.findall` scans: on a match, on from its end; otherwise, on by
/// one character.
fn matches(pattern: &Regex, text: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut position = 0;
    while position < text.len() {
        let after_percent = text[..position].ends_with('%');
        if !after_percent && let Some(found_match) = pattern.find(&text[position..]) {
            found.push(position..position + found_match.end());
            position += found_match.end();
            continue;
        }
        position += text[position..].chars().next().map_or(1, char::len_utf8);
    }
    found
}

/// A word a reader would read, not punctuation or a placeholder.
fn carries_a_word(text: &str) -> bool {
    let mut rest = String::with_capacity(text.len());
    let mut last = 0;
    for placeholder in matches(&PLACEHOLDER, text) {
        rest.push_str(&text[last..placeholder.start]);
        last = placeholder.end;
    }
    rest.push_str(&text[last..]);
    LETTER.is_match(&rest)
}

fn is_untranslated(expr: &Expr) -> bool {
    match expr {
        Expr::StringLiteral(string) => carries_a_word(string.value.to_str()),
        Expr::FString(fstring) => joined_values(&fstring.value)
            .iter()
            .any(|value| match value {
                JoinedValue::Constant(constant) => carries_a_word(&constant.value),
                JoinedValue::Interpolation(interpolation) => {
                    is_untranslated(&interpolation.expression)
                }
            }),
        Expr::BinOp(ast::ExprBinOp { left, right, .. }) => {
            is_untranslated(left) || is_untranslated(right)
        }
        _ => false,
    }
}

fn is_whitelisted_argument(arg: &Expr) -> bool {
    match arg {
        Expr::Name(_) | Expr::Attribute(_) | Expr::Subscript(_) | Expr::Call(_) => true,
        Expr::If(ast::ExprIf { body, orelse, .. }) => {
            is_whitelisted_argument(body) && is_whitelisted_argument(orelse)
        }
        Expr::BoolOp(ast::ExprBoolOp { values, .. }) => values.iter().all(is_whitelisted_argument),
        Expr::BinOp(_) | Expr::FString(_) => !is_untranslated(arg),
        _ => false,
    }
}

/// Whether the expression is a translation, formatted or not:
/// `_("...")`, `_("...") % x`, `_("...").format(x)`.
fn is_gettext_result(expr: &Expr) -> bool {
    match expr {
        Expr::Call(call) => match &*call.func {
            Expr::Attribute(attribute) if attribute.attr.as_str() == "format" => {
                is_gettext_result(&attribute.value)
            }
            func => matches!(callee_name(func), "_" | "_lt"),
        },
        Expr::BinOp(ast::ExprBinOp {
            left,
            op: Operator::Mod,
            ..
        }) => is_gettext_result(left),
        _ => false,
    }
}

/// E8502, E8503, E8504, E8505, E8511
pub(crate) fn gettext(checker: &Checker, call: &ast::ExprCall) {
    let name = callee_name(&call.func);
    if name.is_empty() || is_test_path(checker.path()) {
        return;
    }
    let first = call.arguments.args.first();
    if ERRORS_REQUIRING_GETTEXT.contains(&name)
        && let Some(first) = first
        && !is_whitelisted_argument(first)
    {
        if checker.is_rule_enabled(Rule::MissingGettext) {
            checker.report_diagnostic(
                MissingGettext {
                    name: name.to_string(),
                },
                call.range(),
            );
        }
        return;
    }
    if ERRORS_REFUSING_GETTEXT.contains(&name)
        && let Some(first) = first
        && is_gettext_result(first)
    {
        if checker.is_rule_enabled(Rule::GettextDeveloperError) {
            checker.report_diagnostic(
                GettextDeveloperError {
                    name: name.to_string(),
                },
                call.range(),
            );
        }
        return;
    }
    if !matches!(name, "_" | "_lt") {
        return;
    }
    let Some(Expr::StringLiteral(message)) = first else {
        if checker.is_rule_enabled(Rule::GettextVariable) {
            checker.report_diagnostic(
                GettextVariable {
                    name: name.to_string(),
                },
                call.range(),
            );
        }
        return;
    };
    let text = message.value.to_str();
    if checker.is_rule_enabled(Rule::GettextPlaceholders) && matches(&PLACEHOLDER, text).len() >= 2
    {
        checker.report_diagnostic(GettextPlaceholders, message.range());
    }
    if checker.is_rule_enabled(Rule::GettextRepr) && !matches(&REPR, text).is_empty() {
        checker.report_diagnostic(GettextRepr, message.range());
    }
}
