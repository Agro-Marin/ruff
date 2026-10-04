use std::sync::LazyLock;

use regex::Regex;
use rustc_hash::FxHashSet;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::statement_visitor::{self, StatementVisitor};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, Expr, Stmt, Suite};
use ruff_text_size::TextSize;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::callee_name;
use crate::rules::odoo::strings::string_constants;

/// ## What it does
/// Checks for `IN %s` or `INTERVAL %s` in a statement passed to `execute`.
///
/// ## Why is this bad?
/// A bound parameter cannot stand where PostgreSQL parses syntax: psycopg binds
/// server-side, so `IN %s` and `INTERVAL %s` in a raw `cr.execute` reach the
/// server as `IN $1` and never parse. Build the statement with `SQL()` (its
/// tuple branch expands, `SQL.literal` inlines) or pass a value the driver
/// adapts, such as a `timedelta` for an interval.
///
/// A statement held in a variable is read through what its scope assigns to it.
/// A statement that `SQL()` builds is not reported.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct SqlBoundPlaceholder {
    shape: &'static str,
}

impl Violation for SqlBoundPlaceholder {
    #[derive_message_formats]
    fn message(&self) -> String {
        let SqlBoundPlaceholder { shape } = self;
        format!(
            "`{shape}` binds a parameter where PostgreSQL parses syntax, so the statement never runs"
        )
    }

    fn fix_title(&self) -> Option<String> {
        Some(
            "Build it with `SQL()` (a tuple expands, `SQL.literal` inlines) or pass a value the driver adapts"
                .to_string(),
        )
    }
}

const EXECUTORS: &[&str] = &["execute", "executemany", "execute_query", "execute_values"];
const BUILDERS: &[&str] = &["SQL", "literal"];

static SHAPES: LazyLock<[(Regex, &'static str); 2]> = LazyLock::new(|| {
    [
        (
            Regex::new(r"(?i)\bIN\s*%(?:\(\w+\))?s").expect("valid regex"),
            "IN %s",
        ),
        (
            Regex::new(r"(?i)\bINTERVAL\s*%(?:\(\w+\))?s").expect("valid regex"),
            "INTERVAL %s",
        ),
    ]
});

/// What a scope binds a name to, nested functions and classes left out.
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
            Stmt::AugAssign(ast::StmtAugAssign { target, value, .. })
            | Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(value),
                ..
            }) => {
                if matches!(&**target, Expr::Name(name) if name.id.as_str() == self.name) {
                    self.values.push(value);
                }
            }
            _ => statement_visitor::walk_stmt(self, stmt),
        }
    }
}

#[derive(Default)]
struct BuilderCall {
    found: bool,
}

impl<'a> Visitor<'a> for BuilderCall {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr
            && BUILDERS.contains(&callee_name(&call.func))
        {
            self.found = true;
        }
        if !self.found {
            visitor::walk_expr(self, expr);
        }
    }
}

/// Every `execute` call, with the body of the function (or module) it runs in.
struct Executions<'a> {
    scope: &'a [Stmt],
    found: Vec<(&'a ast::ExprCall, &'a [Stmt])>,
}

impl<'a> Visitor<'a> for Executions<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        // A function's decorators and defaults run in the scope that defines
        // it, but test_lint reads them in the function's own, as their parent
        // node is the function.
        if let Stmt::FunctionDef(function) = stmt {
            let outer = std::mem::replace(&mut self.scope, &function.body);
            visitor::walk_stmt(self, stmt);
            self.scope = outer;
        } else {
            visitor::walk_stmt(self, stmt);
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr
            && EXECUTORS.contains(&callee_name(&call.func))
        {
            self.found.push((call, self.scope));
        }
        visitor::walk_expr(self, expr);
    }
}

/// E8534
pub(crate) fn sql_bound_placeholder(checker: &Checker, suite: &Suite) {
    let mut executions = Executions {
        scope: suite,
        found: Vec::new(),
    };
    executions.visit_body(suite);
    let mut reported: FxHashSet<TextSize> = FxHashSet::default();
    for (call, scope) in executions.found {
        let Some(statement) = call.arguments.args.first() else {
            continue;
        };
        let parts = match statement {
            Expr::Name(name) => {
                let mut assignments = Assignments {
                    name: name.id.as_str(),
                    values: Vec::new(),
                };
                assignments.visit_body(scope);
                assignments.values
            }
            _ => vec![statement],
        };
        let built = parts.iter().any(|part| {
            let mut builder = BuilderCall::default();
            builder.visit_expr(part);
            builder.found
        });
        if built {
            continue;
        }
        for literal in parts.into_iter().flat_map(string_constants) {
            if !reported.insert(literal.range.start()) {
                continue;
            }
            for (pattern, shape) in SHAPES.iter() {
                if pattern.is_match(&literal.value) {
                    checker.report_diagnostic(SqlBoundPlaceholder { shape }, literal.range);
                }
            }
        }
    }
}
