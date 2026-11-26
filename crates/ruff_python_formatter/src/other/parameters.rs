use std::assert_matches;

use ruff_formatter::{FormatRuleWithOptions, format_args, write};
use ruff_python_ast::{AnyNodeRef, Parameters};
use ruff_python_trivia::{CommentLinePosition, SimpleToken, SimpleTokenKind, SimpleTokenizer};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::comments::{
    SourceComment, dangling_comments, dangling_open_parenthesis_comments, leading_comments,
    leading_node_comments, trailing_comments,
};
use crate::context::{NodeLevel, WithNodeLevel};
use crate::expression::parentheses::empty_parenthesized;
use crate::prelude::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum ParametersParentheses {
    /// By default, parameters will always preserve their surrounding parentheses.
    #[default]
    Preserve,

    /// Handle special cases where parentheses should never be used.
    ///
    /// An example where parentheses are never used for parameters would be with lambda
    /// expressions. The following is invalid syntax:
    /// ```python
    /// lambda (x, y, z): ...
    /// ```
    /// Instead the lambda here should be:
    /// ```python
    /// lambda x, y, z: ...
    /// ```
    Never,
}

#[derive(Default)]
pub struct FormatParameters {
    parentheses: ParametersParentheses,
}

impl FormatRuleWithOptions<Parameters, PyFormatContext<'_>> for FormatParameters {
    type Options = ParametersParentheses;

    fn with_options(mut self, options: Self::Options) -> Self {
        self.parentheses = options;
        self
    }
}

impl FormatNodeRule<Parameters> for FormatParameters {
    fn fmt_fields(&self, item: &Parameters, f: &mut PyFormatter) -> FormatResult<()> {
        let Parameters {
            range: _,
            node_index: _,
            posonlyargs,
            args,
            vararg,
            kwonlyargs,
            kwarg,
        } = item;

        let (slash, star) = find_parameter_separators(f.context().source(), item);

        let comments = f.context().comments().clone();
        let dangling = comments.dangling(item);

        let parenthesis_comments_end = usize::from(dangling.first().is_some_and(|comment| {
            if comment.line_position().is_end_of_line() {
                let mut lexer = SimpleTokenizer::new(
                    f.context().source(),
                    TextRange::new(item.start(), comment.start()),
                )
                .skip_trivia()
                .skip_while(|t| {
                    matches!(
                        t.kind(),
                        SimpleTokenKind::LParen
                            | SimpleTokenKind::LBrace
                            | SimpleTokenKind::LBracket
                    )
                });
                if lexer.next().is_none() {
                    return true;
                }
            }
            false
        }));

        let (parenthesis_dangling, parameters_dangling) =
            dangling.split_at(parenthesis_comments_end);

        let format_inner = format_with(|f: &mut PyFormatter| {
            let separator = format_with(|f: &mut PyFormatter| {
                token(",").fmt(f)?;

                if f.context().node_level().is_parenthesized() {
                    soft_line_break_or_space().fmt(f)
                } else {
                    space().fmt(f)
                }
            });
            let mut joiner = f.join_with(separator);
            let mut last_node: Option<AnyNodeRef> = None;

            for parameter_with_default in posonlyargs {
                joiner.entry(&parameter_with_default.format());

                last_node = Some(parameter_with_default.into());
            }

            let slash_comments_end = if posonlyargs.is_empty() {
                0
            } else {
                let slash_comments_end = parameters_dangling.partition_point(|comment| {
                    let assignment = assign_argument_separator_comment_placement(
                        slash.as_ref(),
                        star.as_ref(),
                        comment.range(),
                        comment.line_position(),
                    )
                    .expect("Unexpected dangling comment type in function parameters");
                    matches!(
                        assignment,
                        ArgumentSeparatorCommentLocation::SlashLeading
                            | ArgumentSeparatorCommentLocation::SlashTrailing
                    )
                });
                joiner.entry(&CommentsAroundText {
                    text: "/",
                    comments: &parameters_dangling[..slash_comments_end],
                });
                slash_comments_end
            };

            for parameter_with_default in args {
                joiner.entry(&parameter_with_default.format());

                last_node = Some(parameter_with_default.into());
            }

            if let Some(vararg) = vararg {
                joiner.entry(&format_args![
                    leading_node_comments(vararg.as_ref()),
                    token("*"),
                    vararg.format()
                ]);
                last_node = Some(vararg.as_ref().into());
            } else if !kwonlyargs.is_empty() {
                joiner.entry(&CommentsAroundText {
                    text: "*",
                    comments: &parameters_dangling[slash_comments_end..],
                });
            }

            for parameter_with_default in kwonlyargs {
                joiner.entry(&parameter_with_default.format());

                last_node = Some(parameter_with_default.into());
            }

            if let Some(kwarg) = kwarg {
                joiner.entry(&format_args![
                    leading_node_comments(kwarg.as_ref()),
                    token("**"),
                    kwarg.format()
                ]);
                last_node = Some(kwarg.as_ref().into());
            }

            joiner.finish()?;

            if self.parentheses == ParametersParentheses::Never {
                if has_trailing_comma(item, last_node, f.context().source()) {
                    write!(f, [token(",")])?;
                }
            } else {
                write!(f, [if_group_breaks(&token(","))])?;

                if f.options().magic_trailing_comma().is_respect()
                    && has_trailing_comma(item, last_node, f.context().source())
                {
                    write!(f, [hard_line_break()])?;
                }
            }

            Ok(())
        });

        let num_parameters = item.len();

        if self.parentheses == ParametersParentheses::Never {
            write!(f, [group(&format_inner), dangling_comments(dangling)])
        } else if num_parameters == 0 {
            let mut f = WithNodeLevel::new(NodeLevel::ParenthesizedExpression, f);
            write!(f, [empty_parenthesized("(", dangling, ")")])
        } else if num_parameters == 1 && posonlyargs.is_empty() && kwonlyargs.is_empty() {
            let mut f = WithNodeLevel::new(NodeLevel::ParenthesizedExpression, f);
            write!(
                f,
                [
                    token("("),
                    dangling_open_parenthesis_comments(parenthesis_dangling),
                    soft_block_indent(&format_inner),
                    token(")")
                ]
            )
        } else {
            let mut f = WithNodeLevel::new(NodeLevel::ParenthesizedExpression, f);
            write!(
                f,
                [
                    token("("),
                    dangling_open_parenthesis_comments(parenthesis_dangling),
                    soft_block_indent(&group(&format_inner)),
                    token(")")
                ]
            )
        }
    }
}

