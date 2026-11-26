use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{
    self as ast, Expr, Stmt, comparable::ComparableExpr, helpers::any_over_expr,
};
use ruff_python_semantic::{Binding, analyze::typing::is_dict};
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::preview::is_fix_manual_dict_comprehension_enabled;
use crate::rules::perflint::helpers::{comment_strings_in_range, statement_deletion_range};
use crate::rules::refurb::helpers::{IterLocation, parenthesize_loop_iter_if_necessary};
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for `for` loops that can be replaced by a dictionary comprehension.
///
/// ## Why is this bad?
/// When creating or extending a dictionary in a for-loop, prefer a dictionary
/// comprehension. Comprehensions are more readable and more performant.
///
/// For example, when comparing `{x: x for x in list(range(1000))}` to the `for`
/// loop version, the comprehension is ~10% faster on Python 3.11.
///
/// Note that, as with all `perflint` rules, this is only intended as a
/// micro-optimization, and will have a negligible impact on performance in
/// most cases.
///
/// ## Example
/// ```python
/// pairs = (("a", 1), ("b", 2))
/// result = {}
/// for x, y in pairs:
///     if y % 2:
///         result[x] = y
/// ```
///
/// Use instead:
/// ```python
/// pairs = (("a", 1), ("b", 2))
/// result = {x: y for x, y in pairs if y % 2}
/// ```
///
/// If you're appending to an existing dictionary, use the `update` method instead:
/// ```python
/// pairs = (("a", 1), ("b", 2))
/// result.update({x: y for x, y in pairs if y % 2})
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.5.0", category = Category::Pedantic)]
pub(crate) struct ManualDictComprehension {
    fix_type: DictComprehensionType,
    is_async: bool,
}

impl Violation for ManualDictComprehension {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let modifier = if self.is_async { "an async" } else { "a" };

        match self.fix_type {
            DictComprehensionType::Comprehension => {
                format!("Use a dictionary comprehension instead of {modifier} for-loop")
            }
            DictComprehensionType::Update => {
                format!("Use `dict.update` instead of {modifier} for-loop")
            }
        }
    }
    fn fix_title(&self) -> Option<String> {
        let modifier = if self.is_async { "async " } else { "" };
        match self.fix_type {
            DictComprehensionType::Comprehension => Some(format!(
                "Replace {modifier}for loop with dict comprehension"
            )),
            DictComprehensionType::Update => {
                Some(format!("Replace {modifier}for loop with `dict.update`"))
            }
        }
    }
}

