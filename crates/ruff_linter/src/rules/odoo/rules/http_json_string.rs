use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{in_addon, is_test_path, returns, route_decorator};

/// ## What it does
/// Checks for a `type="http"` route that returns `json.dumps(...)`.
///
/// ## Why is this bad?
/// A `type="http"` route answering JSON must say so: a bare `json.dumps` string
/// goes out as `text/html`, and the client's `post()`/`get()` helpers refuse to
/// parse it.
///
/// A route without `type=` is an HTTP route.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct HttpJsonString;

impl Violation for HttpJsonString {
    #[derive_message_formats]
    fn message(&self) -> String {
        "A `type=\"http\"` route returns `json.dumps()` bare, so the JSON goes out as `text/html`"
            .to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Return `request.prepare_json_response(payload)`".to_string())
    }
}

/// The first `type=` keyword decides; a route without one is an HTTP route.
fn is_http_route(route: &ast::ExprCall) -> bool {
    route
        .arguments
        .keywords
        .iter()
        .find(|keyword| keyword.arg.as_ref().is_some_and(|arg| arg.as_str() == "type"))
        .is_none_or(|keyword| matches!(&keyword.value, Expr::StringLiteral(kind) if kind.value.to_str() == "http"))
}

fn is_json_dumps(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    match &*call.func {
        Expr::Attribute(attribute) => {
            attribute.attr.as_str() == "dumps"
                && matches!(&*attribute.value, Expr::Name(name) if name.id.as_str() == "json")
        }
        Expr::Name(name) => matches!(name.id.as_str(), "dumps" | "json_dumps"),
        _ => false,
    }
}

/// E8515
pub(crate) fn http_json_string(checker: &Checker, function: &ast::StmtFunctionDef) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) {
        return;
    }
    if !route_decorator(function).is_some_and(is_http_route) {
        return;
    }
    for ret in returns(function) {
        if ret.value.as_deref().is_some_and(is_json_dumps) {
            checker.report_diagnostic(HttpJsonString, ret.range());
        }
    }
}
