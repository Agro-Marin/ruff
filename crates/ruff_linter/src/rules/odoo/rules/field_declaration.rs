use rustc_hash::FxHashMap;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_source_file::SourceRow;
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::{Category, Rule};
use crate::rules::odoo::helpers::is_test_path;
use crate::rules::odoo::model::{field_call, field_type};

/// ## What it does
/// Checks for a class attribute assigned again after a field declaration, or a
/// field declared over an earlier assignment of the same name.
///
/// ## Why is this bad?
/// The class body keeps the last assignment: the earlier declaration is dead
/// while it still reads as the one in force.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct FieldRedeclared {
    class: String,
    name: String,
    row: SourceRow,
}

impl Violation for FieldRedeclared {
    #[derive_message_formats]
    fn message(&self) -> String {
        let FieldRedeclared { class, name, row } = self;
        format!("`{class}.{name}` is declared again here, so the declaration at {row} is dead")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Declare the field once".to_string())
    }
}

/// ## What it does
/// Checks for a field `default=` that calls a function whose value is meant
/// per record, such as `fields.Date.today()` or `uuid4()`.
///
/// ## Why is this bad?
/// A call in the declaration runs once, when the module is imported, and every
/// record created afterwards gets that same value.
///
/// ## Example
/// ```python
/// date = fields.Date(default=fields.Date.today())
/// ```
///
/// Use instead:
/// ```python
/// date = fields.Date(default=fields.Date.today)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct DefaultEvaluatedAtImport {
    name: String,
    default: String,
    callable: String,
}

impl Violation for DefaultEvaluatedAtImport {
    #[derive_message_formats]
    fn message(&self) -> String {
        let DefaultEvaluatedAtImport { name, default, .. } = self;
        format!("`{name}`: `default={default}` ran once, when the module was imported")
    }

    fn fix_title(&self) -> Option<String> {
        let DefaultEvaluatedAtImport { callable, .. } = self;
        Some(format!("Pass the callable: `default={callable}`"))
    }
}

/// ## What it does
/// Checks for a selection key listed more than once in a literal selection.
///
/// ## Why is this bad?
/// `Selection` stores the list as a dict: the last label wins and the others
/// are dead.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct SelectionDuplicateKey {
    name: String,
    key: String,
    count: usize,
}

impl Violation for SelectionDuplicateKey {
    #[derive_message_formats]
    fn message(&self) -> String {
        let SelectionDuplicateKey { name, key, count } = self;
        format!("`{name}`: selection key `{key}` appears {count} times")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Give each selection key one label".to_string())
    }
}

/// ## What it does
/// Checks for a `compute=`, `inverse=`, `search=` or `selection=` method name
/// outside the family its attribute reserves: `_compute_*`, `_inverse_*`,
/// `_search_*`, `_selection_*`.
///
/// ## Why is this bad?
/// A hook named for its family tells a reader, and the naming gates, a field
/// hook from a helper.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Style)]
pub(crate) struct FieldHookPrefix {
    name: String,
    attribute: &'static str,
    method: String,
    prefix: &'static str,
}

impl Violation for FieldHookPrefix {
    #[derive_message_formats]
    fn message(&self) -> String {
        let FieldHookPrefix {
            name,
            attribute,
            method,
            prefix,
        } = self;
        format!(
            "`{name}`: `{attribute}=\"{method}\"` names a method outside the `{prefix}*` family"
        )
    }

    fn fix_title(&self) -> Option<String> {
        let FieldHookPrefix { prefix, .. } = self;
        Some(format!("Rename the method to `{prefix}*`"))
    }
}

/// ## What it does
/// Checks for a field declaration that passes an argument by position.
///
/// ## Why is this bad?
/// A positional label, comodel or selection reads as a bare string, and only
/// the field class's signature says which it is.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Style)]
pub(crate) struct FieldPositionalArgument {
    name: String,
    count: usize,
    spelled: String,
}

impl Violation for FieldPositionalArgument {
    #[derive_message_formats]
    fn message(&self) -> String {
        let FieldPositionalArgument { name, count, .. } = self;
        format!("`{name}`: {count} positional argument(s)")
    }