/// PERF403
pub(crate) fn manual_dict_comprehension(checker: &Checker, for_stmt: &ast::StmtFor) {
    let ast::StmtFor {
        body, target, iter, ..
    } = for_stmt;

    let body = body.as_slice();
    let target = target.as_ref();
    let (stmt, if_test) = match body {
        [
            Stmt::If(ast::StmtIf {
                body,
                elif_else_clauses,
                test,
                ..
            }),
        ] => {
            if !elif_else_clauses.is_empty() {
                return;
            }
            let [stmt] = body.as_slice() else {
                return;
            };
            (stmt, Some(test))
        }
        [stmt] => (stmt, None),
        _ => return,
    };

    let Stmt::Assign(ast::StmtAssign {
        targets,
        value,
        range,
        node_index: _,
    }) = stmt
    else {
        return;
    };

    let [
        Expr::Subscript(ast::ExprSubscript {
            value: subscript_value,
            slice: key,
            ..
        }),
    ] = targets.as_slice()
    else {
        return;
    };

    if any_over_expr(iter, &Expr::is_named_expr) {
        return;
    }

    if has_post_loop_references(checker, target, for_stmt.end()) {
        return;
    }

    match target {
        Expr::Tuple(tuple) => {
            if !tuple
                .iter()
                .any(|element| ComparableExpr::from(key) == ComparableExpr::from(element))
            {
                return;
            }
            if !tuple
                .iter()
                .any(|element| ComparableExpr::from(value) == ComparableExpr::from(element))
            {
                return;
            }
        }
        Expr::Name(_) => {
            if ComparableExpr::from(key) != ComparableExpr::from(target) {
                return;
            }
            if ComparableExpr::from(value) != ComparableExpr::from(target) {
                return;
            }
        }
        _ => return,
    }

    let Expr::Name(name) = &**subscript_value else {
        return;
    };
    let Some(binding) = checker
        .semantic()
        .only_binding(name)
        .map(|id| checker.semantic().binding(id))
    else {
        return;
    };
    if !is_dict(binding, checker.semantic()) {
        return;
    }

    if if_test.is_some_and(|test| {
        any_over_expr(test, |expr| {
            ComparableExpr::from(expr) == ComparableExpr::from(name)
        })
    }) {
        return;
    }

    if is_fix_manual_dict_comprehension_enabled(checker.settings()) {
        let binding_stmt = binding.statement(checker.semantic());
        let binding_value = binding_stmt.and_then(|binding_stmt| match binding_stmt {
            ast::Stmt::AnnAssign(assign) => assign.value.as_deref(),
            ast::Stmt::Assign(assign) => Some(&assign.value),
            _ => None,
        });

        let binding_is_empty_dict =
            binding_value.is_some_and(|binding_value| match binding_value {
                Expr::Dict(dict_expr) => dict_expr.is_empty(),
                Expr::Call(call) => {
                    checker
                        .semantic()
                        .resolve_builtin_symbol(&call.func)
                        .is_some_and(|name| name == "dict")
                        && call.arguments.is_empty()
                }
                _ => false,
            });

        let assignment_in_same_statement = binding.source.is_some_and(|binding_source| {
            let for_loop_parent = checker.semantic().current_statement_parent_id();
            let binding_parent = checker.semantic().parent_statement_id(binding_source);
            for_loop_parent == binding_parent
        });
        let binding_has_one_target = binding_stmt.is_some_and(|binding_stmt| match binding_stmt {
            ast::Stmt::AnnAssign(_) => true,
            ast::Stmt::Assign(assign) => assign.targets.len() == 1,
            _ => false,
        });

        let binding_unused_between = assignment_in_same_statement
            && binding_stmt.is_some_and(|binding_stmt| {
                let from_assign_to_loop = TextRange::new(binding_stmt.end(), for_stmt.start());
                !binding
                    .references()
                    .map(|ref_id| checker.semantic().reference(ref_id).range())
                    .any(|text_range| from_assign_to_loop.contains_range(text_range))
            });
        let fix_type = if binding_is_empty_dict
            && assignment_in_same_statement
            && binding_has_one_target
            && binding_unused_between
        {
            DictComprehensionType::Comprehension
        } else {
            DictComprehensionType::Update
        };

        let mut diagnostic = checker.report_diagnostic(
            ManualDictComprehension {
                fix_type,
                is_async: for_stmt.is_async,
            },
            *range,
        );
        diagnostic.try_set_optional_fix(|| {
            Ok(convert_to_dict_comprehension(
                fix_type,
                binding,
                for_stmt,
                if_test.map(std::convert::AsRef::as_ref),
                key.as_ref(),
                value.as_ref(),
                checker,
            ))
        });
    } else {
        checker.report_diagnostic(
            ManualDictComprehension {
                fix_type: DictComprehensionType::Comprehension,
                is_async: for_stmt.is_async,
            },
            *range,
        );
    }
}

