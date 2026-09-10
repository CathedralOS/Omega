use super::place_types::expression_type_reference_in_state;
use super::*;

pub(in crate::flow::ownership) fn expression_requires_ownership(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> OperatorResultOwnership {
    if !return_type.is_valid() {
        return OperatorResultOwnership::NoTransfer;
    }

    match program.type_reference_table.type_reference(return_type) {
        // `&[T]`, `&mut [T]`, `&string`, `&T` — a borrowed view/window.
        typed_trees::types::TypeReferenceNode::Reference { .. } => {
            OperatorResultOwnership::BorrowedView
        }
        // Unit results (`Vec::push`, `String::push_str`) transfer nothing.
        typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | typed_trees::types::TypeReferenceNode::Unit => OperatorResultOwnership::NoTransfer,
        // A constrained result classifies by its base type.
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            classify_operator_result_ownership(program, *base_type)
        }
        // Any other aggregate/owned return classifies as owned vs copy by the
        // shared ownership rule (e.g. `Vec<T>` owns, `usize`/`u8` copy).
        typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | typed_trees::types::TypeReferenceNode::Slice { .. }
        | typed_trees::types::TypeReferenceNode::Generic { .. }
        | typed_trees::types::TypeReferenceNode::Named { .. } => {
            if type_requires_ownership(program, return_type) {
                OperatorResultOwnership::OwnedValue
            } else {
                OperatorResultOwnership::NoTransfer
            }
        }
    }
}

pub(in crate::flow::ownership) fn type_requires_ownership(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    type_reference.is_valid()
        && program.type_multiplicity(type_reference)
            != language_semantics::Multiplicity::Unrestricted
}

fn expression_is_place_like(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(_) => false,
        // Forming a loan does not transfer the referent. Nested call arguments
        // retain their own moves through ordinary call discovery.
        ExpressionNode::Borrow(_) => false,
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => true,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Match(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

/// Equality observes the tags of an exact payload-free nominal sum. The shared
/// typed classifier excludes authored and selected operator meanings first.
pub(super) fn intrinsic_enum_equality(
    program: &typed_trees::TypedTrees,
    state: SymbolHandle,
    expression: ExpressionHandle,
) -> bool {
    use typed_trees::types::TypeReferenceNode;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    let spelling = match binary.operator {
        typed_trees::expression::BinaryOperator::Equal => {
            language_core::operator_spelling::OperatorSpelling::Equal
        }
        typed_trees::expression::BinaryOperator::NotEqual => {
            language_core::operator_spelling::OperatorSpelling::NotEqual
        }
        _ => return false,
    };
    let machine_symbol = program.symbols.get(state).parent;
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|candidate| candidate.symbol == state)
    else {
        return false;
    };
    if program.symbols.get(state.symbol).kind != symbols::SymbolKind::State
        || program.symbols.get(machine.symbol).kind != symbols::SymbolKind::Machine
    {
        return false;
    }
    // Equality observes values, including constructed cases; a place-only query
    // cannot establish the type of a fresh constructor operand.
    let operands = [binary.left, binary.right].map(|operand| {
        validation::expression_result_type_reference(program, machine, state, operand)
    });
    let nominal = |reference: Option<typed_trees::types::TypeReferenceHandle>| {
        let mut reference = reference?;
        let mut seen = Vec::new();
        while !seen.contains(&reference) {
            seen.push(reference);
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Reference { referee, .. } => reference = *referee,
                TypeReferenceNode::Named { symbol, .. } => return Some(*symbol),
                _ => return None,
            }
        }
        None
    };
    let Some(symbol) = nominal(operands[0]) else {
        return false;
    };
    if nominal(operands[1]) != Some(symbol) {
        return false;
    }
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)
    else {
        return false;
    };
    let members = program.data_members(data);
    if members.is_empty() || !members.iter().all(|member| matches!(member, typed_trees::data::DataMember::Variant(variant) if program.data_payload_fields(variant).is_empty())) { return false; }
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &operands,
    )
}
