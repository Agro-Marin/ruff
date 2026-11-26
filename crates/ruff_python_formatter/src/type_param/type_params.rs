use ruff_formatter::FormatResult;
use ruff_python_ast::TypeParams;
use ruff_text_size::Ranged;

use crate::builders::PyFormatterExtensions;
use crate::expression::parentheses::parenthesized;
use crate::prelude::*;

#[derive(Default)]
pub struct FormatTypeParams;

/// Formats a sequence of [`TypeParam`](ruff_python_ast::TypeParam) nodes.
impl FormatNodeRule<TypeParams> for FormatTypeParams {
    fn fmt_fields(&self, item: &TypeParams, f: &mut PyFormatter) -> FormatResult<()> {
        let comments = f.context().comments().clone();
        let dangling_comments = comments.dangling(item);

        let items = format_with(|f| {
            f.join_comma_separated(item.end())
                .nodes(item.type_params.iter())
                .finish()
        });

        parenthesized("[", &items, "]")
            .with_dangling_comments(dangling_comments)
            .fmt(f)
    }
}
