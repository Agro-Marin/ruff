use std::rc::Rc;
use std::sync::LazyLock;

use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::statement_visitor::{StatementVisitor, walk_stmt};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, Expr, Operator, Pattern, Stmt, Suite};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{def_start, is_test_path};
use crate::rules::odoo::strings::{JoinedValue, joined_values};

/// ## What it does
/// Checks for a query passed to `cr.execute`, `executemany`, `SQL()` or
/// `get_rows_autocommit` that is built by interpolating a value that is not
/// known to be constant, and for a function whose result builds such a query.
///
/// ## Why is this bad?
/// A value formatted into the statement is parsed as SQL: whoever controls it
/// controls the query. `SQL()` passes it as a parameter instead.
///
/// A value is constant when it is a literal, a name every binding of which is
/// constant (or that a guard asserts on), a `%d` substitution, an identifier
/// attribute such as `_table` or `field.name`, or the result of a function of
/// the module whose every return is constant. A call to a function the module
/// defines later is judged when the definition is reached, and reported there.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Security)]
pub(crate) struct SqlInjection {
    function: Option<(String, String)>,
}

impl Violation for SqlInjection {
    #[derive_message_formats]
    fn message(&self) -> String {
        match &self.function {
            None => "Query built by interpolating a value not known to be constant".to_string(),
            Some((name, sites)) => {
                format!(
                    "`{name}` returns a value not known to be constant, and builds a query at {sites}"
                )
            }
        }
    }

    fn fix_title(&self) -> Option<String> {
        Some("Build the query with `SQL()` so the value is passed as a parameter".to_string())
    }
}

const CURSOR_SUFFIXES: &[&str] = &[".cr", "._cr", "_cr"];
const CURSOR_NAMES: &[&str] = &["cr", "_cr", "cursor"];
const SQL_BUILDERS: &[&str] = &["odoo.tools", "tools"];
const ATTRIBUTE_WHITELIST: &[&str] = &["_table", "id", "get_lang.code"];
/// A field's name and lang are identifiers; a record's are whatever a user typed.
const FIELD_ATTRIBUTES: &[&str] = &["name", "lang"];
const FIELD_RECEIVERS: &[&str] = &["field.", "fields.", "self._fields"];
const FUNCTION_WHITELIST: &[&str] = &[
    "create", "read", "write", "browse", "select", "get", "strip", "items", "_select", "_from",
    "_where", "any", "join", "split", "tuple", "get_sql", "search", "list", "set", "next", "SQL",
];

static PERCENT_SPEC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"%(?:\([^)]*\))?[-+ 0#]*\d*(?:\.\d+)?[diouxXeEfFgGcrsa%]").expect("valid regex")
});

fn is_cursor_expression(name: &str) -> bool {
    SQL_BUILDERS.contains(&name)
        || CURSOR_NAMES.contains(&name)
        || CURSOR_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

fn address<T>(node: &T) -> usize {
    std::ptr::from_ref(node) as usize
}

/// The scope a name is looked up in: a function, or the module. A class body
/// and a lambda are not scopes here.
#[derive(Clone, Copy)]
enum Scope<'a> {
    Module,
    Function(&'a ast::StmtFunctionDef),
}

impl Scope<'_> {
    fn id(self) -> usize {
        match self {
            Scope::Module => 0,
            Scope::Function(function) => address(function),
        }
    }
}

/// Where a node sits: its scope, and the innermost class around it.
#[derive(Clone, Copy)]
struct Place<'a> {
    scope: Scope<'a>,
    class: Option<&'a str>,
}

/// The place of every expression and every function of the module.
struct Places<'a> {
    current: Place<'a>,
    expressions: FxHashMap<usize, Place<'a>>,
    functions: FxHashMap<usize, Place<'a>>,
    definitions: Vec<Visit<'a>>,
}

/// A node the check visits, in walk order: a call, or a function definition.
#[derive(Clone, Copy)]
enum Visit<'a> {
    Call(&'a ast::ExprCall),
    Function(&'a ast::StmtFunctionDef),
}

impl<'a> Visitor<'a> for Places<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(function) => {
                self.functions.insert(address(function), self.current);
                self.definitions.push(Visit::Function(function));
                let outer = self.current;
                // Decorators, defaults and annotations are children of the
                // function node, so test_lint places them in its scope.
                self.current.scope = Scope::Function(function);
                visitor::walk_stmt(self, stmt);
                self.current = outer;
            }
            Stmt::ClassDef(class) => {
                let outer = self.current;
                self.current.class = Some(class.name.as_str());
                visitor::walk_stmt(self, stmt);
                self.current = outer;
            }
            _ => visitor::walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        self.expressions.insert(address(expr), self.current);
        if let Expr::Call(call) = expr {
            self.definitions.push(Visit::Call(call));
        }
        visitor::walk_expr(self, expr);
    }
}

