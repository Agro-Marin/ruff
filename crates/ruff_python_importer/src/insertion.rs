//! Insert statements into Python code.

use std::ops::Add;

use ruff_diagnostics::Edit;
use ruff_python_ast::Stmt;
use ruff_python_ast::helpers::is_docstring_stmt;
use ruff_python_ast::token::{TokenKind, Tokens};
use ruff_python_codegen::Stylist;
use ruff_python_trivia::is_python_whitespace;
use ruff_python_trivia::{PythonWhitespace, textwrap::indent};
use ruff_source_file::{LineRanges, UniversalNewlineIterator};
use ruff_text_size::{Ranged, TextRange, TextSize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Placement<'a> {
    /// The content will be inserted inline with the existing code (i.e., within semicolon-delimited
    /// statements).
    Inline,
    /// The content will be inserted on its own line.
    OwnLine,
    /// The content will be inserted as an indented block.
    Indented(&'a str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Insertion<'a> {
    /// The content to add before the insertion.
    prefix: &'a str,
    /// The location at which to insert.
    location: TextSize,
    /// The content to add after the insertion.
    suffix: &'a str,
    /// The line placement of insertion.
    placement: Placement<'a>,
}

impl<'a> Insertion<'a> {
    /// Create an [`Insertion`] to insert (e.g.) an import statement at the start of a given
    /// file or cell, along with a prefix and suffix to use for the insertion.
    ///
    /// For example, given the following code:
    ///
    /// ```python
    /// """Hello, world!"""
    ///
    /// import os
    /// ```
    ///
    /// The insertion returned will begin at the start of the `import os` statement, and will
    /// include a trailing newline.
    ///
    /// If `within_range` is set, the insertion will be limited to the specified range. That is,
    /// the insertion is constrained to the given range rather than the start of the file.
    /// This is used for insertions in notebook cells where the source code and AST are for
    /// the entire notebook but the insertion should be constrained to a specific cell.
    pub fn start_of_file(
        body: &[Stmt],
        contents: &str,
        stylist: &Stylist,
        within_range: Option<TextRange>,
    ) -> Insertion<'static> {
        let body = within_range
            .map(|range| {
                let start = body.partition_point(|stmt| stmt.start() < range.start());
                let end = body.partition_point(|stmt| stmt.end() <= range.end());

                &body[start..end]
            })
            .unwrap_or(body);

        let mut location = if let Some(mut location) = match_docstring_end(body) {
            if let Some(offset) = match_semicolon(&contents[location.to_usize()..]) {
                return Insertion::inline(" ", location.add(offset).add(TextSize::of(';')), ";");
            }

            while match_continuation(&contents[location.to_usize()..]).is_some() {
                location = contents.full_line_end(location);
            }

            contents.full_line_end(location)
        } else if let Some(range) = within_range
            && range.start() != TextSize::ZERO
        {
            range.start()
        } else {
            contents.bom_start_offset()
        };

        let mut seen_comment = false;
        for line in
            UniversalNewlineIterator::with_offset(&contents[location.to_usize()..], location)
        {
            let trimmed_line = line.trim_whitespace_start();
            if trimmed_line.is_empty() {
                if seen_comment {
                    location = line.full_end();
                }
                continue;
            }

            if trimmed_line.starts_with('#') {
                location = line.full_end();
                seen_comment = true;
            } else {
                break;
            }
        }

        Insertion::own_line("", location, stylist.line_ending().as_str())
    }

    /// Create an [`Insertion`] to insert (e.g.) an import after the end of the given
    /// [`Stmt`], along with a prefix and suffix to use for the insertion.
    ///
    /// For example, given the following code:
    ///
    /// ```python
    /// """Hello, world!"""
    ///
    /// import os
    /// import math
    ///
    ///
    /// def foo():
    ///     pass
    /// ```
    ///
    /// The insertion returned will begin after the newline after the last import statement, which
    /// in this case is the line after `import math`, and will include a trailing newline.
    ///
    /// The statement itself is assumed to be at the top-level of the module.
    pub fn end_of_statement(stmt: &Stmt, contents: &str, stylist: &Stylist) -> Insertion<'static> {
        let location = stmt.end();
        if let Some(offset) = match_semicolon(&contents[location.to_usize()..]) {
            Insertion::inline(" ", location.add(offset).add(TextSize::of(';')), ";")
        } else if match_continuation(&contents[location.to_usize()..]).is_some() {
            Insertion::inline("; ", location, "")
        } else {
            Insertion::own_line(
                "",
                contents.full_line_end(location),
                stylist.line_ending().as_str(),
            )
        }
    }

    /// Create an [`Insertion`] to insert an additional member to import
    /// into a `from <module> import member1, member2, ...` statement.
    ///
    /// For example, given the following code:
    ///
    /// ```python
    /// """Hello, world!"""
    ///
    /// from collections import Counter
    ///
    ///
    /// def foo():
    ///     pass
    /// ```
    ///
    /// The insertion returned will begin after `Counter` but before the
    /// newline terminator. Callers can then call [`Insertion::into_edit`]
    /// with the additional member to add. A comma delimiter is handled
    /// automatically.
    ///
    /// The statement itself is assumed to be at the top-level of the module.
    ///
    /// This returns `None` when `stmt` isn't a `from ... import ...`
    /// statement.
    pub fn existing_import(stmt: &Stmt, tokens: &Tokens) -> Option<Insertion<'static>> {
        let Stmt::ImportFrom(ref import_from) = *stmt else {
            return None;
        };
        if let Some(at) = import_from.names.last().map(Ranged::end) {
            return Some(Insertion::inline(", ", at, ""));
        }
        let at = import_from.end();
        if !matches!(
            tokens
                .before(at)
                .last()
                .map(ruff_python_ast::token::Token::kind),
            Some(TokenKind::Import)
        ) {
            return None;
        }
        Some(Insertion::inline(" ", at, ""))
    }

    /// Create an [`Insertion`] to insert (e.g.) an import statement at the start of a given
    /// block, along with a prefix and suffix to use for the insertion.
    ///
    /// For example, given the following code:
    ///
    /// ```python
    /// if TYPE_CHECKING:
    ///     import os
    /// ```
    ///
    /// The insertion returned will begin at the start of the `import os` statement, and will
    /// include a trailing newline.
    ///
    /// The block itself is assumed to be at the top-level of the module.
    pub fn start_of_block(
        mut location: TextSize,
        contents: &'a str,
        stylist: &Stylist,
        tokens: &Tokens,
    ) -> Insertion<'a> {
        enum Awaiting {
            Colon(u32),
            Newline,
            Indent,
        }

        let mut state = Awaiting::Colon(0);
        for token in tokens.after(location) {
            match state {
                Awaiting::Colon(depth) => match token.kind() {
                    TokenKind::Colon if depth == 0 => {
                        state = Awaiting::Newline;
                    }
                    TokenKind::Lpar | TokenKind::Lbrace | TokenKind::Lsqb => {
                        state = Awaiting::Colon(depth.saturating_add(1));
                    }
                    TokenKind::Rpar | TokenKind::Rbrace | TokenKind::Rsqb => {
                        state = Awaiting::Colon(depth.saturating_sub(1));
                    }
                    _ => {}
                },
                Awaiting::Newline => match token.kind() {
                    TokenKind::Comment => {}
                    TokenKind::Newline => {
                        state = Awaiting::Indent;
                    }
                    _ => {
                        location = token.start();
                        break;
                    }
                },
                Awaiting::Indent => match token.kind() {
                    TokenKind::Comment => {}
                    TokenKind::NonLogicalNewline => {}
                    TokenKind::Indent => {
                        return Insertion::indented(
                            "",
                            token.start(),
                            stylist.line_ending().as_str(),
                            &contents[token.range()],
                        );
                    }
                    _ => {
                        location = token.start();
                        break;
                    }
                },
            }
        }

        Insertion::inline("", location, "; ")
    }

    /// Convert this [`Insertion`] into an [`Edit`] that inserts the given content.
    pub fn into_edit(self, content: &str) -> Edit {
        let Insertion {
            prefix,
            location,
            suffix,
            placement,
        } = self;
        let content = format!("{prefix}{content}{suffix}");
        Edit::insertion(
            match placement {
                Placement::Indented(indentation) if !indentation.is_empty() => {
                    indent(&content, indentation).to_string()
                }
                _ => content,
            },
            location,
        )
    }

    /// Returns `true` if this [`Insertion`] is inline.
    pub fn is_inline(&self) -> bool {
        matches!(self.placement, Placement::Inline)
    }

    /// Create an [`Insertion`] that inserts content inline (i.e., within semicolon-delimited
    /// statements).
    fn inline(prefix: &'a str, location: TextSize, suffix: &'a str) -> Self {
        Self {
            prefix,
            location,
            suffix,
            placement: Placement::Inline,
        }
    }

    /// Create an [`Insertion`] that starts on its own line.
    fn own_line(prefix: &'a str, location: TextSize, suffix: &'a str) -> Self {
        Self {
            prefix,
            location,
            suffix,
            placement: Placement::OwnLine,
        }
    }

    /// Create an [`Insertion`] that starts on its own line, with the given indentation.
    fn indented(
        prefix: &'a str,
        location: TextSize,
        suffix: &'a str,
        indentation: &'a str,
    ) -> Self {
        Self {
            prefix,
            location,
            suffix,
            placement: Placement::Indented(indentation),
        }
    }
}

