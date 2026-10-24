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
    let stored = capture_at_snapshot(program, control, state, snapshot, source)?;
    let Some(captures) = stored.captures() else {
        return Some(Vec::new());
    };
    let mut places = Vec::new();
    for selected in captures {
        let stored = super::operand_values::reference_captures_at_path(
            program,
            control,
            selected.capture,
            &selected.projection,
            snapshot,
        )?;
        let Some(descriptors) = stored.captures() else {
            return Some(Vec::new());
        };
        for descriptor in descriptors {
            if !descriptor.projection.is_empty() {
                return Some(Vec::new());
            }
            let capture = control.operands.get(descriptor.capture);
            if capture.referents.is_empty() {
                return Some(Vec::new());
            }
            for referent in control.operand_referents.span_or_empty(capture.referents) {
                let place = CanonicalPlace {
                    root: referent.root,
                    segments: control
                        .operand_referent_segments
                        .span_or_empty(referent.segments)
                        .to_vec(),
                };
                if !places.contains(&place) {
                    places.push(place);
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
