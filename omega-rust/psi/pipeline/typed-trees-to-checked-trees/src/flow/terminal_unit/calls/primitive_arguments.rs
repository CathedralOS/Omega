//! Whole primitive referents shared by Unit calls and scalar computations.

use super::*;

pub(crate) fn primitive_computation_argument(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    target: &StateParameter,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if target.is_self || target.is_const {
        return None;
    }
    let target_type = plain_primitive_referent(program, target.type_reference)?;
    let target_access = structural_access_for_type_reference(program, target.type_reference)?;
    let named = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(argument) => {
            let TypeReferenceNode::Reference { access, .. } = program
                .type_reference_table
                .type_reference(target.type_reference)
            else {
                return None;
            };
            if argument.access != *access {
                return None;
            }
            argument.target
        }
        ExpressionNode::Name(_) => expression,
        _ => return None,
    };
    let ExpressionNode::Name(name) = program.expression_table.expression(named) else {
        return None;
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        call.statement_index,
        expression,
    )?;
    if place.root != facts::PlaceRoot::Symbol(name.symbol) || !place.segments.is_empty() {
        return None;
    }
    rejoin_computation_accesses(program, borrow, machine, state.symbol, call)?;
    if super::super::primitive_store::primitive_local_before(
        program,
        state,
        call.statement_index,
        name.symbol,
    )
    .is_some()
    {
        return primitive_local_argument(
            program,
            borrow,
            machine,
            state,
            call,
            &place,
            target.type_reference,
        );
    }
    let parameters = program.state_parameters(state);
    let source_position = parameters
        .iter()
        .position(|source| source.symbol == name.symbol)?;
    let source = &parameters[source_position];
    if source.is_self
        || source.is_const
        || plain_primitive_referent(program, source.type_reference)? != target_type
    {
        return None;
    }
    let source_access = structural_access_for_type_reference(program, source.type_reference)?;
    if !matches!(
        (source_access, target_access),
        (
            CheckedStructuralAccess::SharedBorrow,
            CheckedStructuralAccess::SharedBorrow
        ) | (
            CheckedStructuralAccess::MutableBorrow,
            CheckedStructuralAccess::SharedBorrow
                | CheckedStructuralAccess::MutableBorrow
                | CheckedStructuralAccess::WriteOnlyBorrow
        ) | (
            CheckedStructuralAccess::WriteOnlyBorrow,
            CheckedStructuralAccess::WriteOnlyBorrow
        )
    ) {
        return None;
    }
    let access = exact_structural_borrow_access(
        program,
        borrow,
        machine,
        state.symbol,
        call,
        &place,
        target_access,
    )?;
    if access != target_access {
        return None;
    }
    let parameter_index = parameters[..source_position]
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_none()
        })
        .count();
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(parameter_index).ok()?,
        },
        path: Vec::new(),
        type_identity: base_type_identity(program, target.type_reference, &[])?,
        access,
    })
}

/// Borrow rows are an ordered observation roster, not a set of roots. Rebuild
/// with the existing collector so scalar reads occupy their actual positions.
fn rejoin_computation_accesses(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    source: &checked_trees::FlowCallFact,
) -> Option<()> {
    let mut states = borrow
        .states
        .iter()
        .map(|(_, row)| row)
        .filter(|row| row.machine_symbol == machine && row.state_symbol == state);
    let state_row = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let mut calls = borrow.calls.span(state_row.calls)?.iter().filter(|call| {
        call.statement_index == source.statement_index && call.call_ordinal == source.call_ordinal
    });
    let call = calls.next()?;
    if calls.next().is_some()
        || call.target_symbol != source.target_symbol
        || call.accesses != source.accesses
        || call.has_receiver != source.has_receiver
        || call.receiver_symbol != source.receiver_symbol
        || !program
            .expression_table
            .expression_is_valid(source.authored_expression)
    {
        return None;
    }
    let ExpressionNode::Call(authored) = program
        .expression_table
        .expression(source.authored_expression)
    else {
        return None;
    };
    let arguments = program
        .expression_table
        .expression_handles(authored.arguments);
    if arguments.len() != authored.arguments.count() as usize {
        return None;
    }
    let mut expected_segments = arena::Arena::new();
    let mut expected_accesses = arena::Arena::new();
    let expected = crate::borrow::accesses::collect_call_argument_accesses(
        program,
        &mut expected_segments,
        &mut expected_accesses,
        arguments,
        state,
        source.statement_index,
        machine,
    );
    let expected = expected_accesses.span(expected)?;
    let actual = borrow.argument_accesses.span(call.accesses)?;
    if expected.len() != actual.len() {
        return None;
    }
    for (expected, actual) in expected.iter().zip(actual) {
        if expected.root_symbol != actual.root_symbol
            || expected.kind != actual.kind
            || expected_segments.span(expected.segments)?
                != borrow.access_segments.span(actual.segments)?
        {
            return None;
        }
        if actual.kind.is_exclusive()
            && borrow
                .argument_accesses
                .span(call.accesses)?
                .iter()
                .filter(|candidate| candidate.root_symbol == actual.root_symbol)
                .count()
                != 1
        {
            return None;
        }
    }
    Some(())
}

fn plain_primitive_referent(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if !matches!(
        program.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Named { .. }
    ) {
        return None;
    }
    program.primitive_type_reference(*referee)
}

pub(super) fn primitive_local_argument(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    place: &crate::flow::CanonicalPlace,
    target_type: TypeReferenceHandle,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let local = super::super::primitive_store::primitive_local_before(
        program,
        state,
        call.statement_index,
        symbol,
    )?;
    let target_access = structural_access_for_type_reference(program, target_type)?;
    let access = exact_structural_borrow_access(
        program,
        borrow,
        machine,
        state.symbol,
        call,
        place,
        target_access,
    )?;
    let type_identity = base_type_identity(program, target_type, &[])?;
    if !place.segments.is_empty()
        || target_access == CheckedStructuralAccess::Owned
        || access != target_access
        || base_type_identity(program, local.type_reference, &[])? != type_identity
    {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol },
        path: Vec::new(),
        type_identity,
        access,
    })
}
