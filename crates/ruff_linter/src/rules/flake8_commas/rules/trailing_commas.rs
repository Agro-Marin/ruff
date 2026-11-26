use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::token::{TokenKind, Tokens};
use ruff_python_index::Indexer;
use ruff_text_size::{Ranged, TextRange};

use crate::Locator;
use crate::checkers::ast::LintContext;
use crate::codes::Category;
use crate::{AlwaysFixableViolation, Violation};
use crate::{Edit, Fix};

/// Simplified token type.
#[derive(Copy, Clone, PartialEq, Eq)]
enum TokenType {
    Named,
    String,
    Newline,
    NonLogicalNewline,
    OpeningBracket,
    ClosingBracket,
    OpeningSquareBracket,
    Colon,
    Comma,
    OpeningCurlyBracket,
    Def,
    For,
    Lambda,
    Class,
    Type,
    Irrelevant,
}

/// Simplified token specialized for the task.
#[derive(Copy, Clone)]
struct SimpleToken {
    ty: TokenType,
    range: TextRange,
}

impl Ranged for SimpleToken {
    fn range(&self) -> TextRange {
        self.range
    }
}

impl SimpleToken {
    fn new(ty: TokenType, range: TextRange) -> Self {
        Self { ty, range }
    }

    fn irrelevant() -> SimpleToken {
        SimpleToken {
            ty: TokenType::Irrelevant,
            range: TextRange::default(),
        }
    }
}

impl From<(TokenKind, TextRange)> for SimpleToken {
    fn from((tok, range): (TokenKind, TextRange)) -> Self {
        let ty = match tok {
            TokenKind::Identifier => TokenType::Named,
            TokenKind::String => TokenType::String,
            TokenKind::Newline => TokenType::Newline,
            TokenKind::NonLogicalNewline => TokenType::NonLogicalNewline,
            TokenKind::Lpar => TokenType::OpeningBracket,
            TokenKind::Rpar => TokenType::ClosingBracket,
            TokenKind::Lsqb => TokenType::OpeningSquareBracket,
            TokenKind::Rsqb => TokenType::ClosingBracket,
            TokenKind::Colon => TokenType::Colon,
            TokenKind::Comma => TokenType::Comma,
            TokenKind::Lbrace => TokenType::OpeningCurlyBracket,
            TokenKind::Rbrace => TokenType::ClosingBracket,
            TokenKind::Def => TokenType::Def,
            TokenKind::Class => TokenType::Class,
            TokenKind::Type => TokenType::Type,
            TokenKind::For => TokenType::For,
            TokenKind::Lambda => TokenType::Lambda,
            TokenKind::Import => TokenType::Named,
            _ => TokenType::Irrelevant,
        };
        #[expect(clippy::inconsistent_struct_constructor)]
        Self { range, ty }
    }
}

/// Comma context type - types of comma-delimited Python constructs.
#[derive(Copy, Clone, PartialEq, Eq)]
enum ContextType {
    No,
    /// Function definition parameter list, e.g. `def foo(a,b,c)`.
    FunctionParameters,
    /// Call argument-like item list, e.g. `f(1,2,3)`, `foo()(1,2,3)`.
    CallArguments,
    /// Tuple-like item list, e.g. `(1,2,3)`.
    Tuple,
    /// Subscript item list, e.g. `x[1,2,3]`, `foo()[1,2,3]`.
    Subscript,
    /// List-like item list, e.g. `[1,2,3]`.
    List,
    /// Dict-/set-like item list, e.g. `{1,2,3}`.
    Dict,
    /// Lambda parameter list, e.g. `lambda a, b`.
    LambdaParameters,
    /// Type parameter list, e.g. `def foo[T, U](): ...`
    TypeParameters,
}

/// Comma context - described a comma-delimited "situation".
#[derive(Copy, Clone)]
struct Context {
    ty: ContextType,
    num_commas: u32,
}

impl Context {
    const fn new(ty: ContextType) -> Self {
        Self { ty, num_commas: 0 }
    }

    fn inc(&mut self) {
        self.num_commas += 1;
    }
}

