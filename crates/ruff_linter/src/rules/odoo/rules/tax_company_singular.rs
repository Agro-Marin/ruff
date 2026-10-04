use rustc_hash::{FxHashMap, FxHashSet};

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, Expr, Stmt, Suite};
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;

/// ## What it does
/// Checks for `company_id` read on a tax in a `filtered` lambda over a tax
/// field, and for a `("company_id", ...)` domain leaf against `account.tax`,
/// `account.tax.group`, `account.tax.repartition.line` or `account.account`.
///
/// ## Why is this bad?
/// These models carry `company_ids` (a many2many), not `company_id`: reading it
/// raises `AttributeError`, and a domain leaf on it matches nothing and raises.
/// `filtered_domain(env['account.tax']._check_company_domain(company))` selects
/// a company's taxes.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct TaxCompanySingular {
    domain: bool,
}

impl Violation for TaxCompanySingular {
    #[derive_message_formats]
    fn message(&self) -> String {
        if self.domain {
            "A `company_id` domain leaf against a model that carries `company_ids`".to_string()
        } else {
            "`account.tax` has `company_ids` (many2many), not `company_id`".to_string()
        }
    }

    fn fix_title(&self) -> Option<String> {
        Some("Use `filtered_domain(env['account.tax']._check_company_domain(company))`".to_string())
    }
}

const TAX_FIELDS: &[&str] = &[
    "tax_id",
    "tax_ids",
    "taxes_id",
    "supplier_taxes_id",
    "original_tax_ids",
    "l10n_tax_ids",
];

const DIVERGED_MODELS: &[&str] = &[
    "account.tax",
    "account.tax.group",
    "account.tax.repartition.line",
    "account.account",
];

const DOMAIN_METHODS: &[&str] = &[
    "search",
    "search_count",
    "search_fetch",
    "_search",
    "read_group",
    "_read_group",
    "filtered_domain",
];

/// `for model in ("account.tax", ...)`: the models a loop variable names.
#[derive(Default)]
struct ModelAliases<'a> {
    aliases: FxHashMap<&'a str, FxHashSet<&'a str>>,
}

impl<'a> Visitor<'a> for ModelAliases<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let Stmt::For(for_stmt) = stmt
            && let Expr::Name(target) = &*for_stmt.target
            && let Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) =
                &*for_stmt.iter
        {
            let names: Vec<&str> = elts
                .iter()
                .filter_map(|elt| match elt {
                    Expr::StringLiteral(name) => Some(name.value.to_str()),
                    _ => None,
                })
                .collect();
            if !names.is_empty() {
                self.aliases
                    .entry(target.id.as_str())
                    .or_default()
                    .extend(names);
            }
        }
        visitor::walk_stmt(self, stmt);
    }
}

/// The models `<x>.env["model"]` names, through calls and attributes on it.
fn env_models<'a>(mut expr: &'a Expr, aliases: &ModelAliases<'a>) -> Vec<&'a str> {
    loop {
        match expr {
            Expr::Subscript(subscript) if matches!(&*subscript.value, Expr::Attribute(attribute) if attribute.attr.as_str() == "env") =>
            {
                return match &*subscript.slice {
                    Expr::StringLiteral(name) => vec![name.value.to_str()],
                    Expr::Name(alias) => aliases
                        .aliases
                        .get(alias.id.as_str())
                        .map(|names| names.iter().copied().collect())
                        .unwrap_or_default(),
                    _ => Vec::new(),
                };
            }
            Expr::Call(call) => expr = &call.func,
            Expr::Attribute(attribute) => expr = &attribute.value,
            _ => return Vec::new(),
        }
    }
}

/// Every `("company_id", ...)` or `["company_id", ...]` under an expression.
#[derive(Default)]
struct CompanyLeaves {
    found: Vec<TextRange>,
}

impl<'a> Visitor<'a> for CompanyLeaves {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) =
            expr
            && let Some(Expr::StringLiteral(field)) = elts.first()
            && field.value.to_str() == "company_id"
        {
            self.found.push(expr.range());
        }
        visitor::walk_expr(self, expr);
    }
}

/// `<param>.company_id` under a lambda body.
struct CompanyReads<'n> {
    parameter: &'n str,
    found: Vec<TextRange>,
}

impl<'a> Visitor<'a> for CompanyReads<'_> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Attribute(attribute) = expr
            && attribute.attr.as_str() == "company_id"
            && matches!(&*attribute.value, Expr::Name(name) if name.id.as_str() == self.parameter)
        {
            self.found.push(attribute.range());
        }
        visitor::walk_expr(self, expr);
    }
}

/// `<x>.<tax field>.filtered(lambda tax, ...: ...)`.
fn filtered_tax_lambda(call: &ast::ExprCall) -> Option<(&ast::ExprLambda, &str)> {
    let Expr::Attribute(method) = &*call.func else {
        return None;
    };
    let Expr::Attribute(field) = &*method.value else {
        return None;
    };
    if method.attr.as_str() != "filtered" || !TAX_FIELDS.contains(&field.attr.as_str()) {
        return None;
    }
    let Some(Expr::Lambda(lambda)) = call.arguments.args.first() else {
        return None;
    };
    let first = lambda.parameters.as_ref()?.args.first()?;
    Some((lambda, first.name().as_str()))
}

struct Calls<'a> {
    aliases: ModelAliases<'a>,
    found: Vec<(TextRange, bool)>,
}

impl<'a> Visitor<'a> for Calls<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr {
            if let Some((lambda, parameter)) = filtered_tax_lambda(call) {
                let mut reads = CompanyReads {
                    parameter,
                    found: Vec::new(),
                };
                reads.visit_expr(&lambda.body);
                self.found
                    .extend(reads.found.into_iter().map(|range| (range, false)));
            } else if let Expr::Attribute(method) = &*call.func
                && DOMAIN_METHODS.contains(&method.attr.as_str())
                && env_models(&method.value, &self.aliases)
                    .iter()
                    .any(|model| DIVERGED_MODELS.contains(model))
            {
                let mut leaves = CompanyLeaves::default();
                for argument in call
                    .arguments
                    .args
                    .iter()
                    .chain(call.arguments.keywords.iter().map(|keyword| &keyword.value))
                {
                    leaves.visit_expr(argument);
                }
                self.found
                    .extend(leaves.found.into_iter().map(|range| (range, true)));
            }
        }
        visitor::walk_expr(self, expr);
    }
}

/// E8514
pub(crate) fn tax_company_singular(checker: &Checker, suite: &Suite) {
    let mut aliases = ModelAliases::default();
    aliases.visit_body(suite);
    let mut calls = Calls {
        aliases,
        found: Vec::new(),
    };
    calls.visit_body(suite);
    for (range, domain) in calls.found {
        checker.report_diagnostic(TaxCompanySingular { domain }, range);
    }
}
