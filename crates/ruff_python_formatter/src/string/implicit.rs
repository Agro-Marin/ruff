use itertools::Itertools;
use ruff_formatter::{FormatContext, format_args, write};
use ruff_python_ast::str::{Quote, TripleQuotes};
use ruff_python_ast::str_prefix::{
    AnyStringPrefix, ByteStringPrefix, FStringPrefix, StringLiteralPrefix, TStringPrefix,
};
use ruff_python_ast::{
    AnyStringFlags, FString, InterpolatedStringElement, StringFlags, StringLike, StringLikePart,
    TString,
};
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextRange};
use std::borrow::Cow;

use crate::comments::{leading_comments, trailing_comments};
use crate::context::WithInterpolatedStringState;
use crate::expression::parentheses::in_parentheses_only_soft_line_break_or_space;
use crate::other::interpolated_string::{InterpolatedStringContext, InterpolatedStringLayout};
use crate::other::interpolated_string_element::FormatInterpolatedElement;
use crate::prelude::*;
use crate::string::docstring::needs_chaperone_space;
use crate::string::normalize::{
    QuoteMetadata, is_fstring_with_quoted_debug_expression,
    is_fstring_with_triple_quoted_literal_expression_containing_quotes,
    is_interpolated_string_with_quoted_format_spec_and_debug,
};
use crate::string::{StringLikeExtensions, StringNormalizer, StringQuotes, normalize_string};

/// Formats any implicitly concatenated string. This could be any valid combination
/// of string, bytes, f-string, or t-string literals.
pub(crate) struct FormatImplicitConcatenatedString<'a> {
    string: StringLike<'a>,
}

impl<'a> FormatImplicitConcatenatedString<'a> {
    pub(crate) fn new(string: impl Into<StringLike<'a>>) -> Self {
        Self {
            string: string.into(),
        }
    }
}

impl Format<PyFormatContext<'_>> for FormatImplicitConcatenatedString<'_> {
    fn fmt(&self, f: &mut PyFormatter) -> FormatResult<()> {
        let flat = FormatImplicitConcatenatedStringFlat::new(self.string, f.context());
        let expanded = FormatImplicitConcatenatedStringExpanded::new(
            self.string,
            if flat.is_some() {
                ImplicitConcatenatedLayout::MaybeFlat
            } else {
                ImplicitConcatenatedLayout::Multipart
            },
        );

        if let Some(flat) = flat {
            write!(
                f,
                [if_group_fits_on_line(&flat), if_group_breaks(&expanded)]
            )
        } else {
            expanded.fmt(f)
        }
    }
}

/// Formats an implicit concatenated string where parts are separated by a space or line break.
pub(crate) struct FormatImplicitConcatenatedStringExpanded<'a> {
    string: StringLike<'a>,
    layout: ImplicitConcatenatedLayout,
}

impl<'a> FormatImplicitConcatenatedStringExpanded<'a> {
    pub(crate) fn new(string: StringLike<'a>, layout: ImplicitConcatenatedLayout) -> Self {
        assert!(string.is_implicit_concatenated());

        Self { string, layout }
    }
}

