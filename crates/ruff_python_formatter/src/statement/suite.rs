use ruff_formatter::{
    FormatContext, FormatOwnedWithRule, FormatRefWithRule, FormatRuleWithOptions, write,
};
use ruff_python_ast::helpers::is_compound_statement;
use ruff_python_ast::{self as ast, Expr, PySourceType, Stmt, Suite};
use ruff_python_ast::{AnyNodeRef, StmtExpr};
use ruff_python_trivia::{
    SimpleTokenKind, SimpleTokenizer, lines_after, lines_after_ignoring_end_of_line_trivia,
    lines_before,
};
use ruff_text_size::{Ranged, TextRange};

use crate::comments::{
    Comments, LeadingDanglingTrailingComments, has_skip_comment, leading_comments,
    trailing_comments,
};
use crate::context::{NodeLevel, TopLevelStatementPosition, WithIndentLevel, WithNodeLevel};
use crate::other::string_literal::StringLiteralKind;
use crate::prelude::*;
use crate::statement::trailing_semicolon;
use crate::verbatim::{
    write_skipped_statements, write_suppressed_statements_starting_with_leading_comment,
    write_suppressed_statements_starting_with_trailing_comment,
};

/// Level at which the [`Suite`] appears in the source code.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SuiteKind {
    /// Statements at the module level / top level
    TopLevel,

    /// Statements in a function body.
    Function,

    /// Statements in a class body.
    Class,

    /// Statements in any other body (e.g., `if` or `while`).
    Other {
        /// Whether this suite is the last suite in the current statement.
        ///
        /// Below, `last_suite_in_statement` is `false` for the suite containing `foo10` and `foo12`
        /// and `true` for the suite containing `bar`.
        /// ```python
        /// if sys.version_info >= (3, 10):
        ///     def foo10():
        ///         return "new"
        /// elif sys.version_info >= (3, 12):
        ///     def foo12():
        ///         return "new"
        /// else:
        ///     def bar():
        ///         return "old"
        /// ```
        ///
        /// When this value is true, we don't insert trailing empty lines since the containing suite
        /// will do that.
        last_suite_in_statement: bool,
    },
}

impl Default for SuiteKind {
    fn default() -> Self {
        Self::Other {
            last_suite_in_statement: true,
        }
    }
}

impl SuiteKind {
    /// See [`SuiteKind::Other`].
    pub fn other(last_suite_in_statement: bool) -> Self {
        Self::Other {
            last_suite_in_statement,
        }
    }

    pub fn last_suite_in_statement(self) -> bool {
        match self {
            Self::Other {
                last_suite_in_statement,
            } => last_suite_in_statement,
            _ => true,
        }
    }
}

#[derive(Debug, Default)]
pub struct FormatSuite {
    kind: SuiteKind,
}

