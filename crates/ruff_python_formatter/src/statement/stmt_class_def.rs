use ruff_formatter::write;
use ruff_python_ast::{Decorator, NodeKind, StmtClassDef};
use ruff_python_trivia::lines_after_ignoring_end_of_line_trivia;
use ruff_text_size::Ranged;

use crate::comments::format::{
    empty_lines_after_leading_comments, empty_lines_before_trailing_comments,
};
use crate::comments::{SourceComment, leading_comments, trailing_comments};
use crate::prelude::*;
use crate::statement::clause::{ClauseHeader, clause};
use crate::statement::suite::SuiteKind;

#[derive(Default)]
pub struct FormatStmtClassDef;

impl FormatNodeRule<StmtClassDef> for FormatStmtClassDef {
    fn fmt_fields(&self, item: &StmtClassDef, f: &mut PyFormatter) -> FormatResult<()> {
        let StmtClassDef {
            range: _,
            node_index: _,
            name,
            arguments,
            body,
            type_params,
            decorator_list,
        } = item;

        let comments = f.context().comments().clone();

        let dangling_comments = comments.dangling(item);
        let trailing_definition_comments_start =
            dangling_comments.partition_point(|comment| comment.line_position().is_own_line());

        let (leading_definition_comments, trailing_definition_comments) =
            dangling_comments.split_at(trailing_definition_comments_start);

        empty_lines_after_leading_comments(comments.leading(item)).fmt(f)?;

        write!(
            f,
            [
                FormatDecorators {
                    decorators: decorator_list,
                    leading_definition_comments,
                },
                clause(
                    ClauseHeader::Class(item),
                    &format_with(|f| {
                        write!(f, [token("class"), space(), name.format()])?;

                        if let Some(type_params) = type_params.as_deref() {
                            write!(f, [type_params.format()])?;
                        }

                        if let Some(arguments) = arguments.as_deref() {
                            if arguments.is_empty()
                                && comments
                                    .dangling(arguments)
                                    .iter()
                                    .all(|comment| comment.line_position().is_end_of_line())
                            {
                                let dangling = comments.dangling(arguments);
                                write!(f, [trailing_comments(dangling)])?;
                            } else {
                                write!(f, [arguments.format()])?;
                            }
                        }

                        Ok(())
                    }),
                    trailing_definition_comments,
                    body,
                    SuiteKind::Class,
                ),
            ]
        )?;

        empty_lines_before_trailing_comments(comments.trailing(item), NodeKind::StmtClassDef)
            .fmt(f)?;

        Ok(())
    }
}

pub(super) struct FormatDecorators<'a> {
    pub(super) decorators: &'a [Decorator],
    pub(super) leading_definition_comments: &'a [SourceComment],
}

impl Format<PyFormatContext<'_>> for FormatDecorators<'_> {
    fn fmt(&self, f: &mut Formatter<PyFormatContext<'_>>) -> FormatResult<()> {
        if let Some(last_decorator) = self.decorators.last() {
            f.join_with(hard_line_break())
                .entries(self.decorators.iter().formatted())
                .finish()?;

            if self.leading_definition_comments.is_empty() {
                write!(f, [hard_line_break()])?;
            } else {
                let leading_line = if lines_after_ignoring_end_of_line_trivia(
                    last_decorator.end(),
                    f.context().source(),
                ) <= 1
                {
                    hard_line_break()
                } else {
                    empty_line()
                };

                write!(
                    f,
                    [
                        leading_line,
                        leading_comments(self.leading_definition_comments)
                    ]
                )?;
            }
        }

        Ok(())
    }
}
