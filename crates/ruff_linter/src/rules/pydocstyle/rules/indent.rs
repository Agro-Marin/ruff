use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::docstrings::{clean_space, leading_space};
use ruff_source_file::{Line, NewlineWithTrailingNewline};
use ruff_text_size::{Ranged, TextSize};
use ruff_text_size::{TextLen, TextRange};

use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::docstrings::Docstring;
use crate::registry::Rule;
use crate::{AlwaysFixableViolation, Violation};
use crate::{Edit, Fix};

#[expect(clippy::tabs_in_doc_comments)]
/// ## What it does
/// Checks for docstrings that are indented with tabs.
///
/// ## Why is this bad?
/// [PEP 8] recommends using spaces over tabs for indentation.
///
/// ## Example
/// ```python
/// def sort_list(l: list[int]) -> list[int]:
///     """Return a sorted copy of the list.
///
/// 	Sort the list in ascending order and return a copy of the result using the bubble
/// 	sort algorithm.
///     """
/// ```
///
/// Use instead:
/// ```python
/// def sort_list(l: list[int]) -> list[int]:
///     """Return a sorted copy of the list.
///
///     Sort the list in ascending order and return a copy of the result using the bubble
///     sort algorithm.
///     """
/// ```
///
/// ## Formatter compatibility
/// We recommend against using this rule alongside the [formatter]. The
/// formatter enforces consistent indentation, making the rule redundant.
///
/// The rule is also incompatible with the [formatter] when using
/// `format.indent-style="tab"`.
///
/// ## Options
///
/// - `lint.pydocstyle.ignore-decorators`
///
/// ## References
/// - [PEP 257 – Docstring Conventions](https://peps.python.org/pep-0257/)
/// - [NumPy Style Guide](https://numpydoc.readthedocs.io/en/latest/format.html)
/// - [Google Python Style Guide - Docstrings](https://google.github.io/styleguide/pyguide.html#38-comments-and-docstrings)
///
/// [PEP 8]: https://peps.python.org/pep-0008/#tabs-or-spaces
/// [formatter]: https://docs.astral.sh/ruff/formatter
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.75", category = Category::Formatting)]
pub(crate) struct DocstringTabIndentation;

impl Violation for DocstringTabIndentation {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Docstring should be indented with spaces, not tabs".to_string()
    }
}

/// ## What it does
/// Checks for under-indented docstrings.
///
/// ## Why is this bad?
/// [PEP 257] recommends that docstrings be indented to the same level as their
/// opening quotes. Avoid under-indenting docstrings, for consistency.
///
/// ## Example
/// ```python
/// def sort_list(l: list[int]) -> list[int]:
///     """Return a sorted copy of the list.
///
/// Sort the list in ascending order and return a copy of the result using the bubble sort
/// algorithm.
///     """
/// ```
///
/// Use instead:
/// ```python
/// def sort_list(l: list[int]) -> list[int]:
///     """Return a sorted copy of the list.
///
///     Sort the list in ascending order and return a copy of the result using the bubble
///     sort algorithm.
///     """
/// ```
///
/// ## Formatter compatibility
/// We recommend against using this rule alongside the [formatter]. The
/// formatter enforces consistent indentation, making the rule redundant.
///
/// ## Options
///
/// - `lint.pydocstyle.ignore-decorators`
///
/// ## References
/// - [PEP 257 – Docstring Conventions](https://peps.python.org/pep-0257/)
/// - [NumPy Style Guide](https://numpydoc.readthedocs.io/en/latest/format.html)
/// - [Google Python Style Guide - Docstrings](https://google.github.io/styleguide/pyguide.html#38-comments-and-docstrings)
///
/// [PEP 257]: https://peps.python.org/pep-0257/
/// [formatter]: https://docs.astral.sh/ruff/formatter/
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.75", category = Category::Formatting)]
pub(crate) struct UnderIndentation;

impl AlwaysFixableViolation for UnderIndentation {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Docstring is under-indented".to_string()
    }

    fn fix_title(&self) -> String {
        "Increase indentation".to_string()
    }
}

/// ## What it does
/// Checks for over-indented docstrings.
///
/// ## Why is this bad?
/// [PEP 257] recommends that docstrings be indented to the same level as their
/// opening quotes. Avoid over-indenting docstrings, for consistency.
///
/// ## Example
/// ```python
/// def sort_list(l: list[int]) -> list[int]:
///     """Return a sorted copy of the list.
///
///         Sort the list in ascending order and return a copy of the result using the
///         bubble sort algorithm.
///     """
/// ```
///
/// Use instead:
/// ```python
/// def sort_list(l: list[int]) -> list[int]:
///     """Return a sorted copy of the list.
///
///     Sort the list in ascending order and return a copy of the result using the bubble
///     sort algorithm.
///     """
/// ```
///
/// ## Formatter compatibility
/// We recommend against using this rule alongside the [formatter]. The
/// formatter enforces consistent indentation, making the rule redundant.
///
/// ## Options
///
/// - `lint.pydocstyle.ignore-decorators`
///
/// ## References
/// - [PEP 257 – Docstring Conventions](https://peps.python.org/pep-0257/)
/// - [NumPy Style Guide](https://numpydoc.readthedocs.io/en/latest/format.html)
/// - [Google Python Style Guide - Docstrings](https://google.github.io/styleguide/pyguide.html#38-comments-and-docstrings)
///
/// [PEP 257]: https://peps.python.org/pep-0257/
/// [formatter]:https://docs.astral.sh/ruff/formatter/
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.75", category = Category::Formatting)]
pub(crate) struct OverIndentation;