impl FormatRule<Suite, PyFormatContext<'_>> for FormatSuite {
    fn fmt(&self, statements: &Suite, f: &mut PyFormatter) -> FormatResult<()> {
        let mut iter = statements.iter();
        let Some(first) = iter.next() else {
            return Ok(());
        };

        let node_level = match self.kind {
            SuiteKind::TopLevel => NodeLevel::TopLevel(
                iter.clone()
                    .next()
                    .map_or(TopLevelStatementPosition::Last, |_| {
                        TopLevelStatementPosition::Other
                    }),
            ),
            SuiteKind::Function | SuiteKind::Class | SuiteKind::Other { .. } => {
                NodeLevel::CompoundStatement
            }
        };

        let comments = f.context().comments().clone();
        let source = f.context().source();
        let source_type = f.options().source_type();

        let f = WithNodeLevel::new(node_level, f);
        let f = &mut WithIndentLevel::new(f.context().indent_level().increment(), f);

        let first = match self.kind {
            SuiteKind::Other { .. } => {
                if is_class_or_function_definition(first)
                    && !comments.has_leading(first)
                    && !source_type.is_stub()
                {
                    empty_line().fmt(f)?;
                }

                SuiteChildStatement::Other(first)
            }

            SuiteKind::Function | SuiteKind::Class | SuiteKind::TopLevel => {
                if let Some(docstring) =
                    DocstringStmt::try_from_statement(first, self.kind, f.context())
                {
                    SuiteChildStatement::Docstring(docstring)
                } else {
                    SuiteChildStatement::Other(first)
                }
            }
        };

        let first_comments = comments.leading_dangling_trailing(first);

        let (mut preceding, mut empty_line_after_docstring) = if let Some(verbatim_range) =
            skip_range(first.statement(), iter.as_slice(), f.context())
        {
            let preceding =
                write_skipped_statements(first.statement(), &mut iter, verbatim_range, f)?;

            let empty_line_after_docstring =
                matches!(self.kind, SuiteKind::TopLevel | SuiteKind::Class)
                    && DocstringStmt::try_from_statement(preceding, self.kind, f.context())
                        .is_some();

            (preceding, empty_line_after_docstring)
        } else if first_comments
            .leading
            .iter()
            .any(|comment| comment.is_suppression_off_comment(source))
        {
            (
                write_suppressed_statements_starting_with_leading_comment(first, &mut iter, f)?,
                false,
            )
        } else if first_comments
            .trailing
            .iter()
            .any(|comment| comment.is_suppression_off_comment(source))
        {
            (
                write_suppressed_statements_starting_with_trailing_comment(first, &mut iter, f)?,
                false,
            )
        } else {
            let allow_newline_after_block_open = matches!(self.kind, SuiteKind::Function)
                && matches!(first, SuiteChildStatement::Other(_));

            let start = comments
                .leading(first)
                .first()
                .map_or_else(|| first.start(), Ranged::start);

            if allow_newline_after_block_open && lines_before(start, f.context().source()) > 1 {
                empty_line().fmt(f)?;
            }

            first.fmt(f)?;

            let empty_line_after_docstring = if matches!(first, SuiteChildStatement::Docstring(_))
                && self.kind == SuiteKind::Class
            {
                true
            } else {
                self.kind == SuiteKind::TopLevel
                    && DocstringStmt::try_from_statement(first.statement(), self.kind, f.context())
                        .is_some()
            };

            (first.statement(), empty_line_after_docstring)
        };

        let mut preceding_comments = comments.leading_dangling_trailing(preceding);

        while let Some(following) = iter.next() {
            if self.kind == SuiteKind::TopLevel && iter.clone().next().is_none() {
                f.context_mut()
                    .set_node_level(NodeLevel::TopLevel(TopLevelStatementPosition::Last));
            }

            let following_comments = comments.leading_dangling_trailing(following);

            let needs_empty_lines = if is_class_or_function_definition(following) {
                true
            } else {
                trailing_function_or_class_def(Some(preceding), &comments).is_some()
            };

            if needs_empty_lines {
                if source_type.is_stub() {
                    stub_file_empty_lines(
                        self.kind,
                        preceding,
                        following,
                        &preceding_comments,
                        &following_comments,
                        f,
                    )?;
                } else {
                    let is_preceding_stub_function_without_empty_line = following
                        .is_function_def_stmt()
                        && preceding
                            .as_function_def_stmt()
                            .is_some_and(|preceding_stub| {
                                contains_only_an_ellipsis(
                                    &preceding_stub.body,
                                    f.context().comments(),
                                ) && lines_after_ignoring_end_of_line_trivia(
                                    preceding_stub.end(),
                                    f.context().source(),
                                ) < 2
                            })
                        && !preceding_comments.has_trailing_own_line();

                    if !is_preceding_stub_function_without_empty_line {
                        match self.kind {
                            SuiteKind::TopLevel => {
                                write!(f, [empty_line(), empty_line()])?;
                            }
                            SuiteKind::Function | SuiteKind::Class | SuiteKind::Other { .. } => {
                                empty_line().fmt(f)?;
                            }
                        }
                    }
                }
            } else if is_import_definition(preceding)
                && (!is_import_definition(following) || following_comments.has_leading())
            {
                match self.kind {
                    SuiteKind::TopLevel => {
                        let end = if let Some(last_trailing) = preceding_comments.trailing.last() {
                            last_trailing.end()
                        } else {
                            preceding.end()
                        };
                        match lines_after(end, source) {
                            0..=2 => empty_line().fmt(f)?,
                            _ => match source_type {
                                PySourceType::Stub => {
                                    empty_line().fmt(f)?;
                                }
                                PySourceType::Python | PySourceType::Ipynb => {
                                    write!(f, [empty_line(), empty_line()])?;
                                }
                            },
                        }
                    }
                    SuiteKind::Function | SuiteKind::Class | SuiteKind::Other { .. } => {
                        empty_line().fmt(f)?;
                    }
                }
            } else if is_compound_statement(preceding) {
                let start = if let Some(first_leading) = following_comments.leading.first() {
                    first_leading.start()
                } else {
                    following.start()
                };

                match lines_before(start, source) {
                    0 | 1 => hard_line_break().fmt(f)?,
                    2 => empty_line().fmt(f)?,
                    _ => match self.kind {
                        SuiteKind::TopLevel => match source_type {
                            PySourceType::Stub => {
                                empty_line().fmt(f)?;
                            }
                            PySourceType::Python | PySourceType::Ipynb => {
                                write!(f, [empty_line(), empty_line()])?;
                            }
                        },
                        SuiteKind::Function | SuiteKind::Class | SuiteKind::Other { .. } => {
                            empty_line().fmt(f)?;
                        }
                    },
                }
            } else if empty_line_after_docstring {
                empty_line().fmt(f)?;
            } else {

                let end = preceding_comments
                    .trailing
                    .last()
                    .map_or(preceding.end(), |comment| comment.slice().end());

                match node_level {
                    NodeLevel::TopLevel(_) => match lines_after(end, source) {
                        0 | 1 => hard_line_break().fmt(f)?,
                        2 => empty_line().fmt(f)?,
                        _ => match source_type {
                            PySourceType::Stub => {
                                empty_line().fmt(f)?;
                            }
                            PySourceType::Python | PySourceType::Ipynb => {
                                write!(f, [empty_line(), empty_line()])?;
                            }
                        },
                    },
                    NodeLevel::CompoundStatement => match lines_after(end, source) {
                        0 | 1 => hard_line_break().fmt(f)?,
                        _ => empty_line().fmt(f)?,
                    },
                    NodeLevel::Expression(_) | NodeLevel::ParenthesizedExpression => {
                        hard_line_break().fmt(f)?;
                    }
                }
            }

            if let Some(verbatim_range) = skip_range(following, iter.as_slice(), f.context()) {
                preceding = write_skipped_statements(following, &mut iter, verbatim_range, f)?;
                preceding_comments = comments.leading_dangling_trailing(preceding);
            } else if following_comments
                .leading
                .iter()
                .any(|comment| comment.is_suppression_off_comment(source))
            {
                preceding = write_suppressed_statements_starting_with_leading_comment(
                    SuiteChildStatement::Other(following),
                    &mut iter,
                    f,
                )?;
                preceding_comments = comments.leading_dangling_trailing(preceding);
            } else if following_comments
                .trailing
                .iter()
                .any(|comment| comment.is_suppression_off_comment(source))
            {
                preceding = write_suppressed_statements_starting_with_trailing_comment(
                    SuiteChildStatement::Other(following),
                    &mut iter,
                    f,
                )?;
                preceding_comments = comments.leading_dangling_trailing(preceding);
            } else {
                following.format().fmt(f)?;
                preceding = following;
                preceding_comments = following_comments;
            }

            empty_line_after_docstring = false;
        }

        self.between_alternative_blocks_empty_line(statements, &comments, f)?;

        Ok(())
    }
}

