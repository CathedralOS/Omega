//! The trivial affine locals a unit body starts with, and the affine array
//! construction prefix.

use crate::execution::terminal_unit::types::{
    ShapeCollector, parameter_qualifications, type_graph_requires_nominal_drop,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedAffineConstructionElementPlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitStructuralTypeShape, ExpressionNode, Multiplicity, PermissionAccess,
    PermissionClaimIdentity, PermissionEventKind, PermissionEventSource, StatementNode,
    SymbolHandle, TypeReferenceNode, TypedTrees,
};

pub(crate) fn build_unit_trivial_affine_locals(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    statements: &[StatementNode],
) -> Option<Vec<(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)>> {
    statements
        .iter()
        .enumerate()
        .map(|(declaration_ordinal, statement)| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            let TypeReferenceNode::Named { .. } = program
                .type_reference_table
                .type_reference(local.type_reference)
            else {
                return None;
            };
            if local.is_mutable
                || !local.initial_value.is_valid()
                || crate::checks::type_multiplicity(program, local.type_reference)
                    != Multiplicity::Affine
                || !parameter_qualifications(program, shapes, local.type_reference, binders)?
                    .is_empty()
                || type_graph_requires_nominal_drop(program, local.type_reference)
            {
                return None;
            }
            let ExpressionNode::StructLiteral(literal) =
                program.expression_table.expression(local.initial_value)
            else {
                return None;
            };
            if literal.case_name.is_some()
                || !program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty()
            {
                return None;
            }
            let local_events = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.machine_symbol == machine.symbol
                        && event.state_symbol == state.symbol
                        && event.root == facts::PlaceRoot::Symbol(local.symbol)
                })
                .map(|(_, event)| event)
                .collect::<Vec<_>>();
            let [establishment, settlement] = local_events.as_slice() else {
                return None;
            };
            let establishment_source = PermissionEventSource::Statement {
                statement_index: declaration_ordinal,
            };
            let establishment_provenance = language_semantics::PermissionProvenance::Established {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                source: establishment_source,
            };
            if establishment.source != establishment_source
                || establishment.kind != PermissionEventKind::Establish
                || establishment.multiplicity != Multiplicity::Affine
                || establishment.access != PermissionAccess::Owned
                || establishment.claim_identity != PermissionClaimIdentity::Unknown
                || establishment.provenance != establishment_provenance
                || establishment.obligation_live
                || !facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(establishment.segments)
                    .is_empty()
                || !matches!(
                    (settlement.source, settlement.kind),
                    (
                        PermissionEventSource::StateExit,
                        PermissionEventKind::AffineDrop
                    ) | (
                        PermissionEventSource::Call { .. },
                        PermissionEventKind::Transfer
                    )
                )
                || settlement.multiplicity != Multiplicity::Affine
                || settlement.access != PermissionAccess::Owned
                || settlement.claim_identity != PermissionClaimIdentity::Unknown
                || settlement.provenance != establishment_provenance
                || settlement.obligation_live
                || !facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(settlement.segments)
                    .is_empty()
            {
                return None;
            }
            let type_identity = shapes.add_type(local.type_reference, binders, &[])?;
            let shape = shapes.types.get(&type_identity)?;
            if !matches!(
                &shape.shape,
                CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
            ) {
                return None;
            }
            Some((
                CheckedTrivialAffineStructuralLocalPlan {
                    declaration_ordinal: u32::try_from(declaration_ordinal).ok()?,
                    type_identity,
                    construction: None,
                },
                local.symbol,
            ))
        })
        .collect()
}

pub(crate) fn build_affine_array_construction_prefix(
    program: &TypedTrees,
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
    let root_length = match assignments.len() {
        2 => 3,
        3 => 4,
        4 => 5,
        5 => 6,
        6 => 7,
        7 => 8,
        8 => 9,
        9 => 10,
        10 => 11,
        11 => 12,
        12 => 13,
        13 => 14,
        14 => 15,
        15 => 16,
        16 => 17,
        17 => 18,
        18 => 19,
        19 => 20,
        20 => 21,
        21 => 22,
        22 => 23,
        23 => 24,
        24 => 25,
        25 => 26,
        _ => return None,
    };
    if !local.is_mutable
        || local.initial_value.is_valid()
        || crate::checks::type_multiplicity(program, local.type_reference) != Multiplicity::Affine
        || !parameter_qualifications(program, shapes, local.type_reference, binders)?.is_empty()
        || type_graph_requires_nominal_drop(program, local.type_reference)
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
        || type_graph_requires_nominal_drop(program, *element_type)
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
            && usize::try_from(*length) == Ok(root_length)
    ) || !matches!(
        shapes.types.get(&element_type_identity).map(|shape| &shape.shape),
        Some(CheckedUnitStructuralTypeShape::Record { fields }) if fields.is_empty()
    ) {
        return None;
    }

    let mut rows = Vec::with_capacity(assignments.len());
    for (expected_index, statement) in assignments.iter().enumerate() {
        let StatementNode::Assignment(assignment) = statement else {
            return None;
        };
        let target = crate::flow::canonical_place_from_expression_in_state(
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
        let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(assignment.value)
        else {
            return None;
        };
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
                && event.provenance == language_semantics::PermissionProvenance::Unknown
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
