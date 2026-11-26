use ruff_formatter::{FormatRuleWithOptions, RemoveSoftLinesBuffer, format_args, write};
use ruff_python_ast::{AnyNodeRef, Expr, ExprLambda};
use ruff_text_size::Ranged;

use crate::builders::parenthesize_if_expands;
use crate::comments::{SourceComment, dangling_comments, leading_comments, trailing_comments};
use crate::expression::has_own_parentheses;
use crate::expression::parentheses::{NeedsParentheses, OptionalParentheses, Parentheses};
use crate::other::parameters::ParametersParentheses;
use crate::prelude::*;

#[derive(Default)]
pub struct FormatExprLambda {
    layout: ExprLambdaLayout,
}

impl FormatNodeRule<ExprLambda> for FormatExprLambda {
    fn fmt_fields(&self, item: &ExprLambda, f: &mut PyFormatter) -> FormatResult<()> {
        let ExprLambda {
            range: _,
            node_index: _,
            parameters,
            body,
        } = item;

        let body = &**body;
        let parameters = parameters.as_deref();

        let comments = f.context().comments().clone();
        let dangling = comments.dangling(item);

        write!(f, [token("lambda")])?;

        let dangling_header_comments = if let Some(parameters) = parameters {
            let (dangling_before_parameters, dangling_after_parameters) = dangling
                .split_at(dangling.partition_point(|comment| comment.end() < parameters.start()));

            if dangling_before_parameters.is_empty() {
                if comments.has_leading(parameters) {
                    hard_line_break().fmt(f)?;
                } else {
                    write!(f, [space()])?;
                }
            } else {
                write!(f, [dangling_comments(dangling_before_parameters)])?;
            }

            if !comments.contains_comments(parameters.into()) {
                let mut buffer = RemoveSoftLinesBuffer::new(f);
                write!(
                    buffer,
                    [parameters
                        .format()
                        .with_options(ParametersParentheses::Never)]
                )?;
            } else {
                write!(
                    f,
                    [parameters
                        .format()
                        .with_options(ParametersParentheses::Never)]
                )?;
            }

            dangling_after_parameters
        } else {
            dangling
        };

        write!(f, [token(":")])?;

        if dangling_header_comments.is_empty() {
            write!(f, [space()])?;
        }

        let fmt_body = FormatBody {
            body,
            dangling_header_comments,
            needs_parentheses: body.needs_parentheses(item.into(), f.context()),
        };

        match self.layout {
            ExprLambdaLayout::Assignment => fits_expanded(&fmt_body).fmt(f),
            ExprLambdaLayout::Default => fmt_body.fmt(f),
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub enum ExprLambdaLayout {
    #[default]
    Default,

    /// The [`ExprLambda`] is the direct child of an assignment expression, so it needs to use
    /// `fits_expanded` to prefer parenthesizing its own body before the assignment tries to
    /// parenthesize the whole lambda. For example, we want this formatting:
    ///
    /// ```py
    /// long_assignment_target = lambda x, y, z: (
    ///     x + y + z
    /// )
    /// ```
    ///
    /// instead of either of these:
    ///
    /// ```py
    /// long_assignment_target = (
    ///     lambda x, y, z: (
    ///         x + y + z
    ///     )
    /// )
    ///
    /// long_assignment_target = (
    ///     lambda x, y, z: x + y + z
    /// )
    /// ```
    Assignment,
}

impl FormatRuleWithOptions<ExprLambda, PyFormatContext<'_>> for FormatExprLambda {
    type Options = ExprLambdaLayout;

    fn with_options(mut self, options: Self::Options) -> Self {
        self.layout = options;
        self
    }
}

impl NeedsParentheses for ExprLambda {
    fn needs_parentheses(
        &self,
        parent: AnyNodeRef,
        _context: &PyFormatContext,
    ) -> OptionalParentheses {
        if parent.is_expr_await() {
            OptionalParentheses::Always
        } else {
            OptionalParentheses::Multiline
        }
    }
}

struct FormatBody<'a> {
    body: &'a Expr,

    /// Dangling comments attached to the lambda header that should be formatted with the body.
    ///
    /// These can include both own-line and end-of-line comments. For lambdas with parameters, this
    /// means comments after the parameters:
    ///
    /// ```py
    /// (
    ///     lambda x, y  # 1
    ///         # 2
    ///         :  # 3
    ///         # 4
    ///         x + y
    /// )
    /// ```
    ///
    /// Or all dangling comments for lambdas without parameters:
    ///
    /// ```py
    /// (
    ///     lambda  # 1
    ///         # 2
    ///         :  # 3
    ///         # 4
    ///         1
    /// )
    /// ```
    ///
    /// In most cases these should formatted within the parenthesized body, as in:
    ///
    /// ```py
    /// (
    ///     lambda: (  # 1
    ///         # 2
    ///         # 3
    ///         # 4
    ///         1
    ///     )
    /// )
    /// ```
    ///
    /// or without `# 2`:
    ///
    /// ```py
    /// (
    ///     lambda: (  # 1  # 3
    ///         # 4
    ///         1
    ///     )
    /// )
    /// ```
    dangling_header_comments: &'a [SourceComment],
    needs_parentheses: OptionalParentheses,
}

impl Format<PyFormatContext<'_>> for FormatBody<'_> {
    #[expect(clippy::if_same_then_else)]
    fn fmt(&self, f: &mut PyFormatter) -> FormatResult<()> {
        let FormatBody {
            dangling_header_comments,
            body,
            needs_parentheses,
        } = self;

        let body = *body;
        let comments = f.context().comments().clone();
        let body_comments = comments.leading_dangling_trailing(body);

        if !dangling_header_comments.is_empty() {
            let (trailing_header_comments, leading_body_comments) = dangling_header_comments
                .split_at(
                    dangling_header_comments
                        .iter()
                        .position(|comment| comment.line_position().is_own_line())
                        .unwrap_or(dangling_header_comments.len()),
                );

            let comments = f.context().comments();
            if f.context().is_expression_parenthesized(body.into()) && comments.has_leading(body) {
                trailing_comments(dangling_header_comments).fmt(f)?;

                if leading_body_comments.is_empty() {
                    space().fmt(f)?;
                } else {
                    hard_line_break().fmt(f)?;
                }

                body.format().with_options(Parentheses::Always).fmt(f)
            } else {
                write!(
                    f,
                    [
                        space(),
                        token("("),
                        trailing_comments(trailing_header_comments),
                        block_indent(&format_args!(
                            leading_comments(leading_body_comments),
                            body.format().with_options(Parentheses::Never)
                        )),
                        token(")")
                    ]
                )
            }
        }
        else if body_comments.has_leading() || body_comments.has_trailing_own_line() {
            body.format().with_options(Parentheses::Always).fmt(f)
        }
        else if matches!(needs_parentheses, OptionalParentheses::Always) {
            body.format().with_options(Parentheses::Always).fmt(f)
        }
        else if matches!(needs_parentheses, OptionalParentheses::Multiline) {
            parenthesize_if_expands(&body.format().with_options(Parentheses::Never)).fmt(f)
        }
        else if matches!(body, Expr::Call(_) | Expr::Subscript(_)) {
            let unparenthesized = body.format().with_options(Parentheses::Never).memoized();
            if unparenthesized.inspect(f)?.will_break() {
                expand_parent().fmt(f)?;
            }

            best_fitting![
                unparenthesized,
                group(&unparenthesized).should_expand(true),
                format_args![token("("), block_indent(&unparenthesized), token(")")]
            ]
            .fmt(f)
        }
        else if has_own_parentheses(body, f.context()).is_some() {
            body.format().fmt(f)
        }
        else {
            parenthesize_if_expands(&body.format().with_options(Parentheses::Never)).fmt(f)
        }
    }
}
