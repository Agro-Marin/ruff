use ruff_python_ast::{self as ast, Arguments, Expr};

use crate::codes::Category;
use crate::{Edit, Fix, FixAvailability, Violation};
use crate::{
    checkers::ast::Checker, preview::is_fix_manual_list_comprehension_enabled,
    rules::perflint::helpers::statement_deletion_range,
};
use anyhow::{Result, anyhow};

use crate::rules::perflint::helpers::comment_strings_in_range;
use crate::rules::refurb::helpers::{IterLocation, parenthesize_loop_iter_if_necessary};
use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::helpers::any_over_expr;
use ruff_python_semantic::{Binding, analyze::typing::is_list};
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextRange};
/// ## What it does
/// Checks for `for` loops that can be replaced by a list comprehension.
///
/// ## Why is this bad?
/// When creating a transformed list from an existing list using a for-loop,
/// prefer a list comprehension. List comprehensions are more readable and
/// more performant.
///
/// Using the below as an example, the list comprehension is ~10% faster on
/// Python 3.11, and ~25% faster on Python 3.10.
///
/// Note that, as with all `perflint` rules, this is only intended as a
/// micro-optimization, and will have a negligible impact on performance in
/// most cases.
///
/// ## Example
/// ```python
/// original = list(range(10000))
/// filtered = []
/// for i in original:
///     if i % 2:
///         filtered.append(i)
/// ```
///
/// Use instead:
/// ```python
/// original = list(range(10000))
/// filtered = [x for x in original if x % 2]
/// ```
///
/// If you're appending to an existing list, use the `extend` method instead:
/// ```python
/// original = list(range(10000))
/// filtered.extend(x for x in original if x % 2)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.276", category = Category::Pedantic)]
pub(crate) struct ManualListComprehension {
    is_async: bool,
    comprehension_type: Option<ComprehensionType>,
}

impl Violation for ManualListComprehension {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let ManualListComprehension {
            is_async,
            comprehension_type,
        } = self;
        let message_str = match comprehension_type {
            Some(ComprehensionType::Extend) => {
                if *is_async {
                    "`list.extend` with an async comprehension"
                } else {
                    "`list.extend`"
                }
            }
            Some(ComprehensionType::ListComprehension) | None => {
                if *is_async {
                    "an async list comprehension"
                } else {
                    "a list comprehension"
                }
            }
        };
        format!("Use {message_str} to create a transformed list")
    }

    fn fix_title(&self) -> Option<String> {
        match self.comprehension_type? {
            ComprehensionType::ListComprehension => {
                Some("Replace for loop with list comprehension".to_string())
            }
            ComprehensionType::Extend => Some("Replace for loop with list.extend".to_string()),
        }
    }
}

