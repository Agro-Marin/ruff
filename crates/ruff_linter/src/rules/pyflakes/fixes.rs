use std::debug_assert_matches;

use anyhow::{Context, Ok, Result};

use ruff_python_ast as ast;
use ruff_python_ast::Expr;
use ruff_python_codegen::Stylist;
use ruff_python_semantic::Binding;
use ruff_python_trivia::{BackwardsTokenizer, SimpleTokenKind, SimpleTokenizer};
use ruff_text_size::Ranged;

use crate::Edit;
use crate::Locator;
use crate::cst::matchers::{match_call_mut, match_dict, transform_expression};

/// Generate a [`Edit`] to remove unused keys from format dict.
pub(super) fn remove_unused_format_arguments_from_dict(
    unused_arguments: &[usize],
    dict: &ast::ExprDict,
    locator: &Locator,
    stylist: &Stylist,
) -> Result<Edit> {
    let source_code = locator.slice(dict);
    transform_expression(source_code, stylist, |mut expression| {
        let dict = match_dict(&mut expression)?;

        let mut index = 0;
        dict.elements.retain(|_| {
            let is_unused = unused_arguments.contains(&index);
            index += 1;
            !is_unused
        });

        Ok(expression)
    })
    .map(|output| Edit::range_replacement(output, dict.range()))
}

/// Generate a [`Edit`] to remove unused keyword arguments from a `format` call.
pub(super) fn remove_unused_keyword_arguments_from_format_call(
    unused_arguments: &[usize],
    call: &ast::ExprCall,
    locator: &Locator,
    stylist: &Stylist,
) -> Result<Edit> {
    let source_code = locator.slice(call);
    transform_expression(source_code, stylist, |mut expression| {
        let call = match_call_mut(&mut expression)?;

        let mut index = 0;
        call.args.retain(|arg| {
            if arg.keyword.is_none() {
                return true;
            }

            let is_unused = unused_arguments.contains(&index);
            index += 1;
            !is_unused
        });

        Ok(expression)
    })
    .map(|output| Edit::range_replacement(output, call.range()))
}

/// Generate a [`Edit`] to remove unused positional arguments from a `format` call.
pub(crate) fn remove_unused_positional_arguments_from_format_call(
    unused_arguments: &[usize],
    call: &ast::ExprCall,
    locator: &Locator,
    stylist: &Stylist,
) -> Result<Edit> {
    if unused_arguments.len() == call.arguments.len()
        && let Expr::Attribute(attribute) = &*call.func
        && let Expr::StringLiteral(string_expr) = &*attribute.value
        && !string_expr.value.to_str().contains(['{', '}'])
    {
        return Ok(Edit::range_replacement(
            locator.slice(string_expr).to_string(),
            call.range(),
        ));
    }

    let source_code = locator.slice(call);
    transform_expression(source_code, stylist, |mut expression| {
        let call = match_call_mut(&mut expression)?;

        let mut index = 0;
        call.args.retain(|_| {
            let is_unused = unused_arguments.contains(&index);
            index += 1;
            !is_unused
        });

        Ok(expression)
    })
    .map(|output| Edit::range_replacement(output, call.range()))
}

/// Generate a [`Edit`] to remove the binding from an exception handler.
pub(crate) fn remove_exception_handler_assignment(
    bound_exception: &Binding,
    locator: &Locator,
) -> Result<Edit> {
    let mut tokenizer =
        BackwardsTokenizer::up_to(bound_exception.start(), locator.contents(), &[]).skip_trivia();

    let preceding = tokenizer
        .next()
        .context("expected the exception name to be preceded by `as`")?;
    debug_assert_matches!(preceding.kind, SimpleTokenKind::As);

    let preceding = tokenizer
        .next()
        .context("expected the exception name to be preceded by a token")?;

    let following = SimpleTokenizer::starts_at(bound_exception.end(), locator.contents())
        .skip_trivia()
        .next()
        .context("expected the exception name to be followed by a colon")?;
    debug_assert_matches!(following.kind, SimpleTokenKind::Colon);

    Ok(Edit::deletion(preceding.end(), following.start()))
}
