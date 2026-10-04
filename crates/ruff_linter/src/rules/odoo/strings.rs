//! The string constants of an expression as Python's `ast` reports them.
//!
//! Since Python 3.12, an implicit concatenation that holds an f-string is one
//! `JoinedStr`, and each run of literal text between two interpolations is one
//! `Constant`, merged across the parts: `"IN %s" f" {x}"` holds the constant
//! `"IN %s "`. A run starts where its first piece does: a plain string at its
//! prefix or quote, an f-string literal piece at its first character.
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, Expr, FStringPartRef, InterpolatedStringElement};
use ruff_text_size::{Ranged, TextRange};

pub(crate) struct StringConstant {
    pub(crate) range: TextRange,
    pub(crate) value: String,
}

#[derive(Default)]
struct Constants {
    found: Vec<StringConstant>,
    run: Option<StringConstant>,
}

impl Constants {
    fn piece(&mut self, range: TextRange, value: &str) {
        match &mut self.run {
            Some(run) => {
                run.range = run.range.cover(range);
                run.value.push_str(value);
            }
            None => {
                self.run = Some(StringConstant {
                    range,
                    value: value.to_string(),
                });
            }
        }
    }

    fn flush(&mut self) {
        if let Some(run) = self.run.take() {
            self.found.push(run);
        }
    }

    fn elements(&mut self, elements: &[InterpolatedStringElement]) {
        for element in elements {
            match element {
                InterpolatedStringElement::Literal(literal) => {
                    self.piece(literal.range(), &literal.value);
                }
                InterpolatedStringElement::Interpolation(interpolation) => {
                    self.flush();
                    self.visit_expr(&interpolation.expression);
                    if let Some(spec) = &interpolation.format_spec {
                        self.elements(&spec.elements);
                        self.flush();
                    }
                }
            }
        }
    }
}

impl<'a> Visitor<'a> for Constants {
    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::StringLiteral(string) => self.found.push(StringConstant {
                range: string.range(),
                value: string.value.to_str().to_string(),
            }),
            Expr::FString(ast::ExprFString { value, .. }) => {
                // An interpolation's own constants are found before the run it
                // interrupts is complete; the order does not matter here.
                let outer = self.run.take();
                for part in value {
                    match part {
                        FStringPartRef::Literal(literal) => {
                            self.piece(literal.range(), literal.value.as_ref());
                        }
                        FStringPartRef::FString(fstring) => self.elements(&fstring.elements),
                    }
                }
                self.flush();
                self.run = outer;
            }
            Expr::TString(ast::ExprTString { value, .. }) => {
                let outer = self.run.take();
                for tstring in value {
                    self.elements(&tstring.elements);
                }
                self.flush();
                self.run = outer;
            }
            _ => visitor::walk_expr(self, expr),
        }
    }
}

/// Every string constant `ast.walk` meets under the expression, itself included.
pub(crate) fn string_constants(expr: &Expr) -> Vec<StringConstant> {
    let mut constants = Constants::default();
    constants.visit_expr(expr);
    constants.flush();
    constants.found
}