impl AlwaysFixableViolation for OverIndentation {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Docstring is over-indented".to_string()
    }

    fn fix_title(&self) -> String {
        "Remove over-indentation".to_string()
    }
}

/// D206, D207, D208
pub(crate) fn indent(checker: &Checker, docstring: &Docstring) {
    let body = docstring.body();

    let mut lines = NewlineWithTrailingNewline::with_offset(&body, body.start()).peekable();

    let mut current: Option<Line> = lines.next();

    if lines.peek().is_none() {
        return;
    }

    let docstring_indentation = docstring.compute_indentation();
    let mut has_seen_tab = docstring_indentation.contains('\t');
    let docstring_indent_size = docstring_indentation.chars().count();

    let mut over_indented_lines = vec![];
    let mut smallest_over_indent_size = Some(usize::MAX);
    let mut last = None;

    while let Some(line) = current {
        if last.is_none()
            || last
                .as_deref()
                .is_some_and(|last: &str| last.ends_with('\\'))
        {
            last = Some(line);
            current = lines.next();
            continue;
        }

        let is_last = lines.peek().is_none();

        let is_blank = line.trim().is_empty();
        if !is_last && is_blank {
            last = Some(line);
            current = lines.next();
            continue;
        }

        let line_indent = leading_space(&line);
        let line_indent_size = line_indent.chars().count();

        has_seen_tab = has_seen_tab || line_indent.contains('\t');

        if checker.is_rule_enabled(Rule::UnderIndentation) {
            if (is_last || !is_blank) && line_indent_size < docstring_indent_size {
                let mut diagnostic =
                    checker.report_diagnostic(UnderIndentation, TextRange::empty(line.start()));
                diagnostic.set_fix(Fix::safe_edit(Edit::range_replacement(
                    clean_space(docstring_indentation),
                    TextRange::at(line.start(), line_indent.text_len()),
                )));
            }
        }

        let is_last_closing_quotes_only = is_last && is_blank;

        if !is_last_closing_quotes_only {
            smallest_over_indent_size =
                smallest_over_indent_size.and_then(|smallest_over_indent_size| {
                    let over_indent_size = line_indent_size.saturating_sub(docstring_indent_size);

                    if over_indent_size > 0 {
                        over_indented_lines.push(line.clone());
                        Some(smallest_over_indent_size.min(over_indent_size))
                    } else {
                        None
                    }
                });
        }

        last = Some(line);
        current = lines.next();
    }

    if checker.is_rule_enabled(Rule::DocstringTabIndentation) {
        if has_seen_tab {
            checker.report_diagnostic(DocstringTabIndentation, docstring.range());
        }
    }

    if checker.is_rule_enabled(Rule::OverIndentation) {
        if let Some(smallest_over_indent_size) = smallest_over_indent_size {
            for line in over_indented_lines {
                let line_indent = leading_space(&line);
                let indent = clean_space(docstring_indentation);

                let mut diagnostic =
                    checker.report_diagnostic(OverIndentation, TextRange::empty(line.start()));

                let edit = if indent.is_empty() {
                    Edit::range_deletion(TextRange::at(line.start(), line_indent.text_len()))
                } else {
                    let offset = checker
                        .locator()
                        .after(line.start())
                        .chars()
                        .take(docstring_indent_size + smallest_over_indent_size)
                        .map(TextLen::text_len)
                        .sum::<TextSize>();
                    let range = TextRange::at(line.start(), offset);
                    Edit::range_replacement(indent, range)
                };
                diagnostic.set_fix(Fix::safe_edit(edit));
            }
        }

        if let Some(last) = last {
            let line_indent = leading_space(&last);
            let line_indent_size = line_indent.chars().count();
            let last_line_over_indent = line_indent_size.saturating_sub(docstring_indent_size);

            let is_indent_only = line_indent.len() == last.len();
            if last_line_over_indent > 0 && is_indent_only {
                let mut diagnostic =
                    checker.report_diagnostic(OverIndentation, TextRange::empty(last.start()));
                let indent = clean_space(docstring_indentation);
                let range = TextRange::at(last.start(), line_indent.text_len());
                let edit = if indent.is_empty() {
                    Edit::range_deletion(range)
                } else {
                    Edit::range_replacement(indent, range)
                };
                diagnostic.set_fix(Fix::safe_edit(edit));
            }
        }
    }
}
