use std::sync::LazyLock;

use regex::Regex;
use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, CmpOp, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{in_addon, is_constant_side, is_test_path};

/// ## What it does
/// Checks for a bearer token that opens a record (`token`, `access_token`,
/// `booking_token`...) compared with `==`/`!=`, `consteq` or `compare_digest`,
/// or a record searched for by such a token.
///
/// ## Why is this bad?
/// A token compared or searched for outside `access.link` is a capability
/// nobody can list, expire or revoke. A route declares `auth="link"` and reads
/// `request.link_subject`; other code calls
/// `env['access.link']._resolve(token, ...)`, which hashes the token, looks it
/// up by index and checks expiry, revocation, audience and role in one place.
/// A stateless signed token is checked by `tools.is_hmac_valid` on a registered
/// scope.
///
/// A device's or a vendor's credential (`*_api_token`, `oauth_*`, `push_token`...)
/// is not a link. A token that is no link at a reported site (a device's
/// credential, an inbound signature, a public identifier) takes a `noqa` naming
/// its class.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Security)]
pub(crate) struct LinkTokenCompare {
    what: String,
}

impl Violation for LinkTokenCompare {
    #[derive_message_formats]
    fn message(&self) -> String {
        let LinkTokenCompare { what } = self;
        format!("{what} outside `access.link`'s resolver")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Open the record through `access.link`".to_string())
    }
}

static LINK_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|_)token$").expect("valid regex"));
static NOT_A_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(api|oauth|refresh|csrf|push|sync|page|next|bot|iot)_token$|^oauth")
        .expect("valid regex")
});

const COMPARES: &[&str] = &["consteq", "compare_digest"];
const LOOKUPS: &[&str] = &[
    "search",
    "search_count",
    "search_fetch",
    "search_read",
    "_search",
];

/// A name that holds a bearer token opening a record.
pub(crate) fn is_link_token(name: &str) -> bool {
    LINK_TOKEN.is_match(name) && !NOT_A_LINK.is_match(name)
}

/// The last name an expression spells, through calls: `token` for
/// `self.access_token` and `record._get_token()`.
fn terminal(expr: &Expr) -> &str {
    match expr {
        Expr::Call(call) => terminal(&call.func),
        Expr::Attribute(attribute) => attribute.attr.as_str(),
        Expr::Name(name) => name.id.as_str(),
        _ => "",
    }
}

/// The stored side says what the secret is: a value compared with a push
/// subscription's token is that credential, whatever the argument is called.
fn compares_link_tokens<'a>(sides: impl Iterator<Item = &'a Expr> + Clone) -> bool {
    sides.clone().any(|side| is_link_token(terminal(side)))
        && !sides
            .into_iter()
            .any(|side| NOT_A_LINK.is_match(terminal(side)))
}

fn compares_with_equality(compare: &ast::ExprCompare) -> bool {
    let ([op], [right]) = (&*compare.ops, &*compare.comparators) else {
        return false;
    };
    if !matches!(op, CmpOp::Eq | CmpOp::NotEq) {
        return false;
    }
    let sides = [&*compare.left, right];
    if sides.iter().any(|side| is_constant_side(side)) {
        return false;
    }
    compares_link_tokens(sides.into_iter().filter(|side| !side.is_call_expr()))
}

/// A `(field, "=" or "in", value)` leaf whose field is a link token, unless the
/// value is `None` or `False`: rows without a token are no token's lookup.
#[derive(Default)]
struct LookedUpField {
    found: Option<String>,
}

impl<'a> Visitor<'a> for LookedUpField {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if self.found.is_some() {
            return;
        }
        if let Expr::Tuple(tuple) = expr
            && let [
                Expr::StringLiteral(field),
                Expr::StringLiteral(operator),
                value,
            ] = &*tuple.elts
            && matches!(operator.value.to_str(), "=" | "in")
            && is_link_token(field.value.to_str().rsplit('.').next().unwrap_or_default())
            && !matches!(
                value,
                Expr::NoneLiteral(_)
                    | Expr::BooleanLiteral(ast::ExprBooleanLiteral { value: false, .. })
            )
        {
            self.found = Some(field.value.to_str().to_string());
            return;
        }
        visitor::walk_expr(self, expr);
    }
}

/// E8537, at a comparison.
pub(crate) fn link_token_compare(checker: &Checker, compare: &ast::ExprCompare) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) || !compares_with_equality(compare) {
        return;
    }
    checker.report_diagnostic(
        LinkTokenCompare {
            what: "A bearer token compared with `==`/`!=`".to_string(),
        },
        compare.range(),
    );
}

/// E8537, at a call.
pub(crate) fn link_token_call(checker: &Checker, call: &ast::ExprCall) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) {
        return;
    }
    let name = terminal(&call.func);
    let what = if COMPARES.contains(&name) && compares_link_tokens(call.arguments.args.iter()) {
        "A bearer token compared".to_string()
    } else if LOOKUPS.contains(&name)
        && let Some(first) = call.arguments.args.first()
    {
        let mut lookup = LookedUpField::default();
        lookup.visit_expr(first);
        let Some(field) = lookup.found else {
            return;
        };
        format!("A record looked up by its bearer token (`{field}`)")
    } else {
        return;
    };
    checker.report_diagnostic(LinkTokenCompare { what }, call.range());
}
