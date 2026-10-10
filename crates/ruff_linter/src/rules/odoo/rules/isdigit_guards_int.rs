use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::comparable::ComparableExpr;
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, BoolOp, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for `x.isdigit()` where the same function then converts `int(x)` or
/// `int(x[i])` (or `str(x).isdigit()` guarding `int(x)`), with no `x.isascii()`
/// beside it.
///
/// ## Why is this bad?
/// `str.isdigit()` is true for 128 characters `int()` rejects: superscripts
/// (`²`), circled and parenthesized digits (`①`, `⑴`), the rest of Unicode
/// category No. Such a string passes the guard and `int()` raises
/// `ValueError`: on a route, a 500 from one query parameter. `isdecimal()` is
/// exactly the set of characters `int()` accepts, so it keeps every string that
/// converted and refuses the others instead of raising.
///
/// ## Example
/// ```python
/// ids = [int(part) for part in raw.split(",") if part.isdigit()]
/// ```
///
/// Use instead:
/// ```python
/// ids = [int(part) for part in raw.split(",") if part.isdecimal()]
/// ```
///
/// ## Fix safety
/// The fix is unsafe: a string of those 128 characters is refused where it used
/// to raise, which a caller catching the `ValueError` would notice.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.10", category = Category::Correctness)]
pub(crate) struct IsdigitGuardsInt {
    value: String,
}

impl Violation for IsdigitGuardsInt {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let IsdigitGuardsInt { value } = self;
        format!("`{value}.isdigit()` admits characters `int()` rejects")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Use `isdecimal()`, the characters `int()` accepts".to_string())
    }
}

/// What `int()` must be handed for this guard to protect it: the receiver,
/// and for `str(x)` or `str(x or default)`, `x`.
fn guarded(receiver: &Expr) -> Vec<&Expr> {
    let mut values = vec![receiver];
    if let Expr::Call(call) = receiver
        && matches!(&*call.func, Expr::Name(name) if name.id.as_str() == "str")
        && let [argument] = &*call.arguments.args
        && call.arguments.keywords.is_empty()
    {
        values.push(argument);
        if let Expr::BoolOp(bool_op) = argument
            && bool_op.op == BoolOp::Or
            && let Some(first) = bool_op.values.first()
        {
            values.push(first);
        }
    }
    values
}

fn method_receiver<'a>(call: &'a ast::ExprCall, method: &str) -> Option<&'a Expr> {
    if call.arguments.is_empty()
        && let Expr::Attribute(attribute) = &*call.func
        && attribute.attr.as_str() == method
    {
        Some(&attribute.value)
    } else {
        None
    }
}

struct Conversions<'r> {
    guarded: Vec<ComparableExpr<'r>>,
    receiver: ComparableExpr<'r>,
    converted: bool,
    ascii_checked: bool,
}

impl Conversions<'_> {
    /// `int(x)`, or `int(x[i])`: a character or a slice of a guarded string.
    fn guards(&self, argument: &Expr) -> bool {
        let indexed = argument
            .as_subscript_expr()
            .map(|subscript| &*subscript.value);
        std::iter::once(argument).chain(indexed).any(|converted| {
            let converted = ComparableExpr::from(converted);
            self.guarded.contains(&converted)
        })
    }
}

impl<'a> Visitor<'a> for Conversions<'_> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr
            && matches!(&*call.func, Expr::Name(name) if name.id.as_str() == "int")
            && let [argument] = &*call.arguments.args
            && self.guards(argument)
        {
            self.converted = true;
        }
        if let Expr::Call(call) = expr
            && let Some(value) = method_receiver(call, "isascii")
            && ComparableExpr::from(value) == self.receiver
        {
            self.ascii_checked = true;
        }
        visitor::walk_expr(self, expr);
    }
}

/// E8545
pub(crate) fn isdigit_guards_int(checker: &Checker, call: &ast::ExprCall) {
    let Some(receiver) = method_receiver(call, "isdigit") else {
        return;
    };
    let semantic = checker.semantic();
    let scope: &[Stmt] = match semantic
        .current_statements()
        .find_map(|statement| statement.as_function_def_stmt())
    {
        Some(function) => &function.body,
        None => match semantic.current_statements().last() {
            Some(statement) => std::slice::from_ref(statement),
            None => return,
        },
    };
    let mut conversions = Conversions {
        guarded: guarded(receiver)
            .into_iter()
            .map(ComparableExpr::from)
            .collect(),
        receiver: ComparableExpr::from(receiver),
        converted: false,
        ascii_checked: false,
    };
    conversions.visit_body(scope);
    if !conversions.converted || conversions.ascii_checked {
        return;
    }
    let Expr::Attribute(attribute) = &*call.func else {
        return;
    };
    let mut diagnostic = checker.report_diagnostic(
        IsdigitGuardsInt {
            value: checker.locator().slice(receiver).to_string(),
        },
        attribute.attr.range(),
    );
    diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
        "isdecimal".to_string(),
        attribute.attr.range(),
    )));
}
