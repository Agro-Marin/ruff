use ruff_python_ast::helpers::map_callable;
use ruff_python_ast::{self as ast, ExceptHandler, Stmt};

use crate::model::SemanticModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terminal {
    /// There is no known terminal.
    None,
    /// There is an implicit return (e.g., a path that doesn't return).
    Implicit,
    /// Every path through the function ends with a `raise` statement.
    Raise,
    /// Every path through the function ends with a `raise NotImplementedError` statement.
    RaiseNotImplemented,
    /// No path through the function ends with a `return` statement.
    Return,
    /// Every path through the function ends with a `return` or `raise` statement.
    RaiseOrReturn,
    /// At least one path through the function ends with a `return` statement.
    ConditionalReturn,
}

impl Terminal {
    /// Returns the [`Terminal`] behavior of the function, if it can be determined.
    pub fn from_function(function: &ast::StmtFunctionDef, semantic: &SemanticModel) -> Terminal {
        Self::from_body(&function.body, semantic)
    }

    /// Returns `true` if the [`Terminal`] behavior includes at least one `return` path.
    fn has_any_return(self) -> bool {
        matches!(
            self,
            Self::Return | Self::RaiseOrReturn | Self::ConditionalReturn
        )
    }

    /// Returns `true` if the [`Terminal`] behavior includes at least one implicit `return` path.
    pub fn has_implicit_return(self) -> bool {
        matches!(self, Self::None | Self::Implicit | Self::ConditionalReturn)
    }

    /// Returns `true` if the [`Terminal`] behavior is `Raise` or `RaiseNotImplemented`.
    pub fn is_always_raise(self) -> bool {
        matches!(self, Self::Raise | Self::RaiseNotImplemented)
    }