impl FormatSuite {
    /// Add an empty line between a function or class and an alternative body.
    ///
    /// We only insert an empty if we're between suites in a multi-suite statement. In the
    /// if-else-statement below, we insert an empty line after the `foo` in the if-block, but none
    /// after the else-block `foo`, since in the latter case the enclosing suite already adds
    /// empty lines.
    ///
    /// ```python
    /// if sys.version_info >= (3, 10):
    ///     def foo():
    ///         return "new"
    /// else:
    ///     def foo():
    ///         return "old"
    /// class Bar:
    ///     pass
    /// ```
    fn between_alternative_blocks_empty_line(
        &self,
        statements: &Suite,
        comments: &Comments,
        f: &mut PyFormatter,
    ) -> FormatResult<()> {
        if self.kind.last_suite_in_statement() {
            return Ok(());
        }

        let Some(last_def_or_class) = trailing_function_or_class_def(statements.last(), comments)
        else {
            return Ok(());
        };

        let node_with_last_trailing_comment = std::iter::successors(
            statements.last().map(AnyNodeRef::from),
            AnyNodeRef::last_child_in_body,
        )
        .find(|last_child| comments.has_trailing_own_line(*last_child));

        let end_of_def_or_class = node_with_last_trailing_comment
            .and_then(|child| comments.trailing(child).last().map(Ranged::end))
            .unwrap_or(last_def_or_class.end());
        let existing_newlines =
            lines_after_ignoring_end_of_line_trivia(end_of_def_or_class, f.context().source());
        if existing_newlines < 2 {
            if f.context().is_preview() {
                empty_line().fmt(f)?;
            } else {
                if last_def_or_class.is_stmt_class_def() && f.options().source_type().is_stub() {
                    empty_line().fmt(f)?;
                }
            }
        }
        Ok(())
    }
}

