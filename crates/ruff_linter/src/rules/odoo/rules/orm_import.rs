use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{Stmt, StmtImport, StmtImportFrom};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{in_addon, is_test_path};

/// ## What it does
/// Checks for addon code outside tests that imports `odoo.orm` or one of its
/// submodules.
///
/// ## Why is this bad?
/// Addon code reaches the ORM through the façades `odoo.api`, `odoo.fields`
/// and `odoo.models`; `odoo.orm.*` is the framework's internal layout and moves
/// without notice.
///
/// ## Example
/// ```python
/// from odoo.orm.fields import Char
/// ```
///
/// Use instead:
/// ```python
/// from odoo import fields
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Restriction)]
pub(crate) struct OrmImport {
    statement: String,
}

impl Violation for OrmImport {
    #[derive_message_formats]
    fn message(&self) -> String {
        let OrmImport { statement } = self;
        format!("Addon code imports the ORM's internals: `{statement}`")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Import through `odoo.api`, `odoo.fields` or `odoo.models`".to_string())
    }
}

fn is_orm(name: &str) -> bool {
    name == "odoo.orm" || name.starts_with("odoo.orm.")
}

/// E8508
pub(crate) fn orm_import(checker: &Checker, stmt: &Stmt) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) || checker.semantic().in_type_checking_block() {
        return;
    }
    match stmt {
        Stmt::Import(StmtImport { names, .. }) => {
            for alias in names {
                if is_orm(alias.name.as_str()) {
                    checker.report_diagnostic(
                        OrmImport {
                            statement: format!("import {}", alias.name),
                        },
                        stmt.range(),
                    );
                }
            }
        }
        Stmt::ImportFrom(StmtImportFrom {
            module: Some(module),
            names,
            level: 0,
            ..
        }) => {
            if module.as_str() == "odoo" && names.iter().any(|alias| alias.name.as_str() == "orm") {
                checker.report_diagnostic(
                    OrmImport {
                        statement: "from odoo import orm".to_string(),
                    },
                    stmt.range(),
                );
            } else if is_orm(module.as_str()) {
                let imported = names
                    .iter()
                    .map(|alias| alias.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                checker.report_diagnostic(
                    OrmImport {
                        statement: format!("from {module} import {imported}"),
                    },
                    stmt.range(),
                );
            }
        }
        _ => {}
    }
}
