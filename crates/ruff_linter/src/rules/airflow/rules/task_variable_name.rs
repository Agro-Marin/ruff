use crate::Violation;
use crate::codes::Category;
use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast as ast;
use ruff_python_ast::Expr;
use ruff_python_semantic::Modules;
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;

/// ## What it does
/// Checks that the task variable name matches the `task_id` value for
/// Airflow Operators.
///
/// ## Why is this bad?
/// When initializing an Airflow Operator, for consistency, the variable
/// name should match the `task_id` value. This makes it easier to
/// follow the flow of the Dag.
///
/// ## Example
/// ```python
/// from airflow.operators import PythonOperator
///
///
/// incorrect_name = PythonOperator(task_id="my_task")
/// ```
///
/// Use instead:
/// ```python
/// from airflow.operators import PythonOperator
///
///
/// my_task = PythonOperator(task_id="my_task")
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.271", category = Category::Style)]
pub(crate) struct AirflowVariableNameTaskIdMismatch {
    task_id: String,
}

impl Violation for AirflowVariableNameTaskIdMismatch {
    #[derive_message_formats]
    fn message(&self) -> String {
        let AirflowVariableNameTaskIdMismatch { task_id } = self;
        format!("Task variable name should match the `task_id`: \"{task_id}\"")
    }
}

/// AIR001
pub(crate) fn variable_name_task_id(checker: &Checker, targets: &[Expr], value: &Expr) {
    if !checker.semantic().seen_module(Modules::AIRFLOW) {
        return;
    }

    let [target] = targets else {
        return;
    };
    let Expr::Name(ast::ExprName { id, .. }) = target else {
        return;
    };

    let Expr::Call(ast::ExprCall {
        func, arguments, ..
    }) = value
    else {
        return;
    };

    if !checker
        .semantic()
        .resolve_qualified_name(func)
        .is_some_and(|qualified_name| {
            match qualified_name.segments() {
                ["airflow", "operators", ..] => true,

                ["airflow", "providers", rest @ ..] => {
                    if let Some(pos) = rest.iter().position(|&s| s == "operators") {
                        pos + 1 < rest.len()
                    } else {
                        false
                    }
                }

                _ => false,
            }
        })
    {
        return;
    }

    let Some(keyword) = arguments.find_keyword("task_id") else {
        return;
    };

    let Some(ast::ExprStringLiteral { value: task_id, .. }) =
        keyword.value.as_string_literal_expr()
    else {
        return;
    };

    if task_id == id.as_str() {
        return;
    }

    checker.report_diagnostic(
        AirflowVariableNameTaskIdMismatch {
            task_id: task_id.to_string(),
        },
        target.range(),
    );
}
