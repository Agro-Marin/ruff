//! How the Odoo rules read a model class: its attributes, the models it
//! inherits, and its field declarations.
use ruff_python_ast::{self as ast, Expr, Stmt};

/// The strings of an attribute value, as `ast.literal_eval` would read it: a
/// string, or a list or tuple whose every element is a literal. Anything that is
/// not wholly literal reads as nothing, as `literal_eval` raises on it.
pub(crate) fn names(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::StringLiteral(string) => vec![string.value.to_str().to_string()],
        Expr::List(ast::ExprList { elts, .. }) | Expr::Tuple(ast::ExprTuple { elts, .. }) => {
            if !elts.iter().all(is_literal) {
                return Vec::new();
            }
            elts.iter()
                .filter_map(|elt| match elt {
                    Expr::StringLiteral(string) => Some(string.value.to_str().to_string()),
                    _ => None,
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Whether `ast.literal_eval` accepts the expression.
pub(crate) fn is_literal(expr: &Expr) -> bool {
    match expr {
        Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::EllipsisLiteral(_) => true,
        Expr::List(ast::ExprList { elts, .. })
        | Expr::Tuple(ast::ExprTuple { elts, .. })
        | Expr::Set(ast::ExprSet { elts, .. }) => elts.iter().all(is_literal),
        Expr::Dict(dict) => dict
            .items
            .iter()
            .all(|item| item.key.as_ref().is_some_and(is_literal) && is_literal(&item.value)),
        Expr::UnaryOp(ast::ExprUnaryOp { op, operand, .. }) => {
            matches!(op, ast::UnaryOp::USub | ast::UnaryOp::UAdd)
                && matches!(&**operand, Expr::NumberLiteral(_))
        }
        Expr::BinOp(ast::ExprBinOp {
            left, op, right, ..
        }) => {
            matches!(op, ast::Operator::Add | ast::Operator::Sub)
                && is_literal(left)
                && matches!(&**right, Expr::NumberLiteral(_))
        }
        _ => false,
    }
}

/// `fields.<Capitalised>(...)`.
pub(crate) fn field_call(expr: &Expr) -> Option<&ast::ExprCall> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let Expr::Attribute(attribute) = &*call.func else {
        return None;
    };
    let is_fields = matches!(&*attribute.value, Expr::Name(name) if name.id.as_str() == "fields");
    let capitalised = attribute
        .attr
        .as_str()
        .chars()
        .next()
        .is_some_and(char::is_uppercase);
    (is_fields && capitalised).then_some(call)
}

/// The field type of a `fields.X(...)` call: `X`.
pub(crate) fn field_type(call: &ast::ExprCall) -> &str {
    match &*call.func {
        Expr::Attribute(attribute) => attribute.attr.as_str(),
        _ => "",
    }
}

/// `name = value` and `name: annotation = value`, in declaration order.
pub(crate) fn class_attributes(
    class: &ast::StmtClassDef,
) -> impl Iterator<Item = (&str, &Expr, &Stmt)> {
    class.body.iter().filter_map(|statement| match statement {
        Stmt::Assign(ast::StmtAssign { targets, value, .. }) => match targets.as_slice() {
            [Expr::Name(name)] => Some((name.id.as_str(), &**value, statement)),
            _ => None,
        },
        Stmt::AnnAssign(ast::StmtAnnAssign {
            target,
            value: Some(value),
            ..
        }) => match &**target {
            Expr::Name(name) => Some((name.id.as_str(), &**value, statement)),
            _ => None,
        },
        _ => None,
    })
}

/// The models the class's first `_inherit` names.
pub(crate) fn inherited(class: &ast::StmtClassDef) -> Vec<String> {
    class_attributes(class)
        .find(|(name, ..)| *name == "_inherit")
        .map(|(_, value, _)| names(value))
        .unwrap_or_default()
}

/// The fields the class declares: public names bound to a `fields.X(...)` call.
pub(crate) fn field_declarations(
    class: &ast::StmtClassDef,
) -> impl Iterator<Item = (&str, &ast::ExprCall, &Stmt)> {
    class_attributes(class).filter_map(|(name, value, statement)| {
        if name.starts_with('_') {
            return None;
        }
        field_call(value).map(|call| (name, call, statement))
    })
}