/// Find nested class or function definitions that need an empty line after them.
///
/// ```python
/// def f():
///     if True:
///
///         def double(s):
///             return s + s
///
///     print("below function")
/// ```
fn trailing_function_or_class_def<'a>(
    preceding: Option<&'a Stmt>,
    comments: &Comments,
) -> Option<AnyNodeRef<'a>> {
    std::iter::successors(
        preceding.map(AnyNodeRef::from),
        AnyNodeRef::last_child_in_body,
    )
    .take_while(|last_child| !comments.has_trailing_own_line(*last_child))
    .find(|last_child| {
        matches!(
            last_child,
            AnyNodeRef::StmtFunctionDef(_) | AnyNodeRef::StmtClassDef(_)
        )
    })
}

/// Stub files have bespoke rules for empty lines.
///
/// These rules are ported from black (preview mode at time of writing) using the stubs test case:
/// <https://github.com/psf/black/blob/c160e4b7ce30c661ac4f2dfa5038becf1b8c5c33/src/black/lines.py#L576-L744>
fn stub_file_empty_lines(
    kind: SuiteKind,
    preceding: &Stmt,
    following: &Stmt,
    preceding_comments: &LeadingDanglingTrailingComments,
    following_comments: &LeadingDanglingTrailingComments,
    f: &mut PyFormatter,
) -> FormatResult<()> {
    let source = f.context().source();
    let empty_line_condition = preceding_comments.has_trailing()
        || following_comments.has_leading()
        || !stub_suite_can_omit_empty_line(preceding, following, f);
    let require_empty_line = should_insert_blank_line_after_class_in_stub_file(
        preceding.into(),
        Some(following.into()),
        f.context(),
    );
    match kind {
        SuiteKind::TopLevel => {
            if empty_line_condition || require_empty_line {
                empty_line().fmt(f)
            } else {
                hard_line_break().fmt(f)
            }
        }
        SuiteKind::Class | SuiteKind::Other { .. } | SuiteKind::Function => {
            if (empty_line_condition
                && lines_after_ignoring_end_of_line_trivia(preceding.end(), source) > 1)
                || require_empty_line
            {
                empty_line().fmt(f)
            } else {
                hard_line_break().fmt(f)
            }
        }
    }
}

