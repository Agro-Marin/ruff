use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_source_file::Line;

use crate::codes::Category;
use crate::{Violation, checkers::ast::LintContext};

const BIDI_UNICODE: [char; 11] = [
    '\u{202A}',
    '\u{202B}',
    '\u{202C}',
    '\u{202D}',
    '\u{202E}',
    '\u{2066}',
    '\u{2067}',
    '\u{2068}',
    '\u{2069}',
    '\u{200F}',
    '\u{061C}',
];

/// ## What it does
/// Checks for bidirectional formatting characters.
///
/// ## Why is this bad?
/// The interaction between bidirectional formatting characters and the
/// surrounding code can be surprising to those that are unfamiliar
/// with right-to-left writing systems.
///
/// In some cases, bidirectional formatting characters can also be used to
/// obfuscate code and introduce or mask security vulnerabilities.
///
/// ## Example
/// ```python
/// example = "x‏" * 100  #    "‏x" is assigned
/// ```
///
/// The example uses two `RIGHT-TO-LEFT MARK`s to make the `100 * ` appear inside the comment.
/// Without the `RIGHT-TO-LEFT MARK`s, the code looks like this:
///
/// ```py
/// example = "x" * 100  #    "x" is assigned
/// ```
///
/// ## References
/// - [PEP 672: Bidirectional Marks, Embeddings, Overrides and Isolates](https://peps.python.org/pep-0672/#bidirectional-marks-embeddings-overrides-and-isolates)
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.244", category = Category::Suspicious)]
pub(crate) struct BidirectionalUnicode;

impl Violation for BidirectionalUnicode {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Contains control characters that can permit obfuscated code".to_string()
    }
}

/// PLE2502
pub(crate) fn bidirectional_unicode(line: &Line, context: &LintContext) {
    let text = line.as_str();
    if let Some(start) = memchr::memchr2(0xD8, 0xE2, text.as_bytes())
        && text[start..].contains(BIDI_UNICODE)
    {
        context.report_diagnostic(BidirectionalUnicode, line.full_range());
    }
}
