use std::sync::LazyLock;

use regex::Regex;
use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::statement_visitor::{StatementVisitor, walk_stmt};
use ruff_python_ast::{self as ast, CmpOp, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::is_constant_side;
use crate::rules::odoo::rules::is_link_token;

/// ## What it does
/// Checks for a token, a signature or a computed HMAC compared with `==` or
/// `!=`.
///
/// ## Why is this bad?
/// `==` returns at the first differing character, so the answer time tells a
/// guesser how much of the secret is right. `consteq` takes the same time
/// whatever matched.
///
/// A bearer token that opens a record is `link-token-compare`'s (E8537): its fix
/// is `access.link`.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Security)]
pub(crate) struct TokenCompare;

impl Violation for TokenCompare {
    #[derive_message_formats]
    fn message(&self) -> String {
        "A token compared with `==`/`!=` leaks how many leading characters matched through the time it takes".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Compare with `consteq`".to_string())
    }
}

/// A name that holds a bearer secret, or a call that computes one.
static TOKEN_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|_)token$").expect("valid regex"));
static TOKEN_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(_generate_\w*token|_sign_token|_encode_link\w*|^hmac)$").expect("valid regex")
});

/// The last name an expression spells, and whether it is called; a subscript by
/// a string spells the string. (`test_lint` spells any constant subscript with
/// `str()`, but no other constant's spelling ends in `token`.)
fn terminal(expr: &Expr) -> (String, bool) {
    match expr {
        Expr::Call(call) => (terminal(&call.func).0, true),
        Expr::Attribute(attribute) => (attribute.attr.to_string(), false),
        Expr::Name(name) => (name.id.to_string(), false),
        Expr::Subscript(subscript) => match &*subscript.slice {
            Expr::StringLiteral(key) => (key.value.to_str().to_string(), false),
            _ => (String::new(), false),
        },
        _ => (String::new(), false),
    }
}

/// `hmac.digest(...)`, `hmac.new(...)`, and `.hexdigest()` or `.digest()` of
/// one: a keyed digest.
fn is_hmac(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    let Expr::Attribute(attribute) = &*call.func else {
        return false;
    };
    match attribute.attr.as_str() {
        "new" | "digest" if matches!(&*attribute.value, Expr::Name(name) if name.id.as_str() == "hmac") => {
            true
        }
        "hexdigest" | "digest" => is_hmac(&attribute.value),
        _ => false,
    }
}

fn computes_a_secret(expr: &Expr) -> bool {
    let (name, called) = terminal(expr);
    is_hmac(expr) || (called && TOKEN_CALL.is_match(&name))
}

/// What the enclosing function assigns to a name, nested scopes left out.
struct Assignments<'a, 'n> {
    name: &'n str,
    values: Vec<&'a Expr>,
}

impl<'a> StatementVisitor<'a> for Assignments<'a, '_> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                if targets.iter().any(
                    |target| matches!(target, Expr::Name(name) if name.id.as_str() == self.name),
                ) {
                    self.values.push(value);
                }
            }
            _ => walk_stmt(self, stmt),
        }
    }
}

fn is_secret(checker: &Checker, side: &Expr) -> bool {
    let (name, called) = terminal(side);
    if called || is_hmac(side) {
        return computes_a_secret(side);
    }
    if TOKEN_NAME.is_match(&name) {
        return true;
    }
    let Expr::Name(variable) = side else {
        return false;
    };
    let Some(function) = checker
        .semantic()
        .current_statements()
        .find_map(|statement| statement.as_function_def_stmt())
    else {
        return false;
    };
    let mut assignments = Assignments {
        name: variable.id.as_str(),
        values: Vec::new(),
    };
    assignments.visit_body(&function.body);
    assignments.values.into_iter().any(computes_a_secret)
}

/// A bearer token that opens a record is link-token-compare's.
fn is_link_token_side(side: &Expr) -> bool {
    let (name, called) = terminal(side);
    !called && is_link_token(&name)
}

/// E8536
pub(crate) fn token_compare(checker: &Checker, compare: &ast::ExprCompare) {
    let ([op], [right]) = (&*compare.ops, &*compare.comparators) else {
        return;
    };
    if !matches!(op, CmpOp::Eq | CmpOp::NotEq) {
        return;
    }
    let sides = [&*compare.left, right];
    if sides.iter().any(|side| is_constant_side(side))
        || sides.iter().any(|side| is_link_token_side(side))
    {
        return;
    }
    if sides.iter().any(|side| is_secret(checker, side)) {
        checker.report_diagnostic(TokenCompare, compare.range());
    }
}
