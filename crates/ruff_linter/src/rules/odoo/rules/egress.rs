use std::sync::LazyLock;

use regex::Regex;
use rustc_hash::FxHashMap;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, Expr, Operator, Stmt, Suite};
use ruff_text_size::{Ranged, TextRange};

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::{Category, Rule};
use crate::rules::odoo::helpers::{class_start, in_addon, is_test_path};

/// ## What it does
/// Checks for a call that dials out of Odoo directly (`requests`, `httpx`,
/// `urllib`, `aiohttp`, `smtplib`, `paramiko`, `boto3`...), and for a class
/// that subclasses a dialing client.
///
/// ## Why is this bad?
/// A call through `env['ir.egress']` has its address checked, its connection
/// pinned, every redirect checked again and its response capped; a direct dial
/// has none of that. A configured vendor goes through integration's
/// `get_api_client(env, code)`, an XML-RPC peer through
/// `ir.egress.xmlrpc_proxy`. A dial no HTTP session can carry checks its host
/// through `ir.egress.check_host` and connects through `netguard.dial`, as
/// SMTP, IMAP and POP do: a subclass whose socket hook calls `netguard.dial` is
/// pinned and not reported.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Security)]
pub(crate) struct RawEgress {
    target: String,
    subclass: Option<String>,
}

impl Violation for RawEgress {
    #[derive_message_formats]
    fn message(&self) -> String {
        let RawEgress { target, subclass } = self;
        match subclass {
            None => format!("`{target}()` dials out of Odoo outside `ir.egress`"),
            Some(class) => format!(
                "`{class}` subclasses `{target}`, so every instance dials out of Odoo outside `ir.egress`"
            ),
        }
    }

    fn fix_title(&self) -> Option<String> {
        Some("Send the call through `env['ir.egress']`".to_string())
    }
}

/// ## What it does
/// Checks for a secret (a key whose name holds `KEY`, `TOKEN`, `SECRET`,
/// `PASSW` or `CREDENTIAL`) written into `os.environ`.
///
/// ## Why is this bad?
/// `os.environ` belongs to the whole worker: every later subprocess and every
/// other company's work inherits it. The child process takes the secret in its
/// own `env=` mapping.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Security)]
pub(crate) struct SecretInEnviron;

impl Violation for SecretInEnviron {
    #[derive_message_formats]
    fn message(&self) -> String {
        "A secret written into `os.environ` is inherited by the whole worker".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Hand the secret to the child process in its own `env=` mapping".to_string())
    }
}

const REQUESTS_CALLS: &[&str] = &[
    "get", "post", "put", "patch", "delete", "head", "options", "request", "Session", "session",
];
const HTTPX_CALLS: &[&str] = &[
    "get",
    "post",
    "put",
    "patch",
    "delete",
    "head",
    "options",
    "request",
    "stream",
    "Client",
    "AsyncClient",
];
const MODBUS_NETWORK_CLIENTS: &[&str] = &[
    "ModbusTcpClient",
    "ModbusTlsClient",
    "ModbusUdpClient",
    "AsyncModbusTcpClient",
    "AsyncModbusTlsClient",
    "AsyncModbusUdpClient",
];
const DIAL_CALLS: &[&str] = &[
    "urllib.request.urlopen",
    "zeep.Transport",
    "zeep.transports.Transport",
    "boto3.client",
    "boto3.resource",
    "boto3.Session",
    "boto3.session.Session",
    "xmlrpc.client.ServerProxy",
    "xmlrpc.client.Server",
    "http.client.HTTPConnection",
    "http.client.HTTPSConnection",
    "socket.create_connection",
    "websocket.WebSocketApp",
    "websocket.WebSocket",
    "websocket.create_connection",
    "paho.mqtt.client.Client",
    "paramiko.SSHClient",
    "paramiko.Transport",
    "smtplib.SMTP",
    "smtplib.SMTP_SSL",
    "imaplib.IMAP4",
    "imaplib.IMAP4_SSL",
    "poplib.POP3",
    "poplib.POP3_SSL",
    "ftplib.FTP",
    "ftplib.FTP_TLS",
];

