use ruff_python_ast::Suite;

use crate::checkers::ast::Checker;
use crate::codes::Rule;
use crate::rules::{flake8_bugbear, odoo, ruff};

/// Run lint rules over a module.
pub(crate) fn module(suite: &Suite, checker: &Checker) {
    if checker.is_rule_enabled(Rule::FStringDocstring) {
        flake8_bugbear::rules::f_string_docstring(checker, suite);
    }
    if checker.is_rule_enabled(Rule::NPlusOneQuery) {
        odoo::rules::n_plus_one_query(checker, suite);
    }
    if checker.is_rule_enabled(Rule::SqlBoundPlaceholder) {
        odoo::rules::sql_bound_placeholder(checker, suite);
    }
    if checker.is_rule_enabled(Rule::WsgiEnvironOptionalKey) {
        odoo::rules::wsgi_environ_optional_key(checker, suite);
    }
    if checker.is_rule_enabled(Rule::TaxCompanySingular) {
        odoo::rules::tax_company_singular(checker, suite);
    }
    if checker.is_rule_enabled(Rule::LinkDoorDeclared) {
        odoo::rules::link_door_declared(checker, suite);
    }
    if checker.any_rule_enabled(&[Rule::RawEgress, Rule::SecretInEnviron]) {
        odoo::rules::egress(checker, suite);
    }
    if checker.is_rule_enabled(Rule::InvalidFormatterSuppressionComment) {
        ruff::rules::ignored_formatter_suppression_comment(checker, suite);
    }
}