/// Find the end of the docstring (first string statement).
fn match_docstring_end(body: &[Stmt]) -> Option<TextSize> {
    let stmt = body.first()?;
    if !is_docstring_stmt(stmt) {
        return None;
    }
    Some(stmt.end())
}

/// If the next token is a semicolon, return its offset.
fn match_semicolon(s: &str) -> Option<TextSize> {
    for (offset, c) in s.char_indices() {
        match c {
            _ if is_python_whitespace(c) => continue,
            ';' => return Some(TextSize::try_from(offset).unwrap()),
            _ => break,
        }
    }
    None
}

/// If the next token is a continuation (`\`), return its offset.
fn match_continuation(s: &str) -> Option<TextSize> {
    for (offset, c) in s.char_indices() {
        match c {
            _ if is_python_whitespace(c) => continue,
            '\\' => return Some(TextSize::try_from(offset).unwrap()),
            _ => break,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use ruff_python_codegen::Stylist;
    use ruff_python_parser::parse_module;
    use ruff_source_file::LineEnding;
    use ruff_text_size::{Ranged, TextSize};

    use super::Insertion;

    #[test]
    fn start_of_file() -> Result<()> {
        fn insert(contents: &str) -> Result<Insertion<'_>> {
            let parsed = parse_module(contents)?;
            let stylist = Stylist::from_tokens(parsed.tokens(), contents);
            Ok(Insertion::start_of_file(
                parsed.suite(),
                contents,
                &stylist,
                None,
            ))
        }

        let contents = "";
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(0), LineEnding::default().as_str())
        );

        let contents = r#"
"""Hello, world!""""#
            .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(19), LineEnding::default().as_str())
        );

        let contents = r#"
