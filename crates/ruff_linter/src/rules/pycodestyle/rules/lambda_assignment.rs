use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::token::parenthesized_range;
use ruff_python_ast::{
    self as ast, DecoratorList, Expr, ExprEllipsisLiteral, ExprLambda, Identifier, Parameter,
    ParameterWithDefault, Parameters, Stmt,
};
use ruff_python_semantic::SemanticModel;
use ruff_python_trivia::{has_leading_content, has_trailing_content, leading_indentation};
use ruff_source_file::UniversalNewlines;
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::{Applicability, Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for lambda expressions which are assigned to a variable.
///
/// ## Why is this bad?
/// Per PEP 8, you should "Always use a def statement instead of an assignment
/// statement that binds a lambda expression directly to an identifier."
///
/// Using a `def` statement leads to better tracebacks, and the assignment
/// itself negates the primary benefit of using a `lambda` expression (i.e.,
/// that it can be embedded inside another expression).
///
/// ## Example
/// ```python
/// f = lambda x: 2 * x
/// ```
///
/// Use instead:
/// ```python
/// def f(x):
///     return 2 * x
/// ```
///
/// ## Fix safety
/// This fix is marked as unsafe because converting a lambda assignment into a
/// function definition changes observable properties of the callable.
///
/// In particular, a lambda function has the name `"<lambda>"`, while the
/// generated function uses the name of the assigned variable. Code that relies
/// on function metadata, such as logging, registration, or introspection, may
/// therefore behave differently after the fix.
///
/// [PEP 8]: https://peps.python.org/pep-0008/#programming-recommendations
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.28", category = Category::Pedantic)]
pub(crate) struct LambdaAssignment {
    name: String,
}

impl Violation for LambdaAssignment {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Do not assign a `lambda` expression, use a `def`".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        let LambdaAssignment { name } = self;
        Some(format!("Rewrite `{name}` as a `def`"))
    }
}

/// E731
pub(crate) fn lambda_assignment(
    checker: &Checker,
    target: &Expr,
    value: &Expr,
    annotation: Option<&Expr>,
    stmt: &Stmt,
) {
    let Expr::Name(ast::ExprName { id, .. }) = target else {
        return;
    };

    let Expr::Lambda(lambda) = value else {
        return;
    };

    if annotation.is_some() && checker.semantic().current_scope().kind.is_class() {
        return;
    }

    let mut diagnostic = checker.report_diagnostic(
        LambdaAssignment {
            name: id.to_string(),
        },
        stmt.range(),
    );

    if !has_leading_content(stmt.start(), checker.source())
        && !has_trailing_content(stmt.end(), checker.source())
    {
        let first_line = checker.locator().line_str(stmt.start());
        let indentation = leading_indentation(first_line);
        let mut indented = String::new();
        for (idx, line) in function(id, lambda, annotation, stmt, checker)
            .universal_newlines()
            .enumerate()
        {
            if idx == 0 {
                indented.push_str(&line);
            } else {
                indented.push_str(checker.stylist().line_ending().as_str());
                indented.push_str(indentation);
                indented.push_str(&line);
            }
        }

        let applicability = if checker
            .semantic()
            .current_scope()
            .get_all(id)
            .any(|binding_id| checker.semantic().binding(binding_id).kind.is_annotation())
        {
            Applicability::DisplayOnly
        } else {
            Applicability::Unsafe
        };

        diagnostic.set_fix(Fix::applicable_edit(
            Edit::range_replacement(indented, stmt.range()),
            applicability,
        ));
    }
}

/// Extract the argument types and return type from a `Callable` annotation.
/// The `Callable` import can be from either `collections.abc` or `typing`.
/// If an ellipsis is used for the argument types, an empty list is returned.
/// The returned values are cloned, so they can be used as-is.
fn extract_types(annotation: &Expr, semantic: &SemanticModel) -> Option<(Vec<Expr>, Expr)> {
    let Expr::Subscript(ast::ExprSubscript { value, slice, .. }) = &annotation else {
        return None;
    };
    let Expr::Tuple(ast::ExprTuple { elts, .. }) = slice.as_ref() else {
        return None;
    };
    let [param_types, return_type] = elts.as_slice() else {
        return None;
    };

    if !semantic
        .resolve_qualified_name(value)
        .is_some_and(|qualified_name| {
            matches!(
                qualified_name.segments(),
                ["collections", "abc", "Callable"]
            ) || semantic.match_typing_qualified_name(&qualified_name, "Callable")
        })
    {
        return None;
    }

    let params = match param_types {
        Expr::List(ast::ExprList { elts, .. }) => elts.clone(),
        Expr::EllipsisLiteral(_) => vec![],
        _ => vec![],
    };

    let return_type = return_type.clone();

    Some((params, return_type))
}

