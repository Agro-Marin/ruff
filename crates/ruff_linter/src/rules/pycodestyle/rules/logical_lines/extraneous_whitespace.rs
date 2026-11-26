use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::token::TokenKind;
use ruff_text_size::{Ranged, TextRange};

use crate::AlwaysFixableViolation;
use crate::Edit;
use crate::Fix;
use crate::checkers::ast::LintContext;
use crate::codes::Category;

use super::{LogicalLine, Whitespace};

/// ## What it does
/// Checks for the use of extraneous whitespace after "(", "[" or "{".
///
/// ## Why is this bad?
/// [PEP 8] recommends the omission of whitespace in the following cases:
/// - "Immediately inside parentheses, brackets or braces."
/// - "Immediately before a comma, semicolon, or colon."
///
/// ## Example
/// ```python
/// spam( ham[1], {eggs: 2})
/// spam(ham[ 1], {eggs: 2})
/// spam(ham[1], { eggs: 2})
/// ```
///
/// Use instead:
/// ```python
/// spam(ham[1], {eggs: 2})
/// ```
///
/// [PEP 8]: https://peps.python.org/pep-0008/#pet-peeves
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "v0.0.269", category = Category::Formatting)]
pub(crate) struct WhitespaceAfterOpenBracket {
    symbol: char,
}

impl AlwaysFixableViolation for WhitespaceAfterOpenBracket {
    #[derive_message_formats]
    fn message(&self) -> String {
        let WhitespaceAfterOpenBracket { symbol } = self;
        format!("Whitespace after '{symbol}'")
    }

    fn fix_title(&self) -> String {
        let WhitespaceAfterOpenBracket { symbol } = self;
        format!("Remove whitespace before '{symbol}'")
    }
}

/// ## What it does
/// Checks for the use of extraneous whitespace before ")", "]" or "}".
///
/// ## Why is this bad?
/// [PEP 8] recommends the omission of whitespace in the following cases:
/// - "Immediately inside parentheses, brackets or braces."
/// - "Immediately before a comma, semicolon, or colon."
///
/// ## Example
/// ```python
/// spam(ham[1], {eggs: 2} )
/// spam(ham[1 ], {eggs: 2})
/// spam(ham[1], {eggs: 2 })
/// ```
///
/// Use instead:
/// ```python
/// spam(ham[1], {eggs: 2})
/// ```
///
/// [PEP 8]: https://peps.python.org/pep-0008/#pet-peeves
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "v0.0.269", category = Category::Formatting)]
pub(crate) struct WhitespaceBeforeCloseBracket {
    symbol: char,
}

impl AlwaysFixableViolation for WhitespaceBeforeCloseBracket {
    #[derive_message_formats]
    fn message(&self) -> String {
        let WhitespaceBeforeCloseBracket { symbol } = self;
        format!("Whitespace before '{symbol}'")
    }

    fn fix_title(&self) -> String {
        let WhitespaceBeforeCloseBracket { symbol } = self;
        format!("Remove whitespace before '{symbol}'")
    }
}

/// ## What it does
/// Checks for the use of extraneous whitespace before ",", ";" or ":".
///
/// ## Why is this bad?
/// [PEP 8] recommends the omission of whitespace in the following cases:
/// - "Immediately inside parentheses, brackets or braces."
/// - "Immediately before a comma, semicolon, or colon."
///
/// ## Example
/// ```python
/// if x == 4: print(x, y); x, y = y , x
/// ```
///
/// Use instead:
/// ```python
/// if x == 4: print(x, y); x, y = y, x
/// ```
///
/// [PEP 8]: https://peps.python.org/pep-0008/#pet-peeves
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "v0.0.269", category = Category::Formatting)]
pub(crate) struct WhitespaceBeforePunctuation {
    symbol: char,
}

impl AlwaysFixableViolation for WhitespaceBeforePunctuation {
    #[derive_message_formats]
    fn message(&self) -> String {
        let WhitespaceBeforePunctuation { symbol } = self;
        format!("Whitespace before '{symbol}'")
    }

    fn fix_title(&self) -> String {
        let WhitespaceBeforePunctuation { symbol } = self;
        format!("Remove whitespace before '{symbol}'")
    }
}

