use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;

/// ## What it does
/// Checks for an `@api.onchange` method that builds a mapping with a `domain`
/// key.
///
/// ## Why is this bad?
/// A domain built by an onchange applies to the one form view that runs it. On
/// the field, every reader of the field agrees on it.
///
/// Only the first such mapping of a method is reported.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct OnchangeDomain;

impl Violation for OnchangeDomain {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Probable dynamic domain returned from an onchange".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Declare the domain on the field".to_string())
    }
}

fn is_onchange_decorator(decorator: &ast::Decorator) -> bool {
    match &decorator.expression {
        Expr::Call(call) => match &*call.func {
            Expr::Attribute(attribute) => attribute.attr.as_str() == "onchange",
            Expr::Name(name) => name.id.as_str() == "onchange",
            _ => false,
        },
        _ => false,
    }
}

/// `{"domain": ...}`, `result["domain"] = ...`, or `dict(domain=...)`.
fn is_domain_mapping(expr: &Expr) -> bool {
    match expr {
        Expr::Dict(dict) => dict
            .items
            .iter()
            .any(|item| matches!(&item.key, Some(Expr::StringLiteral(key)) if key.value.to_str() == "domain")),
        Expr::Subscript(subscript) => {
            subscript.ctx == ExprContext::Store
                && matches!(&*subscript.slice, Expr::StringLiteral(key) if key.value.to_str() == "domain")
        }
        Expr::Call(call) => {
            matches!(&*call.func, Expr::Name(name) if name.id.as_str() == "dict")
                && call
                    .arguments
                    .keywords
                    .iter()
                    .any(|keyword| keyword.arg.as_ref().is_some_and(|arg| arg.as_str() == "domain"))
        }
        _ => false,
    }
}

#[derive(Default)]
struct FirstDomainMapping {
    found: Option<TextRange>,
}

impl<'a> Visitor<'a> for FirstDomainMapping {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if self.found.is_some() {
            return;
        }
        if is_domain_mapping(expr) {
            self.found = Some(expr.range());
            return;
        }
        visitor::walk_expr(self, expr);
    }
}

/// E8509
pub(crate) fn onchange_domain(checker: &Checker, stmt: &Stmt, function: &ast::StmtFunctionDef) {
    if !function.decorator_list.iter().any(is_onchange_decorator) {
        return;
    }
    let mut visitor = FirstDomainMapping::default();
    visitor::walk_stmt(&mut visitor, stmt);
    if let Some(range) = visitor.found {
        checker.report_diagnostic(OnchangeDomain, range);
    }
}