struct CommentsAroundText<'a> {
    text: &'static str,
    comments: &'a [SourceComment],
}

impl Format<PyFormatContext<'_>> for CommentsAroundText<'_> {
    fn fmt(&self, f: &mut PyFormatter) -> FormatResult<()> {
        if self.comments.is_empty() {
            token(self.text).fmt(f)
        } else {
            let (leading, trailing) = self.comments.split_at(
                self.comments
                    .partition_point(|comment| comment.line_position().is_own_line()),
            );
            write!(
                f,
                [
                    leading_comments(leading),
                    token(self.text),
                    trailing_comments(trailing)
                ]
            )
        }
    }
}

/// `/` and `*` in a function signature
///
/// ```text
/// def f(arg_a, /, arg_b, *, arg_c): pass
///            ^ ^  ^    ^ ^  ^ slash preceding end
///              ^  ^    ^ ^  ^ slash (a separator)
///                 ^    ^ ^  ^ slash following start
///                      ^ ^  ^ star preceding end
///                        ^  ^ star (a separator)
///                           ^ star following start
/// ```
#[derive(Debug)]
pub(crate) struct ParameterSeparator {
    /// The end of the last node or separator before this separator
    preceding_end: TextSize,
    /// The range of the separator itself
    separator: TextRange,
    /// The start of the first node or separator following this separator
    following_start: TextSize,
}

