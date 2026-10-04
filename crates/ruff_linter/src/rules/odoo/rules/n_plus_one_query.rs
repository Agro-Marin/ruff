use rustc_hash::FxHashSet;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, Expr, Stmt, Suite};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::is_test_path;

/// ## What it does
/// Checks for an ORM read issued once per iteration of a loop or comprehension.
///
/// ## Why is this bad?
/// Each iteration costs a round trip to the database; the query hoisted before
/// the loop and indexed in memory costs one.
///
/// Only six read methods are seen (`search`, `search_count`, `search_fetch`,
/// `search_read`, `name_search`, `_read_group`): a write per iteration, or a query
/// reached through a helper, is not.
///
/// ## Example
/// ```python
/// for partner in partners:
///     orders = self.env["sale.order"].search([("partner_id", "=", partner.id)])
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Performance)]
pub(crate) struct NPlusOneQuery {
    method: String,
}

impl Violation for NPlusOneQuery {
    #[derive_message_formats]
    fn message(&self) -> String {
        let NPlusOneQuery { method } = self;
        format!("ORM query `{method}()` inside a loop: potential N+1")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Hoist the query before the loop and index the result in memory".to_string())
    }
}

const QUERY_METHODS: &[&str] = &[
    "search",
    "search_count",
    "search_fetch",
    "search_read",
    "name_search",
    "_read_group",
];

const RECORDSET_METHODS: &[&str] = &[
    "sudo",
    "with_context",
    "with_user",
    "with_company",
    "with_env",
    "browse",
    "exists",
    "filtered",
    "filtered_domain",
    "mapped",
    "sorted",
    "union",
    "concat",
];

/// Python's `name[0].isupper() and not name.isupper()`.
fn is_class_like(name: &str) -> bool {
    let Some(first) = name.chars().next() else {
        return false;
    };
    let first_upper = first.is_uppercase();
    let cased = name
        .chars()
        .filter(|c| c.is_uppercase() || c.is_lowercase());
    let mut any_cased = false;
    let mut all_upper = true;
    for c in cased {
        any_cased = true;
        all_upper &= c.is_uppercase();
    }
    first_upper && !(any_cased && all_upper)
}

fn has_self_root(expr: &Expr) -> bool {
    match expr {
        Expr::Name(name) => name.id.as_str() == "self",
        Expr::Attribute(attribute) => has_self_root(&attribute.value),
        Expr::Subscript(subscript) => has_self_root(&subscript.value),
        Expr::Call(call) => has_self_root(&call.func),
        _ => false,
    }
}

fn looks_like_orm_receiver(expr: &Expr) -> bool {
    match expr {
        Expr::Subscript(subscript) if matches!(&*subscript.value, Expr::Attribute(a) if a.attr.as_str() == "env") => {
            true
        }
        Expr::Name(name) if name.id.as_str() == "self" => true,
        Expr::Attribute(_) | Expr::Subscript(_) => has_self_root(expr),
        Expr::Name(name) => is_class_like(name.id.as_str()),
        Expr::Call(call) => match &*call.func {
            Expr::Attribute(attribute) => {
                RECORDSET_METHODS.contains(&attribute.attr.as_str())
                    || looks_like_orm_receiver(&attribute.value)
            }
            _ => false,
        },
        _ => false,
    }
}

/// `env["model"]` on a local environment, as a hook or a script holds it.
fn is_env_model(expr: &Expr) -> bool {
    match expr {
        Expr::Subscript(subscript) => {
            matches!(&*subscript.value, Expr::Name(name) if name.id.as_str() == "env")
        }
        Expr::Call(call) => match &*call.func {
            Expr::Attribute(attribute) => {
                RECORDSET_METHODS.contains(&attribute.attr.as_str())
                    && is_env_model(&attribute.value)
            }
            _ => false,
        },
        _ => false,
    }
}

/// A loop over a recordset runs its body once per record; a loop over a list, a
/// dict or a range is the caller's own business.
fn iterates_records(expr: &Expr) -> bool {
    if let Expr::Call(call) = expr
        && let Expr::Attribute(attribute) = &*call.func
        && (QUERY_METHODS.contains(&attribute.attr.as_str()) || attribute.attr.as_str() == "browse")
    {
        return looks_like_orm_receiver(&attribute.value) || is_env_model(&attribute.value);
    }
    looks_like_orm_receiver(expr) || is_env_model(expr)
}