impl Format<PyFormatContext<'_>> for FormatImplicitConcatenatedStringExpanded<'_> {
    fn fmt(&self, f: &mut Formatter<PyFormatContext<'_>>) -> FormatResult<()> {
        let comments = f.context().comments().clone();

        if matches!(self.layout, ImplicitConcatenatedLayout::Multipart)
            && self.string.parts().tuple_windows().any(|(a, b)| {
                f.context()
                    .source()
                    .contains_line_break(TextRange::new(a.end(), b.start()))
            })
        {
            expand_parent().fmt(f)?;
        }

        let mut joiner = f.join_with(in_parentheses_only_soft_line_break_or_space());

        for part in self.string.parts() {
            let format_part = format_with(|f: &mut PyFormatter| match part {
                StringLikePart::String(part) => part.format().fmt(f),
                StringLikePart::Bytes(bytes_literal) => bytes_literal.format().fmt(f),
                StringLikePart::FString(part) => part.format().fmt(f),
                StringLikePart::TString(part) => part.format().fmt(f),
            });

            let part_comments = comments.leading_dangling_trailing(part);
            joiner.entry(&format_args![
                leading_comments(part_comments.leading),
                format_part,
                trailing_comments(part_comments.trailing)
            ]);
        }

        joiner.finish()
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum ImplicitConcatenatedLayout {
    /// The string might get joined into a single string if it fits on a single line.
    MaybeFlat,
    /// The string will remain a multipart string.
    Multipart,
}

/// Formats an implicit concatenated string where parts are joined into a single string if possible.
pub(crate) struct FormatImplicitConcatenatedStringFlat<'a> {
    string: StringLike<'a>,
    flags: AnyStringFlags,
    docstring: bool,
}

impl<'a> FormatImplicitConcatenatedStringFlat<'a> {
    /// Creates a new formatter. Returns `None` if the string can't be merged into a single string.
    pub(crate) fn new(string: StringLike<'a>, context: &PyFormatContext) -> Option<Self> {
        fn merge_flags(string: StringLike, context: &PyFormatContext) -> Option<AnyStringFlags> {
            if string.is_multiline(context) {
                return None;
            }

            let first_part = string.parts().next()?;

            let normalizer = StringNormalizer::from_context(context);

            let mut preserve_quotes_requirement: Option<Quote> = None;

            for part in string.parts() {
                if part.flags().is_triple_quoted() || part.flags().is_raw_string() {
                    return None;
                }

                if context.comments().leading_trailing(&part).next().is_some() {
                    return None;
                }

                match part {
                    StringLikePart::FString(fstring) => {
                        if matches!(string, StringLike::TString(_)) {
                            return None;
                        }
                        if context.options().target_version().supports_pep_701() {
                            if is_interpolated_string_with_quoted_format_spec_and_debug(
                                &fstring.elements,
                                fstring.flags.into(),
                                context,
                            ) {
                                if preserve_quotes_requirement
                                    .is_some_and(|quote| quote != part.flags().quote_style())
                                {
                                    return None;
                                }
                                preserve_quotes_requirement = Some(part.flags().quote_style());
                            }
                        }
                        else if is_fstring_with_quoted_debug_expression(fstring, context)
                            || is_fstring_with_triple_quoted_literal_expression_containing_quotes(
                                fstring, context,
                            )
                        {
                            if preserve_quotes_requirement
                                .is_some_and(|quote| quote != part.flags().quote_style())
                            {
                                return None;
                            }
                            preserve_quotes_requirement = Some(part.flags().quote_style());
                        }
                    }
                    StringLikePart::TString(tstring) => {
                        if is_interpolated_string_with_quoted_format_spec_and_debug(
                            &tstring.elements,
                            tstring.flags.into(),
                            context,
                        ) {
                            if preserve_quotes_requirement
                                .is_some_and(|quote| quote != part.flags().quote_style())
                            {
                                return None;
                            }
                            preserve_quotes_requirement = Some(part.flags().quote_style());
                        }
                    }
                    StringLikePart::Bytes(_) | StringLikePart::String(_) => {}
                }
            }

            let mut merged_quotes: Option<QuoteMetadata> = None;

            let prefix = match string {
                StringLike::String(_) => AnyStringPrefix::Regular(StringLiteralPrefix::Empty),
                StringLike::Bytes(_) => AnyStringPrefix::Bytes(ByteStringPrefix::Regular),
                StringLike::FString(_) => AnyStringPrefix::Format(FStringPrefix::Regular),
                StringLike::TString(_) => AnyStringPrefix::Template(TStringPrefix::Regular),
            };

            let quote = if let Some(quote) = preserve_quotes_requirement {
                quote
            } else {
                if let Ok(preferred_quote) =
                    Quote::try_from(normalizer.preferred_quote_style(first_part))
                {
                    for part in string.parts() {
                        let part_quote_metadata =
                            QuoteMetadata::from_part(part, context, preferred_quote);

                        if let Some(merged) = merged_quotes.as_mut() {
                            *merged = part_quote_metadata.merge(merged)?;
                        } else {
                            merged_quotes = Some(part_quote_metadata);
                        }
                    }

                    merged_quotes?.choose(preferred_quote)
                } else {
                    first_part.flags().quote_style()
                }
            };

            Some(AnyStringFlags::new(prefix, quote, TripleQuotes::No))
        }

        if !string.is_implicit_concatenated() {
            return None;
        }

        Some(Self {
            flags: merge_flags(string, context)?,
            string,
            docstring: false,
        })
    }