/// Generate a function definition from a `lambda` expression.
fn function(
    name: &str,
    lambda: &ExprLambda,
    annotation: Option<&Expr>,
    stmt: &Stmt,
    checker: &Checker,
) -> String {
    let body = Stmt::Return(ast::StmtReturn {
        value: Some(Box::new(Expr::EllipsisLiteral(
            ExprEllipsisLiteral::default(),
        ))),
        range: TextRange::default(),
        node_index: ruff_python_ast::AtomicNodeIndex::NONE,
    });
    let parameters = lambda.parameters.as_deref().cloned().unwrap_or_default();
    if let Some(annotation) = annotation {
        if let Some((arg_types, return_type)) = extract_types(annotation, checker.semantic()) {
            let new_posonlyargs = parameters
                .posonlyargs
                .iter()
                .enumerate()
                .map(|(idx, parameter)| ParameterWithDefault {
                    parameter: Parameter {
                        annotation: arg_types
                            .get(idx)
                            .map(|arg_type| Box::new(arg_type.clone())),
                        ..parameter.parameter.clone()
                    },
                    ..parameter.clone()
                })
                .collect::<ast::ParameterWithDefaults>();
            let new_args = parameters
                .args
                .iter()
                .enumerate()
                .map(|(idx, parameter)| ParameterWithDefault {
                    parameter: Parameter {
                        annotation: arg_types
                            .get(idx + new_posonlyargs.len())
                            .map(|arg_type| Box::new(arg_type.clone())),
                        ..parameter.parameter.clone()
                    },
                    ..parameter.clone()
                })
                .collect::<ast::ParameterWithDefaults>();
            let func = Stmt::FunctionDef(ast::StmtFunctionDef {
                is_async: false,
                name: Identifier::new(name.to_string(), TextRange::default()),
                parameters: Box::new(Parameters {
                    posonlyargs: new_posonlyargs,
                    args: new_args,
                    ..parameters
                }),
                body: ast::Suite::from([body]),
                decorator_list: ast::DecoratorList::new(),
                returns: Some(Box::new(return_type)),
                type_params: None,
                range: TextRange::default(),
                node_index: ruff_python_ast::AtomicNodeIndex::NONE,
            });
            let generated = checker.generator().stmt(&func);

            return replace_trailing_ellipsis_with_original_expr(generated, lambda, stmt, checker);
        }
    }
    let function = Stmt::FunctionDef(ast::StmtFunctionDef {
        is_async: false,
        name: Identifier::new(name.to_string(), TextRange::default()),
        parameters: Box::new(parameters),
        body: ast::Suite::from([body]),
        decorator_list: DecoratorList::new(),
        returns: None,
        type_params: None,
        range: TextRange::default(),
        node_index: ruff_python_ast::AtomicNodeIndex::NONE,
    });
    let generated = checker.generator().stmt(&function);

    replace_trailing_ellipsis_with_original_expr(generated, lambda, stmt, checker)
}

fn replace_trailing_ellipsis_with_original_expr(
    mut generated: String,
    lambda: &ExprLambda,
    stmt: &Stmt,
    checker: &Checker,
) -> String {
    let original_expr_range =
        parenthesized_range((&lambda.body).into(), lambda.into(), checker.tokens())
            .unwrap_or(lambda.body.range());

    let original_expr_in_source =
        if parenthesized_range(lambda.into(), stmt.into(), checker.tokens()).is_some() {
            format!("({})", checker.locator().slice(original_expr_range))
        } else {
            checker.locator().slice(original_expr_range).to_string()
        };

    let placeholder_ellipsis_start = generated.rfind("...").unwrap();
    let placeholder_ellipsis_end = placeholder_ellipsis_start + "...".len();

    generated.replace_range(
        placeholder_ellipsis_start..placeholder_ellipsis_end,
        &original_expr_in_source,
    );
    generated
}