/// What binds a name in a scope.
#[derive(Clone, Copy)]
enum Binding<'a> {
    Parameter,
    Assign(&'a ast::StmtAssign),
    Value(&'a Expr),
    Unknown,
}

fn bound_names<'a>(target: &'a Expr, names: &mut Vec<&'a str>) {
    match target {
        Expr::Name(name) => names.push(name.id.as_str()),
        Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) => {
            for elt in elts {
                bound_names(elt, names);
            }
        }
        Expr::Starred(starred) => bound_names(&starred.value, names),
        _ => {}
    }
}

/// The bindings of one scope, nested functions, lambdas and classes left out,
/// recorded after their children (post-order); reversed, that is the order
/// `test_lint`'s stack-based walk meets them in.
#[derive(Default)]
struct Bindings<'a> {
    found: Vec<(&'a str, Binding<'a>)>,
}

impl<'a> Bindings<'a> {
    fn bind(&mut self, target: &'a Expr, binding: Binding<'a>) {
        let mut names = Vec::new();
        bound_names(target, &mut names);
        self.found
            .extend(names.into_iter().map(|name| (name, binding)));
    }
}

impl<'a> Visitor<'a> for Bindings<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => return,
            _ => visitor::walk_stmt(self, stmt),
        }
        match stmt {
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    self.bind(target, Binding::Assign(assign));
                }
            }
            Stmt::AugAssign(assign) => self.bind(&assign.target, Binding::Value(&assign.value)),
            Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(value),
                ..
            }) => self.bind(target, Binding::Value(value)),
            Stmt::For(for_stmt) => self.bind(&for_stmt.target, Binding::Value(&for_stmt.iter)),
            _ => {}
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if expr.is_lambda_expr() {
            return;
        }
        visitor::walk_expr(self, expr);
        if let Expr::Named(named) = expr {
            self.bind(&named.target, Binding::Value(&named.value));
        }
    }

    fn visit_comprehension(&mut self, comprehension: &'a ast::Comprehension) {
        visitor::walk_comprehension(self, comprehension);
        self.bind(&comprehension.target, Binding::Value(&comprehension.iter));
    }

    fn visit_with_item(&mut self, with_item: &'a ast::WithItem) {
        visitor::walk_with_item(self, with_item);
        if let Some(target) = &with_item.optional_vars {
            self.bind(target, Binding::Value(&with_item.context_expr));
        }
    }

    fn visit_except_handler(&mut self, except_handler: &'a ast::ExceptHandler) {
        visitor::walk_except_handler(self, except_handler);
        let ast::ExceptHandler::ExceptHandler(handler) = except_handler;
        if let Some(name) = &handler.name {
            self.found.push((name.as_str(), Binding::Unknown));
        }
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        visitor::walk_pattern(self, pattern);
        let name = match pattern {
            Pattern::MatchAs(ast::PatternMatchAs { name, .. })
            | Pattern::MatchStar(ast::PatternMatchStar { name, .. }) => name.as_ref(),
            _ => None,
        };
        if let Some(name) = name {
            self.found.push((name.as_str(), Binding::Unknown));
        }
    }
}

/// The `return` statements of a function, nested scopes left out, in the order
/// `test_lint`'s stack-based walk meets them: the last one first.
fn return_values(function: &ast::StmtFunctionDef) -> Vec<&Expr> {
    #[derive(Default)]
    struct Returns<'a> {
        found: Vec<&'a Expr>,
    }

    impl<'a> StatementVisitor<'a> for Returns<'a> {
        fn visit_stmt(&mut self, stmt: &'a Stmt) {
            match stmt {
                Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
                Stmt::Return(ast::StmtReturn {
                    value: Some(value), ..
                }) => self.found.push(value),
                _ => walk_stmt(self, stmt),
            }
        }
    }

    let mut returns = Returns::default();
    returns.visit_body(&function.body);
    returns.found.reverse();
    returns.found
}