/// ## What it does
/// Checks for the absence of trailing commas.
///
/// ## Why is this bad?
/// The presence of a trailing comma can reduce diff size when parameters or
/// elements are added or removed from function calls, function definitions,
/// literals, etc.
///
/// ## Example
/// ```python
/// foo = {
///     "bar": 1,
///     "baz": 2
/// }
/// ```
///
/// Use instead:
/// ```python
/// foo = {
///     "bar": 1,
///     "baz": 2,
/// }
/// ```
///
/// ## Formatter compatibility
/// We recommend against using this rule alongside the [formatter]. The
/// formatter enforces consistent use of trailing commas, making the rule redundant.
///
/// [formatter]:https://docs.astral.sh/ruff/formatter/
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.223", category = Category::Formatting)]
pub(crate) struct MissingTrailingComma;

impl AlwaysFixableViolation for MissingTrailingComma {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Trailing comma missing".to_string()
    }

    fn fix_title(&self) -> String {
        "Add trailing comma".to_string()
    }
}

/// ## What it does
/// Checks for the presence of trailing commas on bare (i.e., unparenthesized)
/// tuples.
///
/// ## Why is this bad?
/// The presence of a misplaced comma will cause Python to interpret the value
/// as a tuple, which can lead to unexpected behaviour.
///
/// ## Example
/// ```python
/// import json
///
///
/// foo = json.dumps({"bar": 1}),
/// ```
///
/// Use instead:
/// ```python
/// import json
///
///
/// foo = json.dumps({"bar": 1})
/// ```
///
/// In the event that a tuple is intended, then use instead:
/// ```python
/// import json
///
///
/// foo = (json.dumps({"bar": 1}),)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.223", category = Category::Suspicious)]
pub(crate) struct TrailingCommaOnBareTuple;

impl Violation for TrailingCommaOnBareTuple {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Trailing comma on bare tuple prohibited".to_string()
    }
}

/// ## What it does
/// Checks for the presence of prohibited trailing commas.
///
/// ## Why is this bad?
/// Trailing commas are not essential in some cases and can therefore be viewed
/// as unnecessary.
///
/// ## Example
/// ```python
/// foo = (1, 2, 3,)
/// ```
///
/// Use instead:
/// ```python
/// foo = (1, 2, 3)
/// ```
///
/// ## Formatter compatibility
/// We recommend against using this rule alongside the [formatter]. With the
/// default `format.skip-magic-trailing-comma = false`, trailing commas can be
/// intentional: the formatter treats them as a signal to preserve multiline
/// formatting. When set to `true`, the formatter removes those trailing commas
/// where possible, making this rule redundant.
///
/// [formatter]:https://docs.astral.sh/ruff/formatter/
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.223", category = Category::Formatting)]
pub(crate) struct ProhibitedTrailingComma;

impl AlwaysFixableViolation for ProhibitedTrailingComma {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Trailing comma prohibited".to_string()
    }

    fn fix_title(&self) -> String {
        "Remove trailing comma".to_string()
    }
}

/// COM812, COM818, COM819
pub(crate) fn trailing_commas(
    lint_context: &LintContext,
    tokens: &Tokens,
    locator: &Locator,
    indexer: &Indexer,
) {
    let mut interpolated_strings = 0u32;
    let simple_tokens = tokens.iter().filter_map(|token| {
        match token.kind() {
            TokenKind::Comment => None,
            TokenKind::FStringStart | TokenKind::TStringStart => {
                interpolated_strings = interpolated_strings.saturating_add(1);
                None
            }
            TokenKind::FStringEnd | TokenKind::TStringEnd => {
                interpolated_strings = interpolated_strings.saturating_sub(1);
                if interpolated_strings == 0 {
                    indexer
                        .interpolated_string_ranges()
                        .outermost(token.start())
                        .map(|range| SimpleToken::new(TokenType::String, range))
                } else {
                    None
                }
            }
            _ => {
                if interpolated_strings == 0 {
                    Some(SimpleToken::from(token.as_tuple()))
                } else {
                    None
                }
            }
        }
    });

    let mut prev = SimpleToken::irrelevant();
    let mut prev_prev = SimpleToken::irrelevant();

    let mut stack = vec![Context::new(ContextType::No)];

    for token in simple_tokens {
        if prev.ty == TokenType::NonLogicalNewline && token.ty == TokenType::NonLogicalNewline {
            continue;
        }

        let context = update_context(token, prev, prev_prev, &mut stack);

        check_token(token, prev, prev_prev, context, locator, lint_context);

        let pop_context = match context.ty {
            ContextType::LambdaParameters => token.ty == TokenType::Colon,
            _ => token.ty == TokenType::ClosingBracket,
        };
        if pop_context && stack.len() > 1 {
            stack.pop();
        }

        prev_prev = prev;
        prev = token;
    }
}

