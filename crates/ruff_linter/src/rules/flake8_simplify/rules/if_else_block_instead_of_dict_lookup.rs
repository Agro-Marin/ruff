use rustc_hash::FxHashSet;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::comparable::ComparableLiteral;
use ruff_python_ast::helpers::contains_effect;
use ruff_python_ast::{self as ast, CmpOp, ElifElseClause, Expr, Stmt};
use ruff_python_semantic::analyze::typing::{is_sys_version_block, is_type_checking_block};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;

/// ## What it does
/// Checks for three or more consecutive if-statements with direct returns
///
/// ## Why is this bad?
/// These can be simplified by using a dictionary
///
/// ## Example
/// ```python
/// def find_phrase(x):
///     if x == 1:
///         return "Hello"
///     elif x == 2:
///         return "Goodbye"
///     elif x == 3:
///         return "Good morning"
///     else:
///         return "Goodnight"
/// ```
///
/// Use instead:
/// ```python
/// def find_phrase(x):
///     phrases = {1: "Hello", 2: "Goodbye", 3: "Good morning"}
///     return phrases.get(x, "Goodnight")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.250", category = Category::Pedantic)]
pub(crate) struct IfElseBlockInsteadOfDictLookup;

impl Violation for IfElseBlockInsteadOfDictLookup {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Use a dictionary instead of consecutive `if` statements".to_string()
    }
}
/// SIM116
pub(crate) fn if_else_block_instead_of_dict_lookup(checker: &Checker, stmt_if: &ast::StmtIf) {
    let ast::StmtIf {
        body,
        test,
        elif_else_clauses,
        ..
    } = stmt_if;

    let Expr::Compare(compare) = test.as_ref() else {
        return;
    };
    let Some((Expr::Name(ast::ExprName { id: target, .. }), CmpOp::Eq, expr)) = compare.as_single()
    else {
        return;
    };
    let Some(literal_expr) = expr.as_literal_expr() else {
        return;
    };
    let [
        Stmt::Return(ast::StmtReturn {
            value,
            range: _,
            node_index: _,
        }),
    ] = body.as_slice()
    else {
        return;
    };

    if value.as_ref().is_some_and(|value| {
        contains_effect(value, |id| checker.semantic().has_builtin_binding(id))
    }) {
        return;
    }

    if is_sys_version_block(stmt_if, checker.semantic()) {
        return;
    }

    if is_type_checking_block(stmt_if, checker.semantic()) {
        return;
    }

    let mut literals: FxHashSet<ComparableLiteral> = FxHashSet::default();
    literals.insert(literal_expr.into());

    for clause in elif_else_clauses {
        let ElifElseClause { test, body, .. } = clause;
        let [
            Stmt::Return(ast::StmtReturn {
                value,
                range: _,
                node_index: _,
            }),
        ] = body.as_slice()
        else {
            return;
        };

        match test.as_ref() {
            None => {
                let [
                    Stmt::Return(ast::StmtReturn {
                        value,
                        range: _,
                        node_index: _,
                    }),
                ] = body.as_slice()
                else {
                    return;
                };
                if value.as_ref().is_some_and(|value| {
                    contains_effect(value, |id| checker.semantic().has_builtin_binding(id))
                }) {
                    return;
                }
            }
            Some(Expr::Compare(compare)) => {
                let Some((Expr::Name(ast::ExprName { id, .. }), CmpOp::Eq, expr)) =
                    compare.as_single()
                else {
                    return;
                };
                if id != target {
                    return;
                }
                let Some(literal_expr) = expr.as_literal_expr() else {
                    return;
                };

                if value.as_ref().is_some_and(|value| {
                    contains_effect(value, |id| checker.semantic().has_builtin_binding(id))
                }) {
                    return;
                }

                literals.insert(literal_expr.into());
            }
            _ => {
                return;
            }
        }
    }

    if literals.len() < 3 {
        return;
    }

    checker.report_diagnostic(IfElseBlockInsteadOfDictLookup, stmt_if.range());
}
