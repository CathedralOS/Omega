//! Exact view addresses come from completed stores. The same query serves
//! operand evaluation and later effect proofs; neither may replay an initializer
//! or replace an unresolved newer descriptor with a historical source address.
use super::stored_values::{StoredObservation, StoredOperand};
use arena::{Handle, HandleSpan};
use checked_trees::{FlowControlFacts, FlowStatementFact, FlowStorageSnapshot};
use validation::CanonicalPlace;

pub(crate) fn before_snapshot(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    state: &typed_trees::state::State,
    snapshot: Handle<FlowStorageSnapshot>,
    source: &CanonicalPlace,
) -> Option<Vec<CanonicalPlace>> {
    Some(
        captured_before_snapshot(program, control, state, snapshot, source)?
            .into_iter()
            .map(|reference| reference.place)
            .collect(),
    )
}

pub(crate) fn captured_before_snapshot(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    state: &typed_trees::state::State,
    snapshot: Handle<FlowStorageSnapshot>,
    source: &CanonicalPlace,
) -> Option<Vec<super::CapturedReferencePlace>> {
    let stored = capture_at_snapshot(program, control, state, snapshot, source)?;
    let Some(captures) = stored.captures() else {
        return Some(Vec::new());
    };
    let mut places = Vec::new();
    for selected in captures {
        let Some(selected) = reference_places_at_snapshot(
            program,
            control,
            selected.capture,
            &selected.projection,
            snapshot,
            |_| false,
        ) else {
            return Some(Vec::new());
        };
        if selected.is_empty() {
            return Some(Vec::new());
        }
        for place in selected {
            crate::collections::push_unique(&mut places, place);
        }
    }
    Some(places)
}

/// Value association chooses the descriptor at the correct cut. Geometry and
/// formation still come only from that descriptor's actual captured inventory.
pub(crate) fn reference_places_at_snapshot(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    capture: Handle<checked_trees::FlowOperandFact>,
    path: &[facts::PlaceSegment],
    snapshot: Handle<FlowStorageSnapshot>,
    mut incoming_is_admitted: impl FnMut(&CanonicalPlace) -> bool,
) -> Option<Vec<super::CapturedReferencePlace>> {
    // Preserve a wholly incoming namespace just as a mixed frontier does.
    // Returning None here would make a saved owned subject retry its anchor
    // as storage, discarding the actual entry slot already resolved by the
    // completed graph. The admission callback below remains mandatory.
    let stored = validation::completed_storage::reference_sources_at_path(
        program, control, capture, path, snapshot,
    );
    let mut pending = vec![stored];
    let mut places = Vec::new();
    while let Some(descriptor) = pending.pop() {
        match descriptor {
            StoredOperand::Unavailable => return Some(Vec::new()),
            StoredOperand::Incoming(place) => {
                if !incoming_is_admitted(&place) {
                    return Some(Vec::new());
                }
                // The existing ingress namespace names its descriptor value;
                // it supplies no new formation or concrete pointee address.
                let place = super::CapturedReferencePlace::unformed(place);
                crate::collections::push_unique(&mut places, place);
            }
            StoredOperand::Alternatives(alternatives) => {
                pending.extend(alternatives.into_iter().rev())
            }
            StoredOperand::Value {
                capture,
                reference,
                projection,
            } => {
                if !control.operands.is_valid(capture) {
                    return Some(Vec::new());
                }
                let value = control.operands.get(capture);
                let Some(selected) =
                    super::operand_values::captured_operand_reference_places_with_formation(
                        program,
                        control,
                        value,
                        reference,
                        &projection,
                    )
                else {
                    return Some(Vec::new());
                };
                if selected.is_empty() {
                    return Some(Vec::new());
                }
                for place in selected {
                    crate::collections::push_unique(&mut places, place);
                }
            }
        }
    }
    Some(places)
}

pub(crate) fn capture_at_snapshot(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    state: &typed_trees::state::State,
    snapshot: Handle<FlowStorageSnapshot>,
    source: &CanonicalPlace,
) -> Option<StoredOperand> {
    // Incoming exclusive parameters lend fixed backing, not their outer slot.
    if source.segments.is_empty()
        && source.root.as_symbol().is_some_and(|symbol| {
            program
                .state_parameter_by_symbol(state, symbol)
                .is_some_and(|parameter| {
                    validation::reference_shell(program, parameter.type_reference).is_some_and(
                        |shell| {
                            shell.access != language_semantics::ReferenceAccess::Shared
                                || !parameter.is_mutable
                        },
                    )
                })
        })
    {
        return None;
    }
    if let Some(stored) = super::stored_values::at_snapshot(
        program,
        control,
        snapshot,
        source,
        StoredObservation::Binding,
    ) {
        return Some(stored);
    }
    source
        .root
        .as_symbol()
        .and_then(|root| program.state_local_declaration(state, root))
        .filter(|(_, local)| {
            super::operand_contexts::operand_binds_live_view(program, local.type_reference)
        })
        .map(|_| StoredOperand::Unavailable)
}

/// Resolve only descriptor crossings on an address. Writes to an atomic
/// cell's contents do not replace its containing record's reference binding.
pub(crate) fn address_before_statement(
    program: &dyn validation::CheckingInput,
    _semantic: &facts::FactPlan,
    control: &FlowControlFacts,
    state: &typed_trees::state::State,
    statements: HandleSpan<FlowStatementFact>,
    before: usize,
    source: CanonicalPlace,
) -> Option<Vec<CanonicalPlace>> {
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let end = order.position(before).or_else(|| {
        (before
            == program
                .statement_table
                .statements(state.statement_nodes)
                .len())
        .then_some(order.indices().len())
    })?;
    let snapshot = if let Some(previous) = order.indices()[..end].last() {
        control
            .statements
            .span_or_empty(statements)
            .iter()
            .find(|statement| statement.statement_index == *previous)?
            .storage_snapshot
    } else {
        arena::Handle::invalid()
    };
    let mut pending = vec![(source, 0usize)];
    let mut output = Vec::new();
    while let Some((source, depth)) = pending.pop() {
        if depth >= 64 {
            return None;
        }
        let mut replaced = false;
        for length in 0..=source.segments.len() {
            let prefix = CanonicalPlace {
                root: source.root,
                segments: source.segments[..length].to_vec(),
            };
            let reference =
                validation::canonical_place_type_reference(program, state.symbol, before, &prefix)?;
            if !super::operand_contexts::operand_binds_live_view(program, reference) {
                continue;
            }
            let Some(places) = before_snapshot(program, control, state, snapshot, &prefix) else {
                continue;
            };
            if places.len() == 1 && places[0] == prefix {
                continue;
            }
            for mut place in places {
                place.segments.extend_from_slice(&source.segments[length..]);
                pending.push((place, depth + 1));
            }
            replaced = true;
            break;
        }
        if !replaced && !output.contains(&source) {
            output.push(source);
        }
    }
    Some(output)
}