/// The method each client opens its connection through: answered by
/// `netguard.dial`, the subclass connects only to the addresses `ir.egress`
/// checked.
fn socket_hook(client: &str) -> Option<&'static str> {
    match client {
        "smtplib.SMTP" | "smtplib.SMTP_SSL" => Some("_get_socket"),
        "imaplib.IMAP4" | "imaplib.IMAP4_SSL" | "poplib.POP3" | "poplib.POP3_SSL" => {
            Some("_create_socket")
        }
        _ => None,
    }
}

const NETGUARD_DIAL: &str = "odoo.libs.netguard.dial";

static SECRET_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)KEY|TOKEN|SECRET|PASSW|CREDENTIAL").expect("valid regex"));

/// `a.b.c` for an attribute chain on a name; empty for anything else.
fn dotted(expr: &Expr) -> String {
    let mut parts = Vec::new();
    let mut expr = expr;
    while let Expr::Attribute(attribute) = expr {
        parts.push(attribute.attr.as_str());
        expr = &attribute.value;
    }
    let Expr::Name(name) = expr else {
        return String::new();
    };
    parts.push(name.id.as_str());
    parts.reverse();
    parts.join(".")
}

/// What the module's imports bind, anywhere in it: `import a.b as c` binds `c`
/// to `a.b`, a plain `import a.b` binds `a` (the call then spells `a.b.x`
/// itself), and `from m import x as y` binds `y` to `m.x`.
#[derive(Default)]
struct Imports {
    modules: FxHashMap<String, String>,
    names: FxHashMap<String, String>,
}

impl Imports {
    fn resolve(&self, dotted: &str) -> Option<String> {
        let (head, rest) = dotted.split_once('.').unwrap_or((dotted, ""));
        let target = self.modules.get(head).or_else(|| self.names.get(head))?;
        Some(if rest.is_empty() {
            target.clone()
        } else {
            format!("{target}.{rest}")
        })
    }

    /// The expression's dotted name with its head resolved, or as spelled.
    fn resolved(&self, expr: &Expr) -> String {
        let dotted = dotted(expr);
        self.resolve(&dotted).unwrap_or(dotted)
    }

    fn egress_target(&self, func: &Expr) -> Option<String> {
        let dotted = dotted(func);
        if dotted.is_empty() {
            return None;
        }
        let dotted = self.resolve(&dotted)?;
        let (module, attr) = dotted.rsplit_once('.').unwrap_or(("", &dotted));
        let dials = match module {
            "requests" => REQUESTS_CALLS.contains(&attr),
            "requests.sessions" => matches!(attr, "Session" | "session"),
            "httpx" => HTTPX_CALLS.contains(&attr),
            "aiohttp" | "aiohttp.client" => matches!(attr, "ClientSession" | "request"),
            "urllib3" => matches!(attr, "PoolManager" | "ProxyManager" | "request"),
            "pymodbus.client" => MODBUS_NETWORK_CLIENTS
                .iter()
                .any(|client| attr.starts_with(client)),
            _ => false,
        } || DIAL_CALLS.contains(&dotted.as_str());
        dials.then_some(dotted)
    }
}

impl<'a> Visitor<'a> for Imports {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::Import(import) => {
                for alias in &import.names {
                    let name = alias.name.as_str();
                    match &alias.asname {
                        Some(asname) => {
                            self.modules.insert(asname.to_string(), name.to_string());
                        }
                        None => {
                            let top = name.split('.').next().unwrap_or(name);
                            self.modules.insert(top.to_string(), top.to_string());
                        }
                    }
                }
            }
            Stmt::ImportFrom(import) => {
                if let Some(module) = &import.module {
                    for alias in &import.names {
                        let bound = alias.asname.as_ref().unwrap_or(&alias.name);
                        self.names
                            .insert(bound.to_string(), format!("{module}.{}", alias.name));
                    }
                }
            }
            _ => {}
        }
        visitor::walk_stmt(self, stmt);
    }
}

/// Every class of the module, at any depth, and every call.
#[derive(Default)]
struct Nodes<'a> {
    classes: Vec<&'a ast::StmtClassDef>,
    calls: Vec<&'a ast::ExprCall>,
}

