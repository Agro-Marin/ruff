use ruff_formatter::{Format, FormatResult, format_args};
use ruff_python_ast::AnyNodeRef;
use ruff_python_ast::PatternMatchSequence;
use ruff_python_trivia::{SimpleTokenKind, SimpleTokenizer};
use ruff_text_size::{Ranged, TextRange};

use crate::expression::parentheses::{
    NeedsParentheses, OptionalParentheses, empty_parenthesized, optional_parentheses, parenthesized,
};
use crate::prelude::*;

#[derive(Default)]
pub struct FormatPatternMatchSequence;

impl FormatNodeRule<PatternMatchSequence> for FormatPatternMatchSequence {
    fn fmt_fields(&self, item: &PatternMatchSequence, f: &mut PyFormatter) -> FormatResult<()> {
        let PatternMatchSequence {
            patterns,
            range,
            node_index: _,
        } = item;

        let comments = f.context().comments().clone();
        let dangling = comments.dangling(item);

        let sequence_type = SequenceType::from_pattern(item, f.context().source());

        match (patterns.as_slice(), sequence_type) {
            ([], SequenceType::Tuple | SequenceType::TupleNoParens) => {
                return empty_parenthesized("(", dangling, ")").fmt(f);
            }
            ([], SequenceType::List) => return empty_parenthesized("[", dangling, "]").fmt(f),

            ([elt], SequenceType::Tuple | SequenceType::TupleNoParens) => {
                return parenthesized("(", &format_args![elt.format(), token(",")], ")")
                    .with_dangling_comments(dangling)
                    .fmt(f);
            }

            _ => {}
        }

        let items = format_with(|f| {
            f.join_comma_separated(range.end())
                .nodes(patterns.iter())
                .finish()
        });
        match sequence_type {
            SequenceType::Tuple => parenthesized("(", &items, ")")
                .with_dangling_comments(dangling)
                .fmt(f),
            SequenceType::List => parenthesized("[", &items, "]")
                .with_dangling_comments(dangling)
                .fmt(f),
            SequenceType::TupleNoParens => optional_parentheses(&items).fmt(f),
        }
    }
}

impl NeedsParentheses for PatternMatchSequence {
    fn needs_parentheses(
        &self,
        _parent: AnyNodeRef,
        _context: &PyFormatContext,
    ) -> OptionalParentheses {
        OptionalParentheses::Never
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum SequenceType {
    /// A list literal, e.g., `[1, 2, 3]`.
    List,
    /// A parenthesized tuple literal, e.g., `(1, 2, 3)`.
    Tuple,
    /// A tuple literal without parentheses, e.g., `1, 2, 3`.
    TupleNoParens,
}

impl SequenceType {
    pub(crate) fn from_pattern(pattern: &PatternMatchSequence, source: &str) -> SequenceType {
        let before_first_pattern = &source[TextRange::new(
            pattern.start(),
            pattern
                .patterns
                .first()
                .map(Ranged::start)
                .unwrap_or(pattern.end()),
        )];
        let after_last_pattern = &source[TextRange::new(
            pattern.start(),
            pattern
                .patterns
                .first()
                .map(Ranged::end)
                .unwrap_or(pattern.end()),
        )];

        if before_first_pattern.starts_with('[') && !after_last_pattern.ends_with(',') {
            SequenceType::List
        } else if before_first_pattern.starts_with('(') {
            let Some(elt) = pattern.patterns.first() else {
                return SequenceType::Tuple;
            };

            let open_parentheses_count =
                SimpleTokenizer::new(source, TextRange::new(pattern.start(), elt.start()))
                    .skip_trivia()
                    .filter(|token| token.kind() == SimpleTokenKind::LParen)
                    .count();

            let close_parentheses_count =
                SimpleTokenizer::new(source, TextRange::new(elt.end(), elt.end()))
                    .skip_trivia()
                    .take_while(|token| token.kind() != SimpleTokenKind::Comma)
                    .filter(|token| token.kind() == SimpleTokenKind::RParen)
                    .count();

            if open_parentheses_count > close_parentheses_count {
                SequenceType::Tuple
            } else {
                SequenceType::TupleNoParens
            }
        } else {
            SequenceType::TupleNoParens
        }
    }

    pub(crate) fn is_parenthesized(self) -> bool {
        matches!(self, SequenceType::List | SequenceType::Tuple)
    }
}
