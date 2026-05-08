use crate::runtime_dispatch::guards::StateGuardOperandKind;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

pub(super) fn classify_guard_operand(expression: &Expression) -> StateGuardOperandKind {
    match expression {
        Expression::Name(path) if is_static_symbol_path(path) => {
            StateGuardOperandKind::StaticSymbol
        }
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

fn is_static_symbol_path(path: &[ProgramName]) -> bool {
    path.first()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_uppercase)
}
