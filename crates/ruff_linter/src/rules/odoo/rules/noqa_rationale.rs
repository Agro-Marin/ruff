use std::sync::LazyLock;

use regex::Regex;
use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_trivia::CommentRanges;
use ruff_text_size::TextRange;

use crate::Locator;
use crate::Violation;
use crate::checkers::ast::LintContext;
use crate::codes::Category;

/// ## What it does
/// Checks for a `noqa` comment that gives no reason after its codes.
///
/// ## Why is this bad?
/// A suppression without a reason cannot be reviewed: nobody can tell whether
/// the finding it hides is a false positive, accepted debt, or a mistake. The
/// reason is a few words after the codes, such as
/// `# noqa: F401  re-exported by __init__`.
///
/// This rule reads the comments themselves and cannot be suppressed.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "0.16.7", category = Category::Style)]
pub(crate) struct NoqaRationale;

impl Violation for NoqaRationale {
    #[derive_message_formats]
    fn message(&self) -> String {
        "`noqa` without a rationale".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some(
            "Write the reason after the codes: `# noqa: F401  re-exported by __init__`".to_string(),
        )
    }
}

/// `# noqa`, as `test_lint` reads it, without the lookahead `(?=$|[\s:])` the
/// regex crate lacks: [`rest_of_noqa`] checks the next character itself.
static NOQA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)#\s*noqa").expect("valid regex"));
/// After `noqa`: an optional colon, the comma-separated codes, and the rest.
static TAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A:?\s*(?:[A-Za-z][\w-]*(?:\s*,\s*[A-Za-z][\w-]*)*)?(?P<rest>.*)\z")
        .expect("valid regex")
});
/// ruff reads a space-separated code list (`E8501 E8502` after the colon) as
/// several codes, so codes after the first are no rationale.
static CODE_LEAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A(?:\s*[A-Z]+[0-9]+\b)+").expect("valid regex"));
static RATIONALE_LEAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A[\s\-—–:#>·•|]+").expect("valid regex"));
static LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{L}").expect("valid regex"));

const MIN_RATIONALE_CHARS: usize = 4;

/// What follows the codes of the comment's first `noqa`, if it has one.
fn rest_of_noqa(comment: &str) -> Option<&str> {
    NOQA.find_iter(comment).find_map(|noqa| {
        let after = &comment[noqa.end()..];
        if !after.is_empty() && !after.starts_with(|c: char| c.is_whitespace() || c == ':') {
            return None;
        }
        TAIL.captures(after)
            .and_then(|captures| captures.name("rest"))
            .map(|rest| rest.as_str())
    })
}

fn has_rationale(rest: &str) -> bool {
    let without_codes = CODE_LEAD.replace(rest, "");
    let cleaned = RATIONALE_LEAD.replace(&without_codes, "");
    let cleaned = cleaned.trim();
    cleaned.chars().count() >= MIN_RATIONALE_CHARS && LETTER.is_match(cleaned)
}

/// E8544
pub(crate) fn noqa_rationale(
    context: &LintContext,
    comment_ranges: &CommentRanges,
    locator: &Locator,
) {
    for range in comment_ranges {
        let comment = locator.slice(range);
        if let Some(rest) = rest_of_noqa(comment)
            && !has_rationale(rest)
        {
            context.report_diagnostic(NoqaRationale, TextRange::new(range.start(), range.end()));
        }
    }
}
