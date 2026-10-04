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

/// The run of literal text being merged into one constant.
#[derive(Default)]
struct Run(Option<StringConstant>);

impl Run {
    fn piece(&mut self, range: TextRange, text: &str) {
        match &mut self.0 {
            Some(run) => {
                run.range = run.range.cover(range);
                run.value.push_str(text);
            }
            None => {
                self.0 = Some(StringConstant {
                    range,
                    value: text.to_string(),
                });
            }
        }
    }

    fn take(&mut self) -> Option<StringConstant> {
        self.0.take()
    }
}

/// One value of a `JoinedStr`: a run of literal text, or an interpolation.
pub(crate) enum JoinedValue<'a> {
    Constant(StringConstant),
    Interpolation(&'a ast::InterpolatedElement),
}

fn push_elements<'a>(
    elements: &'a [InterpolatedStringElement],
    run: &mut Run,
    values: &mut Vec<JoinedValue<'a>>,
) {
    for element in elements {
        match element {
            InterpolatedStringElement::Literal(literal) => {
                run.piece(literal.range(), &literal.value);
            }
            InterpolatedStringElement::Interpolation(interpolation) => {
                values.extend(run.take().map(JoinedValue::Constant));
                values.push(JoinedValue::Interpolation(interpolation));
            }
        }
    }
}

/// The values Python's `JoinedStr` holds for an f-string, its implicit
/// concatenation included.
pub(crate) fn joined_values(value: &ast::FStringValue) -> Vec<JoinedValue<'_>> {
    let mut values = Vec::new();
    let mut run = Run::default();
    for part in value {
        match part {
            FStringPartRef::Literal(literal) => run.piece(literal.range(), literal.value.as_ref()),
            FStringPartRef::FString(fstring) => {
                push_elements(&fstring.elements, &mut run, &mut values);
            }
        }
    }
    values.extend(run.take().map(JoinedValue::Constant));
    values
}

/// The values Python's `TemplateStr` holds for a t-string.
fn template_values(value: &ast::TStringValue) -> Vec<JoinedValue<'_>> {
    let mut values = Vec::new();
    let mut run = Run::default();
    for tstring in value {
        push_elements(&tstring.elements, &mut run, &mut values);
    }
    values.extend(run.take().map(JoinedValue::Constant));
    values
}

#[derive(Default)]
struct Constants {
    found: Vec<StringConstant>,
}

impl Constants {
    fn joined(&mut self, values: Vec<JoinedValue<'_>>) {
        for value in values {
            match value {
                JoinedValue::Constant(constant) => self.found.push(constant),
                JoinedValue::Interpolation(interpolation) => {
                    self.visit_expr(&interpolation.expression);
                    if let Some(spec) = &interpolation.format_spec {
                        let mut run = Run::default();
                        let mut spec_values = Vec::new();
                        push_elements(&spec.elements, &mut run, &mut spec_values);
                        spec_values.extend(run.take().map(JoinedValue::Constant));
                        self.joined(spec_values);
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
            Expr::FString(fstring) => self.joined(joined_values(&fstring.value)),
            Expr::TString(tstring) => self.joined(template_values(&tstring.value)),
            _ => visitor::walk_expr(self, expr),
        }
    }
}

/// Every string constant `ast.walk` meets under the expression, itself included.
pub(crate) fn string_constants(expr: &Expr) -> Vec<StringConstant> {
    let mut constants = Constants::default();
    constants.visit_expr(expr);
    constants.found
}
