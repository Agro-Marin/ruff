use ruff_formatter::write;
use ruff_python_ast::AnyNodeRef;
use ruff_python_ast::{Expr, ExprYield, ExprYieldFrom};
use ruff_text_size::{Ranged, TextRange};

use crate::expression::maybe_parenthesize_expression;
use crate::expression::parentheses::{
    NeedsParentheses, OptionalParentheses, Parenthesize, is_type_annotation_of,
};
use crate::prelude::*;

pub(super) enum AnyExpressionYield<'a> {
    Yield(&'a ExprYield),
    YieldFrom(&'a ExprYieldFrom),
}

impl AnyExpressionYield<'_> {
    const fn is_yield_from(&self) -> bool {
        matches!(self, AnyExpressionYield::YieldFrom(_))
    }

    fn value(&self) -> Option<&Expr> {
        match self {
            AnyExpressionYield::Yield(yld) => yld.value.as_deref(),
            AnyExpressionYield::YieldFrom(yld) => Some(&yld.value),
        }
    }
}

impl Ranged for AnyExpressionYield<'_> {
    fn range(&self) -> TextRange {
        match self {
            AnyExpressionYield::Yield(yld) => yld.range(),
            AnyExpressionYield::YieldFrom(yld) => yld.range(),
        }
    }
}

impl NeedsParentheses for AnyExpressionYield<'_> {
    fn needs_parentheses(
        &self,
        parent: AnyNodeRef,
        context: &PyFormatContext,
    ) -> OptionalParentheses {
        if is_type_annotation_of(self.range(), parent) {
            return OptionalParentheses::Always;
        }

        if parent.is_stmt_assign() || parent.is_stmt_ann_assign() || parent.is_stmt_aug_assign() {
            if let Some(value) = self.value() {
                if context.is_expression_parenthesized(value.into()) {
                    OptionalParentheses::Never
                } else {
                    match value.needs_parentheses(self.into(), context) {
                        OptionalParentheses::BestFit => OptionalParentheses::Never,
                        parentheses => parentheses,
                    }
                }
            } else {
                OptionalParentheses::Never
            }
        } else {
            OptionalParentheses::Always
        }
    }
}

impl<'a> From<&'a ExprYield> for AnyExpressionYield<'a> {
    fn from(value: &'a ExprYield) -> Self {
        AnyExpressionYield::Yield(value)
    }
}

impl<'a> From<&'a ExprYieldFrom> for AnyExpressionYield<'a> {
    fn from(value: &'a ExprYieldFrom) -> Self {
        AnyExpressionYield::YieldFrom(value)
    }
}

impl<'a> From<&AnyExpressionYield<'a>> for AnyNodeRef<'a> {
    fn from(value: &AnyExpressionYield<'a>) -> Self {
        match value {
            AnyExpressionYield::Yield(yld) => AnyNodeRef::ExprYield(yld),
            AnyExpressionYield::YieldFrom(yld) => AnyNodeRef::ExprYieldFrom(yld),
        }
    }
}

impl Format<PyFormatContext<'_>> for AnyExpressionYield<'_> {
    fn fmt(&self, f: &mut PyFormatter) -> FormatResult<()> {
        let keyword = if self.is_yield_from() {
            "yield from"
        } else {
            "yield"
        };

        if let Some(val) = self.value() {
            write!(
                f,
                [
                    token(keyword),
                    space(),
                    maybe_parenthesize_expression(val, self, Parenthesize::Optional)
                ]
            )?;
        } else {
            write!(f, [token(keyword)])?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct FormatExprYield;

impl FormatNodeRule<ExprYield> for FormatExprYield {
    fn fmt_fields(&self, item: &ExprYield, f: &mut PyFormatter) -> FormatResult<()> {
        AnyExpressionYield::from(item).fmt(f)
    }
}

impl NeedsParentheses for ExprYield {
    fn needs_parentheses(
        &self,
        parent: AnyNodeRef,
        context: &PyFormatContext,
    ) -> OptionalParentheses {
        AnyExpressionYield::from(self).needs_parentheses(parent, context)
    }
}
