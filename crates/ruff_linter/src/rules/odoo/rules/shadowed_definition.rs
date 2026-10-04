use rustc_hash::FxHashSet;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::def_start;

/// ## What it does
/// Checks for a method defined twice in one class body.
///
/// ## Why is this bad?
/// Python keeps the last definition, so the earlier one is dead code that still
/// reads as live.
///
/// `@overload` stubs, a property followed by its own `@<name>.setter`,
/// `.getter` or `.deleter`, and `@<dispatcher>.register` implementations of a
/// `singledispatchmethod` are not redefinitions.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct ShadowedDefinition {
    class: String,
    name: String,
    shadowed: String,
}

impl Violation for ShadowedDefinition {
    #[derive_message_formats]
    fn message(&self) -> String {
        let ShadowedDefinition {
            class,
            name,
            shadowed,
        } = self;
        format!(
            "`{class}.{name}` is defined again here, so the definition at {shadowed} never runs"
        )
    }

    fn fix_title(&self) -> Option<String> {
        Some("Define the member once".to_string())
    }
}

/// Every name a decorator spells: `api.depends("x")` gives `api` and `depends`.
fn decorator_names(function: &ast::StmtFunctionDef) -> FxHashSet<&str> {
    let mut names = FxHashSet::default();
    for decorator in &function.decorator_list {
        let mut expr = match &decorator.expression {
            Expr::Call(call) => &*call.func,
            expr => expr,
        };
        while let Expr::Attribute(attribute) = expr {
            names.insert(attribute.attr.as_str());
            expr = &attribute.value;
        }
        if let Expr::Name(name) = expr {
            names.insert(name.id.as_str());
        }
    }
    names
}

/// `@<name>.setter`, `@<name>.getter` or `@<name>.deleter`.
fn redecorates(function: &ast::StmtFunctionDef, name: &str) -> bool {
    function.decorator_list.iter().any(|decorator| {
        matches!(&decorator.expression, Expr::Attribute(attribute)
            if matches!(attribute.attr.as_str(), "setter" | "deleter" | "getter")
                && matches!(&*attribute.value, Expr::Name(owner) if owner.id.as_str() == name))
    })
}

fn is_legitimate(definitions: &[&ast::StmtFunctionDef]) -> bool {
    let decorators: Vec<FxHashSet<&str>> = definitions.iter().map(|f| decorator_names(f)).collect();
    if decorators[..decorators.len() - 1]
        .iter()
        .all(|names| names.contains("overload"))
    {
        return true;
    }
    // `@other.setter` above a second `f` re-decorates `other`, not `f`: the first
    // `f` dies exactly as it would under a bare redefinition.
    (decorators[0].contains("property") || decorators[0].contains("cached_property"))
        && definitions[1..]
            .iter()
            .all(|function| redecorates(function, definitions[0].name.as_str()))
}

/// The dispatchers a function registers on: `@<owner>.register`.
fn registers_on(function: &ast::StmtFunctionDef) -> impl Iterator<Item = &str> {
    function.decorator_list.iter().filter_map(|decorator| {
        let expr = match &decorator.expression {
            Expr::Call(call) => &*call.func,
            expr => expr,
        };
        match expr {
            Expr::Attribute(attribute) if attribute.attr.as_str() == "register" => {
                match &*attribute.value {
                    Expr::Name(owner) => Some(owner.id.as_str()),
                    _ => None,
                }
            }
            _ => None,
        }
    })
}

/// E8513
pub(crate) fn shadowed_definition(checker: &Checker, class: &ast::StmtClassDef) {
    let functions: Vec<&ast::StmtFunctionDef> = class
        .body
        .iter()
        .filter_map(|statement| match statement {
            Stmt::FunctionDef(function) => Some(function),
            _ => None,
        })
        .collect();
    let dispatchers: FxHashSet<&str> = functions
        .iter()
        .filter(|function| decorator_names(function).contains("singledispatchmethod"))
        .map(|function| function.name.as_str())
        .collect();
    let mut definitions: Vec<(&str, Vec<&ast::StmtFunctionDef>)> = Vec::new();
    for function in functions {
        let name = function.name.as_str();
        // `@f.register` hands back the plain function, so an implementation takes
        // a throwaway name; under `f` itself it replaces the dispatcher.
        if !dispatchers.contains(name)
            && registers_on(function).any(|owner| dispatchers.contains(owner))
        {
            continue;
        }
        match definitions.iter_mut().find(|(known, _)| *known == name) {
            Some((_, found)) => found.push(function),
            None => definitions.push((name, vec![function])),
        }
    }
    for (name, found) in definitions {
        let [earlier @ .., last] = found.as_slice() else {
            continue;
        };
        if earlier.is_empty() || is_legitimate(&found) {
            continue;
        }
        let shadowed = earlier
            .iter()
            .map(|function| {
                checker
                    .compute_source_row(def_start(function, checker.source()))
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(", ");
        let start = def_start(last, checker.source());
        checker.report_diagnostic(
            ShadowedDefinition {
                class: class.name.to_string(),
                name: name.to_string(),
                shadowed,
            },
            TextRange::new(start, last.name.end()),
        );
    }
}