fn check_token(
    token: SimpleToken,
    prev: SimpleToken,
    prev_prev: SimpleToken,
    context: Context,
    locator: &Locator,
    lint_context: &LintContext,
) {
    let comma_allowed = token.ty == TokenType::ClosingBracket
        && match context.ty {
            ContextType::No => false,
            ContextType::FunctionParameters => true,
            ContextType::CallArguments => true,
            ContextType::TypeParameters => true,
            ContextType::Tuple => context.num_commas != 0,
            ContextType::Subscript => context.num_commas != 0,
            ContextType::List => true,
            ContextType::Dict => true,
            ContextType::LambdaParameters => false,
        };

    let comma_prohibited = prev.ty == TokenType::Comma && {
        let is_singleton_tuplish =
            matches!(context.ty, ContextType::Subscript | ContextType::Tuple)
                && context.num_commas <= 1;
        if comma_allowed && !is_singleton_tuplish {
            true
        } else {
            context.ty == ContextType::LambdaParameters && token.ty == TokenType::Colon
        }
    };

    if comma_prohibited {
        if let Some(mut diagnostic) =
            lint_context.report_diagnostic_if_enabled(ProhibitedTrailingComma, prev.range())
        {
            diagnostic.set_fix(Fix::safe_edit(Edit::range_deletion(prev.range)));
            return;
        }
    }

    let bare_comma_prohibited = prev.ty == TokenType::Comma && token.ty == TokenType::Newline;
    if bare_comma_prohibited {
        lint_context.report_diagnostic_if_enabled(TrailingCommaOnBareTuple, prev.range());
        return;
    }

    if !comma_allowed {
        return;
    }

    let comma_required = prev.ty == TokenType::NonLogicalNewline
        && !matches!(
            prev_prev.ty,
            TokenType::Comma
                | TokenType::OpeningBracket
                | TokenType::OpeningSquareBracket
                | TokenType::OpeningCurlyBracket
        );
    if comma_required {
        if let Some(mut diagnostic) = lint_context
            .report_diagnostic_if_enabled(MissingTrailingComma, TextRange::empty(prev_prev.end()))
        {
            let contents = locator.slice(prev_prev.range());
            diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
                format!("{contents},"),
                prev_prev.range(),
            )));
        }
    }
}

fn update_context(
    token: SimpleToken,
    prev: SimpleToken,
    prev_prev: SimpleToken,
    stack: &mut Vec<Context>,
) -> Context {
    let new_context = match token.ty {
        TokenType::OpeningBracket => match (prev.ty, prev_prev.ty) {
            (TokenType::Named, TokenType::Def) => Context::new(ContextType::FunctionParameters),
            (TokenType::Named | TokenType::ClosingBracket, _) => {
                Context::new(ContextType::CallArguments)
            }
            _ => Context::new(ContextType::Tuple),
        },
        TokenType::OpeningSquareBracket => match (prev.ty, prev_prev.ty) {
            (TokenType::Named, TokenType::Def | TokenType::Class | TokenType::Type) => {
                Context::new(ContextType::TypeParameters)
            }
            (TokenType::ClosingBracket | TokenType::Named | TokenType::String, _) => {
                Context::new(ContextType::Subscript)
            }
            _ => Context::new(ContextType::List),
        },
        TokenType::OpeningCurlyBracket => Context::new(ContextType::Dict),
        TokenType::Lambda => Context::new(ContextType::LambdaParameters),
        TokenType::For => {
            let last = stack.last_mut().expect("Stack to never be empty");
            *last = Context::new(ContextType::No);
            return *last;
        }
        TokenType::Comma => {
            let last = stack.last_mut().expect("Stack to never be empty");
            last.inc();
            return *last;
        }
        _ => return stack.last().copied().expect("Stack to never be empty"),
    };

    stack.push(new_context);
    new_context
}
