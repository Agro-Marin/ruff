use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{callee_name, in_addon, is_test_path, returns, route_decorator};

/// ## What it does
/// Checks for a route that returns an HTTP exception, such as
/// `return NotFound()`.
///
/// ## Why is this bad?
/// A returned HTTP exception is raised for the route anyway, with a warning on
/// every hit; before that, a jsonrpc or json2 route answered it as a successful
/// result.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct HttpExceptionReturned;

impl Violation for HttpExceptionReturned {
    #[derive_message_formats]
    fn message(&self) -> String {
        "A route returns an HTTP exception it should raise".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Raise it: `raise request.prepare_not_found_error()`, `raise NotFound()`".to_string())
    }
}

/// werkzeug's `default_exceptions`, and the fork's own HTTP errors.
const HTTP_EXCEPTIONS: &[&str] = &[
    "BadGateway",
    "BadRequest",
    "Conflict",
    "ExpectationFailed",
    "FailedDependency",
    "Forbidden",
    "GatewayTimeout",
    "Gone",
    "HTTPVersionNotSupported",
    "ImATeapot",
    "InternalServerError",
    "LengthRequired",
    "Locked",
    "MethodNotAllowed",
    "MisdirectedRequest",
    "NotAcceptable",
    "NotFound",
    "NotImplemented",
    "PreconditionFailed",
    "PreconditionRequired",
    "RequestEntityTooLarge",
    "RequestHeaderFieldsTooLarge",
    "RequestTimeout",
    "RequestURITooLarge",
    "RequestedRangeNotSatisfiable",
    "ServiceUnavailable",
    "TooManyRequests",
    "Unauthorized",
    "UnavailableForLegalReasons",
    "UnprocessableEntity",
    "UnsupportedMediaType",
    "HTTPException",
    "ParameterError",
    "prepare_not_found_error",
];

fn is_http_exception(expr: &Expr) -> bool {
    matches!(expr, Expr::Call(call) if HTTP_EXCEPTIONS.contains(&callee_name(&call.func)))
}

/// E8541
pub(crate) fn http_exception_returned(checker: &Checker, function: &ast::StmtFunctionDef) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) {
        return;
    }
    if route_decorator(function).is_none() {
        return;
    }
    for ret in returns(function) {
        if ret.value.as_deref().is_some_and(is_http_exception) {
            checker.report_diagnostic(HttpExceptionReturned, ret.range());
        }
    }
}
