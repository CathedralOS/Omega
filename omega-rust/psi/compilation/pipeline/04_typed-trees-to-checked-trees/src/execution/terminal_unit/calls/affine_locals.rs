//! The affine array construction prefix.

use crate::execution::terminal_unit::types::{
    ShapeCollector, parameter_qualifications, type_graph_requires_nominal_drop,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedAffineConstructionElementPlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitStructuralTypeShape, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, StatementNode, SymbolHandle, TypeReferenceNode,
};
use std::collections::BTreeSet;

pub(crate) fn build_affine_array_construction_prefix(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    statements: &[StatementNode],
) -> Option<(
    Vec<(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)>,
    usize,
)> {
    let [StatementNode::LocalData(local), assignments @ ..] = statements else {
        return None;
    };
    // The accepted prefix is a declared root length of 3..=26: the assignments
    // fill one element each and the remaining zeroed tail element holds no data.
    let root_length = assignments
        .len()
        .checked_add(1)
        .filter(|length| (3..=26).contains(length))?;
    if !local.is_mutable
        || local.initial_value.is_valid()
        || crate::checks::type_multiplicity(program, local.type_reference) != Multiplicity::Affine
        || !parameter_qualifications(program, shapes, local.type_reference, binders)?.is_empty()
        || type_graph_requires_nominal_drop(
            program,
            local.type_reference,
            &[],
            &mut BTreeSet::new(),
        )
    {
        return None;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(actual_length),
    } = program
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return None;
    };
    if *actual_length != root_length {
        return None;
    }
    if crate::checks::type_multiplicity(program, *element_type) != Multiplicity::Affine
        || type_graph_requires_nominal_drop(program, *element_type, &[], &mut BTreeSet::new())
    {
        return None;
    }
    let TypeReferenceNode::Named {
        symbol: element_symbol,
        ..
    } = program.type_reference_table.type_reference(*element_type)
    else {
        return None;
    };
    let root_type_identity =
        shapes.add_affine_array_construction_type(local.type_reference, binders)?;
    let element_type_identity = shapes.add_type(*element_type, binders, &[])?;
    let root_shape = shapes.types.get(&root_type_identity)?;
    if !matches!(
        &root_shape.shape,
        CheckedUnitStructuralTypeShape::FixedArray {
            element_type_identity: element,
            length,
        } if element == &element_type_identity
            && length
                .literal()
                .is_some_and(|length| usize::try_from(length) == Ok(root_length))
    ) || !matches!(
        shapes.types.get(&element_type_identity).map(|shape| &shape.shape),
        Some(CheckedUnitStructuralTypeShape::Record { fields, .. }) if fields.is_empty()
    ) {
        return None;
    }

    let mut rows = Vec::with_capacity(assignments.len());
    for (expected_index, statement) in assignments.iter().enumerate() {
        let assignment = statement.as_assignment()?;
        let target = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            expected_index + 1,
            assignment.target,
        )?;
        if target.root != facts::PlaceRoot::Symbol(local.symbol)
            || target.segments.as_slice()
                != [facts::PlaceSegment::FixedIndex {
                    index: expected_index,
                }]
        {
            return None;
        }
        let literal = program
            .expression_table
            .struct_literal_node(assignment.value)?;
        if literal.case_name.is_some()
            || !program
                .expression_table
                .struct_fields(literal.fields)
                .is_empty()
        {
            return None;
        }
        if literal.type_symbol != *element_symbol {
            return None;
        }
        rows.push((
            CheckedTrivialAffineStructuralLocalPlan {
                declaration_ordinal: u32::try_from(expected_index).ok()?,
                type_identity: element_type_identity.clone(),
                construction: Some(CheckedAffineConstructionElementPlan {
                    root_type_identity: root_type_identity.clone(),
                    index: u64::try_from(expected_index).ok()?,
                }),
            },
            local.symbol,
        ));
    }
    // A declaration-only array of zero-constructible elements is established
    // zeroed at its declaration, so the root's exit drop carries that
    // establishment's provenance. The elements here are fieldless and need no
    // nominal drop: the zeroed tail holds no data and takes no cleanup action,
    // so the element-wise construction and its reverse cleanup still describe
    // the whole root.
    let declaration_provenance = language_semantics::PermissionProvenance::Established {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        source: PermissionEventSource::Statement { statement_index: 0 },
    };
    let exit_events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
                && event.multiplicity == Multiplicity::Affine
                && event.access == PermissionAccess::Owned
                && event.claim_identity == PermissionClaimIdentity::Unknown
                && event.provenance == declaration_provenance
                && event.root == facts::PlaceRoot::Symbol(local.symbol)
                && !event.obligation_live
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    if !matches!(exit_events.as_slice(), [event]
        if facts.flow.ownership.segments.span_or_empty(event.segments).is_empty())
    {
        return None;
    }
    Some((rows, 1))
}
