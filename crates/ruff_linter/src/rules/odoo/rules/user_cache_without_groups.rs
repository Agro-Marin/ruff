use std::sync::LazyLock;

use regex::Regex;
use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::odoo::helpers::{callee_name, is_test_path};

/// ## What it does
/// Checks for an `@ormcache` keyed on the user (`uid`, `env.user`) but not on
/// what the user holds.
///
/// ## Why is this bad?
/// A membership change clears the users' group states and nothing else: a
/// value cached per user survives a membership, a grant or a company switch.
/// `self.env.access_signature` carries the uid, the groups the user holds in
/// the companies in use and those companies, so a change reads a new key. A
/// value that does not depend on the user's groups (a language, a password
/// check, the user's own `ir.default` rows) says so with a `noqa`.
///
/// A cache in the `memberships` bucket is cleared by a membership and is not
/// reported.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Correctness)]
pub(crate) struct UserCacheWithoutGroups {
    name: String,
}

impl Violation for UserCacheWithoutGroups {
    #[derive_message_formats]
    fn message(&self) -> String {
        let UserCacheWithoutGroups { name } = self;
        format!("`{name}` is cached per user but not per what the user holds")
    }

    fn fix_title(&self) -> Option<String> {
        Some("Key the cache on `self.env.access_signature`".to_string())
    }
}

static READS_THE_USER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b_?uid\b|\benv\.user\b").expect("valid regex"));
static FOLLOWS_THE_GROUPS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\baccess_signature\b|\b_get_group_ids\b|\b_get_group_signature\b|\.share\b")
        .expect("valid regex")
});

/// E8540
pub(crate) fn user_cache_without_groups(checker: &Checker, function: &ast::StmtFunctionDef) {
    if is_test_path(checker.path()) {
        return;
    }
    for decorator in &function.decorator_list {
        let Expr::Call(call) = &decorator.expression else {
            continue;
        };
        if callee_name(&call.func) != "ormcache" {
            continue;
        }
        let bucket = call
            .arguments
            .keywords
            .iter()
            .find(|keyword| {
                keyword
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.as_str() == "cache")
                    && keyword.value.is_literal_expr()
            })
            .map(|keyword| &keyword.value);
        if matches!(bucket, Some(Expr::StringLiteral(name)) if name.value.to_str() == "memberships")
        {
            continue;
        }
        let keys: Vec<&str> = call
            .arguments
            .args
            .iter()
            .filter_map(|arg| match arg {
                Expr::StringLiteral(key) => Some(key.value.to_str()),
                _ => None,
            })
            .collect();
        if keys.iter().any(|key| READS_THE_USER.is_match(key))
            && !keys.iter().any(|key| FOLLOWS_THE_GROUPS.is_match(key))
        {
            checker.report_diagnostic(
                UserCacheWithoutGroups {
                    name: function.name.to_string(),
                },
                call.range(),
            );
        }
    }
}