impl<'a> Visitor<'a> for Nodes<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let Stmt::ClassDef(class) = stmt {
            self.classes.push(class);
        }
        visitor::walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr {
            self.calls.push(call);
        }
        visitor::walk_expr(self, expr);
    }
}

/// Whether a socket hook calls `netguard.dial`, and whether it calls its own
/// `super()` method.
fn hook_calls(statement: &Stmt, hook: &ast::StmtFunctionDef, imports: &Imports) -> (bool, bool) {
    let mut calls = Nodes::default();
    visitor::walk_stmt(&mut calls, statement);
    let mut dials = false;
    let mut supers = false;
    for call in calls.calls {
        if imports.resolved(&call.func) == NETGUARD_DIAL {
            dials = true;
        } else if let Expr::Attribute(attribute) = &*call.func
            && attribute.attr.as_str() == hook.name.as_str()
            && let Expr::Call(inner) = &*attribute.value
            && dotted(&inner.func) == "super"
        {
            supers = true;
        }
    }
    (dials, supers)
}

/// The classes that subclass a socket client and connect through
/// `netguard.dial`, directly or through a pinned class of the module, with the
/// hook they pin.
fn pinned_classes<'a>(
    classes: &[&'a ast::StmtClassDef],
    imports: &Imports,
) -> FxHashMap<&'a str, &'static str> {
    let mut pinned: FxHashMap<&str, &'static str> = FxHashMap::default();
    let mut ordered = classes.to_vec();
    ordered.sort_by_key(Ranged::start);
    for class in ordered {
        let clients: Vec<&'static str> = class
            .bases()
            .iter()
            .filter_map(|base| socket_hook(&imports.resolved(base)))
            .collect();
        let local: Vec<&'static str> = class
            .bases()
            .iter()
            .filter_map(|base| match base {
                Expr::Name(name) => pinned.get(name.id.as_str()).copied(),
                _ => None,
            })
            .collect();
        if clients.is_empty() && local.is_empty() {
            continue;
        }
        let hook_names: Vec<&'static str> = clients.iter().chain(&local).copied().collect();
        let calls: Vec<(bool, bool)> = class
            .body
            .iter()
            .filter_map(|statement| match statement {
                Stmt::FunctionDef(hook)
                    if !hook.is_async && hook_names.contains(&hook.name.as_str()) =>
                {
                    Some(hook_calls(statement, hook, imports))
                }
                _ => None,
            })
            .collect();
        if calls.iter().any(|(dials, _)| *dials)
            || (!local.is_empty() && calls.iter().all(|(_, supers)| *supers))
        {
            pinned.insert(class.name.as_str(), hook_names[0]);
        }
    }
    pinned
}

/// E8518
fn raw_egress(checker: &Checker, suite: &Suite, imports: &Imports) {
    let mut nodes = Nodes::default();
    nodes.visit_body(suite);
    let pinned = pinned_classes(&nodes.classes, imports);
    for call in nodes.calls {
        if let Some(target) = imports.egress_target(&call.func) {
            checker.report_diagnostic(
                RawEgress {
                    target,
                    subclass: None,
                },
                call.range(),
            );
        }
    }
    for class in nodes.classes {
        if pinned.contains_key(class.name.as_str()) {
            continue;
        }
        for base in class.bases() {
            if let Some(target) = imports.egress_target(base) {
                let start = class_start(class, checker.source());
                checker.report_diagnostic(
                    RawEgress {
                        target,
                        subclass: Some(class.name.to_string()),
                    },
                    TextRange::new(start, class.name.end()),
                );
            }
        }
    }
}

fn is_secret_key(expr: Option<&Expr>) -> bool {
    matches!(expr, Some(Expr::StringLiteral(key)) if SECRET_NAME.is_match(key.value.to_str()))
}

fn has_secret_key(expr: &Expr) -> bool {
    matches!(expr, Expr::Dict(dict) if dict.items.iter().any(|item| is_secret_key(item.key.as_ref())))
}

/// The spellings `os.environ` goes by in the module: imported, or aliased by a
/// plain assignment.
struct Environ {
    spellings: Vec<String>,
}