/// Checks if an empty line should be inserted after a class definition.
///
/// This is only valid if the [`blank_line_after_nested_stub_class`](https://github.com/astral-sh/ruff/issues/8891)
/// preview rule is enabled and the source to be formatted is a stub file.
///
/// If `following` is `None`, then the preceding node is the last one in a suite. The
/// caller needs to make sure that the suite which the preceding node is part of is
/// followed by an alternate branch and shouldn't be a top-level suite.
pub(crate) fn should_insert_blank_line_after_class_in_stub_file(
    preceding: AnyNodeRef<'_>,
    following: Option<AnyNodeRef<'_>>,
    context: &PyFormatContext,
) -> bool {
    if !context.options().source_type().is_stub() {
        return false;
    }

    let Some(following) = following else {
        return false;
    };

    let comments = context.comments();
    match preceding.as_stmt_class_def() {
        Some(class) if contains_only_an_ellipsis(&class.body, comments) => {
            let preceding_has_decorators = !class.decorator_list.is_empty();

            let following_is_class_without_only_ellipsis_or_has_decorators =
                following.as_stmt_class_def().is_some_and(|following| {
                    !contains_only_an_ellipsis(&following.body, comments)
                        || !following.decorator_list.is_empty()
                });

            preceding_has_decorators
                || following_is_class_without_only_ellipsis_or_has_decorators
                || following.is_stmt_function_def()
        }
        Some(_) => {
            !comments.has_trailing_own_line(preceding)
        }
        None => {
            std::iter::successors(
                preceding.last_child_in_body(),
                AnyNodeRef::last_child_in_body,
            )
            .take_while(|last_child| !comments.has_trailing_own_line(*last_child))
            .any(|last_child| last_child.is_stmt_class_def())
        }
    }
}

/// Only a function to compute it lazily
fn stub_suite_can_omit_empty_line(preceding: &Stmt, following: &Stmt, f: &PyFormatter) -> bool {
    let class_sequences_with_ellipsis_only = preceding
        .as_class_def_stmt()
        .is_some_and(|class| contains_only_an_ellipsis(&class.body, f.context().comments()))
        && following.as_class_def_stmt().is_some_and(|class| {
            contains_only_an_ellipsis(&class.body, f.context().comments())
                && class.decorator_list.is_empty()
        });

    let function_with_ellipsis = preceding
        .as_function_def_stmt()
        .is_some_and(|function| contains_only_an_ellipsis(&function.body, f.context().comments()))
        && following.is_function_def_stmt();

    class_sequences_with_ellipsis_only || function_with_ellipsis
}

/// Returns `true` if a function or class body contains only an ellipsis with no comments.
fn contains_only_an_ellipsis(body: &[Stmt], comments: &Comments) -> bool {
    as_only_an_ellipsis(body, comments).is_some()
}

/// Returns `Some(Stmt::Ellipsis)` if a function or class body contains only an ellipsis with no
/// comments.
pub(crate) fn as_only_an_ellipsis<'a>(body: &'a [Stmt], comments: &Comments) -> Option<&'a Stmt> {
    if let [node @ Stmt::Expr(ast::StmtExpr { value, .. })] = body
        && value.is_ellipsis_literal_expr()
        && !comments.has_leading(node)
        && !comments.has_trailing_own_line(node)
    {
        return Some(node);
    }

    None
}

/// Returns `true` if a [`Stmt`] is a class or function definition.
const fn is_class_or_function_definition(stmt: &Stmt) -> bool {
    matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_))
}

/// Returns `true` if a [`Stmt`] is an import.
const fn is_import_definition(stmt: &Stmt) -> bool {
    matches!(stmt, Stmt::Import(_) | Stmt::ImportFrom(_))
}

