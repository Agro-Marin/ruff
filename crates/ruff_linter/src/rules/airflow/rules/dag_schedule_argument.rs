use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::Expr;
use ruff_python_ast::{self as ast};
use ruff_python_semantic::Modules;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;

/// ## What it does
/// Checks for a `DAG()` class or `@dag()` decorator without an explicit
/// `schedule` (or `schedule_interval` for Airflow 1) parameter.
///
/// ## Why is this bad?
/// The default value of the `schedule` parameter on Airflow 2 and
/// `schedule_interval` on Airflow 1 is `timedelta(days=1)`, which is almost
/// never what a user is looking for. Airflow 3 changed the default value to `None`,
/// and would break existing Dags using the implicit default.
///
/// If your Dag does not have an explicit `schedule` / `schedule_interval` argument,
/// Airflow 2 schedules a run for it every day (at the time determined by `start_date`).
/// Such a Dag will no longer be scheduled on Airflow 3 at all, without any
/// exceptions or other messages visible to the user.
///
/// ## Example
/// ```python
/// from airflow import DAG
///
///
/// # Using the implicit default schedule.
/// dag = DAG(dag_id="my_dag")
/// ```
///
/// Use instead:
/// ```python
/// from datetime import timedelta
///
/// from airflow import DAG
///
///
/// dag = DAG(dag_id="my_dag", schedule=timedelta(days=1))
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.13.0", category = Category::Suspicious)]
pub(crate) struct AirflowDagNoScheduleArgument;

impl Violation for AirflowDagNoScheduleArgument {
    #[derive_message_formats]
    fn message(&self) -> String {
        "`DAG` or `@dag` should have an explicit `schedule` argument".to_string()
    }
}

/// AIR002
pub(crate) fn dag_no_schedule_argument(checker: &Checker, expr: &Expr) {
    if !checker.semantic().seen_module(Modules::AIRFLOW) {
        return;
    }

    let Expr::Call(ast::ExprCall {
        func, arguments, ..
    }) = expr
    else {
        return;
    };

    if !checker
        .semantic()
        .resolve_qualified_name(func)
        .is_some_and(|qualname| matches!(qualname.segments(), ["airflow", .., "DAG" | "dag"]))
    {
        return;
    }

    if ["schedule", "schedule_interval", "timetable"]
        .iter()
        .any(|a| arguments.find_keyword(a).is_some())
    {
        return;
    }

    checker.report_diagnostic(AirflowDagNoScheduleArgument, expr.range());
}