/// Finds slash and star in `f(a, /, b, *, c)` or `lambda a, /, b, *, c: 1`.
///
/// Returns the location of the slash and star separators, if any.
pub(crate) fn find_parameter_separators(
    contents: &str,
    parameters: &Parameters,
) -> (Option<ParameterSeparator>, Option<ParameterSeparator>) {
    let slash = if let Some(preceding_end) = parameters.posonlyargs.last().map(Ranged::end) {
        let range = TextRange::new(preceding_end, parameters.end());
        let mut tokens = SimpleTokenizer::new(contents, range).skip_trivia();

        let comma = tokens
            .next()
            .expect("The function definition can't end here");
        debug_assert!(comma.kind() == SimpleTokenKind::Comma, "{comma:?}");
        let slash = tokens
            .next()
            .expect("The function definition can't end here");
        debug_assert!(slash.kind() == SimpleTokenKind::Slash, "{slash:?}");

        Some((preceding_end, slash.range))
    } else {
        None
    };

    let star = if parameters.vararg.is_some() {
        None
    } else if let Some(first_keyword_argument) = parameters.kwonlyargs.first() {
        let after_parameters = parameters
            .args
            .last()
            .map(|arg| arg.range.end())
            .or(slash.map(|(_, slash)| slash.end()));
        if let Some(preceding_end) = after_parameters {
            let range = TextRange::new(preceding_end, parameters.end());
            let mut tokens = SimpleTokenizer::new(contents, range).skip_trivia();

            let comma = tokens
                .next()
                .expect("The function definition can't end here");
            debug_assert!(comma.kind() == SimpleTokenKind::Comma, "{comma:?}");
            let star = tokens
                .next()
                .expect("The function definition can't end here");
            debug_assert!(star.kind() == SimpleTokenKind::Star, "{star:?}");

            Some(ParameterSeparator {
                preceding_end,
                separator: star.range,
                following_start: first_keyword_argument.start(),
            })
        } else {
            let mut tokens = SimpleTokenizer::new(contents, parameters.range).skip_trivia();

            let lparen_or_star = tokens
                .next()
                .expect("The function definition can't end here");

            let star = if lparen_or_star.kind == SimpleTokenKind::LParen {
                tokens
                    .next()
                    .expect("The function definition can't end here")
            } else {
                lparen_or_star
            };
            debug_assert!(star.kind() == SimpleTokenKind::Star, "{star:?}");

            Some(ParameterSeparator {
                preceding_end: parameters.start(),
                separator: star.range,
                following_start: first_keyword_argument.start(),
            })
        }
    } else {
        None
    };

    let slash_following_start = parameters
        .args
        .first()
        .map(Ranged::start)
        .or(parameters.vararg.as_ref().map(|first| first.start()))
        .or(star.as_ref().map(|star| star.separator.start()))
        .or(parameters.kwarg.as_deref().map(Ranged::start))
        .unwrap_or(parameters.end());
    let slash = slash.map(|(preceding_end, slash)| ParameterSeparator {
        preceding_end,
        separator: slash,
        following_start: slash_following_start,
    });

    (slash, star)
}