/// E201, E202, E203
pub(crate) fn extraneous_whitespace(line: &LogicalLine, context: &LintContext) {
    let mut interpolated_strings = 0u32;
    let mut brackets = vec![];
    let mut prev_token = None;
    let mut iter = line.tokens().iter().peekable();

    while let Some(token) = iter.next() {
        let kind = token.kind();
        match kind {
            TokenKind::FStringStart | TokenKind::TStringStart => interpolated_strings += 1,
            TokenKind::FStringEnd | TokenKind::TStringEnd => {
                interpolated_strings = interpolated_strings.saturating_sub(1);
            }
            TokenKind::Lsqb => {
                brackets.push(kind);
            }
            TokenKind::Rsqb => {
                brackets.pop();
            }
            TokenKind::Lbrace => {
                brackets.push(kind);
            }
            TokenKind::Rbrace => {
                brackets.pop();
            }
            _ => {}
        }
        if let Some(symbol) = BracketOrPunctuation::from_kind(kind) {
            match symbol {
                BracketOrPunctuation::OpenBracket(symbol)
                    if symbol != '{' || interpolated_strings == 0 =>
                {
                    let (trailing, trailing_len) = line.trailing_whitespace(token);
                    if !matches!(trailing, Whitespace::None) {
                        let range = TextRange::at(token.end(), trailing_len);
                        if let Some(mut diagnostic) = context.report_diagnostic_if_enabled(
                            WhitespaceAfterOpenBracket { symbol },
                            range,
                        ) {
                            diagnostic.set_fix(Fix::safe_edit(Edit::range_deletion(range)));
                        }
                    }
                }
                BracketOrPunctuation::CloseBracket(symbol)
                    if (symbol != '}' || interpolated_strings == 0)
                        && !matches!(prev_token, Some(TokenKind::Comma)) =>
                {
                    if let (Whitespace::Single | Whitespace::Many | Whitespace::Tab, offset) =
                        line.leading_whitespace(token)
                    {
                        let range = TextRange::at(token.start() - offset, offset);
                        if let Some(mut diagnostic) = context.report_diagnostic_if_enabled(
                            WhitespaceBeforeCloseBracket { symbol },
                            range,
                        ) {
                            diagnostic.set_fix(Fix::safe_edit(Edit::range_deletion(range)));
                        }
                    }
                }
                BracketOrPunctuation::Punctuation(symbol)
                    if !matches!(prev_token, Some(TokenKind::Comma)) =>
                {
                    let whitespace = line.leading_whitespace(token);
                    if let (Whitespace::Single | Whitespace::Many | Whitespace::Tab, offset) =
                        whitespace
                    {
                        if symbol == ':'
                            && brackets
                                .last()
                                .is_some_and(|kind| matches!(kind, TokenKind::Lsqb))
                        {
                            if matches!(prev_token, Some(TokenKind::Colon)) {
                                let range = TextRange::at(token.start() - offset, offset);
                                if let Some(mut diagnostic) = context.report_diagnostic_if_enabled(
                                    WhitespaceBeforePunctuation { symbol },
                                    range,
                                ) {
                                    diagnostic.set_fix(Fix::safe_edit(Edit::range_deletion(range)));
                                }
                            } else if iter.peek().is_some_and(|token| {
                                matches!(token.kind(), TokenKind::Rsqb | TokenKind::Comma)
                            }) {
                                if let (Whitespace::Many | Whitespace::Tab, offset) = whitespace {
                                    let range = TextRange::at(token.start() - offset, offset);
                                    if let Some(mut diagnostic) = context
                                        .report_diagnostic_if_enabled(
                                            WhitespaceBeforePunctuation { symbol },
                                            range,
                                        )
                                    {
                                        diagnostic
                                            .set_fix(Fix::safe_edit(Edit::range_deletion(range)));
                                    }
                                }
                            } else if iter.peek().is_some_and(|token| {
                                matches!(
                                    token.kind(),
                                    TokenKind::NonLogicalNewline | TokenKind::Comment
                                )
                            }) {
                                if let (Whitespace::Many | Whitespace::Tab, offset) = whitespace {
                                    let range = TextRange::at(token.start() - offset, offset);
                                    if let Some(mut diagnostic) = context
                                        .report_diagnostic_if_enabled(
                                            WhitespaceBeforePunctuation { symbol },
                                            range,
                                        )
                                    {
                                        diagnostic.set_fix(Fix::safe_edits(
                                            Edit::range_deletion(range),
                                            [Edit::insertion(" ".into(), token.start() - offset)],
                                        ));
                                    }
                                }
                            } else {
                                let token = iter
                                    .peek()
                                    .filter(|next| matches!(next.kind(), TokenKind::Colon))
                                    .unwrap_or(&token);
                                if line.trailing_whitespace(token) != whitespace {
                                    let range = TextRange::at(token.start() - offset, offset);
                                    if let Some(mut diagnostic) = context
                                        .report_diagnostic_if_enabled(
                                            WhitespaceBeforePunctuation { symbol },
                                            range,
                                        )
                                    {
                                        diagnostic
                                            .set_fix(Fix::safe_edit(Edit::range_deletion(range)));
                                    }
                                }
                            }
                        } else {
                            if interpolated_strings > 0
                                && symbol == ':'
                                && matches!(prev_token, Some(TokenKind::Equal))
                            {
                                continue;
                            }
                            let range = TextRange::at(token.start() - offset, offset);
                            if let Some(mut diagnostic) = context.report_diagnostic_if_enabled(
                                WhitespaceBeforePunctuation { symbol },
                                range,
                            ) {
                                diagnostic.set_fix(Fix::safe_edit(Edit::range_deletion(range)));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        prev_token = Some(kind);
    }
}

#[derive(Debug)]
enum BracketOrPunctuation {
    OpenBracket(char),
    CloseBracket(char),
    Punctuation(char),
}

impl BracketOrPunctuation {
    fn from_kind(kind: TokenKind) -> Option<BracketOrPunctuation> {
        match kind {
            TokenKind::Lbrace => Some(BracketOrPunctuation::OpenBracket('{')),
            TokenKind::Lpar => Some(BracketOrPunctuation::OpenBracket('(')),
            TokenKind::Lsqb => Some(BracketOrPunctuation::OpenBracket('[')),
            TokenKind::Rbrace => Some(BracketOrPunctuation::CloseBracket('}')),
            TokenKind::Rpar => Some(BracketOrPunctuation::CloseBracket(')')),
            TokenKind::Rsqb => Some(BracketOrPunctuation::CloseBracket(']')),
            TokenKind::Comma => Some(BracketOrPunctuation::Punctuation(',')),
            TokenKind::Colon => Some(BracketOrPunctuation::Punctuation(':')),
            TokenKind::Semi => Some(BracketOrPunctuation::Punctuation(';')),
            _ => None,
        }
    }
}