/// The query method a call invokes, and where its name starts.
fn query_call(call: &ast::ExprCall, over_records: bool) -> Option<(&str, TextSize)> {
    let Expr::Attribute(attribute) = &*call.func else {
        return None;
    };
    let attr = attribute.attr.as_str();
    if !QUERY_METHODS.contains(&attr) {
        return None;
    }
    if attr != "search"
        || looks_like_orm_receiver(&attribute.value)
        || (over_records && is_env_model(&attribute.value))
    {
        Some((attr, attribute.attr.range().start()))
    } else {
        None
    }
}

fn address<T>(node: &T) -> usize {
    std::ptr::from_ref(node) as usize
}

/// What runs per iteration: collects the query calls under a loop body, marks
/// the loops it meets as nested, and stops at function, class and lambda bodies.
struct Collector<'s> {
    nested: &'s mut FxHashSet<usize>,
    found: &'s mut Vec<(TextRange, TextSize, String)>,
    over_records: bool,
}

impl<'a> Visitor<'a> for Collector<'_> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            Stmt::For(_) => {
                self.nested.insert(address(stmt));
                visitor::walk_stmt(self, stmt);
            }
            _ => visitor::walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Lambda(_) => {}
            Expr::ListComp(_) | Expr::SetComp(_) | Expr::Generator(_) | Expr::DictComp(_) => {
                self.nested.insert(address(expr));
                visitor::walk_expr(self, expr);
            }
            Expr::Call(call) => {
                if let Some((method, name_start)) = query_call(call, self.over_records) {
                    self.found
                        .push((call.range(), name_start, method.to_string()));
                }
                visitor::walk_expr(self, expr);
            }
            _ => visitor::walk_expr(self, expr),
        }
    }
}

/// Visits every node, and treats each loop or record comprehension that no
/// enclosing loop already covered.
#[derive(Default)]
struct Walker {
    nested: FxHashSet<usize>,
    found: Vec<(TextRange, TextSize, String)>,
}

impl Walker {
    fn collect_stmt(&mut self, stmt: &Stmt, over_records: bool) {
        let mut collector = Collector {
            nested: &mut self.nested,
            found: &mut self.found,
            over_records,
        };
        collector.visit_stmt(stmt);
    }

    fn collect_expr(&mut self, expr: &Expr, over_records: bool) {
        let mut collector = Collector {
            nested: &mut self.nested,
            found: &mut self.found,
            over_records,
        };
        collector.visit_expr(expr);
    }
}

impl<'a> Visitor<'a> for Walker {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let Stmt::For(for_stmt) = stmt
            && !self.nested.contains(&address(stmt))
        {
            let over_records = iterates_records(&for_stmt.iter);
            for body_stmt in &for_stmt.body {
                self.collect_stmt(body_stmt, over_records);
            }
        }
        visitor::walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if !self.nested.contains(&address(expr)) {
            let parts: Option<(&[ast::Comprehension], Vec<&Expr>)> = match expr {
                Expr::ListComp(c) => Some((&c.generators, vec![&*c.elt])),
                Expr::SetComp(c) => Some((&c.generators, vec![&*c.elt])),
                Expr::Generator(c) => Some((&c.generators, vec![&*c.elt])),
                Expr::DictComp(c) => Some((
                    &c.generators,
                    c.key
                        .as_deref()
                        .into_iter()
                        .chain(std::iter::once(&*c.value))
                        .collect(),
                )),
                _ => None,
            };
            // What runs per record: the element, the conditions and the iterables
            // of the inner generators.
            if let Some((generators, elements)) = parts
                && let Some((first, inner)) = generators.split_first()
                && iterates_records(&first.iter)
            {
                let mut per_record = elements;
                per_record.extend(first.ifs.iter());
                for generator in inner {
                    per_record.push(&generator.iter);
                    per_record.extend(generator.ifs.iter());
                }
                for part in per_record {
                    self.collect_expr(part, true);
                }
            }
        }
        visitor::walk_expr(self, expr);
    }
}

/// E8507
pub(crate) fn n_plus_one_query(checker: &Checker, suite: &Suite) {
    if is_test_path(checker.path()) {
        return;
    }
    let mut walker = Walker::default();
    walker.visit_body(suite);
    // The finding spans the whole call, but a chained call is usually broken
    // across lines, and its `noqa` sits beside the method name:
    //     records = (
    //         self.env["res.partner"]
    //         .search(domain)  # noqa: E8507  one query per company
    //     )
    for (range, name_start, method) in walker.found {
        let mut diagnostic = checker.report_diagnostic(NPlusOneQuery { method }, range);
        diagnostic.set_parent(name_start);
    }
}
