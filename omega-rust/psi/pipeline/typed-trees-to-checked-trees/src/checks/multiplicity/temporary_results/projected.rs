//! A temporary's selected subtree moves; its exact complement dies at the call.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn append(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    calls: &[checked_trees::FlowCallFact],
    event: &crate::flow::DiscoveredMoveEvent,
    permissions: &mut Vec<FlowPermissionEventFact>,
) {
    let facts::PlaceRoot::Expression(expression) = event.root else {
        return;
    };
    let FlowOwnershipEventSource::Call {
        statement_index,
        call_ordinal,
        ..
    } = event.source
    else {
        return;
    };
    let path = facts
        .flow
        .ownership
        .segments
        .span_or_empty(event.segments)
        .to_vec();
    if path.is_empty()
        || !path.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
            )
        })
        || permission_kind_for_move(program, facts, machine, state, event)
            != PermissionEventKind::Transfer
        || permissions.iter().any(|prior| {
            prior.machine_symbol == machine
                && prior.state_symbol == state
                && prior.root == event.root
        })
    {
        return;
    }
    let mut producers = calls.iter().filter(|call| {
        call.statement_index == statement_index
            && call.authored_expression == expression
            && call.call_ordinal > call_ordinal
    });
    let Some(producer) = producers.next() else {
        return;
    };
    if producers.next().is_some() {
        return;
    }
    let root = crate::flow::CanonicalPlace {
        root: event.root,
        segments: Vec::new(),
    };
    let selected = crate::flow::CanonicalPlace {
        root: event.root,
        segments: path.clone(),
    };
    let plain_affine = |place: &crate::flow::CanonicalPlace| {
        crate::flow::canonical_place_type_reference(program, state, statement_index, place)
            .is_some_and(|reference| {
                matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Named { .. }
                        | TypeReferenceNode::Generic { .. }
                        | TypeReferenceNode::FixedArray { .. }
                ) && type_multiplicity(program, reference) == Multiplicity::Affine
                    && validation::has_plain_owned_contents(program, reference)
                    && !type_carries_linear_obligation(program, reference)
            })
    };
    if !plain_affine(&root) || !plain_affine(&selected) {
        return;
    }
    let mut residuals = Vec::new();
    if complement(
        program,
        state,
        statement_index,
        &root,
        &path,
        &mut residuals,
    )
    .is_none()
    {
        return;
    }
    let source = PermissionEventSource::Call {
        statement_index,
        call_ordinal: producer.call_ordinal,
        target_symbol: producer.target_symbol,
    };
    let provenance = PermissionProvenance::Established {
        machine_symbol: machine,
        state_symbol: state,
        source,
    };
    let base = FlowPermissionEventFact {
        machine_symbol: machine,
        state_symbol: state,
        source,
        kind: PermissionEventKind::Establish,
        multiplicity: Multiplicity::Affine,
        access: PermissionAccess::Owned,
        claim_identity: PermissionClaimIdentity::Unknown,
        provenance,
        root: event.root,
        segments: HandleSpan::empty(),
        obligation_live: false,
    };
    permissions.push(base.clone());
    permissions.push(FlowPermissionEventFact {
        source: permission_source(event.source),
        kind: PermissionEventKind::Transfer,
        segments: event.segments,
        ..base.clone()
    });
    for path in residuals {
        permissions.push(FlowPermissionEventFact {
            source: permission_source(event.source),
            kind: PermissionEventKind::AffineDrop,
            segments: facts.flow.ownership.segments.insert_many(path),
            ..base.clone()
        });
    }
}

// Only traverse the selected branch. Untouched siblings are maximal residuals,
// not leaf expansion, and retain their exact authored field/index identities.
fn complement(
    program: &typed_trees::TypedTrees,
    state: SymbolHandle,
    statement: usize,
    root: &crate::flow::CanonicalPlace,
    selected: &[facts::PlaceSegment],
    output: &mut Vec<Vec<facts::PlaceSegment>>,
) -> Option<()> {
    let Some(next) = selected.first() else {
        return Some(());
    };
    let reference = crate::flow::canonical_place_type_reference(program, state, statement, root)?;
    let children = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            ..
        } => {
            let data = program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == *symbol)?;
            program
                .data_members(data)
                .iter()
                .map(|member| match member {
                    typed_trees::data::DataMember::Field(field) => {
                        Some(facts::PlaceSegment::Field {
                            symbol: field.symbol,
                        })
                    }
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?
        }
        TypeReferenceNode::FixedArray {
            length: typed_trees::types::FixedArrayLength::Literal(length),
            ..
        } if *length > 0 => (0..*length)
            .map(|index| facts::PlaceSegment::FixedIndex { index })
            .collect(),
        _ => return None,
    };
    if !children.contains(next) {
        return None;
    }
    for child in children.into_iter().rev() {
        let mut place = root.clone();
        place.segments.push(child);
        if child == *next {
            complement(program, state, statement, &place, &selected[1..], output)?;
        } else {
            let reference =
                crate::flow::canonical_place_type_reference(program, state, statement, &place)?;
            if type_multiplicity(program, reference) == Multiplicity::Unrestricted {
                continue;
            }
            if type_multiplicity(program, reference) != Multiplicity::Affine
                || !validation::has_plain_owned_contents(program, reference)
            {
                return None;
            }
            output.push(place.segments);
        }
    }
    Some(())
}
