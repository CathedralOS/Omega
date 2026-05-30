use super::place_types::expression_type_reference_in_state;
use super::*;

pub(in crate::flow::ownership) fn expression_requires_ownership(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> bool {
    if !expression_is_place_like(program, expression) {
        return false;
    }

    expression_type_reference_in_state(program, state_symbol, statement_index, expression)
        .map(|type_reference| type_requires_ownership(program, type_reference))
        .unwrap_or(true)
}

/// The ownership disposition of a value produced by a slice/string/collection
/// operator result.
///
/// This is the seam the ownership lane uses to keep operator semantics in one
/// place and to host a future user-defined copy/drop policy: today the result
/// of a view operator borrows, the result of a conversion owns, and an in-place
/// mutation produces no transferable value. A later policy lookup (per element
/// type, per operator) plugs in here without touching the event emitters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::flow::ownership) enum OperatorResultOwnership {
    /// The result is a borrowed view into storage owned elsewhere
    /// (`as_slice`, `as_mut_slice`, `as_view`, subslice/`Str::range`, ...).
    BorrowedView,
    /// The result is freshly owned storage that must be moved/dropped
    /// (`Vec::with_capacity`, `String::with_capacity`, ...).
    OwnedValue,
    /// The result carries no transferable ownership: either unit (an in-place
    /// mutation like `Vec::push`/`String::push_str`) or a copy primitive
    /// (`Slice::Length`, `Slice::index` of a scalar, ...).
    NoTransfer,
}

/// Classify the ownership disposition of a boundary/slice/string operator's
/// *result*, given the operator's return type.
///
/// The return type is authoritative for the three built-in dispositions:
/// a reference return is a borrowed view, an owned aggregate return is owned,
/// and unit / copy-primitive returns transfer nothing. This mirrors
/// [`type_requires_ownership`] but is expressed in operator-result terms so the
/// move/borrow/drop emitters and a future copy/drop policy share one decision.
pub(in crate::flow::ownership) fn classify_operator_result_ownership(
    program: &omega_typed_trees::TypedTrees,
    return_type: omega_typed_trees::types::TypeReferenceHandle,
) -> OperatorResultOwnership {
    if !return_type.is_valid() {
        return OperatorResultOwnership::NoTransfer;
    }

    match program.type_reference_table.type_reference(return_type) {
        // `&[T]`, `&mut [T]`, `&string`, `&T` — a borrowed view/window.
        omega_typed_trees::types::TypeReferenceNode::Reference { .. } => {
            OperatorResultOwnership::BorrowedView
        }
        // Unit results (`Vec::push`, `String::push_str`) transfer nothing.
        omega_typed_trees::types::TypeReferenceNode::Unit => OperatorResultOwnership::NoTransfer,
        // A constrained result classifies by its base type.
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            classify_operator_result_ownership(program, *base_type)
        }
        // Any other aggregate/owned return classifies as owned vs copy by the
        // shared ownership rule (e.g. `Vec<T>` owns, `usize`/`u8` copy).
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Generic { .. }
        | omega_typed_trees::types::TypeReferenceNode::Named { .. } => {
            if type_requires_ownership(program, return_type) {
                OperatorResultOwnership::OwnedValue
            } else {
                OperatorResultOwnership::NoTransfer
            }
        }
    }
}

pub(in crate::flow::ownership) fn type_requires_ownership(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }

    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { .. } => false,
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_requires_ownership(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
            type_requires_ownership(program, *element_type)
        }
        omega_typed_trees::types::TypeReferenceNode::Named { name, .. } => !matches!(
            omega_typed_trees::types::PrimitiveType::from_name(name.as_str()),
            Some(
                omega_typed_trees::types::PrimitiveType::Bool
                    | omega_typed_trees::types::PrimitiveType::F32
                    | omega_typed_trees::types::PrimitiveType::F64
                    | omega_typed_trees::types::PrimitiveType::I32
                    | omega_typed_trees::types::PrimitiveType::U32
                    | omega_typed_trees::types::PrimitiveType::U64
                    | omega_typed_trees::types::PrimitiveType::Usize
            )
        ),
        omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Generic { .. } => true,
        omega_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}

fn expression_is_place_like(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => expression_is_place_like(program, *inner),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => true,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => false,
    }
}