    fn fix_title(&self) -> Option<String> {
        let FieldPositionalArgument { spelled, .. } = self;
        Some(format!("Spell them as {spelled}"))
    }
}

/// ## What it does
/// Checks that a field declaration's keywords follow the canonical attribute
/// order, one per line once there are two, none on the line of the call.
///
/// ## Why is this bad?
/// One order reads the same in every model: what the field is, what it says,
/// its shape, how its value is produced, how it is stored, what it points at,
/// who tracks it, then `groups=` and `help=` last.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Style)]
pub(crate) struct FieldAttributeOrder {
    name: String,
    problem: String,
}

impl Violation for FieldAttributeOrder {
    #[derive_message_formats]
    fn message(&self) -> String {
        let FieldAttributeOrder { name, problem } = self;
        format!("`{name}`: {problem}")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Run `_sort_field_attributes.py`".to_string())
    }
}

/// ## What it does
/// Checks for a field attribute the field's setup ignores: `index=` without a
/// column, `precompute=` without `store=True`, `compute=` beside `related=`.
///
/// ## Why is this bad?
/// The attribute reads as in force and does nothing.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct DeadFieldAttribute {
    name: String,
    problem: &'static str,
}

impl Violation for DeadFieldAttribute {
    #[derive_message_formats]
    fn message(&self) -> String {
        let DeadFieldAttribute { name, problem } = self;
        format!("`{name}`: {problem}")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Drop the attribute".to_string())
    }
}

/// ## What it does
/// Checks for a related field declared with `store=True`.
///
/// ## Why is this bad?
/// A related field over many2one hops filters, groups, sorts and aggregates
/// through the join; a stored copy is a second column kept in sync on every
/// write of the source. A copy that carries a composite index or a `UNIQUE`
/// stays, with a `noqa` naming what needs the column.
///
/// `Binary` and `Image` fields are not reported: `Image(related="image_1920",
/// max_width=128, store=True)` stores a resize, not a copy.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Performance)]
pub(crate) struct StoredRelated {
    name: String,
    related: String,
}

impl Violation for StoredRelated {
    #[derive_message_formats]
    fn message(&self) -> String {
        let StoredRelated { name, related } = self;
        format!("`{name}`: a stored copy of `{related}`")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Drop `store=True`".to_string())
    }
}

const HOOK_PREFIXES: [(&str, &str); 4] = [
    ("compute", "_compute_"),
    ("inverse", "_inverse_"),
    ("search", "_search_"),
    ("selection", "_selection_"),
];

/// Calls whose value is meant per record, so `default=` wants the callable, not
/// what it returned once at import.
const CALLED_ONCE_AT_IMPORT: &[&str] = &[
    "today",
    "now",
    "utcnow",
    "context_today",
    "context_now",
    "uuid4",
    "uuid1",
    "token_hex",
    "token_urlsafe",
    "token_bytes",
    "random",
    "randint",
    "choice",
    "time",
    "_",
];

/// The order a declaration's keywords are read in. An attribute the table does
/// not know sorts alphabetically before the tail: `write_groups`, `groups`,
/// `help`.
const FIELD_ATTRIBUTE_ORDER: &[&str] = &[
    "comodel_name",
    "inverse_name",
    "relation",
    "column1",
    "column2",
    "selection",
    "selection_add",
    "related",
    "count_of",
    "model_field",
    "definition",
    "definition_record",
    "definition_record_field",
    "delegate",
    "string",
    "export_string_translation",
    "size",
    "trim",
    "digits",
    "min_display_digits",
    "currency_field",
    "translate",
    "sanitize",
    "sanitize_overridable",
    "sanitize_tags",
    "sanitize_attributes",
    "sanitize_style",
    "sanitize_form",
    "sanitize_conditional_comments",
    "sanitize_output_method",
    "strip_style",
    "strip_classes",
    "attachment",
    "max_width",
    "max_height",
    "verify_resolution",
    "bin_size_field",
    "validate",
    "compute",
    "inverse",
    "search",
    "depends",
    "depends_context",
    "precompute",
    "compute_sudo",
    "related_sudo",
    "recursive",
    "inherited",
    "default",
    "change_default",
    "store",
    "index",
    "copy",
    "readonly",
    "required",
    "prefetch",
    "exportable",
    "company_dependent",
    "config_parameter",
    "aggregator",
    "group_expand",
    "falsy_value",
    "falsy_value_label",
    "domain",
    "context",
    "ondelete",
    "check_company",
    "bypass_search_access",
    "auto_join",
    "tracking",
    "implied_group",
    "write_groups",
    "groups",
    "help",
];
const TAIL: &[&str] = &["write_groups", "groups", "help"];

