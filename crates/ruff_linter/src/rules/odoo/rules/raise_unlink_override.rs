use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::statement_visitor::{StatementVisitor, walk_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;

/// ## What it does
/// Checks for a `raise` inside a model's `unlink` override.
///
/// ## Why is this bad?
/// Raising in `unlink` also blocks uninstalling the module.
/// `@api.ondelete(at_uninstall=False)` refuses the deletion and lets the uninstall
/// through.
///
/// A bare `raise` is not reported: it hands on what `super().unlink()` or the
/// database refused, and the override adds no refusal of its own.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct RaiseUnlinkOverride;

impl Violation for RaiseUnlinkOverride {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Raise inside an `unlink` override".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Refuse in a method decorated `@api.ondelete(at_uninstall=False)`".to_string())
    }
}

const MODEL_BASES: &[&str] = &["Model", "AbstractModel", "TransientModel", "BaseModel"];

fn looks_like_model_class(class: &ast::StmtClassDef) -> bool {
    class.bases().iter().any(|base| match base {
        Expr::Attribute(attribute) => MODEL_BASES.contains(&attribute.attr.as_str()),
        Expr::Name(name) => MODEL_BASES.contains(&name.id.as_str()),
        _ => false,
    })
}

#[derive(Default)]
struct Raises<'a> {
    found: Vec<&'a ast::StmtRaise>,
}

impl<'a> StatementVisitor<'a> for Raises<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let Stmt::Raise(raise) = stmt
            && raise.exc.is_some()
        {
            self.found.push(raise);
        }
        walk_stmt(self, stmt);
    }
}

/// E8506
pub(crate) fn raise_unlink_override(checker: &Checker, class: &ast::StmtClassDef) {
    if !looks_like_model_class(class) {
        return;
    }
    for statement in &class.body {
        if let Stmt::FunctionDef(function) = statement
            && function.name.as_str() == "unlink"
        {
            let mut raises = Raises::default();
            raises.visit_body(&function.body);
            for raise in raises.found {
                checker.report_diagnostic(RaiseUnlinkOverride, raise.range());
            }
        }
    }
}
