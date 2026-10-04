use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, InterpolatedStringElement, Operator};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::is_test_path;

/// ## What it does
/// Checks for `Markup()` wrapping a string that was already interpolated: a
/// concatenation, a gettext call given values, a `%` or `.format()` result, an
/// f-string.
///
/// ## Why is this bad?
/// `Markup(env._("...", x=value))`, `Markup("..." % value)` and
/// `Markup(f"...{value}...")` interpolate first and then vouch for the result,
/// so a value carrying markup reaches the page as markup. Wrapping the
/// template, then formatting it, escapes every value:
/// `Markup(env._("... %(x)s ...")) % {"x": value}` or
/// `Markup("<b>{}</b>").format(value)`.
///
/// A value that is a constant, or already escaped by `escape()`,
/// `html_escape()` or `Markup()`, is not reported.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Security)]
pub(crate) struct MarkupPreformatted {
    what: String,
}

impl Violation for MarkupPreformatted {
    #[derive_message_formats]
    fn message(&self) -> String {
        let MarkupPreformatted { what } = self;
        format!(
            "`Markup()` wraps {what}: the values were interpolated before the wrap, so none of them was escaped"
        )
    }

    fn fix_title(&self) -> Option<String> {
        Some("Wrap the template, then format it".to_string())
    }
}

fn called_name(expr: &Expr) -> &str {
    match expr {
        Expr::Call(call) => match &*call.func {
            Expr::Name(name) => name.id.as_str(),
            Expr::Attribute(attribute) => attribute.attr.as_str(),
            _ => "",
        },
        _ => "",
    }
}

fn is_markup(func: &Expr) -> bool {
    match func {
        Expr::Name(name) => name.id.as_str() == "Markup",
        Expr::Attribute(attribute) => attribute.attr.as_str() == "Markup",
        _ => false,
    }
}

fn is_gettext(expr: &Expr) -> bool {
    match expr {
        Expr::Call(call) => match &*call.func {
            Expr::Name(name) => matches!(name.id.as_str(), "_" | "_lt"),
            Expr::Attribute(attribute) => attribute.attr.as_str() == "_",
            _ => false,
        },
        _ => false,
    }
}

fn is_escaped(value: &Expr) -> bool {
    match value {
        value if value.is_literal_expr() => true,
        Expr::If(ast::ExprIf { body, orelse, .. }) => is_escaped(body) && is_escaped(orelse),
        _ => matches!(called_name(value), "escape" | "html_escape" | "Markup"),
    }
}

fn mod_operands(right: &Expr) -> Vec<&Expr> {
    match right {
        Expr::Tuple(tuple) => tuple.elts.iter().collect(),
        Expr::Dict(dict) if dict.items.iter().all(|item| item.key.is_some()) => {
            dict.items.iter().map(|item| &item.value).collect()
        }
        _ => vec![right],
    }
}

fn concatenated<'a>(expr: &'a Expr, operands: &mut Vec<&'a Expr>) {
    match expr {
        Expr::BinOp(ast::ExprBinOp {
            left,
            op: Operator::Add,
            right,
            ..
        }) => {
            concatenated(left, operands);
            concatenated(right, operands);
        }
        _ => operands.push(expr),
    }
}

fn call_values(call: &ast::ExprCall, skip: usize) -> Vec<&Expr> {
    call.arguments
        .args
        .iter()
        .skip(skip)
        .chain(call.arguments.keywords.iter().map(|keyword| &keyword.value))
        .collect()
}

/// What the argument interpolated, and the values it interpolated.
fn interpolated(argument: &Expr) -> Option<(&'static str, Vec<&Expr>)> {
    match argument {
        Expr::BinOp(ast::ExprBinOp {
            op: Operator::Add, ..
        }) => {
            let mut operands = Vec::new();
            concatenated(argument, &mut operands);
            Some(("a concatenation", operands))
        }
        Expr::Call(call) if is_gettext(argument) && !call.arguments.args.is_empty() => {
            let values = call_values(call, 1);
            (!values.is_empty()).then_some(("a gettext call", values))
        }
        Expr::BinOp(ast::ExprBinOp {
            left,
            op: Operator::Mod,
            right,
            ..
        }) => Some((
            if is_gettext(left) {
                "a gettext result formatted with %"
            } else {
                "a string formatted with %"
            },
            mod_operands(right),
        )),
        Expr::Call(call)
            if matches!(&*call.func, Expr::Attribute(attribute) if attribute.attr.as_str() == "format")
                && !(call.arguments.args.is_empty() && call.arguments.keywords.is_empty()) =>
        {
            Some(("str.format", call_values(call, 0)))
        }
        Expr::FString(fstring) => Some((
            "an f-string",
            fstring
                .value
                .elements()
                .filter_map(|element| match element {
                    InterpolatedStringElement::Interpolation(interpolation) => {
                        Some(&*interpolation.expression)
                    }
                    InterpolatedStringElement::Literal(_) => None,
                })
                .collect(),
        )),
        _ => None,
    }
}

/// E8538
pub(crate) fn markup_preformatted(checker: &Checker, call: &ast::ExprCall) {
    if !is_markup(&call.func) || is_test_path(checker.path()) {
        return;
    }
    let Some(argument) = call.arguments.args.first() else {
        return;
    };
    if let Some((what, values)) = interpolated(argument)
        && !values.into_iter().all(is_escaped)
    {
        checker.report_diagnostic(
            MarkupPreformatted {
                what: what.to_string(),
            },
            call.range(),
        );
    }
}
