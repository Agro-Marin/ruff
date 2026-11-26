use std::borrow::Cow;

use itertools::Itertools;
use ruff_diagnostics::Applicability;
use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::name::QualifiedName;
use ruff_python_ast::{self as ast, Expr};
use ruff_python_trivia::PythonWhitespace;
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::linter::float::as_non_finite_float_string_literal;
use crate::{Edit, Fix, FixAvailability, Violation};
/// ## What it does
/// Checks for unnecessary string literal or float casts in `Decimal`
/// constructors.
///
/// ## Why is this bad?
/// The `Decimal` constructor accepts a variety of arguments, including
/// integers, floats, and strings. However, it's not necessary to cast
/// integer literals to strings when passing them to the `Decimal`.
///
/// Similarly, `Decimal` accepts `inf`, `-inf`, and `nan` as string literals,
/// so there's no need to wrap those values in a `float` call when passing
/// them to the `Decimal` constructor.
///
/// Prefer the more concise form of argument passing for `Decimal`
/// constructors, as it's more readable and idiomatic.
///
/// Note that this rule does not flag quoted float literals such as `Decimal("0.1")`, which will
/// produce a more precise `Decimal` value than the unquoted `Decimal(0.1)`.
///
/// ## Example
/// ```python
/// from decimal import Decimal
///
/// Decimal("0")
/// Decimal(float("Infinity"))
/// ```
///
/// Use instead:
/// ```python
/// from decimal import Decimal
///
/// Decimal(0)
/// Decimal("Infinity")
/// ```
///
/// ## Fix safety
/// This rule's fix is marked as safe, unless the expression contains comments.
///
/// ## References
/// - [Python documentation: `decimal`](https://docs.python.org/3/library/decimal.html)
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.12.0", category = Category::Complexity)]
pub(crate) struct VerboseDecimalConstructor {
    replacement: String,
}

impl Violation for VerboseDecimalConstructor {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Verbose expression in `Decimal` constructor".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        let VerboseDecimalConstructor { replacement } = self;
        Some(format!("Replace with `{replacement}`"))
    }
}

/// FURB157
pub(crate) fn verbose_decimal_constructor(
    checker: &Checker,
    call: &ast::ExprCall,
    qualified_name: &QualifiedName,
) {
    if !matches!(qualified_name.segments(), ["decimal", "Decimal"]) {
        return;
    }

    let Some(value) = call.arguments.find_argument_value("value", 0) else {
        return;
    };

    match value {
        Expr::StringLiteral(ast::ExprStringLiteral {
            value: str_literal, ..
        }) => {
            let original_str = str_literal.to_str().trim_whitespace();
            let sign_check_str = original_str.trim_start_matches('_');
            let (unary, sign_check_str) = if let Some(trimmed) = sign_check_str.strip_prefix('+') {
                ("+", trimmed)
            } else if let Some(trimmed) = sign_check_str.strip_prefix('-') {
                ("-", trimmed)
            } else {
                ("", sign_check_str)
            };
            let str_after_sign_for_normalization = sign_check_str;
            let mut rest = Cow::from(sign_check_str);
            let has_digit_separators = memchr::memchr(b'_', original_str.as_bytes()).is_some();
            if has_digit_separators {
                rest = Cow::from(rest.replace('_', ""));
            }

            if (rest.len() > PYTHONINTMAXSTRDIGITS) || (rest.is_empty()) {
                return;
            }

            let rest = rest.trim_start_matches('0');

            if !rest.bytes().all(|c| c.is_ascii_digit()) {
                return;
            }

            let rest = if has_digit_separators {
                Cow::from(normalize_digit_separators(str_after_sign_for_normalization))
            } else {
                Cow::from(rest)
            };

            let rest = match (unary, rest.is_empty()) {
                ("-", true) => {
                    return;
                }
                (_, true) => "0",
                _ => &rest,
            };

            let replacement = format!("{unary}{rest}");

            let mut diagnostic = checker.report_diagnostic(
                VerboseDecimalConstructor {
                    replacement: replacement.clone(),
                },
                value.range(),
            );

            let applicability = if checker.comment_ranges().intersects(value.range()) {
                Applicability::Unsafe
            } else {
                Applicability::Safe
            };

            diagnostic.set_fix(Fix::applicable_edit(
                Edit::range_replacement(replacement, value.range()),
                applicability,
            ));
        }
        Expr::Call(ast::ExprCall {
            func, arguments, ..
        }) => {
            if !checker.semantic().match_builtin_expr(func, "float") {
                return;
            }

            if !arguments.keywords.is_empty() {
                return;
            }
            let [float] = arguments.args.as_ref() else {
                return;
            };
            let Some(float_str) = as_non_finite_float_string_literal(float) else {
                return;
            };

            let mut replacement = checker.locator().slice(float).to_string();
            if float_str == "-nan" {
                replacement = "\"nan\"".to_string();
            }
            let mut diagnostic = checker.report_diagnostic(
                VerboseDecimalConstructor {
                    replacement: replacement.clone(),
                },
                value.range(),
            );

            let applicability = if checker.comment_ranges().intersects(value.range()) {
                Applicability::Unsafe
            } else {
                Applicability::Safe
            };

            diagnostic.set_fix(Fix::applicable_edit(
                Edit::range_replacement(replacement, value.range()),
                applicability,
            ));
        }
        _ => {}
    }
}

/// Normalizes digit separators in a numeric string by:
/// - Stripping leading and trailing underscores
/// - Collapsing medial underscore sequences to single underscores
fn normalize_digit_separators(original_str: &str) -> String {
    let trimmed = original_str
        .trim_start_matches(['_', '0'])
        .trim_end_matches('_');

    trimmed
        .chars()
        .dedup_by(|a, b| *a == '_' && a == b)
        .collect()
}

const PYTHONINTMAXSTRDIGITS: usize = 640;