    /// Returns the [`Terminal`] behavior of the body, if it can be determined.
    fn from_body(stmts: &[Stmt], semantic: &SemanticModel) -> Terminal {
        let mut terminal = Terminal::None;

        for stmt in stmts {
            match stmt {
                Stmt::For(ast::StmtFor { body, orelse, .. })
                | Stmt::While(ast::StmtWhile { body, orelse, .. }) => {
                    if always_breaks(body) {
                        continue;
                    }

                    terminal = terminal.and_then(Self::from_body(body, semantic));

                    if !sometimes_breaks(body, semantic) {
                        terminal = terminal.and_then(Self::from_body(orelse, semantic));
                    }
                }
                Stmt::If(ast::StmtIf {
                    body,
                    elif_else_clauses,
                    ..
                }) => {
                    let branch_terminal = Terminal::branches(
                        std::iter::once(Self::from_body(body, semantic)).chain(
                            elif_else_clauses
                                .iter()
                                .map(|clause| Self::from_body(&clause.body, semantic)),
                        ),
                    );

                    if elif_else_clauses.iter().any(|clause| clause.test.is_none()) {
                        terminal = terminal.and_then(branch_terminal);
                    } else if branch_terminal.has_any_return() {
                        terminal = terminal.and_then(Terminal::ConditionalReturn);
                    }
                }
                Stmt::Match(ast::StmtMatch { cases, .. }) => {
                    let branch_terminal = terminal.and_then(Terminal::branches(
                        cases
                            .iter()
                            .map(|case| Self::from_body(&case.body, semantic)),
                    ));

                    if cases.iter().any(is_wildcard) {
                        terminal = terminal.and_then(branch_terminal);
                    } else {
                        if branch_terminal.has_any_return() {
                            terminal = terminal.and_then(Terminal::ConditionalReturn);
                        }
                    }
                }
                Stmt::Try(ast::StmtTry {
                    body,
                    handlers,
                    orelse,
                    finalbody,
                    ..
                }) => {
                    let body_terminal = Self::from_body(body, semantic);
                    if body_terminal.has_any_return() {
                        terminal = terminal.and_then(Terminal::ConditionalReturn);
                    }

                    terminal = terminal.and_then(Self::from_body(finalbody, semantic));

                    let branch_terminal = Terminal::branches(handlers.iter().map(|handler| {
                        let ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                            body,
                            ..
                        }) = handler;
                        Self::from_body(body, semantic)
                    }));

                    if orelse.is_empty() {
                        // If there's no `else`, we may fall through, so only mark that this can't
                        if branch_terminal.has_any_return() {
                            terminal = terminal.and_then(Terminal::ConditionalReturn);
                        }
                    } else {
                        // If there's an `else`, we won't fall through. If all the handlers and
                        terminal = terminal.and_then(
                            branch_terminal.branch(Terminal::from_body(orelse, semantic)),
                        );
                    }
                }
                Stmt::With(ast::StmtWith { body, .. }) => {
                    terminal = terminal.and_then(Self::from_body(body, semantic));
                }
                Stmt::Return(_) => {
                    terminal = terminal.and_then(Terminal::RaiseOrReturn);
                }
                Stmt::Raise(raise) => {
                    let is_not_implemented = raise.exc.as_ref().is_some_and(|exc| {
                        let exc = map_callable(exc);
                        semantic.match_builtin_expr(exc, "NotImplementedError")
                    });

                    if is_not_implemented {
                        terminal = terminal.and_then(Terminal::RaiseNotImplemented);
                    } else {
                        terminal = terminal.and_then(Terminal::Raise);
                    }
                }
                _ => {}
            }
        }

        match terminal {
            Terminal::None => Terminal::Implicit,
            _ => terminal,
        }
    }

    /// Combine two [`Terminal`] operators, with one appearing after the other.
    fn and_then(self, other: Self) -> Self {
        match (self, other) {
            (Self::None, other) => other,
            (other, Self::None) => other,

            (Self::Implicit, other) => other,
            (other, Self::Implicit) => other,

            (Self::ConditionalReturn, Self::ConditionalReturn) => Self::ConditionalReturn,

            (Self::Raise | Self::RaiseNotImplemented, Self::ConditionalReturn) => {
                Self::RaiseOrReturn
            }
            (Self::ConditionalReturn, Self::Raise | Self::RaiseNotImplemented) => {
                Self::RaiseOrReturn
            }

            (Self::Return, Self::ConditionalReturn) => Self::Return,
            (Self::ConditionalReturn, Self::Return) => Self::Return,

            (Self::Raise, Self::Raise) => Self::Raise,
            (Self::RaiseNotImplemented, Self::RaiseNotImplemented) => Self::RaiseNotImplemented,
            (Self::RaiseNotImplemented, Self::Raise) | (Self::Raise, Self::RaiseNotImplemented) => {
                Self::Raise
            }

            (Self::Return, Self::Return) => Self::Return,

            (Self::Raise | Self::RaiseNotImplemented, Self::Return) => Self::RaiseOrReturn,

            (Self::Return, Self::Raise | Self::RaiseNotImplemented) => Self::RaiseOrReturn,

            (Self::RaiseOrReturn, _) => Self::RaiseOrReturn,
            (_, Self::RaiseOrReturn) => Self::RaiseOrReturn,
        }
    }

    /// Combine two [`Terminal`] operators from different branches.
    fn branch(self, other: Self) -> Self {
        match (self, other) {
            (Self::None, other) => other,
            (other, Self::None) => other,

            (Self::Implicit, Self::Implicit) => Self::Implicit,
            (Self::Implicit, Self::Raise | Self::RaiseNotImplemented) => Self::Implicit,
            (Self::Raise | Self::RaiseNotImplemented, Self::Implicit) => Self::Implicit,
            (Self::Implicit, Self::Return) => Self::ConditionalReturn,
            (Self::Return, Self::Implicit) => Self::ConditionalReturn,
            (Self::Implicit, Self::RaiseOrReturn) => Self::ConditionalReturn,
            (Self::RaiseOrReturn, Self::Implicit) => Self::ConditionalReturn,
            (Self::Implicit, Self::ConditionalReturn) => Self::ConditionalReturn,
            (Self::ConditionalReturn, Self::Implicit) => Self::ConditionalReturn,

            (Self::ConditionalReturn, Self::ConditionalReturn) => Self::ConditionalReturn,

            (Self::Raise | Self::RaiseNotImplemented, Self::ConditionalReturn) => {
                Self::RaiseOrReturn
            }
            (Self::ConditionalReturn, Self::Raise | Self::RaiseNotImplemented) => {
                Self::RaiseOrReturn
            }

            (Self::Return, Self::ConditionalReturn) => Self::Return,
            (Self::ConditionalReturn, Self::Return) => Self::Return,

            (Self::Raise, Self::Raise) => Self::Raise,
            (Self::RaiseNotImplemented, Self::RaiseNotImplemented) => Self::RaiseNotImplemented,
            (Self::RaiseNotImplemented, Self::Raise) | (Self::Raise, Self::RaiseNotImplemented) => {
                Self::Raise
            }
            (Self::Return, Self::Return) => Self::Return,
            (Self::Raise | Self::RaiseNotImplemented, Self::Return) => Self::RaiseOrReturn,
            (Self::Return, Self::Raise | Self::RaiseNotImplemented) => Self::RaiseOrReturn,
            (Self::RaiseOrReturn, _) => Self::RaiseOrReturn,
            (_, Self::RaiseOrReturn) => Self::RaiseOrReturn,
        }
    }

    /// Combine a series of [`Terminal`] operators.
    fn branches(iter: impl Iterator<Item = Terminal>) -> Terminal {
        iter.fold(Terminal::None, Terminal::branch)
    }
}

