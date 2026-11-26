use ruff_db::parsed::parsed_module;
use ruff_db::source::source_text;
use ruff_diagnostics::{Edit, Fix};
use ruff_python_ast::token::Tokens;
use ruff_python_trivia::indentation_at_offset;
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextLen, TextRange, TextSize};
use std::fmt::Write as _;

use crate::lint::LintId;
use crate::suppression::{
    CheckSuppressionsContext, Suppression, SuppressionKind, SuppressionTarget,
    UNUSED_IGNORE_COMMENT, UNUSED_TYPE_IGNORE_COMMENT, select_preferred_suppression,
};

/// Checks for unused suppression comments in `file` and
/// adds diagnostic for each of them to `diagnostics`.
///
/// Does nothing if the [`UNUSED_IGNORE_COMMENT`] rule is disabled.
pub(super) fn check_unused_suppressions(context: &mut CheckSuppressionsContext) {
    if context.is_lint_disabled(&UNUSED_IGNORE_COMMENT)
        && context.is_lint_disabled(&UNUSED_TYPE_IGNORE_COMMENT)
    {
        return;
    }

    let diagnostics = context.diagnostics.get_mut();

    let all = context.suppressions;
    let mut unused = Vec::with_capacity(
        all.file
            .len()
            .saturating_add(all.inline.len())
            .saturating_sub(diagnostics.used_len()),
    );

    for suppression in all.iter() {
        if diagnostics.is_used(suppression.id()) {
            continue;
        }

        if let Some(unused_suppression) = select_preferred_suppression(
            all.lint_suppressions(suppression.range, LintId::of(&UNUSED_IGNORE_COMMENT))
                .filter(|candidate| {
                    candidate.target.is_lint() && candidate.id() != suppression.id()
                }),
            suppression.range,
        ) {
            diagnostics.mark_used(unused_suppression.id());
            continue;
        }

        unused.push(suppression);
    }

    let mut unused_iter = unused
        .iter()
        .filter(|suppression| {
            // a = 10 / 2  # ty: ignore[unused-ignore-comment, division-by-zero]
            !context.is_suppression_used(suppression.id())
        })
        .peekable();

    if unused_iter.peek().is_none() {
        return;
    }

    let source = source_text(context.db, context.file());
    let parsed = parsed_module(context.db, context.python_file).load(context.db);

    while let Some(suppression) = unused_iter.next() {
        let unused_lint = match suppression.kind {
            SuppressionKind::Ty => &UNUSED_IGNORE_COMMENT,
            SuppressionKind::TypeIgnore => &UNUSED_TYPE_IGNORE_COMMENT,
        };

        let mut diag = match suppression.target {
            SuppressionTarget::All => {
                let Some(diag) = context.report_unchecked(unused_lint, suppression.range) else {
                    continue;
                };

                diag.into_diagnostic(format_args!(
                    "Unused blanket `{}` directive",
                    suppression.kind
                ))
            }
            SuppressionTarget::Lint(_) => {

                let includes_first_code = source[..suppression.range.start().to_usize()]
                    .trim_end()
                    .ends_with('[');

                let mut current = suppression;
                let mut unused_codes = Vec::new();

                while let Some(next) = unused_iter.peek() {
                    if next.target.is_lint()
                        && next.comment_range == current.comment_range
                        && source[TextRange::new(current.range.end(), next.range.start())]
                            .chars()
                            .all(|c| c.is_whitespace() || c == ',')
                    {
                        unused_codes.push(&source[next.range]);
                        current = *next;
                        unused_iter.next();
                    } else {
                        break;
                    }
                }

                let includes_last_code = source[current.range.end().to_usize()..]
                    .trim_start()
                    .starts_with(']');

                if !includes_first_code || !includes_last_code {
                    let mut codes = format!("'{}'", &source[suppression.range]);
                    for code in &unused_codes {
                        let _ = write!(&mut codes, ", '{code}'");
                    }

                    if let Some(diag) = context.report_unchecked(
                        unused_lint,
                        TextRange::new(suppression.range.start(), current.range.end()),
                    ) {
                        let mut diag = diag.into_diagnostic(format_args!(
                            "Unused `{kind}` directive: {codes}",
                            kind = suppression.kind
                        ));

                        diag.primary_annotation_mut()
                            .unwrap()
                            .push_tag(ruff_db::diagnostic::DiagnosticTag::Unnecessary);

                        let trailing_len: TextSize = source[current.range.end().to_usize()..]
                            .chars()
                            .take_while(|c: &char| c.is_whitespace() || *c == ',')
                            .map(TextLen::text_len)
                            .sum();

                        let leading_len: TextSize = if includes_last_code {
                            source[..suppression.range.start().to_usize()]
                                .chars()
                                .rev()
                                .take_while(|c: &char| c.is_whitespace() || *c == ',')
                                .map(TextLen::text_len)
                                .sum()
                        } else {
                            TextSize::default()
                        };

                        let fix_range = TextRange::new(
                            suppression.range.start() - leading_len,
                            current.range.end() + trailing_len,
                        );
                        diag.set_fix(Fix::safe_edit(Edit::range_deletion(fix_range)));

                        if unused_codes.is_empty() {
                            diag.help("Remove the unused suppression code");
                        } else {
                            diag.help("Remove the unused suppression codes");
                        }
                    }

                    continue;
                }

                let Some(diag) = context.report_unchecked(unused_lint, suppression.comment_range)
                else {
                    continue;
                };

                diag.into_diagnostic(format_args!(
                    "Unused `{kind}` directive",
                    kind = suppression.kind
                ))
            }
            SuppressionTarget::Empty => {
                let Some(diag) = context.report_unchecked(unused_lint, suppression.range) else {
                    continue;
                };
                diag.into_diagnostic(format_args!(
                    "Unused `{kind}` without a code",
                    kind = suppression.kind
                ))
            }
        };

        diag.primary_annotation_mut()
            .unwrap()
            .push_tag(ruff_db::diagnostic::DiagnosticTag::Unnecessary);
        diag.set_fix(remove_comment_fix(suppression, &source, parsed.tokens()));
        diag.help("Remove the unused suppression comment");
    }
}