impl<'a> Visitor<'a> for Environ {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let Stmt::Assign(ast::StmtAssign { targets, value, .. }) = stmt
            && let [Expr::Name(target)] = targets.as_slice()
            && self.spellings.contains(&dotted(value))
        {
            self.spellings.push(target.id.to_string());
        }
        visitor::walk_stmt(self, stmt);
    }
}

/// What writes a secret into `os.environ`.
struct Writes<'s> {
    environ: &'s [String],
    putenv: &'s [String],
    found: Vec<TextRange>,
}

impl Writes<'_> {
    fn is_environ(&self, expr: &Expr) -> bool {
        self.environ.contains(&dotted(expr))
    }
}

impl<'a> Visitor<'a> for Writes<'_> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::Assign(ast::StmtAssign { targets, .. }) => {
                for target in targets {
                    if let Expr::Subscript(subscript) = target
                        && self.is_environ(&subscript.value)
                        && is_secret_key(Some(&subscript.slice))
                    {
                        self.found.push(stmt.range());
                    }
                }
            }
            Stmt::AugAssign(ast::StmtAugAssign {
                target,
                op: Operator::BitOr,
                value,
                ..
            }) if self.is_environ(target) && has_secret_key(value) => {
                self.found.push(stmt.range());
            }
            _ => {}
        }
        visitor::walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr {
            let dotted = dotted(&call.func);
            let (receiver, method) = dotted.rsplit_once('.').unwrap_or(("", &dotted));
            let receiver_is_environ = self.environ.iter().any(|spelling| spelling == receiver);
            let sets =
                (receiver_is_environ && method == "setdefault") || self.putenv.contains(&dotted);
            if sets && let Some(first) = call.arguments.args.first() {
                if is_secret_key(Some(first)) {
                    self.found.push(call.range());
                }
            } else if receiver_is_environ && method == "update" {
                for arg in &call.arguments.args {
                    if has_secret_key(arg) {
                        self.found.push(call.range());
                    }
                }
                if call.arguments.keywords.iter().any(|keyword| {
                    keyword
                        .arg
                        .as_ref()
                        .is_some_and(|arg| SECRET_NAME.is_match(arg.as_str()))
                }) {
                    self.found.push(call.range());
                }
            }
        }
        visitor::walk_expr(self, expr);
    }
}

/// E8519
fn secret_in_environ(checker: &Checker, suite: &Suite, imports: &Imports) {
    let mut spellings: Vec<String> = imports
        .modules
        .iter()
        .filter(|(_, module)| module.as_str() == "os")
        .map(|(alias, _)| format!("{alias}.environ"))
        .collect();
    spellings.extend(
        imports
            .names
            .iter()
            .filter(|(_, target)| target.as_str() == "os.environ")
            .map(|(name, _)| name.clone()),
    );
    let mut environ = Environ { spellings };
    environ.visit_body(suite);
    let environ = environ.spellings;
    let mut putenv: Vec<String> = environ
        .iter()
        .filter_map(|spelling| spelling.rsplit_once('.'))
        .filter(|(alias, _)| !alias.is_empty())
        .map(|(alias, _)| format!("{alias}.putenv"))
        .collect();
    putenv.extend(
        imports
            .names
            .iter()
            .filter(|(_, target)| target.as_str() == "os.putenv")
            .map(|(name, _)| name.clone()),
    );
    let mut writes = Writes {
        environ: &environ,
        putenv: &putenv,
        found: Vec::new(),
    };
    writes.visit_body(suite);
    for range in writes.found {
        checker.report_diagnostic(SecretInEnviron, range);
    }
}

/// E8518, E8519
pub(crate) fn egress(checker: &Checker, suite: &Suite) {
    let path = checker.path();
    if is_test_path(path) {
        return;
    }
    let mut imports = Imports::default();
    imports.visit_body(suite);
    if checker.is_rule_enabled(Rule::RawEgress) && in_addon(path) {
        raw_egress(checker, suite, &imports);
    }
    if checker.is_rule_enabled(Rule::SecretInEnviron) {
        secret_in_environ(checker, suite, &imports);
    }
}