/// Returns `true` if the body may break via a `break` statement.
fn sometimes_breaks(stmts: &[Stmt], semantic: &SemanticModel) -> bool {
    for stmt in stmts {
        match stmt {
            Stmt::For(ast::StmtFor { body, orelse, .. }) => {
                if Terminal::from_body(body, semantic).has_any_return() {
                    return false;
                }
                if sometimes_breaks(orelse, semantic) {
                    return true;
                }
            }
            Stmt::While(ast::StmtWhile { body, orelse, .. }) => {
                if Terminal::from_body(body, semantic).has_any_return() {
                    return false;
                }
                if sometimes_breaks(orelse, semantic) {
                    return true;
                }
            }
            Stmt::If(ast::StmtIf {
                body,
                elif_else_clauses,
                ..
            }) if std::iter::once(body)
                .chain(elif_else_clauses.iter().map(|clause| &clause.body))
                .any(|body| sometimes_breaks(body, semantic)) =>
            {
                return true;
            }
            Stmt::Match(ast::StmtMatch { cases, .. })
                if cases
                    .iter()
                    .any(|case| sometimes_breaks(&case.body, semantic)) =>
            {
                return true;
            }
            Stmt::Try(ast::StmtTry {
                body,
                handlers,
                orelse,
                finalbody,
                ..
            }) if (sometimes_breaks(body, semantic)
                || handlers.iter().any(|handler| {
                    let ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                        body, ..
                    }) = handler;
                    sometimes_breaks(body, semantic)
                })
                || sometimes_breaks(orelse, semantic)
                || sometimes_breaks(finalbody, semantic)) =>
            {
                return true;
            }
            Stmt::With(ast::StmtWith { body, .. }) if sometimes_breaks(body, semantic) => {
                return true;
            }
            Stmt::Break(_) => return true,
            Stmt::Return(_) => return false,
            Stmt::Raise(_) => return false,
            _ => {}
        }
    }
    false
}

/// Returns `true` if the body may break via a `break` statement.
fn always_breaks(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        match stmt {
            Stmt::Break(_) => return true,
            Stmt::Return(_) => return false,
            Stmt::Raise(_) => return false,
            _ => {}
        }
    }
    false
}

/// Returns true if the [`MatchCase`] is a wildcard pattern.
fn is_wildcard(pattern: &ast::MatchCase) -> bool {
    /// Returns true if the [`Pattern`] is a wildcard pattern.
    fn is_wildcard_pattern(pattern: &ast::Pattern) -> bool {
        match pattern {
            ast::Pattern::MatchValue(_)
            | ast::Pattern::MatchSingleton(_)
            | ast::Pattern::MatchSequence(_)
            | ast::Pattern::MatchMapping(_)
            | ast::Pattern::MatchClass(_)
            | ast::Pattern::MatchStar(_) => false,

            ast::Pattern::MatchAs(ast::PatternMatchAs { pattern, .. }) => pattern.is_none(),
            ast::Pattern::MatchOr(ast::PatternMatchOr { patterns, .. }) => {
                patterns.iter().all(is_wildcard_pattern)
            }
        }
    }

    pattern.guard.is_none() && is_wildcard_pattern(&pattern.pattern)
}
