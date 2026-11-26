use ruff_db::diagnostic::Diagnostic;
use ruff_python_ast::name::QualifiedName;
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::SemanticModel;
use ruff_python_semantic::analyze::function_type::is_subject_to_liskov_substitution_principle;

use crate::checkers::ast::Checker;
use crate::settings::LinterSettings;

/// Returns `true` if a function call is allowed to use a boolean trap.
fn is_allowed_func_call(name: &str) -> bool {
    matches!(
        name,
        "__setattr__"
            | "append"
            | "assertEqual"
            | "assertEquals"
            | "assertNotEqual"
            | "assertNotEquals"
            | "bool"
            | "bytes"
            | "coalesce"
            | "count"
            | "failIfEqual"
            | "failUnlessEqual"
            | "float"
            | "fromkeys"
            | "get"
            | "getattr"
            | "getboolean"
            | "getfloat"
            | "getint"
            | "ifnull"
            | "index"
            | "insert"
            | "int"
            | "is_"
            | "is_not"
            | "isnull"
            | "next"
            | "nvl"
            | "param"
            | "pop"
            | "remove"
            | "set_blocking"
            | "set_enabled"
            | "setattr"
            | "setdefault"
            | "str"
    )
}

/// Returns `true` if a call is semantically allowed to use a boolean trap.
fn is_semantically_allowed_func_call(call: &ast::ExprCall, semantic: &SemanticModel) -> bool {
    semantic.matches_qualified_name(&call.func, &["multiprocessing", "Value"])
}

/// Returns `true` if a call is allowed by the user to use a boolean trap.
fn is_user_allowed_func_call(
    call: &ast::ExprCall,
    semantic: &SemanticModel,
    settings: &LinterSettings,
) -> bool {
    semantic
        .resolve_qualified_name(call.func.as_ref())
        .is_some_and(|qualified_name| {
            settings
                .flake8_boolean_trap
                .extend_allowed_calls
                .iter()
                .map(|target| QualifiedName::from_dotted_name(target))
                .any(|target| qualified_name == target)
        })
}

/// Returns `true` if a function defines a binary operator.
///
/// This only includes operators, i.e., functions that are usually not called directly.
///
/// See: <https://docs.python.org/3/library/operator.html>
fn is_operator_method(name: &str) -> bool {
    match name {
        "__contains__" => true,
        "__getitem__" | "__setitem__" | "__delitem__" => true,
        "__add__" | "__radd__" | "__iadd__" => true,
        "__sub__" | "__rsub__" | "__isub__" => true,
        "__mul__" | "__rmul__" | "__imul__" => true,
        "__truediv__" | "__rtruediv__" | "__itruediv__" => true,
        "__floordiv__" | "__rfloordiv__" | "__ifloordiv__" => true,
        "__mod__" | "__rmod__" | "__imod__" => true,
        "__pow__" | "__rpow__" | "__ipow__" => true,
        "__lshift__" | "__rlshift__" | "__ilshift__" => true,
        "__rshift__" | "__rrshift__" | "__irshift__" => true,
        "__matmul__" | "__rmatmul__" | "__imatmul__" => true,
        "__and__" | "__rand__" | "__iand__" => true,
        "__or__" | "__ror__" | "__ior__" => true,
        "__xor__" | "__rxor__" | "__ixor__" => true,
        "__gt__" | "__lt__" | "__ge__" | "__le__" | "__eq__" | "__ne__" => true,
        "__pos__" | "__neg__" | "__invert__" => true,
        _ => false,
    }
}

/// Returns `true` if a function definition is allowed to use a boolean trap.
pub(super) fn is_allowed_func_def(name: &str) -> bool {
    matches!(name, "__post_init__") || is_operator_method(name)
}

/// Returns `true` if an argument is allowed to use a boolean trap. To return
/// `true`, the function name must be explicitly allowed, and the argument must
/// be either the first or second argument in the call.
pub(super) fn allow_boolean_trap(call: &ast::ExprCall, checker: &Checker) -> bool {
    let func_name = match call.func.as_ref() {
        Expr::Attribute(ast::ExprAttribute { attr, .. }) => attr.as_str(),
        Expr::Name(ast::ExprName { id, .. }) => id.as_str(),
        _ => return false,
    };

    if is_allowed_func_call(func_name) {
        return true;
    }

    if call.arguments.args.len() == 1 {
        if func_name == "set" {
            return true;
        }

        if func_name
            .strip_prefix("set")
            .is_some_and(|suffix| suffix.starts_with(|c: char| c == '_' || c.is_ascii_uppercase()))
        {
            return true;
        }
    }

    if is_semantically_allowed_func_call(call, checker.semantic()) {
        return true;
    }

    if is_user_allowed_func_call(call, checker.semantic(), checker.settings()) {
        return true;
    }

    false
}

pub(super) fn add_liskov_substitution_principle_help(
    diagnostic: &mut Diagnostic,
    function_name: &str,
    decorator_list: &[ast::Decorator],
    checker: &Checker,
) {
    let semantic = checker.semantic();
    let parent_scope = semantic.current_scope();
    let pep8_settings = &checker.settings().pep8_naming;
    if is_subject_to_liskov_substitution_principle(
        function_name,
        decorator_list,
        parent_scope,
        semantic,
        &pep8_settings.classmethod_decorators,
        &pep8_settings.staticmethod_decorators,
    ) {
        diagnostic.help(
            "Consider adding `@typing.override` if changing the function signature \
                would violate the Liskov Substitution Principle",
        );
    }
}