    pub(crate) fn set_docstring(&mut self, is_docstring: bool) {
        self.docstring = is_docstring;
    }

    pub(crate) fn string(&self) -> StringLike<'a> {
        self.string
    }
}

impl Format<PyFormatContext<'_>> for FormatImplicitConcatenatedStringFlat<'_> {
    fn fmt(&self, f: &mut Formatter<PyFormatContext<'_>>) -> FormatResult<()> {
        let quotes = StringQuotes::from(self.flags);

        write!(f, [self.flags.prefix(), quotes])?;

        let mut parts = self.string.parts().peekable();

        if self.docstring {
            for part in self.string.parts().rev() {
                assert!(part.is_string_literal());

                if f.context().source()[part.content_range()].trim().is_empty() {
                    parts.next_back();
                } else {
                    break;
                }
            }
        }

        let mut first_non_empty = self.docstring;

        while let Some(part) = parts.next() {
            match part {
                StringLikePart::String(_) | StringLikePart::Bytes(_) => {
                    FormatLiteralContent {
                        range: part.content_range(),
                        flags: self.flags,
                        is_interpolated_string: false,
                        trim_start: first_non_empty && self.docstring,
                        trim_end: self.docstring && parts.peek().is_none(),
                    }
                    .fmt(f)?;

                    if first_non_empty {
                        first_non_empty = f.context().source()[part.content_range()]
                            .trim_start()
                            .is_empty();
                    }
                }

                StringLikePart::FString(FString { elements, .. })
                | StringLikePart::TString(TString { elements, .. }) => {
                    let context = InterpolatedStringContext::new(
                        self.flags,
                        InterpolatedStringLayout::from_interpolated_string_elements(
                            elements,
                            f.context().source(),
                        ),
                    );
                    let state = f
                        .context()
                        .interpolated_string_state()
                        .enter_string(context);
                    let f = &mut WithInterpolatedStringState::new(state, &mut *f);
                    for element in elements {
                        match element {
                            InterpolatedStringElement::Literal(literal) => {
                                FormatLiteralContent {
                                    range: literal.range(),
                                    flags: self.flags,
                                    is_interpolated_string: true,
                                    trim_end: false,
                                    trim_start: false,
                                }
                                .fmt(f)?;
                            }
                            InterpolatedStringElement::Interpolation(expression) => {
                                FormatInterpolatedElement::new(expression, context).fmt(f)?;
                            }
                        }
                    }
                }
            }
        }

        quotes.fmt(f)
    }
}

struct FormatLiteralContent {
    range: TextRange,
    flags: AnyStringFlags,
    is_interpolated_string: bool,
    trim_start: bool,
    trim_end: bool,
}

impl Format<PyFormatContext<'_>> for FormatLiteralContent {
    fn fmt(&self, f: &mut PyFormatter) -> FormatResult<()> {
        let content = &f.context().source()[self.range];
        let mut normalized = normalize_string(
            content,
            0,
            self.flags,
            self.flags.is_interpolated_string() && !self.is_interpolated_string,
        );

        if self.trim_start {
            let trimmed = normalized.trim_start();
            if trimmed.len() < normalized.len() {
                normalized = trimmed.to_string().into();
            }
        }

        if self.trim_end {
            let trimmed = normalized.trim_end();
            if trimmed.len() < normalized.len() {
                normalized = trimmed.to_string().into();
            }
        }

        if !normalized.is_empty() {
            match &normalized {
                Cow::Borrowed(_) => source_text_slice(self.range).fmt(f)?,
                Cow::Owned(normalized) => text(normalized).fmt(f)?,
            }

            if self.trim_end && needs_chaperone_space(self.flags, &normalized) {
                space().fmt(f)?;
            }
        }
        Ok(())
    }
}
