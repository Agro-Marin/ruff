use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;

/// ## What it does
/// Checks for a mutation (`unlink`, `write`, `update`, `copy`,
/// `action_archive`, `action_unarchive`) called on `env["model"]`.
///
/// ## Why is this bad?
/// `env["model"]` is the empty recordset: the call iterates nothing and
/// returns, and the line reads as a cleanup or a setting while the rows it
/// names, often demo data, stay. Call it on the records it is meant to change:
/// `search([...])`, `browse(ids)`, a fixture. A settings value is applied with
/// `create({...}).execute()`.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct EmptyRecordsetMutation {
    model: String,
    method: String,
}

impl Violation for EmptyRecordsetMutation {
    #[derive_message_formats]
    fn message(&self) -> String {
        let EmptyRecordsetMutation { model, method } = self;
        format!("`env[{model}].{method}()` runs on the empty recordset and changes nothing")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Call it on the records it is meant to change".to_string())
    }
}

const MUTATIONS: &[&str] = &[
    "unlink",
    "write",
    "update",
    "copy",
    "action_archive",
    "action_unarchive",
];

const ENV_SWITCHES: &[&str] = &[
    "sudo",
    "with_context",
    "with_company",
    "with_user",
    "with_env",
    "with_privilege",
];

/// `env[...]` or `<x>.env[...]`, through any environment switches.
fn model_lookup(mut expr: &Expr) -> Option<&ast::ExprSubscript> {
    while let Expr::Call(call) = expr
        && let Expr::Attribute(attribute) = &*call.func
        && ENV_SWITCHES.contains(&attribute.attr.as_str())
    {
        expr = &attribute.value;
    }
    match expr {
        Expr::Subscript(subscript)
            if matches!(&*subscript.value, Expr::Name(name) if name.id.as_str() == "env")
                || matches!(&*subscript.value, Expr::Attribute(attribute) if attribute.attr.as_str() == "env") =>
        {
            Some(subscript)
        }
        _ => None,
    }
}

/// E8543
pub(crate) fn empty_recordset_mutation(checker: &Checker, call: &ast::ExprCall) {
    if let Expr::Attribute(attribute) = &*call.func
        && MUTATIONS.contains(&attribute.attr.as_str())
        && let Some(lookup) = model_lookup(&attribute.value)
    {
        checker.report_diagnostic(
            EmptyRecordsetMutation {
                model: checker.locator().slice(&*lookup.slice).to_string(),
                method: attribute.attr.to_string(),
            },
            call.range(),
        );
    }
}
