use crate::{BoolOp, Expr, ExprRef, Operator, UnaryOp};

/// Represents the precedence levels for Python expressions.
/// Variants at the top have lower precedence and variants at the bottom have
/// higher precedence.
///
/// See: <https://docs.python.org/3/reference/expressions.html#operator-precedence>
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperatorPrecedence {
    /// The lowest (virtual) precedence level
    None,
    /// Precedence of `yield` and `yield from` expressions.
    Yield,
    /// Precedence of assignment expressions (`name := expr`).
    Assign,
    /// Precedence of starred expressions (`*expr`).
    Starred,
    /// Precedence of lambda expressions (`lambda args: expr`).
    Lambda,
    /// Precedence of if/else expressions (`expr if cond else expr`).
    IfElse,
    /// Precedence of boolean `or` expressions.
    Or,
    /// Precedence of boolean `and` expressions.
    And,
    /// Precedence of boolean `not` expressions.
    Not,
    /// Precedence of comparisons (`<`, `<=`, `>`, `>=`, `!=`, `==`),
    /// memberships (`in`, `not in`) and identity tests (`is`, `is not`).
    ComparisonsMembershipIdentity,
    /// Precedence of bitwise `|` operator.
    BitOr,
    /// Precedence of bitwise `^` operator.
    BitXor,
    /// Precedence of bitwise `&` operator.
    BitAnd,
    /// Precedence of left and right shift expressions (`<<`, `>>`).
    LeftRightShift,
    /// Precedence of addition and subtraction expressions (`+`, `-`).
    AddSub,
    /// Precedence of multiplication (`*`), matrix multiplication (`@`), division (`/`),
    /// floor division (`//`) and remainder (`%`) expressions.
    MulDivRemain,
    /// Precedence of unary positive (`+`), negative (`-`), and bitwise NOT (`~`) expressions.
    PosNegBitNot,
    /// Precedence of exponentiation expressions (`**`).
    Exponent,
    /// Precedence of `await` expressions.
    Await,
    /// Precedence of call expressions (`()`), attribute access (`.`), and subscript (`[]`) expressions.
    CallAttribute,
    /// Precedence of atomic expressions (literals, names, containers).
    Atomic,
}

impl OperatorPrecedence {
    fn from_expr_ref(expr: ExprRef) -> Self {
        match expr {
            ExprRef::Tuple(_)
            | ExprRef::Dict(_)
            | ExprRef::Set(_)
            | ExprRef::ListComp(_)
            | ExprRef::List(_)
            | ExprRef::SetComp(_)
            | ExprRef::DictComp(_)
            | ExprRef::Generator(_)
            | ExprRef::Name(_)
            | ExprRef::StringLiteral(_)
            | ExprRef::BytesLiteral(_)
            | ExprRef::NumberLiteral(_)
            | ExprRef::BooleanLiteral(_)
            | ExprRef::NoneLiteral(_)
            | ExprRef::EllipsisLiteral(_)
            | ExprRef::FString(_)
            | ExprRef::TString(_) => Self::Atomic,
            ExprRef::Attribute(_)
            | ExprRef::Subscript(_)
            | ExprRef::Call(_)
            | ExprRef::Slice(_) => Self::CallAttribute,

            ExprRef::Await(_) => Self::Await,


            ExprRef::UnaryOp(operator) => match operator.op {
                UnaryOp::UAdd | UnaryOp::USub | UnaryOp::Invert => Self::PosNegBitNot,
                UnaryOp::Not => Self::Not,
            },

            ExprRef::BinOp(binary_operation) => Self::from(binary_operation.op),

            ExprRef::Compare(_) => Self::ComparisonsMembershipIdentity,


            ExprRef::BoolOp(bool_op) => Self::from(bool_op.op),

            ExprRef::If(_) => Self::IfElse,

            ExprRef::Lambda(_) => Self::Lambda,

            ExprRef::Starred(_) => Self::Starred,

            ExprRef::Named(_) => Self::Assign,

            ExprRef::Yield(_) | ExprRef::YieldFrom(_) => Self::Yield,

            ExprRef::IpyEscapeCommand(_) => Self::None,
        }
    }

    pub fn from_expr(expr: &Expr) -> Self {
        Self::from(ExprRef::from(expr))
    }

    /// Returns `true` if the precedence is right-associative i.e., the operations are evaluated
    /// from right to left.
    pub fn is_right_associative(self) -> bool {
        matches!(self, OperatorPrecedence::Exponent)
    }
}

impl From<&Expr> for OperatorPrecedence {
    fn from(expr: &Expr) -> Self {
        Self::from_expr(expr)
    }
}

impl<'a> From<ExprRef<'a>> for OperatorPrecedence {
    fn from(expr_ref: ExprRef<'a>) -> Self {
        Self::from_expr_ref(expr_ref)
    }
}

impl From<Operator> for OperatorPrecedence {
    fn from(operator: Operator) -> Self {
        match operator {
            Operator::Mult
            | Operator::MatMult
            | Operator::Div
            | Operator::Mod
            | Operator::FloorDiv => Self::MulDivRemain,
            Operator::Add | Operator::Sub => Self::AddSub,
            Operator::LShift | Operator::RShift => Self::LeftRightShift,
            Operator::BitAnd => Self::BitAnd,
            Operator::BitXor => Self::BitXor,
            Operator::BitOr => Self::BitOr,
            Operator::Pow => Self::Exponent,
        }
    }
}

impl From<BoolOp> for OperatorPrecedence {
    fn from(operator: BoolOp) -> Self {
        match operator {
            BoolOp::And => Self::And,
            BoolOp::Or => Self::Or,
        }
    }
}

impl From<UnaryOp> for OperatorPrecedence {
    fn from(unary_op: UnaryOp) -> Self {
        match unary_op {
            UnaryOp::UAdd | UnaryOp::USub | UnaryOp::Invert => Self::PosNegBitNot,
            UnaryOp::Not => Self::Not,
        }
    }
}
