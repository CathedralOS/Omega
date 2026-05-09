use omega_typed_program::expression::{Expression, NamePath};

pub(crate) fn expression_place_eq(left: &Expression, right: &Expression) -> bool {
    match (left, right) {
        (Expression::Name(left), Expression::Name(right)) => name_path_eq(left, right),
        (Expression::Indexed(left), Expression::Indexed(right)) => {
            expression_place_eq(&left.collection, &right.collection) && left.index == right.index
        }
        (Expression::Mutable(left), right) => expression_place_eq(left, right),
        (left, Expression::Mutable(right)) => expression_place_eq(left, right),
        _ => left == right,
    }
}

fn name_path_eq(left: &NamePath, right: &NamePath) -> bool {
    if left.len() != right.len() {
        return false;
    }

    if left.head_symbol().is_valid() && right.head_symbol().is_valid() {
        return left.head_symbol() == right.head_symbol()
            && left
                .iter()
                .skip(1)
                .zip(right.iter().skip(1))
                .all(|(left, right)| left == right);
    }

    left.iter()
        .zip(right.iter())
        .all(|(left, right)| left == right)
}
