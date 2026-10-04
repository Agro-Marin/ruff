use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{callee_name, def_start, in_addon, is_test_path};

/// ## What it does
/// Checks for a route a program calls (`type="json2"`, or `auth="bearer"` or
/// `auth="receiver"`) without `typed=True`, or typed with a parameter that
/// carries no annotation.
///
/// ## Why is this bad?
/// The parameters of such a route are a contract. Declared, each one is
/// coerced and refused at the door instead of reaching the handler as a
/// string, and the `OpenAPI` document states the contract.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct RouteUntyped {
    name: String,
    reached_as: String,
    undeclared: Option<String>,
}

impl Violation for RouteUntyped {
    #[derive_message_formats]
    fn message(&self) -> String {
        let RouteUntyped {
            name,
            reached_as,
            undeclared,
        } = self;
        match undeclared {
            None => format!(
                "`{name}`: {reached_as} takes its parameters from a program without declaring them"
            ),
            Some(undeclared) => {
                format!("`{name}`: {reached_as} is typed but {undeclared} carries no annotation")
            }
        }
    }

    fn fix_title(&self) -> Option<String> {
        let RouteUntyped { undeclared, .. } = self;
        Some(match undeclared {
            None => "Declare `typed=True` and annotate every parameter".to_string(),
            Some(_) => "Annotate every parameter".to_string(),
        })
    }
}

/// A constant keyword of the decorator: the last of a repeated name wins, and a
/// keyword whose value is not a constant is not read.
fn constant<'a>(route: &'a ast::ExprCall, name: &str) -> Option<&'a Expr> {
    route
        .arguments
        .keywords
        .iter()
        .rev()
        .filter(|keyword| keyword.value.is_literal_expr())
        .find(|keyword| keyword.arg.as_ref().is_some_and(|arg| arg.as_str() == name))
        .map(|keyword| &keyword.value)
}

fn string<'a>(route: &'a ast::ExprCall, name: &str) -> Option<&'a str> {
    match constant(route, name) {
        Some(Expr::StringLiteral(value)) => Some(value.value.to_str()),
        _ => None,
    }
}

/// E8533
pub(crate) fn route_untyped(checker: &Checker, function: &ast::StmtFunctionDef) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) {
        return;
    }
    for decorator in &function.decorator_list {
        let Expr::Call(route) = &decorator.expression else {
            continue;
        };
        if callee_name(&route.func) != "route" {
            continue;
        }
        let machine_type = string(route, "type") == Some("json2");
        let auth = constant(route, "auth");
        let machine_auth = matches!(string(route, "auth"), Some("bearer" | "receiver"));
        if !machine_type && !machine_auth {
            continue;
        }
        let reached_as = if machine_type {
            "`type=\"json2\"`".to_string()
        } else {
            format!(
                "`auth={}`",
                auth.map_or("None", |auth| checker.locator().slice(auth))
            )
        };
        let undeclared = if matches!(
            constant(route, "typed"),
            Some(Expr::BooleanLiteral(ast::ExprBooleanLiteral {
                value: true,
                ..
            }))
        ) {
            let parameters = &function.parameters;
            let undeclared: Vec<&str> = parameters
                .posonlyargs
                .iter()
                .chain(&parameters.args)
                .chain(&parameters.kwonlyargs)
                .skip(1)
                .filter(|parameter| parameter.annotation().is_none())
                .map(|parameter| parameter.name().as_str())
                .collect();
            if undeclared.is_empty() {
                continue;
            }
            Some(undeclared.join(", "))
        } else {
            None
        };
        let start = def_start(function, checker.source());
        checker.report_diagnostic(
            RouteUntyped {
                name: function.name.to_string(),
                reached_as,
                undeclared,
            },
            TextRange::new(start, function.name.end()),
        );
    }
}
