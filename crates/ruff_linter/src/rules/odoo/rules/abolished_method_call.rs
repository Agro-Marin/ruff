use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;

/// ## What it does
/// Checks for a call to a method this Odoo fork removed.
///
/// ## Why is this bad?
/// This fork removed the method, so the call raises `AttributeError` the first
/// time the line runs, which a path no test takes hides until production. The
/// receiver's type is unknown here, so only names that no surviving API reuses are
/// listed: `ensure_one` (now `check_singleton`) and `_for_xml_id`.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct AbolishedMethodCall {
    method: String,
    replacement: &'static str,
}

impl Violation for AbolishedMethodCall {
    #[derive_message_formats]
    fn message(&self) -> String {
        let AbolishedMethodCall { method, .. } = self;
        format!("`{method}()` does not exist in this fork: the call raises `AttributeError`")
    }

    fn fix_title(&self) -> Option<String> {
        let AbolishedMethodCall { replacement, .. } = self;
        Some(format!("Use `{replacement}`"))
    }
}

fn replacement(method: &str) -> Option<&'static str> {
    match method {
        "ensure_one" => Some("check_singleton()"),
        "_for_xml_id" => Some(r#"env["ir.actions.actions"]._get_action_dict_by_xml_id(xml_id)"#),
        _ => None,
    }
}

/// E8535
pub(crate) fn abolished_method_call(checker: &Checker, call: &ast::ExprCall) {
    if let Expr::Attribute(attribute) = &*call.func
        && let Some(replacement) = replacement(attribute.attr.as_str())
    {
        checker.report_diagnostic(
            AbolishedMethodCall {
                method: attribute.attr.to_string(),
                replacement,
            },
            call.range(),
        );
    }
}
