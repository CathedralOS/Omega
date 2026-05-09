use crate::StateGuardOperandKind;
use omega_typed_program::expression::Expression;

pub(super) fn classify_guard_operand(
    expression: &Expression,
    has_resolved_value: bool,
) -> StateGuardOperandKind {
    match expression {
        Expression::Name(_) if has_resolved_value => StateGuardOperandKind::StaticSymbol,
        Expression::Name(_) | Expression::Indexed(_) => StateGuardOperandKind::Place,
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => StateGuardOperandKind::Literal,
        Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Mutable(_)
        | Expression::StructLiteral(_) => StateGuardOperandKind::OtherExpression,
    }
}