/// The names tested by a guard anywhere in a scope: an `assert`, or an `if`
/// whose body raises.
#[derive(Default)]
struct Guarded<'a> {
    names: FxHashSet<&'a str>,
}

impl<'a> Visitor<'a> for Guarded<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        let test = match stmt {
            Stmt::Assert(assert) => Some(&*assert.test),
            Stmt::If(if_stmt) if if_stmt.body.iter().any(Stmt::is_raise_stmt) => {
                Some(&*if_stmt.test)
            }
            _ => None,
        };
        if let Some(test) = test {
            let mut names = NameCollector::default();
            names.visit_expr(test);
            self.names.extend(names.names);
        }
        visitor::walk_stmt(self, stmt);
    }
}

#[derive(Default)]
struct NameCollector<'a> {
    names: Vec<&'a str>,
}

impl<'a> Visitor<'a> for NameCollector<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Name(name) = expr {
            self.names.push(name.id.as_str());
        }
        visitor::walk_expr(self, expr);
    }
}

type FunctionKey<'a> = (Option<&'a str>, &'a str);

/// A call made to a function before its definition was reached: the position
/// of the value asked for, whether its arguments were constant, and the query
/// call it was reached from.
#[derive(Clone, Copy)]
struct Callsite<'a> {
    position: Option<usize>,
    const_args: bool,
    root: &'a ast::ExprCall,
}

struct SqlInjectionCheck<'a> {
    suite: &'a Suite,
    places: Places<'a>,
    module_constants: FxHashMap<&'a str, &'a Expr>,
    function_defs: FxHashMap<FunctionKey<'a>, Vec<&'a ast::StmtFunctionDef>>,
    callsites: FxHashMap<FunctionKey<'a>, Vec<Callsite<'a>>>,
    root_call: Option<&'a ast::ExprCall>,
    const_def_cache: FxHashMap<(usize, Option<usize>, bool), bool>,
    assign_cache: FxHashMap<usize, Rc<FxHashMap<&'a str, Vec<Binding<'a>>>>>,
    assert_cache: FxHashMap<usize, Rc<FxHashSet<&'a str>>>,
    resolving_names: FxHashSet<(usize, &'a str)>,
    resolving_nodes: FxHashSet<usize>,
}

impl<'a> SqlInjectionCheck<'a> {
    fn new(suite: &'a Suite) -> Self {
        let mut places = Places {
            current: Place {
                scope: Scope::Module,
                class: None,
            },
            expressions: FxHashMap::default(),
            functions: FxHashMap::default(),
            definitions: Vec::new(),
        };
        places.visit_body(suite);
        let mut module_constants = FxHashMap::default();
        for statement in suite {
            match statement {
                Stmt::Assign(assign) => {
                    for target in &assign.targets {
                        if let Expr::Name(name) = target {
                            module_constants.insert(name.id.as_str(), &*assign.value);
                        }
                    }
                }
                Stmt::AnnAssign(ast::StmtAnnAssign {
                    target,
                    value: Some(value),
                    ..
                }) => {
                    if let Expr::Name(name) = &**target {
                        module_constants.insert(name.id.as_str(), &**value);
                    }
                }
                _ => {}
            }
        }
        Self {
            suite,
            places,
            module_constants,
            function_defs: FxHashMap::default(),
            callsites: FxHashMap::default(),
            root_call: None,
            const_def_cache: FxHashMap::default(),
            assign_cache: FxHashMap::default(),
            assert_cache: FxHashMap::default(),
            resolving_names: FxHashSet::default(),
            resolving_nodes: FxHashSet::default(),
        }
    }

