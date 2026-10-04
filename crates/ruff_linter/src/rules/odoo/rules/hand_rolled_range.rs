use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Stmt};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{in_addon, is_test_path};
use crate::rules::odoo::model::{field_declarations, field_type, inherited};

/// ## What it does
/// Checks for a model that declares a numeric pair `<x>_min`/`<x>_max` (or
/// `min_<x>`/`max_<x>`) without inheriting `mixin.band`.
///
/// ## Why is this bad?
/// A pair that classifies a value is a band rolled by hand: inclusive on both
/// ends in one model, half-open in the next, an integer pair over a float score
/// with a gap between 79 and 80. `mixin.band` owns the range: half-open,
/// overlap-checked, scoped. A tolerance, a slider or a filter bound is a pair
/// too and is not a scale; those stay, and the floor names them.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Restriction)]
pub(crate) struct HandRolledRange {
    name: String,
    partner: String,
}

impl Violation for HandRolledRange {
    #[derive_message_formats]
    fn message(&self) -> String {
        let HandRolledRange { name, partner } = self;
        format!("`{name}`/`{partner}` is a hand-rolled range")
    }

    fn fix_title(&self) -> Option<String> {
        Some("A range that classifies a value is a `mixin.band`".to_string())
    }
}

/// `<x>_min` pairs with `<x>_max`, `min_<x>` with `max_<x>`.
fn partner_of(name: &str) -> Option<String> {
    if let Some(stem) = name.strip_suffix("_min")
        && !stem.is_empty()
    {
        return Some(format!("{stem}_max"));
    }
    if let Some(stem) = name.strip_prefix("min_")
        && !stem.is_empty()
    {
        return Some(format!("max_{stem}"));
    }
    None
}

/// E8532
pub(crate) fn hand_rolled_range(checker: &Checker, class: &ast::StmtClassDef) {
    let path = checker.path();
    if !in_addon(path) || is_test_path(path) || path.to_string_lossy().contains("/addons/base/") {
        return;
    }
    if inherited(class)
        .iter()
        .any(|parent| parent == "mixin.band" || parent.starts_with("mixin.score."))
    {
        return;
    }
    // A name declared twice keeps its first position and its last statement, as
    // a dict comprehension does.
    let mut numeric: Vec<(&str, &Stmt)> = Vec::new();
    for (name, call, statement) in field_declarations(class) {
        if !matches!(field_type(call), "Float" | "Integer" | "Monetary") {
            continue;
        }
        match numeric.iter_mut().find(|(known, _)| *known == name) {
            Some(entry) => entry.1 = statement,
            None => numeric.push((name, statement)),
        }
    }
    for (name, statement) in &numeric {
        if let Some(partner) = partner_of(name)
            && numeric.iter().any(|(other, _)| *other == partner)
        {
            checker.report_diagnostic(
                HandRolledRange {
                    name: (*name).to_string(),
                    partner,
                },
                statement.range(),
            );
        }
    }
}
