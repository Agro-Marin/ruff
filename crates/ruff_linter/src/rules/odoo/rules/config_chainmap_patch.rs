use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;

/// ## What it does
/// Checks for `patch.dict(config.options, ...)`.
///
/// ## Why is this bad?
/// `config.options` is a `ChainMap`: `patch.dict` flattens every lower layer
/// into `_override_options`, and the damage lands on the next test.
/// `config.patch(**values)` sets and restores the override layer only.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct ConfigChainmapPatch;

impl Violation for ConfigChainmapPatch {
    #[derive_message_formats]
    fn message(&self) -> String {
        "`patch.dict` flattens the `config.options` ChainMap into `_override_options`".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Use `config.patch(**values)`".to_string())
    }
}

/// `patch.dict` or `<module>.patch.dict`.
fn is_patch_dict(func: &Expr) -> bool {
    match func {
        Expr::Attribute(attribute) if attribute.attr.as_str() == "dict" => {
            match &*attribute.value {
                Expr::Name(name) => name.id.as_str() == "patch",
                Expr::Attribute(inner) => inner.attr.as_str() == "patch",
                _ => false,
            }
        }
        _ => false,
    }
}

/// `config.options`, `<module>.config.options`, or a string naming either.
fn is_config_options(expr: &Expr) -> bool {
    match expr {
        Expr::Attribute(attribute) if attribute.attr.as_str() == "options" => {
            match &*attribute.value {
                Expr::Name(name) => name.id.as_str() == "config",
                Expr::Attribute(inner) => inner.attr.as_str() == "config",
                _ => false,
            }
        }
        Expr::StringLiteral(string) => {
            let target = string.value.to_str();
            target == "config.options" || target.ends_with(".config.options")
        }
        _ => false,
    }
}

/// E8510
pub(crate) fn config_chainmap_patch(checker: &Checker, call: &ast::ExprCall) {
    if is_patch_dict(&call.func) && call.arguments.args.first().is_some_and(is_config_options) {
        checker.report_diagnostic(ConfigChainmapPatch, call.range());
    }
}