    fn place(&self, expr: &Expr) -> Place<'a> {
        self.places
            .expressions
            .get(&address(expr))
            .copied()
            .unwrap_or(Place {
                scope: Scope::Module,
                class: None,
            })
    }

    fn outer_scope(&self, scope: Scope<'a>) -> Option<Scope<'a>> {
        match scope {
            Scope::Module => None,
            Scope::Function(function) => self
                .places
                .functions
                .get(&address(function))
                .map(|place| place.scope),
        }
    }

    fn assignments(&mut self, scope: Scope<'a>) -> Rc<FxHashMap<&'a str, Vec<Binding<'a>>>> {
        if let Some(cached) = self.assign_cache.get(&scope.id()) {
            return Rc::clone(cached);
        }
        let mut mapping: FxHashMap<&'a str, Vec<Binding<'a>>> = FxHashMap::default();
        let mut parameters: FxHashSet<&str> = FxHashSet::default();
        let mut bindings = Bindings::default();
        match scope {
            Scope::Module => bindings.visit_body(self.suite),
            Scope::Function(function) => {
                for parameter in &function.parameters {
                    parameters.insert(parameter.name().as_str());
                    mapping.insert(parameter.name().as_str(), vec![Binding::Parameter]);
                }
                bindings.visit_body(&function.body);
            }
        }
        for (name, binding) in bindings.found.into_iter().rev() {
            if !parameters.contains(name) {
                mapping.entry(name).or_default().push(binding);
            }
        }
        let mapping = Rc::new(mapping);
        self.assign_cache.insert(scope.id(), Rc::clone(&mapping));
        mapping
    }

    fn is_asserted(&mut self, scope: Scope<'a>, name: &str) -> bool {
        let cached = if let Some(cached) = self.assert_cache.get(&scope.id()) {
            Rc::clone(cached)
        } else {
            let mut guarded = Guarded::default();
            match scope {
                Scope::Module => guarded.visit_body(self.suite),
                Scope::Function(function) => {
                    for statement in &function.body {
                        guarded.visit_stmt(statement);
                    }
                }
            }
            let names = Rc::new(guarded.names);
            self.assert_cache.insert(scope.id(), Rc::clone(&names));
            names
        };
        cached.contains(name)
    }

    fn all_const<I: IntoIterator<Item = &'a Expr>>(
        &mut self,
        nodes: I,
        args_allowed: bool,
        position: Option<usize>,
    ) -> bool {
        for node in nodes {
            if !self.is_constexpr(node, args_allowed, position) {
                return false;
            }
        }
        true
    }

    fn is_constexpr(
        &mut self,
        node: &'a Expr,
        args_allowed: bool,
        position: Option<usize>,
    ) -> bool {
        match node {
            node if node.is_literal_expr() => true,
            Expr::List(ast::ExprList { elts, .. }) | Expr::Set(ast::ExprSet { elts, .. }) => {
                self.all_const(elts, args_allowed, None)
            }
            Expr::Tuple(ast::ExprTuple { elts, .. }) => {
                if let Some(position) = position
                    && position < elts.len()
                    && !elts.iter().any(Expr::is_starred_expr)
                {
                    return self.is_constexpr(&elts[position], args_allowed, None);
                }
                self.all_const(elts, args_allowed, None)
            }
            Expr::Dict(dict) => {
                for item in &dict.items {
                    let Some(key) = &item.key else {
                        return false;
                    };
                    if !(self.is_constexpr(key, args_allowed, None)
                        && self.is_constexpr(&item.value, args_allowed, None))
                    {
                        return false;
                    }
                }
                true
            }
            Expr::Starred(starred) => self.is_constexpr(&starred.value, args_allowed, position),
            Expr::BinOp(ast::ExprBinOp {
                left, op, right, ..
            }) => {
                let left_ok = self.is_constexpr(left, args_allowed, None);
                if *op == Operator::Mod
                    && let Expr::StringLiteral(template) = &**left
                {
                    let template = template.value.to_str();
                    let specs: Vec<&str> = PERCENT_SPEC
                        .find_iter(template)
                        .map(|spec| spec.as_str())
                        .filter(|spec| *spec != "%%")
                        .collect();
                    if !specs.is_empty() && specs.iter().all(|spec| spec.ends_with('d')) {
                        return true;
                    }
                }
                let right_ok = self.is_constexpr(right, args_allowed, None);
                left_ok && right_ok
            }
            Expr::Name(name) => self.check_name_constexpr(node, name.id.as_str(), args_allowed),
            Expr::FString(fstring) => {
                let values: Vec<&'a Expr> = joined_values(&fstring.value)
                    .into_iter()
                    .filter_map(|value| match value {
                        JoinedValue::Interpolation(interpolation) => Some(&*interpolation.expression),
                        JoinedValue::Constant(_) => None,
                    })
                    .filter(|value| !matches!(value, Expr::Attribute(attribute) if attribute.attr.as_str().starts_with('_')))
                    .collect();
                self.all_const(values, args_allowed, position)
            }
            Expr::Call(call) => {
                if let Expr::Attribute(attribute) = &*call.func {
                    if attribute.attr.as_str() == "append"
                        && let [first] = &*call.arguments.args
                    {
                        return self.is_constexpr(first, false, None);
                    }
                    if attribute.attr.as_str() == "format" {
                        return self.is_constexpr(&attribute.value, args_allowed, None)
                            && self.all_const(&call.arguments.args, args_allowed, None)
                            && self.all_const(
                                call.arguments.keywords.iter().map(|keyword| &keyword.value),
                                args_allowed,
                                None,
                            );
                    }
                }
                let outer = self.root_call;
                if self.root_call.is_none() {
                    self.root_call = Some(call);
                }
                let result = self.evaluate_function_call(node, call, args_allowed, position);
                self.root_call = outer;
                result
            }
            Expr::If(if_exp) => {
                self.is_constexpr(&if_exp.body, args_allowed, None)
                    && self.is_constexpr(&if_exp.orelse, args_allowed, None)
            }
            Expr::Subscript(subscript) => self.is_constexpr(&subscript.value, args_allowed, None),
            Expr::BoolOp(bool_op) => self.all_const(&bool_op.values, args_allowed, None),
            Expr::Attribute(_) => check_attribute_whitelist(node),
            _ => false,
        }
    }

    fn check_name_constexpr(&mut self, node: &'a Expr, name: &'a str, args_allowed: bool) -> bool {
        let scope = self.place(node).scope;
        let key = (scope.id(), name);
        // `q = q % {...}` asks about q while q is being resolved. Assume the
        // answer is yes: the other bindings decide, since a non-constant seed
        // fails on its own line and a constant one stays constant through an
        // accumulation of constants.
        if !self.resolving_names.insert(key) {
            return true;
        }
        let result = self.resolve_name(name, scope, args_allowed);
        self.resolving_names.remove(&key);
        result
    }

    fn resolve_name(&mut self, name: &'a str, scope: Scope<'a>, args_allowed: bool) -> bool {
        let bindings = self.assignments(scope);
        let mut results = Vec::new();
        for binding in bindings.get(name).into_iter().flatten() {
            results.push(match *binding {
                Binding::Parameter => args_allowed,
                Binding::Assign(assign) => {
                    let position = tuple_position(&assign.targets, name);
                    self.is_constexpr(&assign.value, args_allowed, position)
                }
                Binding::Value(value) => self.is_constexpr(value, args_allowed, None),
                Binding::Unknown => false,
            });
        }
        if results.is_empty() {
            // Free variables use the enclosing function's binding.
            if let Some(outer @ Scope::Function(_)) = self.outer_scope(scope) {
                return self.resolve_name(name, outer, args_allowed);
            }
            if let Some(value) = self.module_constants.get(name).copied() {
                return self.is_constexpr(value, args_allowed, None);
            }
        }
        if !results.is_empty() && results.iter().all(|result| *result) {
            return true;
        }
        self.is_asserted(scope, name)
    }

    fn evaluate_function_call(
        &mut self,
        node: &'a Expr,
        call: &'a ast::ExprCall,
        args_allowed: bool,
        position: Option<usize>,
    ) -> bool {
        let name = match &*call.func {
            Expr::Attribute(attribute) => attribute.attr.as_str(),
            Expr::Name(name) => name.id.as_str(),
            _ => return false,
        };
        if name == "SQL" {
            return true;
        }
        let place = self.place(node);
        if let Scope::Function(function) = place.scope
            && !function.is_async
            && function.name.as_str() == name
        {
            return true;
        }
        let const_args = self.all_const(&call.arguments.args, args_allowed, None);
        let key = (place.class, name);
        if let Some(root) = self.root_call {
            self.callsites.entry(key).or_default().push(Callsite {
                position,
                const_args,
                root,
            });
        }
        let functions = self.function_defs.get(&key).cloned().unwrap_or_default();
        functions
            .into_iter()
            .all(|function| self.is_const_def(function, position, const_args))
    }

    fn is_const_def(
        &mut self,
        function: &'a ast::StmtFunctionDef,
        position: Option<usize>,
        const_args: bool,
    ) -> bool {
        let key = (address(function), position, const_args);
        if let Some(cached) = self.const_def_cache.get(&key) {
            return *cached;
        }
        let name = function.name.as_str();
        if name.starts_with("__") || FUNCTION_WHITELIST.contains(&name) {
            self.const_def_cache.insert(key, true);
            return true;
        }
        // Seeded before the walk: a helper reached again through a cycle adds no
        // return of its own, and the other returns decide.
        self.const_def_cache.insert(key, true);
        let result = return_values(function)
            .into_iter()
            .all(|value| self.is_constexpr(value, const_args, position));
        self.const_def_cache.insert(key, result);
        result
    }

    fn allowable(&mut self, node: &'a Expr) -> bool {
        looks_like_psycopg(node)
            || self.is_constexpr(node, false, None)
            || matches!(node, Expr::Attribute(attribute)
                if attribute.value.is_name_expr() && attribute.attr.as_str().starts_with('_'))
    }

    fn all_allowable<I: IntoIterator<Item = &'a Expr>>(&mut self, nodes: I) -> bool {
        for node in nodes {
            if !self.allowable(node) {
                return false;
            }
        }
        true
    }

    /// A name bound once, by a plain assignment, is that assignment's value.
    /// Bound more than once (`q = "..."; q += tbl`) it is left to
    /// `is_constexpr`, which weighs every binding.
    fn resolve(&mut self, node: &'a Expr) -> &'a Expr {
        let Expr::Name(name) = node else {
            return node;
        };
        let scope = self.place(node).scope;
        let bindings = self.assignments(scope);
        match bindings.get(name.id.as_str()).map(Vec::as_slice) {
            Some([Binding::Assign(assign)]) if !assign.targets.iter().any(Expr::is_tuple_expr) => {
                &assign.value
            }
            _ => node,
        }
    }

    fn check_concatenation(&mut self, node: &'a Expr) -> Option<bool> {
        let node = self.resolve(node);
        if self.allowable(node) {
            return Some(false);
        }
        if !self.resolving_nodes.insert(address(node)) {
            return None;
        }
        let result = self.check_concatenation_inner(node);
        self.resolving_nodes.remove(&address(node));
        result
    }

    fn check_concatenation_inner(&mut self, node: &'a Expr) -> Option<bool> {
        match node {
            Expr::BinOp(ast::ExprBinOp {
                left,
                op: Operator::Mod | Operator::Add,
                right,
                ..
            }) => {
                let right_ok = match &**right {
                    Expr::Tuple(tuple) => self.all_allowable(&tuple.elts),
                    Expr::Dict(dict) => {
                        self.all_allowable(dict.items.iter().map(|item| &item.value))
                    }
                    right => self.allowable(right),
                };
                if !right_ok {
                    return Some(true);
                }
                self.check_concatenation(left)
            }
            Expr::Call(call) => match &*call.func {
                Expr::Attribute(attribute) if attribute.attr.as_str() == "format" => Some(
                    !(self.allowable(&attribute.value)
                        && self.all_allowable(&call.arguments.args)
                        && self.all_allowable(
                            call.arguments.keywords.iter().map(|keyword| &keyword.value),
                        )),
                ),
                _ => None,
            },
            Expr::FString(fstring) => {
                let values: Vec<&'a Expr> = joined_values(&fstring.value)
                    .into_iter()
                    .filter_map(|value| match value {
                        JoinedValue::Interpolation(interpolation) => {
                            Some(&*interpolation.expression)
                        }
                        JoinedValue::Constant(_) => None,
                    })
                    .collect();
                Some(!self.all_allowable(values))
            }
            _ => None,
        }
    }

    fn is_risky(&mut self, call: &'a ast::ExprCall) -> bool {
        match &*call.func {
            // odoo.db's Connection method forwards its statement to execute;
            // checked at every call site, whatever its receiver.
            Expr::Attribute(attribute) if attribute.attr.as_str() == "get_rows_autocommit" => {}
            Expr::Attribute(attribute)
                if matches!(attribute.attr.as_str(), "execute" | "executemany" | "SQL") =>
            {
                if !is_cursor_expression(&cursor_name(attribute)) {
                    return false;
                }
            }
            Expr::Name(name) if name.id.as_str() == "SQL" => {}
            _ => return false,
        }
        let query = call.arguments.args.first().or_else(|| {
            call.arguments
                .keywords
                .iter()
                .find(|keyword| {
                    keyword
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.as_str() == "query")
                })
                .map(|keyword| &keyword.value)
        });
        let Some(query) = query else {
            return false;
        };
        self.check_concatenation(query).unwrap_or(true)
    }

    /// A function definition, now that it is reached: the calls already made to
    /// it that built a query are judged against it.
    fn visit_function(&mut self, function: &'a ast::StmtFunctionDef) -> Vec<TextSize> {
        let class = self
            .places
            .functions
            .get(&address(function))
            .and_then(|place| place.class);
        let key = (class, function.name.as_str());
        self.function_defs.entry(key).or_default().push(function);
        let sites = self.callsites.get(&key).cloned().unwrap_or_default();
        let mut offending: Vec<TextSize> = Vec::new();
        for site in sites {
            if !self.is_const_def(function, site.position, site.const_args) {
                offending.push(site.root.start());
            }
        }
        offending
    }
}

