use crate::comments::format::{
    empty_lines_after_leading_comments, empty_lines_before_trailing_comments,
};
use crate::expression::maybe_parenthesize_expression;
use crate::expression::parentheses::{Parentheses, Parenthesize};
use crate::prelude::*;
use crate::statement::clause::{ClauseHeader, clause};
use crate::statement::stmt_class_def::FormatDecorators;
use crate::statement::suite::SuiteKind;
use ruff_formatter::write;
use ruff_python_ast::{NodeKind, StmtFunctionDef};

#[derive(Default)]
pub struct FormatStmtFunctionDef;

impl FormatNodeRule<StmtFunctionDef> for FormatStmtFunctionDef {
    fn fmt_fields(&self, item: &StmtFunctionDef, f: &mut PyFormatter) -> FormatResult<()> {
        let StmtFunctionDef {
            decorator_list,
            body,
            ..
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
                    ClauseHeader::Function(item),
                    &format_with(|f| format_function_header(f, item)),
                    trailing_definition_comments,
                    body,
                    SuiteKind::Function,
                ),
            ]
        )?;

        empty_lines_before_trailing_comments(comments.trailing(item), NodeKind::StmtFunctionDef)
            .fmt(f)
    }
}

fn format_function_header(f: &mut PyFormatter, item: &StmtFunctionDef) -> FormatResult<()> {
    let StmtFunctionDef {
        range: _,
        node_index: _,
        is_async,
        decorator_list: _,
        name,
        type_params,
        parameters,
        returns,
        body: _,
    } = item;

    let comments = f.context().comments().clone();

    if *is_async {
        write!(f, [token("async"), space()])?;
    }

    write!(f, [token("def"), space(), name.format()])?;

    if let Some(type_params) = type_params.as_ref() {
        type_params.format().fmt(f)?;
    }

    let format_inner = format_with(|f: &mut PyFormatter| {
        parameters.format().fmt(f)?;

        if let Some(return_annotation) = returns.as_deref() {
            write!(f, [space(), token("->"), space()])?;

            if return_annotation.is_tuple_expr() {
                let parentheses = if comments.has_leading(return_annotation) {
                    Parentheses::Always
                } else {
                    Parentheses::Never
                };
                return_annotation.format().with_options(parentheses).fmt(f)
            } else if comments.has_trailing(return_annotation) {

                return_annotation
                    .format()
                    .with_options(Parentheses::Always)
                    .fmt(f)
            } else {
                let parenthesize = if parameters.is_empty() && !comments.has(parameters.as_ref()) {
                    Parenthesize::IfBreaksParenthesized
                } else {
                    Parenthesize::IfBreaks
                };
                maybe_parenthesize_expression(return_annotation, item, parenthesize).fmt(f)
            }
        } else {
            Ok(())
        }
    });

    group(&format_inner).fmt(f)
}
