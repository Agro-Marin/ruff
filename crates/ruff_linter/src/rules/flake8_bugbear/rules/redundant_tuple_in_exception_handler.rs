use ruff_diagnostics::Applicability;
use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, ExceptHandler, Expr};
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::fix::edits::pad;
use crate::{AlwaysFixableViolation, Edit, Fix};

/// ## What it does
/// Checks for single-element tuples in exception handlers (e.g.,
/// `except (ValueError,):`).
///
/// Note: Single-element tuples consisting of a starred expression
/// are allowed.
///
/// ## Why is this bad?
/// A tuple with a single element can be more concisely and idiomatically
/// expressed as a single value.
///
/// ## Example
/// ```python
/// try:
///     ...
/// except (ValueError,):
///     ...
/// ```
///
/// Use instead:
/// ```python
/// try:
///     ...
/// except ValueError:
///     ...
/// ```
///
/// ## Fix safety
/// This rule's fix is marked as safe, unless the exception handler contains comments.
///
/// ## References
/// - [Python documentation: `except` clause](https://docs.python.org/3/reference/compound_stmts.html#except-clause)
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.89", category = Category::Complexity)]
pub(crate) struct RedundantTupleInExceptionHandler {
    name: String,
}

impl AlwaysFixableViolation for RedundantTupleInExceptionHandler {
    #[derive_message_formats]
    fn message(&self) -> String {
        "A length-one tuple literal is redundant in exception handlers".to_string()
    }

    fn fix_title(&self) -> String {
        let RedundantTupleInExceptionHandler { name } = self;
        format!("Replace with `except {name}`")
    }
}

/// B013
pub(crate) fn redundant_tuple_in_exception_handler(checker: &Checker, handlers: &[ExceptHandler]) {
    for handler in handlers {
        let ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
            type_: Some(type_),
            ..
        }) = handler
        else {
            continue;
        };
        let Expr::Tuple(ast::ExprTuple { elts, .. }) = type_.as_ref() else {
            continue;
        };
        let [elt] = elts.as_slice() else {
            continue;
        };
        if elt.is_starred_expr() {
            continue;
        }
        let mut diagnostic = checker.report_diagnostic(
            RedundantTupleInExceptionHandler {
                name: checker.generator().expr(elt),
            },
            type_.range(),
        );

        let applicability = if checker.comment_ranges().intersects(type_.range()) {
            Applicability::Unsafe
        } else {
            Applicability::Safe
        };

        diagnostic.set_fix(Fix::applicable_edit(
            Edit::range_replacement(
                pad(
                    checker.generator().expr(elt),
                    type_.range(),
                    checker.locator(),
                ),
                type_.range(),
            ),
            applicability,
        ));
    }
}
