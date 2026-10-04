use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{Expr, ExprContext, Stmt, Suite};
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::is_test_path;

/// ## What it does
/// Checks for a subscript of a WSGI environ by a key WSGI does not guarantee,
/// such as `environ["REMOTE_ADDR"]` or `environ["HTTP_HOST"]`.
///
/// ## Why is this bad?
/// WSGI guarantees only the request line, the server and the `wsgi.*` keys
/// (and this fork guarantees `REQUEST_URI`), so a subscript of any other key
/// raises `KeyError` under a server that omits it. `.get()` or the request's
/// own accessor (`request.httprequest.remote_addr`, `.headers`) reads it safely.
///
/// `os.environ`, and a bare `environ` imported from `os` at the top of the
/// module, are the process environment and are not reported.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct WsgiEnvironOptionalKey {
    key: String,
}

impl Violation for WsgiEnvironOptionalKey {
    #[derive_message_formats]
    fn message(&self) -> String {
        let WsgiEnvironOptionalKey { key } = self;
        format!("`environ[\"{key}\"]` is not guaranteed by WSGI")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Read it with `.get()` or the request's own accessor".to_string())
    }
}

const GUARANTEED_KEYS: &[&str] = &[
    "REQUEST_METHOD",
    "SCRIPT_NAME",
    "PATH_INFO",
    "SERVER_NAME",
    "SERVER_PORT",
    "SERVER_PROTOCOL",
    "REQUEST_URI",
    "wsgi.version",
    "wsgi.url_scheme",
    "wsgi.input",
    "wsgi.errors",
    "wsgi.multithread",
    "wsgi.multiprocess",
    "wsgi.run_once",
];

/// `from os import environ` among the module's top-level statements.
fn imports_os_environ(suite: &Suite) -> bool {
    suite.iter().any(|statement| {
        matches!(statement, Stmt::ImportFrom(import)
            if import.module.as_ref().is_some_and(|module| module.as_str() == "os")
                && import.names.iter().any(|alias| alias.name.as_str() == "environ"))
    })
}

struct Subscripts {
    bare_is_os: bool,
    found: Vec<(TextRange, String)>,
}

impl Subscripts {
    fn is_wsgi_environ(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Attribute(attribute) => {
                attribute.attr.as_str() == "environ"
                    && !matches!(&*attribute.value, Expr::Name(name) if name.id.as_str() == "os")
            }
            Expr::Name(name) => name.id.as_str() == "environ" && !self.bare_is_os,
            _ => false,
        }
    }
}

impl<'a> Visitor<'a> for Subscripts {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Subscript(subscript) = expr
            && subscript.ctx == ExprContext::Load
            && self.is_wsgi_environ(&subscript.value)
            && let Expr::StringLiteral(key) = &*subscript.slice
            && !GUARANTEED_KEYS.contains(&key.value.to_str())
        {
            self.found
                .push((subscript.range(), key.value.to_str().to_string()));
        }
        visitor::walk_expr(self, expr);
    }
}

/// E8542
pub(crate) fn wsgi_environ_optional_key(checker: &Checker, suite: &Suite) {
    if is_test_path(checker.path()) {
        return;
    }
    let mut subscripts = Subscripts {
        bare_is_os: imports_os_environ(suite),
        found: Vec::new(),
    };
    subscripts.visit_body(suite);
    for (range, key) in subscripts.found {
        checker.report_diagnostic(WsgiEnvironOptionalKey { key }, range);
    }
}