/// PERF401
pub(crate) fn manual_list_comprehension(checker: &Checker, for_stmt: &ast::StmtFor) {
    let Expr::Name(ast::ExprName {
        id: for_stmt_target_id,
        ..
    }) = &*for_stmt.target
    else {
        return;
    };

    let (stmt, if_test) = match &*for_stmt.body {
        [
            ast::Stmt::If(ast::StmtIf {
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

    let ast::Stmt::Expr(ast::StmtExpr { value, .. }) = stmt else {
        return;
    };

    let Expr::Call(ast::ExprCall {
        func,
        arguments:
            Arguments {
                args,
                keywords,
                range: _,
                node_index: _,
            },
        range_start: _,
        node_index: _,
    }) = value.as_ref()
    else {
        return;
    };
    let call_range = value.range();

    if !keywords.is_empty() {
        return;
    }

    let [arg] = &**args else {
        return;
    };

    let Expr::Attribute(ast::ExprAttribute { attr, value, .. }) = &**func else {
        return;
    };

    if attr.as_str() != "append" {
        return;
    }

    let Some(list_name) = value.as_name_expr() else {
        return;
    };

    if !for_stmt.is_async {
        if if_test.is_none() {
            if arg
                .as_name_expr()
                .is_some_and(|arg| arg.id == *for_stmt_target_id)
            {
                return;
            }
        }
    }

    if any_over_expr(&for_stmt.iter, &Expr::is_named_expr) {
        return;
    }

    if any_over_expr(arg, |expr| {
        expr.as_name_expr()
            .is_some_and(|expr| expr.id == list_name.id)
    }) {
        return;
    }

    let Some(list_binding) = checker
        .semantic()
        .only_binding(list_name)
        .map(|id| checker.semantic().binding(id))
    else {
        return;
    };

    if !is_list(list_binding, checker.semantic()) {
        return;
    }

    if if_test.is_some_and(|test| {
        any_over_expr(test, |expr| {
            expr.as_name_expr()
                .is_some_and(|expr| expr.id == list_name.id)
        })
    }) {
        return;
    }

    let target_binding = checker
        .semantic()
        .bindings
        .iter()
        .find(|binding| for_stmt.target.range() == binding.range)
        .unwrap();
    if target_binding.is_global() || target_binding.is_nonlocal() {
        return;
    }
    if target_binding
        .references()
        .map(|reference| checker.semantic().reference(reference))
        .any(|other_reference| for_stmt.end() < other_reference.start())
    {
        return;
    }

    let list_binding_stmt = list_binding.statement(checker.semantic());
    let list_binding_value = list_binding_stmt.and_then(|binding_stmt| match binding_stmt {
        ast::Stmt::AnnAssign(assign) => assign.value.as_deref(),
        ast::Stmt::Assign(assign) => Some(&assign.value),
        _ => None,
    });

    let binding_is_empty_list =
        list_binding_value.is_some_and(|binding_value| match binding_value {
            Expr::List(list_expr) => list_expr.is_empty(),
            Expr::Call(call) => {
                checker
                    .semantic()
                    .resolve_builtin_symbol(&call.func)
                    .is_some_and(|name| name == "list")
                    && call.arguments.is_empty()
            }
            _ => false,
        });

    let assignment_in_same_statement = {
        list_binding.source.is_some_and(|binding_source| {
            let for_loop_parent = checker.semantic().current_statement_parent_id();
            let binding_parent = checker.semantic().parent_statement_id(binding_source);
            for_loop_parent == binding_parent
        })
    };

    let binding_has_one_target = list_binding_stmt.is_some_and(|binding_stmt| match binding_stmt {
        ast::Stmt::AnnAssign(_) => true,
        ast::Stmt::Assign(assign) => assign.targets.len() == 1,
        _ => false,
    });


    let binding_unused_between = assignment_in_same_statement
        && list_binding_stmt.is_some_and(|binding_stmt| {
            let from_assign_to_loop = TextRange::new(binding_stmt.end(), for_stmt.start());
            !list_binding
                .references()
                .map(|ref_id| checker.semantic().reference(ref_id).range())
                .any(|text_range| from_assign_to_loop.contains_range(text_range))
        });

    let comprehension_type = if binding_is_empty_list
        && assignment_in_same_statement
        && binding_has_one_target
        && binding_unused_between
    {
        ComprehensionType::ListComprehension
    } else {
        ComprehensionType::Extend
    };

    let mut diagnostic = checker.report_diagnostic(
        ManualListComprehension {
            is_async: for_stmt.is_async,
            comprehension_type: Some(comprehension_type),
        },
        call_range,
    );

    if is_fix_manual_list_comprehension_enabled(checker.settings()) {
        diagnostic.try_set_fix(|| {
            convert_to_list_extend(
                comprehension_type,
                list_binding,
                for_stmt,
                if_test.map(std::convert::AsRef::as_ref),
                arg,
                checker,
            )
        });
    }
}

fn convert_to_list_extend(
    fix_type: ComprehensionType,
    binding: &Binding,
    for_stmt: &ast::StmtFor,
    if_test: Option<&Expr>,
    to_append: &Expr,
    checker: &Checker,
) -> Result<Fix> {
    let semantic = checker.semantic();
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

    let (for_iter_str, for_iter_range) =
        parenthesize_loop_iter_if_necessary(for_stmt, checker, IterLocation::Comprehension);

    let for_type = if for_stmt.is_async {
        "async for"
    } else {
        "for"
    };
    let target_str = locator.slice(for_stmt.target.range());
    let elt_str = locator.slice(to_append);
    let generator_str = if to_append
        .as_generator_expr()
        .is_some_and(|generator| !generator.parenthesized)
    {
        format!("({elt_str}) {for_type} {target_str} in {for_iter_str}{if_str}")
    } else {
        format!("{elt_str} {for_type} {target_str} in {for_iter_str}{if_str}")
    };

    let variable_name = locator.slice(binding);
    let mut ranges_to_ignore = vec![to_append.range(), for_iter_range, for_stmt.target.range()];
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

    match fix_type {
        ComprehensionType::Extend => {
            let generator_str = if for_stmt.is_async {
                format!("[{generator_str}]")
            } else {
                generator_str
            };

            let comprehension_body = format!("{variable_name}.extend({generator_str})");

            let indentation = if for_loop_inline_comments.is_empty() {
                String::new()
            } else {
                format!("{newline}{indent}")
            };
            let text_to_replace = format!(
                "{}{indentation}{comprehension_body}",
                for_loop_inline_comments.join(&indentation)
            );
            Ok(Fix::unsafe_edit(Edit::range_replacement(
                text_to_replace,
                for_stmt.range,
            )))
        }
        ComprehensionType::ListComprehension => {
            let binding_stmt = binding.statement(semantic);
            let binding_stmt_range = binding_stmt
                .and_then(|stmt| match stmt {
                    ast::Stmt::AnnAssign(assign) => Some(assign.range),
                    ast::Stmt::Assign(assign) => Some(assign.range),
                    _ => None,
                })
                .ok_or(anyhow!(
                    "Binding must have a statement to convert into a list comprehension"
                ))?;

            let (binding_stmt_deletion_range, binding_is_multiple_stmts) =
                statement_deletion_range(checker, binding_stmt_range);

            let annotations = match binding_stmt.and_then(|stmt| stmt.as_ann_assign_stmt()) {
                Some(assign) => format!(": {}", locator.slice(assign.annotation.range())),
                None => String::new(),
            };

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
                format!("{leading_comments}{variable_name}{annotations} = [{generator_str}]");

            Ok(Fix::unsafe_edits(
                Edit::range_deletion(binding_stmt_deletion_range),
                [Edit::range_replacement(comprehension_body, for_stmt.range)],
            ))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComprehensionType {
    Extend,
    ListComprehension,
}