"""Hello, world!"""
"#
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(20), "\n")
        );

        let contents = r#"
"""Hello, world!"""
"""Hello, world!"""
"#
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(20), "\n")
        );

        let contents = r#"
"""Hello, world!"""\

"#
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(22), "\n")
        );

        let contents = r#"
"""Hello, world!"""\
\

"#
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(24), "\n")
        );

        let contents = r"
x = 1
"
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(0), "\n")
        );

        let contents = r"
#!/usr/bin/env python3
"
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(23), "\n")
        );

        let contents = r#"
#!/usr/bin/env python3
"""Hello, world!"""
"#
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(43), "\n")
        );

        let contents = r#"
"""Hello, world!"""
#!/usr/bin/env python3
"#
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(43), "\n")
        );

        let contents = r#"
"""%s""" % "Hello, world!"
"#
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(0), "\n")
        );

        let contents = r#"
"""Hello, world!"""; x = 1
"#
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::inline(" ", TextSize::from(20), ";")
        );

        let contents = r#"
"""Hello, world!"""; x = 1; y = \
    2
"#
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::inline(" ", TextSize::from(20), ";")
        );

        let contents = r"
# /// script
# dependencies = ['anyio']
# ///

import datetime as dt
"
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(47), "\n")
        );

        let contents = r"
