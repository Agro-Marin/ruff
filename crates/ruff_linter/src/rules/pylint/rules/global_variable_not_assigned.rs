use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_semantic::{ResolvedReference, Scope};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;

/// ## What it does
/// Checks for `global` variables that are not assigned a value in the current
/// scope.
///
/// ## Why is this bad?
/// The `global` keyword allows an inner scope to modify a variable declared
/// in the outer scope. If the variable is not modified within the inner scope,
/// there is no need to use `global`.
///
/// ## Example
/// ```python
/// DEBUG = True
///
///
/// def foo():
///     global DEBUG
///     if DEBUG:
///         print("foo() called")
///     ...
/// ```
///
/// Use instead:
/// ```python
/// DEBUG = True
///
///
/// def foo():
///     if DEBUG:
///         print("foo() called")
///     ...
/// ```
///
/// ## References
/// - [Python documentation: The `global` statement](https://docs.python.org/3/reference/simple_stmts.html#the-global-statement)
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.174", category = Category::Style)]
pub(crate) struct GlobalVariableNotAssigned {
    name: String,
}

impl Violation for GlobalVariableNotAssigned {
    #[derive_message_formats]
    fn message(&self) -> String {
        let GlobalVariableNotAssigned { name } = self;
        format!("Using global for `{name}` but no assignment is done")
    }
}

/// PLW0602
pub(crate) fn global_variable_not_assigned(checker: &Checker, scope: &Scope) {
    for (name, binding_id) in scope.bindings() {
        let binding = checker.semantic().binding(binding_id);
        if binding.kind.is_global() {
            if binding
                .references
                .iter()
                .map(|id| checker.semantic().reference(*id))
                .all(ResolvedReference::is_load)
            {
                checker.report_diagnostic(
                    GlobalVariableNotAssigned {
                        name: (*name).to_string(),
                    },
                    binding.range(),
                );
            }
        }
    }
}