fn remove_comment_fix(suppression: &Suppression, source: &str, tokens: &Tokens) -> Fix {
    let comment_end = suppression.comment_range.end();
    let comment_start = suppression.comment_range.start();
    let after_comment = &source[comment_end.to_usize()..];

    if !after_comment.starts_with(['\n', '\r']) && !after_comment.is_empty() {
        // For example: `# ty: ignore # fmt: off`
        let edit = Edit::range_deletion(suppression.comment_range);

        if indentation_at_offset(comment_start, source).is_some() {
            // Removing `# ty: ignore` from `# ty: ignore # fmt: off` would promote
            // `# fmt: off` to the primary own-line comment.
            return Fix::unsafe_edit(edit);
        }

        return Fix::safe_edit(edit);
    }

    if indentation_at_offset(comment_start, source).is_some() {
        let line = source.full_line_range(comment_start);
        let previous_end = tokens
            .before(comment_start)
            .last()
            .map_or_else(|| source.bom_start_offset(), Ranged::end);

        //         # ty: ignore[invalid-assignment]
        if previous_end < line.start() {
            return Fix::safe_edit(Edit::deletion(previous_end, comment_end));
        }

        return Fix::safe_edit(Edit::range_deletion(line));
    }

    let before_comment = &source[..comment_start.to_usize()];

    let mut leading_len = TextSize::default();

    for c in before_comment.chars().rev() {
        match c {
            '\n' | '\r' => break,
            c if c.is_whitespace() => leading_len += c.text_len(),
            _ => break,
        }
    }

    Fix::safe_edit(Edit::range_deletion(TextRange::new(
        comment_start - leading_len,
        comment_end,
    )))
}