/// The positional parameters of each field class's `__init__`, in order; every
/// other class takes the label first.
fn positional_names(kind: &str) -> &'static [&'static str] {
    match kind {
        "Many2one" => &["comodel_name", "string"],
        "One2many" | "One2one" => &["comodel_name", "inverse_name", "string"],
        "Many2many" => &["comodel_name", "relation", "column1", "column2", "string"],
        "Selection" | "Reference" => &["selection", "string"],
        "Count" => &["count_of", "string"],
        "Float" => &["string", "digits", "min_display_digits"],
        "Monetary" => &["string", "currency_field"],
        _ => &["string"],
    }
}

fn rank(name: &str) -> Option<usize> {
    FIELD_ATTRIBUTE_ORDER
        .iter()
        .position(|known| *known == name)
}

fn canonical_order<'a>(names: &[&'a str]) -> Vec<&'a str> {
    let mut known: Vec<&str> = names
        .iter()
        .copied()
        .filter(|name| rank(name).is_some() && !TAIL.contains(name))
        .collect();
    known.sort_by_key(|name| rank(name));
    let mut unknown: Vec<&str> = names
        .iter()
        .copied()
        .filter(|name| rank(name).is_none())
        .collect();
    unknown.sort_unstable();
    let mut tail: Vec<&str> = names
        .iter()
        .copied()
        .filter(|name| TAIL.contains(name))
        .collect();
    tail.sort_by_key(|name| rank(name));
    known.into_iter().chain(unknown).chain(tail).collect()
}

/// The keywords of a call as a dict reads them: the last of a repeated name wins.
fn keyword<'a>(call: &'a ast::ExprCall, name: &str) -> Option<&'a Expr> {
    call.arguments
        .keywords
        .iter()
        .rev()
        .find(|keyword| keyword.arg.as_ref().is_some_and(|arg| arg.as_str() == name))
        .map(|keyword| &keyword.value)
}

fn constant_str(expr: Option<&Expr>) -> Option<&str> {
    match expr {
        Some(Expr::StringLiteral(string)) => Some(string.value.to_str()),
        _ => None,
    }
}

/// Whether the expression is a constant Python reads as false: `False`, `None`,
/// a zero, an empty string or bytes.
fn is_falsy_constant(expr: &Expr) -> bool {
    match expr {
        Expr::BooleanLiteral(boolean) => !boolean.value,
        Expr::NoneLiteral(_) => true,
        Expr::NumberLiteral(number) => match &number.value {
            ast::Number::Int(int) => *int == ast::Int::ZERO,
            ast::Number::Float(float) => *float == 0.0,
            ast::Number::Complex { real, imag } => *real == 0.0 && *imag == 0.0,
        },
        Expr::StringLiteral(string) => string.value.is_empty(),
        Expr::BytesLiteral(bytes) => bytes.value.is_empty(),
        _ => false,
    }
}

fn truthy(call: &ast::ExprCall, name: &str) -> bool {
    keyword(call, name).is_some_and(|value| !is_falsy_constant(value))
}

/// The names a class-body statement binds: every plain-name target of an
/// assignment, or the target of an annotated assignment with a value.
fn bound_names(statement: &Stmt) -> Option<(Vec<&str>, &Expr)> {
    match statement {
        Stmt::Assign(ast::StmtAssign { targets, value, .. }) => Some((
            targets
                .iter()
                .filter_map(|target| match target {
                    Expr::Name(name) => Some(name.id.as_str()),
                    _ => None,
                })
                .collect(),
            &**value,
        )),
        Stmt::AnnAssign(ast::StmtAnnAssign {
            target,
            value: Some(value),
            ..
        }) => match &**target {
            Expr::Name(name) => Some((vec![name.id.as_str()], &**value)),
            _ => None,
        },
        _ => None,
    }
}

