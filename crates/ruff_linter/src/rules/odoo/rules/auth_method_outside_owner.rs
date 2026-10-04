use std::path::{Component, Path};

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{def_start, in_addon, is_test_path};

/// ## What it does
/// Checks for an `_auth_method_<scheme>` method, a new `auth=` scheme on
/// `ir.http`, outside `base` and `integration`.
///
/// ## Why is this bad?
/// An identity is a scheme on a receiver row, not a method on `ir.http`: the
/// check belongs in the subject's `_is_inbound_request_authentic` (or a
/// resolution's verifier) behind `auth="receiver"`, and a bearer link to a record
/// is an `access.link` behind `auth="link"`. `base` owns `user`, `none`,
/// `public`, `bearer` and `link`; `integration` owns `receiver`. An override of an
/// owned scheme, such as website's `public`, keeps the scheme and is not reported.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Security)]
pub(crate) struct AuthMethodOutsideOwner {
    name: String,
}

impl Violation for AuthMethodOutsideOwner {
    #[derive_message_formats]
    fn message(&self) -> String {
        let AuthMethodOutsideOwner { name } = self;
        format!("`{name}`: a new `auth=` scheme outside `base` and `integration`")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Authenticate behind `auth=\"receiver\"` or `auth=\"link\"`".to_string())
    }
}

const OWNED_METHODS: &[&str] = &["user", "none", "public", "bearer", "link", "receiver"];
const OWNER_MODULES: &[&str] = &["base", "integration", "test_auth_custom"];

/// The module of a path: the directory after its first `addons` directory.
fn module_of(path: &Path) -> Option<String> {
    let mut components = path.components();
    components
        .find(|component| matches!(component, Component::Normal(name) if *name == "addons"))?;
    components
        .next()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
}

/// E8531
pub(crate) fn auth_method_outside_owner(checker: &Checker, function: &ast::StmtFunctionDef) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) {
        return;
    }
    let Some(scheme) = function.name.as_str().strip_prefix("_auth_method_") else {
        return;
    };
    if OWNED_METHODS.contains(&scheme)
        || module_of(path).is_some_and(|module| OWNER_MODULES.contains(&module.as_str()))
    {
        return;
    }
    let start = def_start(function, checker.source());
    checker.report_diagnostic(
        AuthMethodOutsideOwner {
            name: function.name.to_string(),
        },
        TextRange::new(start, function.name.end()),
    );
}
