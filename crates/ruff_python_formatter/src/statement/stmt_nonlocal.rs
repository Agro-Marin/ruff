use ruff_formatter::{format_args, write};
use ruff_python_ast::StmtNonlocal;

use crate::prelude::*;

#[derive(Default)]
pub struct FormatStmtNonlocal;

impl FormatNodeRule<StmtNonlocal> for FormatStmtNonlocal {
    fn fmt_fields(&self, item: &StmtNonlocal, f: &mut PyFormatter) -> FormatResult<()> {
        if f.context().comments().has_trailing(item) {
            let joined = format_with(|f| {
                f.join_with(format_args![token(","), space()])
                    .entries(item.names.iter().formatted())
                    .finish()
            });

            write!(f, [token("nonlocal"), space(), &joined])
        } else {
            let joined = format_with(|f| {
                f.join_with(&format_args![
                    token(","),
                    space(),
                    if_group_breaks(&token("\\")),
                    soft_line_break(),
                ])
                .entries(item.names.iter().formatted())
                .finish()
            });

            write!(
                f,
                [
                    token("nonlocal"),
                    space(),
                    group(&format_args!(
                        if_group_breaks(&token("\\")),
                        soft_line_break(),
                        soft_block_indent(&joined)
                    ))
                ]
            )
        }
    }
}
