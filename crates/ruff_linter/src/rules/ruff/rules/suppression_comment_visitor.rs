use std::iter::Peekable;

use ruff_python_ast::{
    AnyNodeRef, Suite,
    helpers::comment_indentation_after,
    visitor::source_order::{self, SourceOrderVisitor, TraversalSignal},
};
use ruff_python_trivia::{
    CommentLinePosition, SimpleTokenizer, SuppressionKind, indentation_at_offset,
};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::Locator;

#[derive(Clone, Copy, Debug)]
pub(super) struct SuppressionComment {
    pub(super) range: TextRange,
    pub(super) kind: SuppressionKind,
}

/// Visitor that captures AST data for suppression comments. This uses a similar approach
/// to `CommentsVisitor` in the formatter crate.
pub(super) struct SuppressionCommentVisitor<
    'src,
    'builder,
    I: Iterator<Item = SuppressionComment> + 'src,
> {
    comments: Peekable<I>,

    parents: Vec<AnyNodeRef<'src>>,
    preceding_node: Option<AnyNodeRef<'src>>,

    builder: &'builder mut (dyn CaptureSuppressionComment<'src> + 'src),
    locator: &'src Locator<'src>,
}

impl<'src, 'builder, I> SuppressionCommentVisitor<'src, 'builder, I>
where
    I: Iterator<Item = SuppressionComment> + 'src,
{
    pub(super) fn new(
        comment_ranges: I,
        builder: &'builder mut (dyn CaptureSuppressionComment<'src> + 'src),
        locator: &'src Locator<'src>,
    ) -> Self {
        Self {
            comments: comment_ranges.peekable(),
            parents: Vec::default(),
            preceding_node: Option::default(),
            builder,
            locator,
        }
    }

    pub(super) fn visit(mut self, suite: &'src Suite) {
        self.visit_body(suite.as_slice());
    }

    fn can_skip(&mut self, node_end: TextSize) -> bool {
        self.comments
            .peek()
            .is_none_or(|next| next.range.start() >= node_end)
    }
}

impl<'ast, I> SourceOrderVisitor<'ast> for SuppressionCommentVisitor<'ast, '_, I>
where
    I: Iterator<Item = SuppressionComment> + 'ast,
{
    fn enter_node(&mut self, node: AnyNodeRef<'ast>) -> TraversalSignal {
        let node_range = node.range();

        let enclosing_node = self.parents.last().copied();

        while let Some(SuppressionComment { range, kind }) = self.comments.peek().copied() {
            if range.end() > node_range.start() {
                break;
            }

            let line_position = CommentLinePosition::for_range(range, self.locator.contents());

            let data = SuppressionCommentData {
                enclosing: enclosing_node,
                preceding: self.preceding_node,
                following: Some(node),
                line_position,
                kind,
                range,
            };

            self.builder.capture(data);
            self.comments.next();
        }

        self.preceding_node = None;
        self.parents.push(node);

        if self.can_skip(node_range.end()) {
            TraversalSignal::Skip
        } else {
            TraversalSignal::Traverse
        }
    }

    fn leave_node(&mut self, node: AnyNodeRef<'ast>) {
        self.parents.pop();

        let node_end = node.end();

        while let Some(SuppressionComment { range, kind }) = self.comments.peek().copied() {
            let line_position = CommentLinePosition::for_range(range, self.locator.contents());
            if range.start() >= node_end {
                let between = TextRange::new(node_end, range.start());
                if SimpleTokenizer::new(self.locator.contents(), between)
                    .skip_trivia()
                    .next()
                    .is_some()
                {
                    break;
                }
                //     # fmt: off
                //     # fmt: on
                if line_position.is_own_line() {
                    let comment_indent =
                        comment_indentation_after(node, range, self.locator.contents());
                    let node_indent = TextSize::of(
                        indentation_at_offset(node.start(), self.locator.contents())
                            .unwrap_or_default(),
                    );
                    if node_indent >= comment_indent {
                        break;
                    }
                }
            }

            let data = SuppressionCommentData {
                enclosing: Some(node),
                preceding: self.preceding_node,
                following: None,
                line_position,
                kind,
                range,
            };

            self.builder.capture(data);
            self.comments.next();
        }

        self.preceding_node = Some(node);
    }
    fn visit_body(&mut self, body: &'ast [ruff_python_ast::Stmt]) {
        match body {
            [] => {
            }
            [only] => {
                self.visit_stmt(only);
            }
            [first, .., last] => {
                if self.can_skip(last.end()) {
                    self.visit_stmt(first);
                    self.preceding_node = Some(last.into());
                } else {
                    source_order::walk_body(self, body);
                }
            }
        }
    }

    fn visit_identifier(&mut self, _identifier: &'ast ruff_python_ast::Identifier) {
    }
}

#[derive(Clone, Debug)]
pub(super) struct SuppressionCommentData<'src> {
    /// The AST node that encloses the comment. If `enclosing` is `None`, this comment is a top-level statement.
    pub(super) enclosing: Option<AnyNodeRef<'src>>,
    /// An AST node that comes directly before the comment. A child of `enclosing`.
    pub(super) preceding: Option<AnyNodeRef<'src>>,
    /// An AST node that comes directly after the comment. A child of `enclosing`.
    pub(super) following: Option<AnyNodeRef<'src>>,

    /// The line position of the comment - it can either be on its own line, or at the end of a line.
    pub(super) line_position: CommentLinePosition,
    /// Whether this comment is `fmt: off`, `fmt: on`, or `fmt: skip` (or `yapf disable` / `yapf enable`)
    pub(super) kind: SuppressionKind,
    /// The range of text that makes up the comment. This includes the `#` prefix.
    pub(super) range: TextRange,
}

pub(super) trait CaptureSuppressionComment<'src> {
    /// This is the entrypoint for the capturer to analyze the next comment.
    fn capture(&mut self, comment: SuppressionCommentData<'src>);
}