impl FormatRuleWithOptions<Suite, PyFormatContext<'_>> for FormatSuite {
    type Options = SuiteKind;

    fn with_options(mut self, options: Self::Options) -> Self {
        self.kind = options;
        self
    }
}

impl<'ast> AsFormat<PyFormatContext<'ast>> for Suite {
    type Format<'a> = FormatRefWithRule<'a, Suite, FormatSuite, PyFormatContext<'ast>>;

    fn format(&self) -> Self::Format<'_> {
        FormatRefWithRule::new(self, FormatSuite::default())
    }
}

impl<'ast> IntoFormat<PyFormatContext<'ast>> for Suite {
    type Format = FormatOwnedWithRule<Suite, FormatSuite, PyFormatContext<'ast>>;

    fn into_format(self) -> Self::Format {
        FormatOwnedWithRule::new(self, FormatSuite::default())
    }
}

/// A statement representing a docstring.
#[derive(Copy, Clone, Debug)]
pub(crate) struct DocstringStmt<'a> {
    /// The [`Stmt::Expr`]
    docstring: &'a Stmt,
    /// The parent suite kind
    suite_kind: SuiteKind,
}

impl<'a> DocstringStmt<'a> {
    /// Checks if the statement is a simple string that can be formatted as a docstring
    fn try_from_statement(
        stmt: &'a Stmt,
        suite_kind: SuiteKind,
        context: &PyFormatContext,
    ) -> Option<DocstringStmt<'a>> {
        if context.options().source_type().is_ipynb() && suite_kind == SuiteKind::TopLevel {
            return None;
        }

        Self::is_docstring_statement(stmt.as_expr_stmt()?, context).then_some(DocstringStmt {
            docstring: stmt,
            suite_kind,
        })
    }

    pub(crate) fn is_docstring_statement(stmt: &StmtExpr, context: &PyFormatContext) -> bool {
        if let Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) = stmt.value.as_ref() {
            !value.is_implicit_concatenated()
                || !value.iter().any(|literal| context.comments().has(literal))
        } else {
            false
        }
    }
}

impl Format<PyFormatContext<'_>> for DocstringStmt<'_> {
    fn fmt(&self, f: &mut Formatter<PyFormatContext<'_>>) -> FormatResult<()> {
        let comments = f.context().comments().clone();
        let node_comments = comments.leading_dangling_trailing(self.docstring);

        // SAFETY: Safe because `DocStringStmt` guarantees that it only ever wraps a `ExprStmt` containing a `ExprStringLiteral`.
        let string_literal = self
            .docstring
            .as_expr_stmt()
            .unwrap()
            .value
            .as_string_literal_expr()
            .unwrap();

        write!(
            f,
            [
                leading_comments(node_comments.leading),
                f.options()
                    .source_map_generation()
                    .is_enabled()
                    .then_some(source_position(self.docstring.start())),
                string_literal
                    .format()
                    .with_options(StringLiteralKind::Docstring),
                f.options()
                    .source_map_generation()
                    .is_enabled()
                    .then_some(source_position(self.docstring.end())),
            ]
        )?;

        if self.suite_kind == SuiteKind::Class {
            if let Some(own_line) = node_comments
                .trailing
                .iter()
                .find(|comment| comment.line_position().is_own_line())
            {
                if lines_before(own_line.start(), f.context().source()) < 2 {
                    empty_line().fmt(f)?;
                }
            }
        }

        trailing_comments(node_comments.trailing).fmt(f)
    }
}

/// A Child of a suite.
#[derive(Copy, Clone, Debug)]
pub(crate) enum SuiteChildStatement<'a> {
    /// A docstring documenting a class or function definition.
    Docstring(DocstringStmt<'a>),

    /// Any other statement.
    Other(&'a Stmt),
}

impl<'a> SuiteChildStatement<'a> {
    pub(crate) const fn statement(self) -> &'a Stmt {
        match self {
            SuiteChildStatement::Docstring(docstring) => docstring.docstring,
            SuiteChildStatement::Other(statement) => statement,
        }
    }
}

