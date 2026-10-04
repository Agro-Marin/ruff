use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{callee_name, def_start, in_addon, is_test_path};

/// ## What it does
/// Checks for a route declared `auth="public"` or `auth="none"` with
/// `csrf=False`.
///
/// ## Why is this bad?
/// Such a route takes calls from machines without declaring who may make them.
/// `auth="receiver", receiver="<model>:<field or _method>"` resolves the subject
/// from the path and admits the request through its gate before the handler runs:
/// an unknown caller is refused, a flood is throttled and every verdict is
/// recorded. A page or a probe that must stay open takes a `noqa` saying why.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Security)]
pub(crate) struct ReceiverFailOpen {
    name: String,
    auth: String,
}

impl Violation for ReceiverFailOpen {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ReceiverFailOpen { name, auth } = self;
        format!(
            "`{name}`: `auth=\"{auth}\"` with `csrf=False` takes calls from machines without declaring who may make them"
        )
    }

    fn fix_title(&self) -> Option<String> {
        Some("Authenticate the caller with `auth=\"receiver\"`".to_string())
    }
}

/// The constant keywords of the first `route(...)` decorator: the last of a
/// repeated name wins, and a keyword whose value is not a constant is not read.
fn route_keyword<'a>(route: &'a ast::ExprCall, name: &str) -> Option<&'a Expr> {
    route
        .arguments
        .keywords
        .iter()
        .rev()
        .filter(|keyword| keyword.value.is_literal_expr())
        .find(|keyword| keyword.arg.as_ref().is_some_and(|arg| arg.as_str() == name))
        .map(|keyword| &keyword.value)
}

/// E8528
pub(crate) fn receiver_fail_open(checker: &Checker, function: &ast::StmtFunctionDef) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) {
        return;
    }
    let Some(route) =
        function
            .decorator_list
            .iter()
            .find_map(|decorator| match &decorator.expression {
                Expr::Call(call) if callee_name(&call.func) == "route" => Some(call),
                _ => None,
            })
    else {
        return;
    };
    let Some(Expr::StringLiteral(auth)) = route_keyword(route, "auth") else {
        return;
    };
    let auth = auth.value.to_str();
    if !matches!(auth, "public" | "none")
        || !matches!(
            route_keyword(route, "csrf"),
            Some(Expr::BooleanLiteral(ast::ExprBooleanLiteral {
                value: false,
                ..
            }))
        )
    {
        return;
    }
    let start = def_start(function, checker.source());
    checker.report_diagnostic(
        ReceiverFailOpen {
            name: function.name.to_string(),
            auth: auth.to_string(),
        },
        TextRange::new(start, function.name.end()),
    );
}