fn convert_to_dict_comprehension(
    fix_type: DictComprehensionType,
    binding: &Binding,
    for_stmt: &ast::StmtFor,
    if_test: Option<&ast::Expr>,
    key: &Expr,
    value: &Expr,
    checker: &Checker,
) -> Option<Fix> {
    let locator = checker.locator();

    let if_str = match if_test {
        Some(test) => {
            match test {
                Expr::Named(_) | Expr::If(_) | Expr::Lambda(_) => {
                    format!(" if ({})", locator.slice(test.range()))
                }
                _ => format!(" if {}", locator.slice(test.range())),
            }
        }
        None => String::new(),
    };

    let (iter_str, iter_range) =
        parenthesize_loop_iter_if_necessary(for_stmt, checker, IterLocation::Comprehension);

    let target_str = locator.slice(for_stmt.target.range());
    let for_type = if for_stmt.is_async {
        "async for"
    } else {
        "for"
    };
    let key_str = if let Expr::Tuple(ast::ExprTuple {
        elts,
        parenthesized,
        ..
    }) = key
    {
        if elts.len() != 1 {
            return None;
        }
        if *parenthesized {
            locator.slice(key).to_string()
        } else {
            format!("({})", locator.slice(key))
        }
    } else {
        locator.slice(key).to_string()
    };

    let value_str = if let Expr::Tuple(ast::ExprTuple {
        parenthesized: false,
        ..
    }) = value
    {
        format!("({})", locator.slice(value))
    } else {
        locator.slice(value).to_string()
    };

    let comprehension_str =
        format!("{{{key_str}: {value_str} {for_type} {target_str} in {iter_str}{if_str}}}");

    let mut ranges_to_ignore = vec![
        key.range(),
        value.range(),
        iter_range,
        for_stmt.target.range(),
    ];
    if let Some(test) = if_test {
        ranges_to_ignore.push(test.range());
    }
    let for_loop_inline_comments =
        comment_strings_in_range(checker, for_stmt.range, &ranges_to_ignore);

    let newline = checker.stylist().line_ending().as_str();

    let indent = locator.slice(TextRange::new(
        locator.line_start(for_stmt.range.start()),
        for_stmt.range.start(),
    ));

    let variable_name = locator.slice(binding);
    match fix_type {
        DictComprehensionType::Update => {
            let indentation = if for_loop_inline_comments.is_empty() {
                String::new()
            } else {
                format!("{newline}{indent}")
            };

            let comprehension_body = format!("{variable_name}.update({comprehension_str})");

            let text_to_replace = format!(
                "{}{indentation}{comprehension_body}",
                for_loop_inline_comments.join(&indentation)
            );

            Some(Fix::unsafe_edit(Edit::range_replacement(
                text_to_replace,
                for_stmt.range,
            )))
        }
        DictComprehensionType::Comprehension => {
            let binding_stmt = binding.statement(checker.semantic());
            debug_assert!(
                binding_stmt.is_some(),
                "must be passed a binding with a statement"
            );
            let binding_stmt = binding_stmt?;

            let binding_stmt_range = binding_stmt.range();

            let annotations = match binding_stmt.as_ann_assign_stmt() {
                Some(assign) => format!(": {}", locator.slice(assign.annotation.range())),
                None => String::new(),
            };

            let (binding_stmt_deletion_range, binding_is_multiple_stmts) =
                statement_deletion_range(checker, binding_stmt_range);

            let comments_to_move = if binding_is_multiple_stmts {
                for_loop_inline_comments
            } else {
                let mut new_comments =
                    comment_strings_in_range(checker, binding_stmt_deletion_range, &[]);
                new_comments.extend(for_loop_inline_comments);
                new_comments
            };

            let indentation = if comments_to_move.is_empty() {
                String::new()
            } else {
                format!("{newline}{indent}")
            };
            let leading_comments = format!("{}{indentation}", comments_to_move.join(&indentation));

            let comprehension_body =
                format!("{leading_comments}{variable_name}{annotations} = {comprehension_str}");
            Some(Fix::unsafe_edits(
                Edit::range_deletion(binding_stmt_deletion_range),
                [Edit::range_replacement(comprehension_body, for_stmt.range)],
            ))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DictComprehensionType {
    Update,
    Comprehension,
}

fn has_post_loop_references(checker: &Checker, expr: &Expr, loop_end: TextSize) -> bool {
    any_over_expr(expr, |expr| match expr {
        Expr::Tuple(ast::ExprTuple { elts, .. }) => elts
            .iter()
            .any(|expr| has_post_loop_references(checker, expr, loop_end)),
        Expr::Name(name) => {
            let Some(target_binding) = checker
                .semantic()
                .bindings
                .iter()
                .find(|binding| name.range() == binding.range)
            else {
                return true;
            };

            target_binding
                .references()
                .map(|reference| checker.semantic().reference(reference))
                .any(|other_reference| other_reference.start() > loop_end)
        }
        _ => false,
    })
}
