use std::collections::BTreeMap;

use ruff_python_ast::token::{Token, TokenKind};
use ruff_text_size::{Ranged, TextRange, TextSize};

/// Stores the ranges of all interpolated strings in a file sorted by [`TextRange::start`].
/// There can be multiple overlapping ranges for nested interpolated strings.
///
/// Note that the ranges for all unterminated interpolated strings are not stored.
#[derive(Debug)]
pub struct InterpolatedStringRanges {
    raw: BTreeMap<TextSize, TextRange>,
}

impl InterpolatedStringRanges {
    /// Returns `true` if the given range intersects with any f-string range.
    pub fn intersects(&self, target: TextRange) -> bool {
        self.raw
            .values()
            .take_while(|range| range.start() < target.end())
            .any(|range| target.intersect(*range).is_some())
    }

    /// Return the [`TextRange`] of the innermost f-string at the given offset.
    pub fn innermost(&self, offset: TextSize) -> Option<TextRange> {
        self.raw
            .range(..=offset)
            .rev()
            .find(|(_, range)| range.contains(offset))
            .map(|(_, range)| *range)
    }

    /// Return the [`TextRange`] of the outermost f-string at the given offset.
    pub fn outermost(&self, offset: TextSize) -> Option<TextRange> {
        self.raw
            .range(..=offset)
            .rev()
            .skip_while(|(_, range)| !range.contains(offset))
            .take_while(|(_, range)| range.contains(offset))
            .last()
            .map(|(_, range)| *range)
    }

    /// Returns an iterator over all interpolated string [`TextRange`] sorted by their
    /// start location.
    ///
    /// For nested interpolated strings, the outermost interpolated string is yielded first, moving
    /// inwards with each iteration.
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &TextRange> + '_ {
        self.raw.values()
    }

    /// Returns the number of interpolated string ranges stored.
    #[inline]
    pub fn len(&self) -> usize {
        self.raw.len()
    }
}

#[derive(Default)]
pub(crate) struct InterpolatedStringRangesBuilder {
    start_locations: Vec<TextSize>,
    raw: BTreeMap<TextSize, TextRange>,
}

impl InterpolatedStringRangesBuilder {
    pub(crate) fn visit_token(&mut self, token: &Token) {
        match token.kind() {
            TokenKind::FStringStart | TokenKind::TStringStart => {
                self.start_locations.push(token.start());
            }
            TokenKind::FStringEnd | TokenKind::TStringEnd => {
                if let Some(start) = self.start_locations.pop() {
                    self.raw.insert(start, TextRange::new(start, token.end()));
                }
            }
            _ => {}
        }
    }

    pub(crate) fn finish(self) -> InterpolatedStringRanges {
        InterpolatedStringRanges { raw: self.raw }
    }
}
