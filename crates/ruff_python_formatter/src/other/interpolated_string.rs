use ruff_python_ast::{AnyStringFlags, InterpolatedStringElements};
use ruff_source_file::LineRanges;
use ruff_text_size::Ranged;

#[derive(Clone, Copy, Debug)]
pub(crate) struct InterpolatedStringContext {
    /// The string flags of the enclosing f/t-string part.
    enclosing_flags: AnyStringFlags,
    layout: InterpolatedStringLayout,
}

impl InterpolatedStringContext {
    pub(crate) const fn new(flags: AnyStringFlags, layout: InterpolatedStringLayout) -> Self {
        Self {
            enclosing_flags: flags,
            layout,
        }
    }

    pub(crate) fn flags(self) -> AnyStringFlags {
        self.enclosing_flags
    }

    pub(crate) const fn is_multiline(self) -> bool {
        matches!(self.layout, InterpolatedStringLayout::Multiline)
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum InterpolatedStringLayout {
    /// Original f/t-string is flat.
    /// Don't break expressions to keep the string flat.
    Flat,
    /// Original f/t-string has multiline expressions in the replacement fields.
    /// Allow breaking expressions across multiple lines.
    Multiline,
}

impl InterpolatedStringLayout {
    pub(crate) fn from_interpolated_string_elements(
        elements: &InterpolatedStringElements,
        source: &str,
    ) -> Self {
        if elements
            .interpolations()
            .any(|expr| source.contains_line_break(expr.range()))
        {
            Self::Multiline
        } else {
            Self::Flat
        }
    }

    pub(crate) const fn is_flat(self) -> bool {
        matches!(self, InterpolatedStringLayout::Flat)
    }

    pub(crate) const fn is_multiline(self) -> bool {
        matches!(self, InterpolatedStringLayout::Multiline)
    }
}