# /// script
# dependencies = ['anyio']
# ///
import datetime as dt
"
        .trim_start();
        assert_eq!(
            insert(contents)?,
            Insertion::own_line("", TextSize::from(46), "\n")
        );

        Ok(())
    }

    #[test]
    fn start_of_block() {
        fn insert(contents: &str, offset: TextSize) -> Insertion<'_> {
            let parsed = parse_module(contents).unwrap();
            let stylist = Stylist::from_tokens(parsed.tokens(), contents);
            Insertion::start_of_block(offset, contents, &stylist, parsed.tokens())
        }

        let contents = "if True: pass";
        assert_eq!(
            insert(contents, TextSize::from(0)),
            Insertion::inline("", TextSize::from(9), "; ")
        );

        let contents = r"
if True:
    pass
"
        .trim_start();
        assert_eq!(
            insert(contents, TextSize::from(0)),
            Insertion::indented("", TextSize::from(9), "\n", "    ")
        );
    }

    #[test]
    fn existing_import_works() {
        fn snapshot(content: &str, member: &str) -> String {
            let parsed = parse_module(content).unwrap();
            let edit = Insertion::existing_import(parsed.suite().first().unwrap(), parsed.tokens())
                .unwrap()
                .into_edit(member);
            let insert_text = edit.content().expect("edit should be non-empty");

            let mut content = content.to_string();
            content.replace_range(edit.range().to_std_range(), insert_text);
            content
        }

        let source = r#"
from collections import Counter
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import Counter, defaultdict
        ",
        );

        let source = r#"
from collections import Counter, OrderedDict
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import Counter, OrderedDict, defaultdict
        ",
        );

        let source = r#"
from collections import (Counter)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (Counter, defaultdict)
        ",
        );

        let source = r#"
from collections import (Counter, OrderedDict)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (Counter, OrderedDict, defaultdict)
        ",
        );

        let source = r#"
from collections import (Counter,)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (Counter, defaultdict,)
        ",
        );

        let source = r#"
from collections import (Counter, OrderedDict,)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (Counter, OrderedDict, defaultdict,)
        ",
        );

        let source = r#"
from collections import (
  Counter
)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (
          Counter, defaultdict
        )
        ",
        );

        let source = r#"
from collections import (
  Counter,
)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (
          Counter, defaultdict,
        )
        ",
        );

        let source = r#"
from collections import (
  Counter,
  OrderedDict
)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (
          Counter,
          OrderedDict, defaultdict
        )
        ",
        );

        let source = r#"
from collections import (
  Counter,
  OrderedDict,
)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (
          Counter,
          OrderedDict, defaultdict,
        )
        ",
        );

        let source = r#"
from collections import \
  Counter
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @r"

        from collections import \
          Counter, defaultdict
        ",
        );

        let source = r#"
from collections import \
  Counter, OrderedDict
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @r"

        from collections import \
          Counter, OrderedDict, defaultdict
        ",
        );

        let source = r#"
from collections import \
  Counter, \
  OrderedDict
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @r"

        from collections import \
          Counter, \
          OrderedDict, defaultdict
        ",
        );


        let source = r#"
from collections import (
  Counter # comment
)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (
          Counter, defaultdict # comment
        )
        ",
        );

        let source = r#"
from collections import (
  Counter, # comment
)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (
          Counter, defaultdict, # comment
        )
        ",
        );

        let source = r#"
from collections import (
  Counter # comment
  ,
)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (
          Counter, defaultdict # comment
          ,
        )
        ",
        );

        let source = r#"
from collections import (
  Counter
  # comment
  ,
)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (
          Counter, defaultdict
          # comment
          ,
        )
        ",
        );

        let source = r#"
from collections import (
  # comment 1
  Counter # comment 2
  # comment 3
)
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import (
          # comment 1
          Counter, defaultdict # comment 2
          # comment 3
        )
        ",
        );

        let source = r#"
from collections import Counter # comment
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import Counter, defaultdict # comment
        ",
        );

        let source = r#"
from collections import Counter, OrderedDict # comment
"#;
        insta::assert_snapshot!(
            snapshot(source, "defaultdict"),
            @"

        from collections import Counter, OrderedDict, defaultdict # comment
        ",
        );
    }
}
