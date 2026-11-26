use ruff_python_ast::{AnyNodeRef, ExprFString, StringLike};

use crate::expression::parentheses::{
    NeedsParentheses, OptionalParentheses, in_parentheses_only_group,
};
use crate::other::interpolated_string::InterpolatedStringLayout;
use crate::prelude::*;
use crate::string::StringLikeExtensions;
use crate::string::implicit::{
    FormatImplicitConcatenatedString, FormatImplicitConcatenatedStringFlat,
};

#[derive(Default)]
pub struct FormatExprFString;

impl FormatNodeRule<ExprFString> for FormatExprFString {
    fn fmt_fields(&self, item: &ExprFString, f: &mut PyFormatter) -> FormatResult<()> {
        if let Some(f_string) = item.as_single_part_fstring() {
            f_string.format().fmt(f)
        } else {
            if !f.context().node_level().is_parenthesized() {
                if let Some(format_flat) =
                    FormatImplicitConcatenatedStringFlat::new(item.into(), f.context())
                {
                    return format_flat.fmt(f);
                }
            }

            in_parentheses_only_group(&FormatImplicitConcatenatedString::new(item)).fmt(f)
        }
    }
}

impl NeedsParentheses for ExprFString {
    fn needs_parentheses(
        &self,
        _parent: AnyNodeRef,
        context: &PyFormatContext,
    ) -> OptionalParentheses {
        if let Some(fstring_part) = self.as_single_part_fstring() {
            if StringLike::FString(self).is_multiline(context)
                || InterpolatedStringLayout::from_interpolated_string_elements(
                    &fstring_part.elements,
                    context.source(),
                )
                .is_multiline()
            {
                OptionalParentheses::Never
            } else {
                OptionalParentheses::BestFit
            }
        } else {
            OptionalParentheses::Multiline
        }
    }
}
