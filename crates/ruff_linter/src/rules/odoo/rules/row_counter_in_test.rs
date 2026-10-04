use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, ExprContext};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::is_test_path;

/// ## What it does
/// Checks for a test that reads `sql_log_count`.
///
/// ## Why is this bad?
/// `sql_log_count` is incremented by the row count (`odoo/db/metrics.py`), so a
/// correctly batched insert of N rows scores N, and a per-record budget measured
/// with it has headroom proportional to the rows each record writes.
/// `cr.sql_statement_count` counts statements.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct RowCounterInTest;

impl Violation for RowCounterInTest {
    #[derive_message_formats]
    fn message(&self) -> String {
        "`sql_log_count` counts rows, not round trips".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Read `cr.sql_statement_count`".to_string())
    }
}

/// E8516
pub(crate) fn row_counter_in_test(checker: &Checker, attribute: &ast::ExprAttribute) {
    if attribute.attr.as_str() != "sql_log_count"
        || attribute.ctx == ExprContext::Store
        || !is_test_path(checker.path())
    {
        return;
    }
    checker.report_diagnostic(RowCounterInTest, attribute.range());
}
