use ruff_python_trivia::{BackwardsTokenizer, CommentRanges, SimpleTokenKind, SimpleTokenizer};
use ruff_text_size::{Ranged, TextLen, TextRange};

use crate::AnyNodeRef;
use crate::ExprRef;

/// Returns an iterator over the ranges of the optional parentheses surrounding an expression.
///
/// E.g. for `((f()))` with `f()` as expression, the iterator returns the ranges (1, 6) and (0, 7).
///
/// Note that without a parent the range can be inaccurate, e.g. `f(a)` we falsely return a set of
/// parentheses around `a` even if the parentheses actually belong to `f`. That is why you should
/// generally prefer [`parenthesized_range`].
///
/// Prefer [`crate::token::parentheses_iterator`] if you have access to [`crate::token::Tokens`].
fn parentheses_iterator<'a>(
    expr: ExprRef<'a>,
    parent: Option<AnyNodeRef>,
    comment_ranges: &'a CommentRanges,
    source: &'a str,
) -> impl Iterator<Item = TextRange> + 'a {
    let right_tokenizer = if let Some(parent) = parent {
        let exclusive_parent_end = if parent.is_arguments() {
            parent.end() - ")".text_len()
        } else {
            parent.end()
        };
        SimpleTokenizer::new(source, TextRange::new(expr.end(), exclusive_parent_end))
    } else {
        SimpleTokenizer::starts_at(expr.end(), source)
    };

    let right_tokenizer = right_tokenizer
        .skip_trivia()
        .take_while(|token| token.kind == SimpleTokenKind::RParen);

    let left_tokenizer = BackwardsTokenizer::up_to(expr.start(), source, comment_ranges)
        .skip_trivia()
        .take_while(|token| token.kind == SimpleTokenKind::LParen);

    right_tokenizer
        .zip(left_tokenizer)
        .map(|(right, left)| TextRange::new(left.start(), right.end()))
}

/// Returns the [`TextRange`] of a given expression including parentheses, if the expression is
/// parenthesized; or `None`, if the expression is not parenthesized.
///
/// Prefer [`crate::token::parenthesized_range`] if you have access to [`crate::token::Tokens`].
pub fn parenthesized_range(
    expr: ExprRef,
    parent: AnyNodeRef,
    comment_ranges: &CommentRanges,
    source: &str,
) -> Option<TextRange> {
    parentheses_iterator(expr, Some(parent), comment_ranges, source).last()
}