/// `a.b.c` for the receiver of `a.b.c.execute`, as far as it is a chain of
/// names.
fn cursor_name(func: &ast::ExprAttribute) -> String {
    let mut parts = Vec::new();
    let mut current = &*func.value;
    while let Expr::Attribute(attribute) = current {
        parts.push(attribute.attr.as_str());
        current = &attribute.value;
    }
    if let Expr::Name(name) = current {
        parts.push(name.id.as_str());
    }
    parts.reverse();
    parts.join(".")
}

fn attribute_chain(expr: &Expr) -> String {
    match expr {
        Expr::Attribute(attribute) => {
            let prefix = attribute_chain(&attribute.value);
            if prefix.is_empty() {
                attribute.attr.to_string()
            } else {
                format!("{prefix}.{}", attribute.attr)
            }
        }
        Expr::Name(name) => name.id.to_string(),
        Expr::Call(call) => attribute_chain(&call.func),
        _ => String::new(),
    }
}

fn check_attribute_whitelist(expr: &Expr) -> bool {
    let chain = attribute_chain(expr);
    let last = chain.rsplit('.').next().unwrap_or(&chain);
    if FIELD_ATTRIBUTES.contains(&last) {
        return FIELD_RECEIVERS
            .iter()
            .any(|receiver| chain.starts_with(receiver));
    }
    let mut rest = chain.as_str();
    while !rest.is_empty() {
        if ATTRIBUTE_WHITELIST.contains(&rest) || rest.starts_with('_') {
            return true;
        }
        match rest.split_once('.') {
            Some((_, tail)) => rest = tail,
            None => break,
        }
    }
    false
}