fn callee_tail(expr: &Expr) -> &str {
    match expr {
        Expr::Attribute(attribute) => attribute.attr.as_str(),
        Expr::Name(name) => name.id.as_str(),
        _ => "",
    }
}

fn check_default(checker: &Checker, name: &str, call: &ast::ExprCall) {
    if let Some(Expr::Call(default)) = keyword(call, "default")
        && CALLED_ONCE_AT_IMPORT.contains(&callee_tail(&default.func))
    {
        checker.report_diagnostic(
            DefaultEvaluatedAtImport {
                name: name.to_string(),
                default: checker.locator().slice(default).to_string(),
                callable: checker.locator().slice(&*default.func).to_string(),
            },
            default.range(),
        );
    }
}

fn check_selection(checker: &Checker, name: &str, call: &ast::ExprCall) {
    let selection = match keyword(call, "selection") {
        Some(selection) => Some(selection),
        None if field_type(call) == "Selection" => call.arguments.args.first(),
        None => None,
    };
    let Some(
        selection @ (Expr::List(ast::ExprList { elts: entries, .. })
        | Expr::Tuple(ast::ExprTuple { elts: entries, .. })),
    ) = selection
    else {
        return;
    };
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for entry in entries {
        let key = match entry {
            Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. })
                if elts.len() == 2 =>
            {
                constant_str(elts.first())
            }
            _ => None,
        };
        if let Some(key) = key {
            match counts.iter_mut().find(|(known, _)| *known == key) {
                Some(entry) => entry.1 += 1,
                None => counts.push((key, 1)),
            }
        }
    }
    for (key, count) in counts {
        if count > 1 {
            checker.report_diagnostic(
                SelectionDuplicateKey {
                    name: name.to_string(),
                    key: key.to_string(),
                    count,
                },
                selection.range(),
            );
        }
    }
}

fn check_hooks(checker: &Checker, name: &str, call: &ast::ExprCall) {
    for (attribute, prefix) in HOOK_PREFIXES {
        if let Some(method) = constant_str(keyword(call, attribute))
            && !method.starts_with(prefix)
        {
            checker.report_diagnostic(
                FieldHookPrefix {
                    name: name.to_string(),
                    attribute,
                    method: method.to_string(),
                    prefix,
                },
                call.range(),
            );
        }
    }
}

fn check_positional(checker: &Checker, name: &str, call: &ast::ExprCall) {
    let args = &call.arguments.args;
    if args.is_empty() {
        return;
    }
    let names = positional_names(field_type(call));
    let spelled = args
        .iter()
        .enumerate()
        .filter(|(_, arg)| !arg.is_starred_expr())
        .map(|(index, _)| {
            names
                .get(index)
                .map_or_else(|| "?".to_string(), |name| format!("`{name}=`"))
        })
        .collect::<Vec<_>>()
        .join(", ");
    checker.report_diagnostic(
        FieldPositionalArgument {
            name: name.to_string(),
            count: args.len(),
            spelled,
        },
        call.range(),
    );
}

fn on_one_line(checker: &Checker, start: TextSize, end: TextSize) -> bool {
    !checker
        .locator()
        .slice(TextRange::new(start, end))
        .contains(['\n', '\r'])
}

fn check_layout(checker: &Checker, name: &str, call: &ast::ExprCall) {
    let keywords = &call.arguments.keywords;
    let names: Vec<&str> = keywords
        .iter()
        .filter_map(|keyword| keyword.arg.as_ref().map(ast::Identifier::as_str))
        .collect();
    let canonical = canonical_order(&names);
    if names != canonical {
        checker.report_diagnostic(
            FieldAttributeOrder {
                name: name.to_string(),
                problem: format!(
                    "keywords read {}; the order is {}",
                    names.join(", "),
                    canonical.join(", ")
                ),
            },
            call.range(),
        );
        return;
    }
    let [first, ..] = keywords.as_slice() else {
        return;
    };
    if keywords.len() < 2 {
        return;
    }
    let crowded = on_one_line(checker, call.start(), first.start())
        || keywords
            .windows(2)
            .any(|pair| on_one_line(checker, pair[0].start(), pair[1].start()));
    if crowded {
        checker.report_diagnostic(
            FieldAttributeOrder {
                name: name.to_string(),
                problem: format!(
                    "{} keywords share a line; one per line, none on the line of the call",
                    keywords.len()
                ),
            },
            call.range(),
        );
    }
}

