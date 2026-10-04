use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use ruff_python_ast::statement_visitor::{StatementVisitor, walk_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_trivia::{SimpleTokenKind, SimpleTokenizer};
use ruff_text_size::{Ranged, TextSize};

/// A test file: a path part named `tests`, or one starting with `test_`.
pub(crate) fn is_test_path(path: &Path) -> bool {
    path.components().any(|component| {
        let part = component.as_os_str().to_string_lossy();
        part == "tests" || part.starts_with("test_")
    })
}

/// A file of an Odoo addon: some ancestor directory holds a `__manifest__.py`.
pub(crate) fn in_addon(path: &Path) -> bool {
    static ADDON_DIRS: LazyLock<Mutex<HashMap<PathBuf, bool>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let mut ancestor = path.parent();
    while let Some(dir) = ancestor {
        if let Some(&known) = ADDON_DIRS.lock().unwrap().get(dir) {
            return known;
        }
        if dir.join("__manifest__.py").is_file() {
            let mut cache = ADDON_DIRS.lock().unwrap();
            let mut below = path.parent();
            while let Some(entry) = below {
                cache.insert(entry.to_path_buf(), true);
                if entry == dir {
                    break;
                }
                below = entry.parent();
            }
            return true;
        }
        ancestor = dir.parent();
    }
    let mut cache = ADDON_DIRS.lock().unwrap();
    let mut below = path.parent();
    while let Some(entry) = below {
        cache.insert(entry.to_path_buf(), false);
        below = entry.parent();
    }
    false
}

/// Where a function's `def` (or `async def`) keyword starts: the position Python
/// gives a function, after its decorators.
pub(crate) fn def_start(function: &ast::StmtFunctionDef, source: &str) -> TextSize {
    let after_decorators = function
        .decorator_list
        .last()
        .map_or(function.start(), Ranged::end);
    SimpleTokenizer::starts_at(after_decorators, source)
        .skip_trivia()
        .find(|token| matches!(token.kind, SimpleTokenKind::Def | SimpleTokenKind::Async))
        .map_or(function.start(), |token| token.start())
}

/// The last name of a callee: `route` for both `http.route` and `route`.
pub(crate) fn callee_name(func: &Expr) -> &str {
    match func {
        Expr::Attribute(attribute) => attribute.attr.as_str(),
        Expr::Name(name) => name.id.as_str(),
        _ => "",
    }
}

/// The first decorator that calls something whose name ends with `route`.
pub(crate) fn route_decorator(function: &ast::StmtFunctionDef) -> Option<&ast::ExprCall> {
    function
        .decorator_list
        .iter()
        .find_map(|decorator| match &decorator.expression {
            Expr::Call(call) if callee_name(&call.func).ends_with("route") => Some(call),
            _ => None,
        })
}

/// The `return` statements of a function body, not those of the functions it
/// defines.
pub(crate) fn returns(function: &ast::StmtFunctionDef) -> Vec<&ast::StmtReturn> {
    #[derive(Default)]
    struct Returns<'a> {
        found: Vec<&'a ast::StmtReturn>,
    }

    impl<'a> StatementVisitor<'a> for Returns<'a> {
        fn visit_stmt(&mut self, stmt: &'a Stmt) {
            match stmt {
                Stmt::FunctionDef(_) => {}
                Stmt::Return(ret) => self.found.push(ret),
                _ => walk_stmt(self, stmt),
            }
        }
    }

    let mut visitor = Returns::default();
    visitor.visit_body(&function.body);
    visitor.found
}

/// Python's `str.isupper()`: at least one cased character, and every cased
/// character upper case.
pub(crate) fn is_upper(name: &str) -> bool {
    let mut cased = name
        .chars()
        .filter(|c| c.is_uppercase() || c.is_lowercase())
        .peekable();
    cased.peek().is_some() && cased.all(char::is_uppercase)
}

/// A side of a comparison that holds no secret: a literal, or a named constant.
pub(crate) fn is_constant_side(expr: &Expr) -> bool {
    expr.is_literal_expr() || matches!(expr, Expr::Name(name) if is_upper(name.id.as_str()))
}
