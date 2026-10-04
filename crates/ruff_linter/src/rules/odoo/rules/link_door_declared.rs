use rustc_hash::FxHashSet;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt, Suite};
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{callee_name, def_start, in_addon, is_test_path};

/// ## What it does
/// Checks for a route that reads an `access_token` without declaring
/// `auth="link"` or reaching a door, and for any use of the retired
/// `_document_check_access`.
///
/// ## Why is this bad?
/// A token checked anywhere but at a door is a capability nobody can list,
/// expire or revoke. A route that reads an `access_token` declares
/// `auth="link"` (`link="<model>:<path argument>"`), or reaches a door:
/// `env['access.link']._open_record` for an RPC route or a helper, `_resolve`,
/// a model's `_from_*_token`, or `tools.is_hmac_valid`, directly or through a
/// function of the same file that does. `_document_check_access` is gone.
///
/// A route whose token is no link takes a `noqa` naming its class.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Security)]
pub(crate) struct LinkDoorDeclared {
    route: Option<String>,
}

impl Violation for LinkDoorDeclared {
    #[derive_message_formats]
    fn message(&self) -> String {
        match &self.route {
            Some(route) => {
                format!("Route `{route}` takes an `access_token` and opens nothing through a door")
            }
            None => "`_document_check_access` is gone".to_string(),
        }
    }

    fn fix_title(&self) -> Option<String> {
        Some(match &self.route {
            Some(_) => "Declare `auth=\"link\"`, or reach a door such as `_open_record`".to_string(),
            None => "A route declares `auth=\"link\"`; other code calls `env['access.link']._open_record`".to_string(),
        })
    }
}

const DOORS: &[&str] = &[
    "_open_record",
    "_resolve",
    "_portal_token_opens",
    "_get_thread_with_access",
    "is_hmac_valid",
    "is_access_token_valid",
    "_can_return_content",
    "_from_booking_token",
    "_from_invitation_token",
    "_from_signer_token",
    "_from_request_token",
    "_from_access_token",
];

const RETIRED: &str = "_document_check_access";

/// What a function does anywhere inside it, its decorators and the functions
/// it defines included: the names it calls, and whether it reads `access_token`.
#[derive(Default)]
struct Facts<'a> {
    called: FxHashSet<&'a str>,
    reads_access_token: bool,
}

impl<'a> Visitor<'a> for Facts<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Call(call) => {
                let name = callee_name(&call.func);
                if !name.is_empty() {
                    self.called.insert(name);
                }
            }
            Expr::Name(name)
                if name.id.as_str() == "access_token" && name.ctx == ExprContext::Load =>
            {
                self.reads_access_token = true;
            }
            _ => {}
        }
        visitor::walk_expr(self, expr);
    }
}

/// Every function of the module, at any depth, and every use of the retired
/// method.
#[derive(Default)]
struct Module<'a> {
    functions: Vec<(&'a ast::StmtFunctionDef, Facts<'a>)>,
    retired: Vec<TextRange>,
}

impl<'a> Visitor<'a> for Module<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let Stmt::FunctionDef(function) = stmt {
            let mut facts = Facts::default();
            visitor::walk_stmt(&mut facts, stmt);
            self.functions.push((function, facts));
        }
        visitor::walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Attribute(attribute) = expr
            && attribute.attr.as_str() == RETIRED
        {
            self.retired.push(attribute.range());
        }
        visitor::walk_expr(self, expr);
    }
}

/// The first `route(...)` decorator.
fn route(function: &ast::StmtFunctionDef) -> Option<&ast::ExprCall> {
    function
        .decorator_list
        .iter()
        .find_map(|decorator| match &decorator.expression {
            Expr::Call(call) if callee_name(&call.func) == "route" => Some(call),
            _ => None,
        })
}

/// The first constant `auth=` of the route is `"link"`.
fn declares_link(route: &ast::ExprCall) -> bool {
    route
        .arguments
        .keywords
        .iter()
        .find(|keyword| {
            keyword.arg.as_ref().is_some_and(|arg| arg.as_str() == "auth")
                && keyword.value.is_literal_expr()
        })
        .is_some_and(|keyword| matches!(&keyword.value, Expr::StringLiteral(auth) if auth.value.to_str() == "link"))
}

/// E8539
pub(crate) fn link_door_declared(checker: &Checker, suite: &Suite) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) {
        return;
    }
    let mut module = Module::default();
    module.visit_body(suite);
    for range in module.retired {
        checker.report_diagnostic(LinkDoorDeclared { route: None }, range);
    }
    // A function that calls a door is a door for the functions of the file.
    let mut doors: FxHashSet<&str> = DOORS.iter().copied().collect();
    let mut grew = true;
    while grew {
        grew = false;
        for (function, facts) in &module.functions {
            let name = function.name.as_str();
            if !doors.contains(name) && facts.called.iter().any(|called| doors.contains(called)) {
                doors.insert(name);
                grew = true;
            }
        }
    }
    for (function, facts) in &module.functions {
        let Some(route) = route(function) else {
            continue;
        };
        if route.arguments.is_empty() || !facts.reads_access_token {
            continue;
        }
        let parameters = &function.parameters;
        let takes_token = parameters
            .args
            .iter()
            .chain(&parameters.kwonlyargs)
            .any(|parameter| parameter.name().as_str() == "access_token");
        if !takes_token
            || declares_link(route)
            || facts.called.iter().any(|called| doors.contains(called))
        {
            continue;
        }
        let start = def_start(function, checker.source());
        checker.report_diagnostic(
            LinkDoorDeclared {
                route: Some(function.name.to_string()),
            },
            TextRange::new(start, function.name.end()),
        );
    }
}
