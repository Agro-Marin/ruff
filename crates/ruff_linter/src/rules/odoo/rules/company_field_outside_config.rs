use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{in_addon, is_test_path};
use crate::rules::odoo::model::{field_declarations, inherited};

/// ## What it does
/// Checks for a field declared on `res.company` by an application.
///
/// ## Why is this bad?
/// An application keeps what it configures per company on its own
/// `mixin.company.config` model and links it from the company through one
/// `<app>_config_id` field: the tenant carries its identity and tenancy, not
/// every application's settings.
///
/// A link to the configuration, a related field read through it, a
/// company-owned One2many, a derivation that neither stores nor writes, and the
/// company's credential doors are not settings, and are not reported.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Restriction)]
pub(crate) struct CompanyFieldOutsideConfig {
    name: String,
}

impl Violation for CompanyFieldOutsideConfig {
    #[derive_message_formats]
    fn message(&self) -> String {
        let CompanyFieldOutsideConfig { name } = self;
        format!("`res.company.{name}` belongs to the application's `mixin.company.config` model")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Declare it on the configuration model, linked through `<app>_config_id`".to_string())
    }
}

fn keyword<'a>(call: &'a ast::ExprCall, name: &str) -> Option<&'a Expr> {
    call.arguments
        .keywords
        .iter()
        .find(|keyword| keyword.arg.as_ref().is_some_and(|arg| arg.as_str() == name))
        .map(|keyword| &keyword.value)
}

/// A related through `<app>_config_id`: the configuration read from a company view.
fn is_related_through_config(call: &ast::ExprCall) -> bool {
    match keyword(call, "related") {
        Some(Expr::StringLiteral(path)) => {
            let path = path.value.to_str();
            path.split('.')
                .next()
                .is_some_and(|head| head.ends_with("_config_id"))
        }
        _ => false,
    }
}

fn is_company_owned_collection(call: &ast::ExprCall) -> bool {
    matches!(&*call.func, Expr::Attribute(attribute) if attribute.attr.as_str() == "One2many")
        && keyword(call, "inverse_name").is_some()
}

/// A derivation that declares neither `store=True` nor an `inverse` holds
/// nothing on the company: a view of data that lives elsewhere. A `store=` other
/// than a literal `False` is not readable here, so it is judged as storing.
fn reads_without_writing(call: &ast::ExprCall) -> bool {
    let mut derives = false;
    for keyword in &call.arguments.keywords {
        match keyword.arg.as_ref().map(ast::Identifier::as_str) {
            Some("store")
                if matches!(
                    keyword.value,
                    Expr::BooleanLiteral(ast::ExprBooleanLiteral { value: false, .. })
                ) => {}
            Some("inverse" | "store") => return false,
            Some("compute" | "related") => derives = true,
            _ => {}
        }
    }
    derives
}

/// `_credential_holder_field = "<name>"`: the link to the company's vault.
fn credential_holder_link(class: &ast::StmtClassDef) -> Option<&str> {
    class.body.iter().find_map(|statement| match statement {
        Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
            match (targets.as_slice(), &**value) {
                ([Expr::Name(target)], Expr::StringLiteral(name))
                    if target.id.as_str() == "_credential_holder_field" =>
                {
                    Some(name.value.to_str())
                }
                _ => None,
            }
        }
        _ => None,
    })
}

/// The keys of `_CREDENTIAL_FIELDS = {...}`: credential doors, not storage.
fn credential_doors(class: &ast::StmtClassDef) -> Vec<&str> {
    class
        .body
        .iter()
        .find_map(|statement| match statement {
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                match (targets.as_slice(), &**value) {
                    ([Expr::Name(target)], Expr::Dict(dict))
                        if target.id.as_str() == "_CREDENTIAL_FIELDS" =>
                    {
                        Some(
                            dict.items
                                .iter()
                                .filter_map(|item| match &item.key {
                                    Some(Expr::StringLiteral(key)) => Some(key.value.to_str()),
                                    _ => None,
                                })
                                .collect(),
                        )
                    }
                    _ => None,
                }
            }
            _ => None,
        })
        .unwrap_or_default()
}

/// E8530
pub(crate) fn company_field_outside_config(checker: &Checker, class: &ast::StmtClassDef) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) || path.to_string_lossy().contains("/addons/base/") {
        return;
    }
    if !inherited(class)
        .iter()
        .any(|parent| parent == "res.company")
    {
        return;
    }
    let doors = credential_doors(class);
    let vault_link = credential_holder_link(class);
    for (name, call, statement) in field_declarations(class) {
        if name.ends_with("_config_id")
            || is_related_through_config(call)
            || is_company_owned_collection(call)
            || reads_without_writing(call)
            || doors.contains(&name)
            || vault_link == Some(name)
        {
            continue;
        }
        checker.report_diagnostic(
            CompanyFieldOutsideConfig {
                name: name.to_string(),
            },
            statement.range(),
        );
    }
}
