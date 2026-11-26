use std::cmp::Ordering;
use std::slice;

use ruff_formatter::{
    FormatOwnedWithRule, FormatRefWithRule, FormatRule, FormatRuleWithOptions, write,
};
use ruff_python_ast::token::parentheses_iterator;
use ruff_python_ast::visitor::source_order::{SourceOrderVisitor, walk_expr};
use ruff_python_ast::{self as ast};
use ruff_python_ast::{AnyNodeRef, Expr, ExprRef, Operator};
use ruff_python_trivia::TriviaRanges;
use ruff_text_size::Ranged;

use crate::builders::parenthesize_if_expands;
use crate::comments::{LeadingDanglingTrailingComments, leading_comments, trailing_comments};
use crate::context::{NodeLevel, WithNodeLevel};
use crate::expression::parentheses::{
    NeedsParentheses, OptionalParentheses, Parentheses, Parenthesize, optional_parentheses,
    parenthesized,
};
use crate::prelude::*;
use crate::preview::is_hug_parens_with_braces_and_square_brackets_enabled;

mod binary_like;
pub(crate) mod expr_attribute;
pub(crate) mod expr_await;
pub(crate) mod expr_bin_op;
pub(crate) mod expr_bool_op;
pub(crate) mod expr_boolean_literal;
pub(crate) mod expr_bytes_literal;
pub(crate) mod expr_call;
pub(crate) mod expr_compare;
pub(crate) mod expr_dict;
pub(crate) mod expr_dict_comp;
pub(crate) mod expr_ellipsis_literal;
pub(crate) mod expr_f_string;
pub(crate) mod expr_generator;
pub(crate) mod expr_if;
pub(crate) mod expr_ipy_escape_command;
pub(crate) mod expr_lambda;
pub(crate) mod expr_list;
pub(crate) mod expr_list_comp;
pub(crate) mod expr_name;
pub(crate) mod expr_named;
pub(crate) mod expr_none_literal;
pub(crate) mod expr_number_literal;
pub(crate) mod expr_set;
pub(crate) mod expr_set_comp;
pub(crate) mod expr_slice;
pub(crate) mod expr_starred;
pub(crate) mod expr_string_literal;
pub(crate) mod expr_subscript;
pub(crate) mod expr_t_string;
pub(crate) mod expr_tuple;
pub(crate) mod expr_unary_op;
pub(crate) mod expr_yield;
pub(crate) mod expr_yield_from;
mod operator;
pub(crate) mod parentheses;

#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct FormatExpr {
    parentheses: Parentheses,
}

impl FormatRuleWithOptions<Expr, PyFormatContext<'_>> for FormatExpr {
    type Options = Parentheses;

    fn with_options(mut self, options: Self::Options) -> Self {
        self.parentheses = options;
        self
    }
}

impl FormatRule<Expr, PyFormatContext<'_>> for FormatExpr {
    fn fmt(&self, expression: &Expr, f: &mut PyFormatter) -> FormatResult<()> {
        let parentheses = self.parentheses;

        let format_expr = format_with(|f| match expression {
            Expr::BoolOp(expr) => expr.format().fmt(f),
            Expr::Named(expr) => expr.format().fmt(f),
            Expr::BinOp(expr) => expr.format().fmt(f),
            Expr::UnaryOp(expr) => expr.format().fmt(f),
            Expr::Lambda(expr) => expr.format().fmt(f),
            Expr::If(expr) => expr.format().fmt(f),
            Expr::Dict(expr) => expr.format().fmt(f),
            Expr::Set(expr) => expr.format().fmt(f),
            Expr::ListComp(expr) => expr.format().fmt(f),
            Expr::SetComp(expr) => expr.format().fmt(f),
            Expr::DictComp(expr) => expr.format().fmt(f),
            Expr::Generator(expr) => expr.format().fmt(f),
            Expr::Await(expr) => expr.format().fmt(f),
            Expr::Yield(expr) => expr.format().fmt(f),
            Expr::YieldFrom(expr) => expr.format().fmt(f),
            Expr::Compare(expr) => expr.format().fmt(f),
            Expr::Call(expr) => expr.format().fmt(f),
            Expr::FString(expr) => expr.format().fmt(f),
            Expr::TString(expr) => expr.format().fmt(f),
            Expr::StringLiteral(expr) => expr.format().fmt(f),
            Expr::BytesLiteral(expr) => expr.format().fmt(f),
            Expr::NumberLiteral(expr) => expr.format().fmt(f),
            Expr::BooleanLiteral(expr) => expr.format().fmt(f),
            Expr::NoneLiteral(expr) => expr.format().fmt(f),
            Expr::EllipsisLiteral(expr) => expr.format().fmt(f),
            Expr::Attribute(expr) => expr.format().fmt(f),
            Expr::Subscript(expr) => expr.format().fmt(f),
            Expr::Starred(expr) => expr.format().fmt(f),
            Expr::Name(expr) => expr.format().fmt(f),
            Expr::List(expr) => expr.format().fmt(f),
            Expr::Tuple(expr) => expr.format().fmt(f),
            Expr::Slice(expr) => expr.format().fmt(f),
            Expr::IpyEscapeCommand(expr) => expr.format().fmt(f),
        });
        let parenthesize = match parentheses {
            Parentheses::Preserve => f.context().is_expression_parenthesized(expression.into()),
            Parentheses::Always => true,
            Parentheses::Never => false,
        };
        if parenthesize {
            let comments = f.context().comments().clone();
            let node_comments = comments.leading_dangling_trailing(expression);
            if !node_comments.has_leading() && !node_comments.has_trailing() {
                parenthesized("(", &format_expr, ")")
                    .with_hugging(is_expression_huggable(expression, f.context()))
                    .fmt(f)
            } else {
                format_with_parentheses_comments(expression, &node_comments, f)
            }
        } else {
            let level = match f.context().node_level() {
                NodeLevel::TopLevel(_) | NodeLevel::CompoundStatement => {
                    NodeLevel::Expression(None)
                }
                saved_level @ (NodeLevel::Expression(_) | NodeLevel::ParenthesizedExpression) => {
                    saved_level
                }
            };

            let mut f = WithNodeLevel::new(level, f);

            write!(f, [format_expr])
        }
    }
}