/// Locates positional only parameters separator `/` or the keywords only parameters
/// separator `*` comments.
///
/// ```python
/// def test(
///     a,
///     # Positional only parameters after here
///     /, # trailing positional argument comment.
///     b,
/// ):
///     pass
/// ```
/// or
/// ```python
/// def f(
///     a="",
///     # Keyword only parameters only after here
///     *, # trailing keyword argument comment.
///     b="",
/// ):
///     pass
/// ```
/// or
/// ```python
/// def f(
///     a,
///     # positional only comment, leading
///     /,  # positional only comment, trailing
///     b,
///     # keyword only comment, leading
///     *, # keyword only comment, trailing
///     c,
/// ):
///     pass
/// ```
/// Notably, the following is possible:
/// ```python
/// def f32(
///     a,
///     # positional only comment, leading
///     /,  # positional only comment, trailing
///     # keyword only comment, leading
///     *, # keyword only comment, trailing
///     c,
/// ):
///     pass
/// ```
///
/// ## Background
///
/// ```text
/// def f(a1, a2): pass
///       ^^^^^^ parameters (args)
/// ```
/// Use a star to separate keyword only parameters:
/// ```text
/// def f(a1, a2, *, a3, a4): pass
///       ^^^^^^            parameters (args)
///                  ^^^^^^ keyword only parameters (kwargs)
/// ```
/// Use a slash to separate positional only parameters. Note that this changes the parameters left
/// of the slash while the star change the parameters right of it:
/// ```text
/// def f(a1, a2, /, a3, a4): pass
///       ^^^^^^            positional only parameters (posonlyargs)
///                  ^^^^^^ parameters (args)
/// ```
/// You can combine both:
/// ```text
/// def f(a1, a2, /, a3, a4, *, a5, a6): pass
///       ^^^^^^                       positional only parameters (posonlyargs)
///                  ^^^^^^            parameters (args)
///                             ^^^^^^ keyword only parameters (kwargs)
/// ```
/// They can all have defaults, meaning that the preceding node ends at the default instead of the
/// argument itself:
/// ```text
/// def f(a1=1, a2=2, /, a3=3, a4=4, *, a5=5, a6=6): pass
///          ^     ^        ^     ^        ^     ^ defaults
///       ^^^^^^^^^^                               positional only parameters (posonlyargs)
///                      ^^^^^^^^^^                parameters (args)
///                                     ^^^^^^^^^^ keyword only parameters (kwargs)
/// ```
/// An especially difficult case is having no regular parameters, so comments from both slash and
/// star will attach to either a2 or a3 and the next token is incorrect.
/// ```text
/// def f(a1, a2, /, *, a3, a4): pass
///       ^^^^^^               positional only parameters (posonlyargs)
///                     ^^^^^^ keyword only parameters (kwargs)
/// ```
pub(crate) fn assign_argument_separator_comment_placement(
    slash: Option<&ParameterSeparator>,
    star: Option<&ParameterSeparator>,
    comment_range: TextRange,
    text_position: CommentLinePosition,
) -> Option<ArgumentSeparatorCommentLocation> {
    if let Some(ParameterSeparator {
        preceding_end,
        separator: slash,
        following_start,
    }) = slash
    {
        if comment_range.start() > *preceding_end
            && comment_range.start() < slash.start()
            && text_position.is_own_line()
        {
            return Some(ArgumentSeparatorCommentLocation::SlashLeading);
        }

        if comment_range.start() > slash.end()
            && comment_range.start() < *following_start
            && text_position.is_end_of_line()
        {
            return Some(ArgumentSeparatorCommentLocation::SlashTrailing);
        }
    }

    if let Some(ParameterSeparator {
        preceding_end,
        separator: star,
        following_start,
    }) = star
    {
        if comment_range.start() > *preceding_end
            && comment_range.start() < star.start()
            && text_position.is_own_line()
        {
            return Some(ArgumentSeparatorCommentLocation::StarLeading);
        }

        if comment_range.start() > star.end()
            && comment_range.start() < *following_start
            && text_position.is_end_of_line()
        {
            return Some(ArgumentSeparatorCommentLocation::StarTrailing);
        }
    }
    None
}

/// ```python
/// def f(
///     a,
///     # before slash
///     /,  # after slash
///     b,
///     # before star
///     *, # after star
///     c,
/// ):
///     pass
/// ```
#[derive(Debug)]
pub(crate) enum ArgumentSeparatorCommentLocation {
    SlashLeading,
    SlashTrailing,
    StarLeading,
    StarTrailing,
}

fn has_trailing_comma(
    parameters: &Parameters,
    last_node: Option<AnyNodeRef>,
    source: &str,
) -> bool {
    let Some(last_node) = last_node else {
        return false;
    };

    let ends_with_pos_only_argument_separator = !parameters.posonlyargs.is_empty()
        && parameters.args.is_empty()
        && parameters.vararg.is_none()
        && parameters.kwonlyargs.is_empty()
        && parameters.kwarg.is_none();

    let mut tokens = SimpleTokenizer::starts_at(last_node.end(), source).skip_trivia();
    if ends_with_pos_only_argument_separator {
        let comma = tokens.next();
        assert_matches!(
            comma,
            Some(SimpleToken {
                kind: SimpleTokenKind::Comma,
                ..
            }),
            "The last positional only argument must be separated by a `,` from the positional only parameters separator `/`."
        );

        let slash = tokens.next();
        assert_matches!(
            slash,
            Some(SimpleToken {
                kind: SimpleTokenKind::Slash,
                ..
            }),
            "The positional argument separator must be present for a function that has positional only parameters."
        );
    }

    tokens
        .next()
        .expect("There must be a token after the argument list")
        .kind()
        == SimpleTokenKind::Comma
}