impl Ranged for SuiteChildStatement<'_> {
    fn range(&self) -> TextRange {
        self.statement().range()
    }
}

impl<'a> From<SuiteChildStatement<'a>> for AnyNodeRef<'a> {
    fn from(value: SuiteChildStatement<'a>) -> Self {
        value.statement().into()
    }
}

impl Format<PyFormatContext<'_>> for SuiteChildStatement<'_> {
    fn fmt(&self, f: &mut Formatter<PyFormatContext<'_>>) -> FormatResult<()> {
        match self {
            SuiteChildStatement::Docstring(docstring) => docstring.fmt(f),
            SuiteChildStatement::Other(statement) => statement.format().fmt(f),
        }
    }
}

pub(crate) fn skip_range(
    first: &Stmt,
    statements: &[Stmt],
    context: &PyFormatContext,
) -> Option<TextRange> {
    let start = first.start();
    let mut last_statement = first;

    let comments = context.comments();
    let source = context.source();

    for statement in statements {
        if new_logical_line_between_statements(
            source,
            TextRange::new(last_statement.end(), statement.start()),
        ) {
            break;
        }
        last_statement = statement;
    }

    if has_skip_comment(comments.trailing(last_statement), source) {
        Some(TextRange::new(
            start,
            trailing_semicolon(last_statement.into(), source)
                .map_or_else(|| last_statement.end(), ruff_text_size::TextRange::end),
        ))
    } else {
        None
    }
}

fn new_logical_line_between_statements(source: &str, between_statement_range: TextRange) -> bool {
    let mut tokenizer = SimpleTokenizer::new(source, between_statement_range).map(|tok| tok.kind());

    while let Some(token) = tokenizer.next() {
        match token {
            SimpleTokenKind::Continuation => {
                tokenizer.next();
            }
            SimpleTokenKind::Newline => {
                return true;
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use ruff_formatter::format;
    use ruff_python_parser::parse_module;
    use ruff_python_trivia::TriviaRanges;

    use crate::PyFormatOptions;
    use crate::comments::Comments;
    use crate::prelude::*;
    use crate::statement::suite::SuiteKind;

    fn format_suite(level: SuiteKind) -> String {
        let source = r"
a = 10



three_leading_newlines = 80


two_leading_newlines = 20

one_leading_newline = 10
no_leading_newline = 30
class InTheMiddle:
    pass
trailing_statement = 1
def func():
    pass
def trailing_func():
    pass
";

        let parsed = parse_module(source).unwrap();
        let trivia = TriviaRanges::from(parsed.tokens());

        let context = PyFormatContext::new(
            PyFormatOptions::default(),
            source,
            Comments::empty(),
            &trivia,
            parsed.tokens(),
        );

        let test_formatter =
            format_with(|f: &mut PyFormatter| parsed.suite().format().with_options(level).fmt(f));

        let formatted = format!(context, [test_formatter]).unwrap();
        let printed = formatted.print().unwrap();

        printed.as_code().to_string()
    }

    #[test]
    fn top_level() {
        let formatted = format_suite(SuiteKind::TopLevel);

        assert_eq!(
            formatted,
            r"a = 10


three_leading_newlines = 80


two_leading_newlines = 20

one_leading_newline = 10
no_leading_newline = 30


class InTheMiddle:
    pass


trailing_statement = 1


def func():
    pass


def trailing_func():
    pass
"
        );
    }

    #[test]
    fn nested_level() {
        let formatted = format_suite(SuiteKind::Other {
            last_suite_in_statement: true,
        });

        assert_eq!(
            formatted,
            r"a = 10

three_leading_newlines = 80

two_leading_newlines = 20

one_leading_newline = 10
no_leading_newline = 30

class InTheMiddle:
    pass

trailing_statement = 1

def func():
    pass

def trailing_func():
    pass
"
        );
    }
}