/// The comments below are trailing on the addition, but it's also outside the
/// parentheses
/// ```python
/// x = [
///     # comment leading
///     (1 + 2)  # comment trailing
/// ]
/// ```
/// as opposed to
/// ```python
/// x = [(
///     # comment leading
///     1 + 2  # comment trailing
/// )]
/// ```
/// , where the comments are inside the parentheses. That is also affects list
/// formatting, where we want to avoid moving the comments after the comma inside
/// the parentheses:
/// ```python
/// data = [
///     (
///         b"\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00"
///         b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
///     ),  # Point (0 0)
/// ]
/// ```
/// We could mark those comments as trailing in list but it's easier to handle
/// them here too.
///
/// So given
/// ```python
/// x = [
///     # comment leading outer
///     (
///         # comment leading inner
///         1 + 2 # comment trailing inner
///     ) # comment trailing outer
/// ]
/// ```
/// we want to keep the inner an outer comments outside the parentheses and the inner ones inside.
/// This is independent of whether they are own line or end-of-line comments, though end-of-line
/// comments can become own line comments when we discard nested parentheses.
///
/// Style decision: When there are multiple nested parentheses around an expression, we consider the
/// outermost parentheses the relevant ones and discard the others.
fn format_with_parentheses_comments(
    expression: &Expr,
    node_comments: &LeadingDanglingTrailingComments,
    f: &mut PyFormatter,
) -> FormatResult<()> {

    let range_with_parens =
        parentheses_iterator(expression.into(), None, f.context().tokens()).last();

    let (leading_split, trailing_split) = if let Some(range_with_parens) = range_with_parens {
        let leading_split = node_comments
            .leading
            .partition_point(|comment| comment.start() < range_with_parens.start());
        let trailing_split = node_comments
            .trailing
            .partition_point(|comment| comment.start() < range_with_parens.end());
        (leading_split, trailing_split)
    } else {
        (0, node_comments.trailing.len())
    };

    let (leading_outer, leading_inner) = node_comments.leading.split_at(leading_split);
    let (trailing_inner, trailing_outer) = node_comments.trailing.split_at(trailing_split);

    let (parentheses_comment, leading_inner) = match leading_inner.split_first() {
        Some((first, rest)) if first.line_position().is_end_of_line() => {
            (slice::from_ref(first), rest)
        }
        _ => (Default::default(), node_comments.leading),
    };



    let fmt_fields = format_with(|f| match expression {
        Expr::BoolOp(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Named(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::BinOp(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::UnaryOp(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Lambda(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::If(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Dict(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Set(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::ListComp(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::SetComp(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::DictComp(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Generator(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Await(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Yield(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::YieldFrom(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Compare(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Call(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::FString(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::TString(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::StringLiteral(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::BytesLiteral(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::NumberLiteral(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::BooleanLiteral(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::NoneLiteral(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::EllipsisLiteral(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Attribute(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Subscript(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Starred(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Name(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::List(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Tuple(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::Slice(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
        Expr::IpyEscapeCommand(expr) => FormatNodeRule::fmt_fields(expr.format().rule(), expr, f),
    });

    leading_comments(leading_outer).fmt(f)?;

    let format_node_rule_fmt = format_with(|f| {
        write!(
            f,
            [
                leading_comments(leading_inner),
                fmt_fields,
                trailing_comments(trailing_inner)
            ]
        )
    });

    write!(
        f,
        [
            parenthesized("(", &format_node_rule_fmt, ")")
                .with_dangling_comments(parentheses_comment),
            trailing_comments(trailing_outer)
        ]
    )
}

/// Wraps an expression in optional parentheses except if its [`NeedsParentheses::needs_parentheses`] implementation
/// indicates that it is okay to omit the parentheses. For example, parentheses can always be omitted for lists,
/// because they already bring their own parentheses.
pub(crate) fn maybe_parenthesize_expression<'a, T>(
    expression: &'a Expr,
    parent: T,
    parenthesize: Parenthesize,
) -> MaybeParenthesizeExpression<'a>
where
    T: Into<AnyNodeRef<'a>>,
{
    MaybeParenthesizeExpression {
        expression,
        parent: parent.into(),
        parenthesize,
    }
}

pub(crate) struct MaybeParenthesizeExpression<'a> {
    expression: &'a Expr,
    parent: AnyNodeRef<'a>,
    parenthesize: Parenthesize,
}

impl Format<PyFormatContext<'_>> for MaybeParenthesizeExpression<'_> {
    fn fmt(&self, f: &mut PyFormatter) -> FormatResult<()> {
        let MaybeParenthesizeExpression {
            expression,
            parent,
            parenthesize,
        } = self;

        let preserve_parentheses = parenthesize.is_optional()
            && f.context()
                .is_expression_parenthesized((*expression).into());

        if preserve_parentheses {
            return expression.format().with_options(Parentheses::Always).fmt(f);
        }

        let comments = f.context().comments().clone();
        let node_comments = comments.leading_dangling_trailing(*expression);

        if node_comments.has_leading() || node_comments.has_trailing_own_line() {
            return expression.format().with_options(Parentheses::Always).fmt(f);
        }

        let needs_parentheses = match expression.needs_parentheses(*parent, f.context()) {
            OptionalParentheses::Always => OptionalParentheses::Always,
            _ if f.context().node_level().is_parenthesized() => {
                return if matches!(parenthesize, Parenthesize::IfBreaksParenthesizedNested) {
                    parenthesize_if_expands(&expression.format().with_options(Parentheses::Never))
                        .with_indent(!is_expression_huggable(expression, f.context()))
                        .fmt(f)
                } else {
                    expression.format().with_options(Parentheses::Never).fmt(f)
                };
            }
            needs_parentheses => needs_parentheses,
        };

        let unparenthesized = expression.format().with_options(Parentheses::Never);

        match needs_parentheses {
            OptionalParentheses::Multiline => match parenthesize {
                Parenthesize::IfRequired => unparenthesized.fmt(f),

                Parenthesize::Optional
                | Parenthesize::IfBreaks
                | Parenthesize::IfBreaksParenthesized
                | Parenthesize::IfBreaksParenthesizedNested => {
                    if can_omit_optional_parentheses(expression, f.context()) {
                        optional_parentheses(&unparenthesized).fmt(f)
                    } else {
                        parenthesize_if_expands(&unparenthesized).fmt(f)
                    }
                }
            },
            OptionalParentheses::BestFit => match parenthesize {
                Parenthesize::IfBreaksParenthesized | Parenthesize::IfBreaksParenthesizedNested => {
                    if can_omit_optional_parentheses(expression, f.context()) {
                        optional_parentheses(&unparenthesized).fmt(f)
                    } else {
                        parenthesize_if_expands(&unparenthesized).fmt(f)
                    }
                }

                Parenthesize::Optional | Parenthesize::IfRequired => unparenthesized.fmt(f),

                Parenthesize::IfBreaks => {
                    if node_comments.has_trailing() {
                        expression.format().with_options(Parentheses::Always).fmt(f)
                    } else {
                        let group_id = f.group_id("optional_parentheses");
                        let f = &mut WithNodeLevel::new(NodeLevel::Expression(Some(group_id)), f);

                        best_fit_parenthesize(&unparenthesized)
                            .with_group_id(Some(group_id))
                            .fmt(f)
                    }
                }
            },
            OptionalParentheses::Never => match parenthesize {
                Parenthesize::Optional
                | Parenthesize::IfBreaks
                | Parenthesize::IfRequired
                | Parenthesize::IfBreaksParenthesized
                | Parenthesize::IfBreaksParenthesizedNested => unparenthesized.fmt(f),
            },

            OptionalParentheses::Always => {
                expression.format().with_options(Parentheses::Always).fmt(f)
            }
        }
    }
}

impl NeedsParentheses for Expr {
    fn needs_parentheses(
        &self,
        parent: AnyNodeRef,
        context: &PyFormatContext,
    ) -> OptionalParentheses {
        match self {
            Expr::BoolOp(expr) => expr.needs_parentheses(parent, context),
            Expr::Named(expr) => expr.needs_parentheses(parent, context),
            Expr::BinOp(expr) => expr.needs_parentheses(parent, context),
            Expr::UnaryOp(expr) => expr.needs_parentheses(parent, context),
            Expr::Lambda(expr) => expr.needs_parentheses(parent, context),
            Expr::If(expr) => expr.needs_parentheses(parent, context),
            Expr::Dict(expr) => expr.needs_parentheses(parent, context),
            Expr::Set(expr) => expr.needs_parentheses(parent, context),
            Expr::ListComp(expr) => expr.needs_parentheses(parent, context),
            Expr::SetComp(expr) => expr.needs_parentheses(parent, context),
            Expr::DictComp(expr) => expr.needs_parentheses(parent, context),
            Expr::Generator(expr) => expr.needs_parentheses(parent, context),
            Expr::Await(expr) => expr.needs_parentheses(parent, context),
            Expr::Yield(expr) => expr.needs_parentheses(parent, context),
            Expr::YieldFrom(expr) => expr.needs_parentheses(parent, context),
            Expr::Compare(expr) => expr.needs_parentheses(parent, context),
            Expr::Call(expr) => expr.needs_parentheses(parent, context),
            Expr::FString(expr) => expr.needs_parentheses(parent, context),
            Expr::TString(expr) => expr.needs_parentheses(parent, context),
            Expr::StringLiteral(expr) => expr.needs_parentheses(parent, context),
            Expr::BytesLiteral(expr) => expr.needs_parentheses(parent, context),
            Expr::NumberLiteral(expr) => expr.needs_parentheses(parent, context),
            Expr::BooleanLiteral(expr) => expr.needs_parentheses(parent, context),
            Expr::NoneLiteral(expr) => expr.needs_parentheses(parent, context),
            Expr::EllipsisLiteral(expr) => expr.needs_parentheses(parent, context),
            Expr::Attribute(expr) => expr.needs_parentheses(parent, context),
            Expr::Subscript(expr) => expr.needs_parentheses(parent, context),
            Expr::Starred(expr) => expr.needs_parentheses(parent, context),
            Expr::Name(expr) => expr.needs_parentheses(parent, context),
            Expr::List(expr) => expr.needs_parentheses(parent, context),
            Expr::Tuple(expr) => expr.needs_parentheses(parent, context),
            Expr::Slice(expr) => expr.needs_parentheses(parent, context),
            Expr::IpyEscapeCommand(expr) => expr.needs_parentheses(parent, context),
        }
    }
}

impl<'ast> AsFormat<PyFormatContext<'ast>> for Expr {
    type Format<'a> = FormatRefWithRule<'a, Expr, FormatExpr, PyFormatContext<'ast>>;

    fn format(&self) -> Self::Format<'_> {
        FormatRefWithRule::new(self, FormatExpr::default())
    }
}

impl<'ast> IntoFormat<PyFormatContext<'ast>> for Expr {
    type Format = FormatOwnedWithRule<Expr, FormatExpr, PyFormatContext<'ast>>;

    fn into_format(self) -> Self::Format {
        FormatOwnedWithRule::new(self, FormatExpr::default())
    }
}

/// Tests if it is safe to omit the optional parentheses.
///
/// We prefer parentheses at least in the following cases:
/// * The expression contains more than one unparenthesized expression with the same precedence. For example,
///   the expression `a * b * c` contains two multiply operations. We prefer parentheses in that case.
///   `(a * b) * c` or `a * b + c` are okay, because the subexpression is parenthesized, or the expression uses operands with a lower precedence
/// * The expression contains at least one parenthesized sub expression (optimization to avoid unnecessary work)
///
/// This mimics Black's [`_maybe_split_omitting_optional_parens`](https://github.com/psf/black/blob/d1248ca9beaf0ba526d265f4108836d89cf551b7/src/black/linegen.py#L746-L820)
pub(crate) fn can_omit_optional_parentheses(expr: &Expr, context: &PyFormatContext) -> bool {
    let mut visitor = CanOmitOptionalParenthesesVisitor::new(context);
    visitor.visit_subexpression(expr);

    if !visitor.any_parenthesized_expressions {
        false
    } else if visitor.max_precedence_count > 1 {
        false
    } else if visitor.max_precedence == OperatorPrecedence::None {
        true
    } else if visitor.max_precedence == OperatorPrecedence::Attribute {
        //      encoding="ascii",
        true
    } else {
        fn is_parenthesized(expr: &Expr, context: &PyFormatContext) -> bool {
            !expr.is_subscript_expr()
                && has_parentheses(expr, context).is_some_and(OwnParentheses::is_non_empty)
        }

        visitor
            .last
            .is_some_and(|last| is_parenthesized(last, context))
            || visitor
                .first
                .expression()
                .is_some_and(|first| is_parenthesized(first, context))
    }
}

#[derive(Clone, Debug)]
struct CanOmitOptionalParenthesesVisitor<'input> {
    max_precedence: OperatorPrecedence,
    max_precedence_count: u32,
    any_parenthesized_expressions: bool,
    last: Option<&'input Expr>,
    first: First<'input>,
    context: &'input PyFormatContext<'input>,
}

impl<'input> CanOmitOptionalParenthesesVisitor<'input> {
    fn new(context: &'input PyFormatContext) -> Self {
        Self {
            context,
            max_precedence: OperatorPrecedence::None,
            max_precedence_count: 0,
            any_parenthesized_expressions: false,
            last: None,
            first: First::None,
        }
    }

    fn update_max_precedence(&mut self, precedence: OperatorPrecedence) {
        self.update_max_precedence_with_count(precedence, 1);
    }

    fn update_max_precedence_with_count(&mut self, precedence: OperatorPrecedence, count: u32) {
        match self.max_precedence.cmp(&precedence) {
            Ordering::Less => {
                self.max_precedence_count = count;
                self.max_precedence = precedence;
            }
            Ordering::Equal => {
                self.max_precedence_count += count;
            }
            Ordering::Greater => {}
        }
    }

    fn visit_subexpression(&mut self, expr: &'input Expr) {
        match expr {
            Expr::Dict(_)
            | Expr::List(_)
            | Expr::Set(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => {
                self.any_parenthesized_expressions = true;
                return;
            }

            Expr::Tuple(ast::ExprTuple {
                parenthesized: true,
                ..
            }) => {
                self.any_parenthesized_expressions = true;
                return;
            }

            Expr::Generator(generator) if generator.parenthesized => {
                self.any_parenthesized_expressions = true;
                return;
            }

            #[expect(clippy::cast_possible_truncation)]
            Expr::BoolOp(ast::ExprBoolOp {
                range: _,
                node_index: _,
                op: _,
                values,
            }) => self.update_max_precedence_with_count(
                OperatorPrecedence::BooleanOperation,
                values.len().saturating_sub(1) as u32,
            ),
            Expr::BinOp(ast::ExprBinOp {
                op,
                left: _,
                right: _,
                range: _,
                node_index: _,
            }) => self.update_max_precedence(OperatorPrecedence::from(*op)),

            Expr::If(_) => {
                self.update_max_precedence_with_count(OperatorPrecedence::Conditional, 2);
            }

            #[expect(clippy::cast_possible_truncation)]
            Expr::Compare(ast::ExprCompare {
                range: _,
                node_index: _,
                ops,
                operands: _,
            }) => {
                self.update_max_precedence_with_count(
                    OperatorPrecedence::Comparator,
                    ops.len() as u32,
                );
            }
            Expr::Call(ast::ExprCall {
                range_start: _,
                node_index: _,
                func,
                arguments: _,
            }) => {
                self.any_parenthesized_expressions = true;
                self.visit_expr(func);
                self.last = Some(expr);
                return;
            }
            Expr::Subscript(ast::ExprSubscript { value, .. }) => {
                self.any_parenthesized_expressions = true;
                self.visit_expr(value);
                self.last = Some(expr);
                return;
            }

            Expr::Attribute(ast::ExprAttribute {
                range: _,
                node_index: _,
                value,
                attr: _,
                ctx: _,
            }) => {
                self.visit_expr(value);
                if has_parentheses(value, self.context).is_some() {
                    self.update_max_precedence(OperatorPrecedence::Attribute);
                }
                self.last = Some(expr);
                return;
            }

            Expr::Named(_) | Expr::Generator(_) | Expr::Tuple(_) => {}

            Expr::UnaryOp(ast::ExprUnaryOp {
                range: _,
                node_index: _,
                op,
                operand: _,
            }) => {
                if op.is_invert() {
                    self.update_max_precedence(OperatorPrecedence::BitwiseInversion);
                }
                self.first.set_if_none(First::Token);
            }

            Expr::Lambda(_)
            | Expr::Await(_)
            | Expr::Yield(_)
            | Expr::YieldFrom(_)
            | Expr::Starred(_) => {
                self.first.set_if_none(First::Token);
            }

            Expr::FString(_)
            | Expr::TString(_)
            | Expr::StringLiteral(_)
            | Expr::BytesLiteral(_)
            | Expr::NumberLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::EllipsisLiteral(_)
            | Expr::Name(_)
            | Expr::Slice(_)
            | Expr::IpyEscapeCommand(_) => {
                return;
            }
        }

        walk_expr(self, expr);
    }
}

impl<'input> SourceOrderVisitor<'input> for CanOmitOptionalParenthesesVisitor<'input> {
    fn visit_expr(&mut self, expr: &'input Expr) {
        self.last = Some(expr);

        if self.context.is_expression_parenthesized(expr.into()) {
            self.any_parenthesized_expressions = true;
        } else {
            self.visit_subexpression(expr);
        }

        self.first.set_if_none(First::Expression(expr));
    }
}

#[derive(Copy, Clone, Debug)]
enum First<'a> {
    None,

    /// Expression starts with a non-parentheses token. E.g. `not a`
    Token,

    Expression(&'a Expr),
}

impl<'a> First<'a> {
    #[inline]
    fn set_if_none(&mut self, first: First<'a>) {
        if matches!(self, First::None) {
            *self = first;
        }
    }

    fn expression(self) -> Option<&'a Expr> {
        match self {
            First::None | First::Token => None,
            First::Expression(expr) => Some(expr),
        }
    }
}

/// A call chain consists only of attribute access (`.` operator), function/method calls and
/// subscripts. We use fluent style for the call chain if there are at least two attribute dots
/// after call parentheses or subscript brackets. In case of fluent style the parentheses/bracket
/// will close on the previous line and the dot gets its own line, otherwise the line will start
/// with the closing parentheses/bracket and the dot follows immediately after.
///
/// Below, the left hand side of the addition has only a single attribute access after a call, the
/// second `.filter`. The first `.filter` is a call, but it doesn't follow a call. The right hand
/// side has two, the `.limit_results` after the call and the `.filter` after the subscript, so it
/// gets formatted in fluent style. The outer expression we assign to `blogs` has zero since the
/// `.all` follows attribute parentheses and not call parentheses.
///
/// ```python
/// blogs = (
///     Blog.objects.filter(
///         entry__headline__contains="Lennon",
///     ).filter(
///         entry__pub_date__year=2008,
///     )
///     + Blog.objects.filter(
///         entry__headline__contains="McCartney",
///     )
///     .limit_results[:10]
///     .filter(
///         entry__pub_date__year=2010,
///     )
/// ).all()
/// ```
///
/// In [`preview`](crate::preview::is_fluent_layout_split_first_call_enabled), we also track the position of the leftmost call or
/// subscript on an attribute in the chain and break just before the dot.
///
/// So, for example, the right-hand summand in the above expression
/// would get formatted as:
/// ```python
///     Blog.objects
///     .filter(
///         entry__headline__contains="McCartney",
///     )
///     .limit_results[:10]
///     .filter(
///         entry__pub_date__year=2010,
///     )
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum CallChainLayout {
    /// The root of a call chain
    #[default]
    Default,

    /// A nested call chain element that uses fluent style.
    Fluent(AttributeState),

    /// A nested call chain element not using fluent style.
    NonFluent,
}

/// Records information about the current position within
/// a call chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeState {
    /// Stores the number of calls or subscripts
    /// to the left of the current position in a chain.
    ///
    /// Consecutive calls/subscripts on a single
    /// object only count once. For example, if we are at
    /// `c` in `a.b()[0]()().c()` then this number would be 1.
    ///
    /// Caveat: If the root of the chain is parenthesized,
    /// it contributes +1 to this count, even if it is not
    /// a call or subscript. But the name
    /// `CallLikeOrParenthesizedRootPreceding`
    /// is a tad unwieldy, and this also rarely occurs.
    CallLikePreceding(u32),
    /// Indicates that we are at the first called or
    /// subscripted object in the chain
    ///
    /// For example, if we are at `b` in `a.b()[0]()().c()`
    FirstCallLike,
    /// Indicates that we are to the left of the first
    /// called or subscripted object in the chain, and therefore
    /// need not break.
    ///
    /// For example, if we are at `a` in `a.b()[0]()().c()`
    BeforeFirstCallLike,
}

impl CallChainLayout {
    /// Returns new state decreasing count of remaining calls/subscripts
    /// to traverse, or the state `FirstCallOrSubscript`, as appropriate.
    #[must_use]
    fn decrement_call_like_count(self) -> Self {
        match self {
            Self::Fluent(AttributeState::CallLikePreceding(x)) => {
                if x > 1 {
                    Self::Fluent(AttributeState::CallLikePreceding(x - 1))
                } else {
                    Self::Fluent(AttributeState::FirstCallLike)
                }
            }
            _ => self,
        }
    }

    /// Returns with state change
    /// `FirstCallOrSubscript` -> `BeforeFirstCallOrSubscript`
    /// and otherwise returns unchanged.
    #[must_use]
    fn transition_after_attribute(self) -> Self {
        match self {
            Self::Fluent(AttributeState::FirstCallLike) => {
                Self::Fluent(AttributeState::BeforeFirstCallLike)
            }
            _ => self,
        }
    }

    fn is_first_call_like(self) -> bool {
        matches!(self, Self::Fluent(AttributeState::FirstCallLike))
    }

    /// Returns either `Fluent` or `NonFluent` depending on a
    /// heuristic computed for the whole chain.
    ///
    /// Explicitly, the criterion to return `Fluent` is
    /// as follows:
    ///
    /// 1. Beginning from the right (i.e. the `expr` itself),
    ///    traverse inwards past calls, subscripts, and attribute
    ///    expressions until we meet the first expression that is
    ///    either none of these or else is parenthesized. This will
    ///    be the _root_ of the call chain.
    /// 2. Count the number of _attribute values_ that are _called
    ///    or subscripted_ in the chain (note that this includes the
    ///    root but excludes the rightmost attribute in the chain since
    ///    it is not the _value_ of some attribute).
    /// 3. If the root is parenthesized, add 1 to that value.
    /// 4. If the total is at least 2, return `Fluent`. Otherwise
    ///    return `NonFluent`
    fn from_expression(mut expr: ExprRef, context: &PyFormatContext) -> Self {

        let mut computed_attribute_values_after_parentheses = 0;

        let mut call_like_count = 0;

        let mut root_value_parenthesized = false;
        loop {
            match expr {
                ExprRef::Attribute(ast::ExprAttribute { value, .. }) => {
                    if context.is_expression_parenthesized(value.into()) {
                        root_value_parenthesized = true;
                        break;
                    } else if matches!(value.as_ref(), Expr::Call(_) | Expr::Subscript(_)) {
                        computed_attribute_values_after_parentheses += 1;
                    }

                    expr = ExprRef::from(value.as_ref());
                }
                ExprRef::Call(ast::ExprCall { func: inner, .. })
                | ExprRef::Subscript(ast::ExprSubscript { value: inner, .. }) => {
                    if context.is_expression_parenthesized(inner.into()) {
                        break;
                    }

                    if !inner.is_call_expr() && !inner.is_subscript_expr() {
                        call_like_count += 1;
                    }

                    expr = ExprRef::from(inner.as_ref());
                }
                _ => {
                    break;
                }
            }
        }

        if computed_attribute_values_after_parentheses + u32::from(root_value_parenthesized) < 2 {
            CallChainLayout::NonFluent
        } else {
            CallChainLayout::Fluent(AttributeState::CallLikePreceding(
                call_like_count + u32::from(root_value_parenthesized),
            ))
        }
    }

    /// Determine whether to actually apply fluent layout in attribute, call and subscript
    /// formatting
    fn apply_in_node<'a>(
        self,
        item: impl Into<ExprRef<'a>>,
        f: &mut PyFormatter,
    ) -> CallChainLayout {
        match self {
            CallChainLayout::Default => {
                if f.context().node_level().is_parenthesized() {
                    CallChainLayout::from_expression(item.into(), f.context())
                } else {
                    CallChainLayout::NonFluent
                }
            }
            layout @ (CallChainLayout::Fluent(_) | CallChainLayout::NonFluent) => layout,
        }
    }

    fn is_fluent(self) -> bool {
        matches!(self, CallChainLayout::Fluent(_))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum OwnParentheses {
    /// The node has parentheses, but they are empty (e.g., `[]` or `f()`).
    Empty,
    /// The node has parentheses, and they are non-empty (e.g., `[1]` or `f(1)`).
    NonEmpty,
}

impl OwnParentheses {
    const fn is_non_empty(self) -> bool {
        matches!(self, OwnParentheses::NonEmpty)
    }
}

/// Returns the [`OwnParentheses`] value for a given [`Expr`], to indicate whether it has its
/// own parentheses or is itself parenthesized.
///
/// Differs from [`has_own_parentheses`] in that it returns [`OwnParentheses::NonEmpty`] for
/// parenthesized expressions, like `(1)` or `([1])`, regardless of whether those expression have
/// their _own_ parentheses.
pub(crate) fn has_parentheses(expr: &Expr, context: &PyFormatContext) -> Option<OwnParentheses> {
    let own_parentheses = has_own_parentheses(expr, context);

    if own_parentheses == Some(OwnParentheses::NonEmpty) {
        return own_parentheses;
    }

    if context.is_expression_parenthesized(expr.into()) {
        return Some(OwnParentheses::NonEmpty);
    }

    own_parentheses
}

/// Returns the [`OwnParentheses`] value for a given [`Expr`], to indicate whether it has its
/// own parentheses, and whether those parentheses are empty.
///
/// A node is considered to have its own parentheses if it includes a `[]`, `()`, or `{}` pair
/// that is inherent to the node (e.g., as in `f()`, `[]`, or `{1: 2}`, but not `(a.b.c)`).
///
/// Parentheses are considered to be non-empty if they contain any elements or comments.
pub(crate) fn has_own_parentheses(
    expr: &Expr,
    context: &PyFormatContext,
) -> Option<OwnParentheses> {
    match expr {
        Expr::ListComp(_) | Expr::SetComp(_) | Expr::DictComp(_) | Expr::Subscript(_) => {
            Some(OwnParentheses::NonEmpty)
        }

        Expr::Generator(generator) if generator.parenthesized => Some(OwnParentheses::NonEmpty),

        Expr::List(ast::ExprList { elts, .. }) | Expr::Set(ast::ExprSet { elts, .. }) => {
            if !elts.is_empty() || context.comments().has_dangling(AnyNodeRef::from(expr)) {
                Some(OwnParentheses::NonEmpty)
            } else {
                Some(OwnParentheses::Empty)
            }
        }

        Expr::Tuple(
            tuple @ ast::ExprTuple {
                parenthesized: true,
                ..
            },
        ) => {
            if !tuple.is_empty() || context.comments().has_dangling(AnyNodeRef::from(expr)) {
                Some(OwnParentheses::NonEmpty)
            } else {
                Some(OwnParentheses::Empty)
            }
        }

        Expr::Dict(ast::ExprDict { items, .. }) => {
            if !items.is_empty() || context.comments().has_dangling(AnyNodeRef::from(expr)) {
                Some(OwnParentheses::NonEmpty)
            } else {
                Some(OwnParentheses::Empty)
            }
        }
        Expr::Call(ast::ExprCall { arguments, .. }) => {
            if !arguments.is_empty() || context.comments().has_dangling(AnyNodeRef::from(expr)) {
                Some(OwnParentheses::NonEmpty)
            } else {
                Some(OwnParentheses::Empty)
            }
        }

        _ => None,
    }
}

/// Returns `true` if the expression can hug directly to enclosing parentheses, as in Black's
/// `hug_parens_with_braces_and_square_brackets` or `multiline_string_handling` preview styles behavior.
///
/// For example, in preview style, given:
/// ```python
/// ([1, 2, 3,])
/// ```
///
/// We want to format it as:
/// ```python
/// ([
///     1,
///     2,
///     3,
/// ])
/// ```
///
/// As opposed to:
/// ```python
/// (
///     [
///         1,
///         2,
///         3,
///     ]
/// )
/// ```
pub(crate) fn is_expression_huggable(expr: &Expr, context: &PyFormatContext) -> bool {
    match expr {
        Expr::Tuple(_)
        | Expr::List(_)
        | Expr::Set(_)
        | Expr::Dict(_)
        | Expr::ListComp(_)
        | Expr::SetComp(_)
        | Expr::DictComp(_) => is_hug_parens_with_braces_and_square_brackets_enabled(context),

        Expr::Starred(ast::ExprStarred { value, .. }) => is_expression_huggable(value, context),

        Expr::BoolOp(_)
        | Expr::Named(_)
        | Expr::BinOp(_)
        | Expr::UnaryOp(_)
        | Expr::Lambda(_)
        | Expr::If(_)
        | Expr::Generator(_)
        | Expr::Await(_)
        | Expr::Yield(_)
        | Expr::YieldFrom(_)
        | Expr::Compare(_)
        | Expr::Call(_)
        | Expr::Attribute(_)
        | Expr::Subscript(_)
        | Expr::Name(_)
        | Expr::Slice(_)
        | Expr::IpyEscapeCommand(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::FString(_)
        | Expr::TString(_)
        | Expr::EllipsisLiteral(_) => false,
    }
}

/// The precedence of [python operators](https://docs.python.org/3/reference/expressions.html#operator-precedence) from
/// highest to lowest priority.
///
/// Ruff uses the operator precedence to decide in which order to split operators:
/// Operators with a lower precedence split before higher-precedence operators.
/// Splitting by precedence ensures that the visual grouping reflects the precedence.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum OperatorPrecedence {
    None,
    Attribute,
    Exponential,
    BitwiseInversion,
    Multiplicative,
    Additive,
    Shift,
    BitwiseAnd,
    BitwiseXor,
    BitwiseOr,
    Comparator,
    BooleanOperation,
    Conditional,
}

impl From<Operator> for OperatorPrecedence {
    fn from(value: Operator) -> Self {
        match value {
            Operator::Add | Operator::Sub => OperatorPrecedence::Additive,
            Operator::Mult
            | Operator::MatMult
            | Operator::Div
            | Operator::Mod
            | Operator::FloorDiv => OperatorPrecedence::Multiplicative,
            Operator::Pow => OperatorPrecedence::Exponential,
            Operator::LShift | Operator::RShift => OperatorPrecedence::Shift,
            Operator::BitOr => OperatorPrecedence::BitwiseOr,
            Operator::BitXor => OperatorPrecedence::BitwiseXor,
            Operator::BitAnd => OperatorPrecedence::BitwiseAnd,
        }
    }
}

/// Returns `true` if `expr` is an expression that can be split into multiple lines.
///
/// Returns `false` for expressions that are guaranteed to never split.
pub(crate) fn is_splittable_expression(expr: &Expr, context: &PyFormatContext) -> bool {
    match expr {
        Expr::Named(_)
        | Expr::Name(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::EllipsisLiteral(_)
        | Expr::Slice(_)
        | Expr::IpyEscapeCommand(_) => false,

        Expr::Compare(_)
        | Expr::BinOp(_)
        | Expr::BoolOp(_)
        | Expr::If(_)
        | Expr::Generator(_)
        | Expr::Subscript(_)
        | Expr::Await(_)
        | Expr::ListComp(_)
        | Expr::SetComp(_)
        | Expr::DictComp(_)
        | Expr::YieldFrom(_) => true,

        Expr::Tuple(tuple) => !tuple.is_empty(),
        Expr::Dict(dict) => !dict.is_empty(),
        Expr::Set(set) => !set.is_empty(),
        Expr::List(list) => !list.is_empty(),

        Expr::UnaryOp(unary) => is_splittable_expression(unary.operand.as_ref(), context),
        Expr::Yield(ast::ExprYield { value, .. }) => value.is_some(),

        Expr::Call(ast::ExprCall {
            arguments, func, ..
        }) => !arguments.is_empty() || context.is_expression_parenthesized(func.as_ref().into()),

        Expr::FString(fstring) => fstring.value.is_implicit_concatenated(),
        Expr::TString(tstring) => tstring.value.is_implicit_concatenated(),
        Expr::StringLiteral(string) => string.value.is_implicit_concatenated(),
        Expr::BytesLiteral(bytes) => bytes.value.is_implicit_concatenated(),

        Expr::Lambda(ast::ExprLambda {
            body: expression, ..
        })
        | Expr::Starred(ast::ExprStarred {
            value: expression, ..
        })
        | Expr::Attribute(ast::ExprAttribute {
            value: expression, ..
        }) => {
            context.is_expression_parenthesized(expression.into())
                || is_splittable_expression(expression.as_ref(), context)
        }
    }
}

/// Returns `true` if `expr` is invalid in a type-expression position unless the source keeps it
/// parenthesized.
pub(crate) const fn is_invalid_type_expression(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Named(_) | Expr::Await(_) | Expr::Yield(_) | Expr::YieldFrom(_)
    )
}

/// Returns the sub-expression to which the left-most character in expression belongs.
///
/// For example, in the expression `a + b * c`, the left-most subexpression is `a`. But for
/// the expression `{ "a": 1 }`, the left-most subexpression is the dictionary, and not `"a"` because
/// the `{` belongs to the dictionary.
///
/// Parenthesized expressions are treated as belonging to the enclosing expression. Therefore, the left
/// most expression for `(a + b) * c` is `a + b` and not `a`.
pub(crate) fn left_most<'expr>(expression: &'expr Expr, trivia: &TriviaRanges) -> &'expr Expr {
    let mut current = expression;
    loop {
        let left = match current {
            Expr::BinOp(ast::ExprBinOp { left, .. })
            | Expr::If(ast::ExprIf { body: left, .. })
            | Expr::Call(ast::ExprCall { func: left, .. })
            | Expr::Attribute(ast::ExprAttribute { value: left, .. })
            | Expr::Subscript(ast::ExprSubscript { value: left, .. }) => Some(&**left),

            Expr::BoolOp(expr_bool_op) => expr_bool_op.values.first(),
            Expr::Compare(compare) => Some(compare.first_operand()),

            Expr::Generator(generator) if !generator.parenthesized => Some(&*generator.elt),

            Expr::Tuple(tuple) if !tuple.parenthesized => tuple.elts.first(),
            Expr::Slice(slice) => slice.lower.as_deref(),

            Expr::List(_)
            | Expr::Tuple(_)
            | Expr::Name(_)
            | Expr::Starred(_)
            | Expr::FString(_)
            | Expr::TString(_)
            | Expr::StringLiteral(_)
            | Expr::BytesLiteral(_)
            | Expr::NumberLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::EllipsisLiteral(_)
            | Expr::Yield(_)
            | Expr::YieldFrom(_)
            | Expr::Await(_)
            | Expr::DictComp(_)
            | Expr::SetComp(_)
            | Expr::ListComp(_)
            | Expr::Set(_)
            | Expr::Dict(_)
            | Expr::UnaryOp(_)
            | Expr::Lambda(_)
            | Expr::Named(_)
            | Expr::IpyEscapeCommand(_)
            | Expr::Generator(_) => None,
        };

        let Some(left) = left else {
            break current;
        };

        if trivia.parenthesized().contains(left.range()) {
            break current;
        }

        current = left;
    }
}
