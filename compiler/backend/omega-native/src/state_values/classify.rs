use super::StateValueKind;
use omega_typed_program::expression::Expression;

pub(super) fn value_kind(expression: &Expression) -> StateValueKind {
    match expression {
        Expression::ArrayLiteral(_) => StateValueKind::Array,
        Expression::Binary(_) => StateValueKind::Binary,
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => StateValueKind::Literal,
        Expression::Indexed(_) | Expression::Name(_) => StateValueKind::Place,
        Expression::Mutable(_) => StateValueKind::MutablePlace,
        Expression::StructLiteral(_) => StateValueKind::Struct,
    }
}