fn looks_like_psycopg(expr: &Expr) -> bool {
    match expr {
        Expr::Call(call) => match &*call.func {
            Expr::Attribute(attribute) if attribute.attr.as_str() == "format" => {
                looks_like_psycopg(&attribute.value)
            }
            Expr::Attribute(attribute) => {
                matches!(&*attribute.value, Expr::Name(name) if matches!(name.id.as_str(), "sql" | "psycopg2" | "psycopg"))
            }
            Expr::Name(name) => name.id.as_str() == "SQL",
            _ => false,
        },
        Expr::Name(name) => name.id.as_str() == "SQL",
        _ => false,
    }
}

/// The position of a name among the elements of the first tuple target that
/// binds it directly.
fn tuple_position(targets: &[Expr], name: &str) -> Option<usize> {
    targets.iter().find_map(|target| match target {
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .position(|elt| matches!(elt, Expr::Name(element) if element.id.as_str() == name)),
        _ => None,
    })
}

/// E8501
pub(crate) fn sql_injection(checker: &Checker, suite: &Suite) {
    if is_test_path(checker.path()) {
        return;
    }
    let mut check = SqlInjectionCheck::new(suite);
    let visits = std::mem::take(&mut check.places.definitions);
    for visit in visits {
        match visit {
            Visit::Call(call) => {
                if check.is_risky(call) {
                    checker.report_diagnostic(SqlInjection { function: None }, call.range());
                }
            }
            Visit::Function(function) => {
                let mut offending = check.visit_function(function);
                if offending.is_empty() {
                    continue;
                }
                offending.sort_unstable();
                offending.dedup_by_key(|offset| checker.compute_source_row(*offset).to_string());
                let mut shown: Vec<String> = offending
                    .iter()
                    .take(3)
                    .map(|offset| checker.compute_source_row(*offset).to_string())
                    .collect();
                if offending.len() > 3 {
                    shown.push(format!("{} more", offending.len() - 3));
                }
                let start = def_start(function, checker.source());
                checker.report_diagnostic(
                    SqlInjection {
                        function: Some((function.name.to_string(), shown.join(", "))),
                    },
                    TextRange::new(start, function.name.end()),
                );
            }
        }
    }
}