fn check_dead(checker: &Checker, name: &str, call: &ast::ExprCall) {
    let kind = field_type(call);
    // A declaration that states no `compute=` may be extending a stored one from
    // another module, so `store=` is judged only beside the compute it belongs to.
    let stored = truthy(call, "store");
    let computed_here = keyword(call, "compute").is_some();
    let mut problems = Vec::new();
    if truthy(call, "index")
        && (matches!(kind, "One2many" | "Many2many") || (computed_here && !stored))
    {
        problems.push("`index=` on a field with no column");
    }
    if truthy(call, "precompute") && computed_here && !stored {
        problems.push("`precompute=` without `store=True` is dropped at setup");
    }
    if truthy(call, "related") && computed_here {
        problems
            .push("`compute=` on a related field is replaced by the related path's own compute");
    }
    for problem in problems {
        checker.report_diagnostic(
            DeadFieldAttribute {
                name: name.to_string(),
                problem,
            },
            call.range(),
        );
    }
}

fn check_stored_related(checker: &Checker, name: &str, call: &ast::ExprCall) {
    if matches!(field_type(call), "Binary" | "Image") {
        return;
    }
    if let Some(related) = constant_str(keyword(call, "related"))
        && let Some(Expr::BooleanLiteral(ast::ExprBooleanLiteral { value: true, .. })) =
            keyword(call, "store")
    {
        checker.report_diagnostic(
            StoredRelated {
                name: name.to_string(),
                related: related.to_string(),
            },
            call.range(),
        );
    }
}

/// E8521, E8522, E8523, E8524, E8525, E8526, E8527, E8529
pub(crate) fn field_declaration(checker: &Checker, class: &ast::StmtClassDef) {
    if is_test_path(checker.path()) {
        return;
    }
    // Each name bound in the class body, where, and whether that binding was a field.
    let mut bound: FxHashMap<&str, (TextSize, bool)> = FxHashMap::default();
    for statement in &class.body {
        let Some((targets, value)) = bound_names(statement) else {
            continue;
        };
        if targets.is_empty() {
            continue;
        }
        let call = field_call(value);
        let is_field = call.is_some();
        for target in &targets {
            if let Some((earlier, was_field)) = bound.get(target)
                && (is_field || *was_field)
                && checker.is_rule_enabled(Rule::FieldRedeclared)
            {
                checker.report_diagnostic(
                    FieldRedeclared {
                        class: class.name.to_string(),
                        name: (*target).to_string(),
                        row: checker.compute_source_row(*earlier),
                    },
                    statement.range(),
                );
            }
            bound.insert(target, (statement.start(), is_field));
        }
        let (Some(call), [name]) = (call, targets.as_slice()) else {
            continue;
        };
        if checker.is_rule_enabled(Rule::DefaultEvaluatedAtImport) {
            check_default(checker, name, call);
        }
        if checker.is_rule_enabled(Rule::SelectionDuplicateKey) {
            check_selection(checker, name, call);
        }
        if checker.is_rule_enabled(Rule::FieldHookPrefix) {
            check_hooks(checker, name, call);
        }
        if checker.is_rule_enabled(Rule::FieldPositionalArgument) {
            check_positional(checker, name, call);
        }
        if checker.is_rule_enabled(Rule::FieldAttributeOrder) {
            check_layout(checker, name, call);
        }
        if checker.is_rule_enabled(Rule::DeadFieldAttribute) {
            check_dead(checker, name, call);
        }
        if checker.is_rule_enabled(Rule::StoredRelated) {
            check_stored_related(checker, name, call);
        }
    }
}
