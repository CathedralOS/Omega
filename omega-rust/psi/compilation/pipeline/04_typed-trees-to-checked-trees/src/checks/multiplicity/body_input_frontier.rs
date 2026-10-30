//! Bind body conservation to custody at the value's actual completion.
//!
//! The permission producer supplies its current selected frontier. Observing
//! entry identity or eventual exit availability is insufficient: every current
//! descendant must be live, routed, unchanged and unborrowed at this completion. These rows
//! remain evidence inputs, not parent-consumption or child-establishment acts.

use super::linear_obligations::LinearPlace;
use checked_trees::CheckFacts;
use checked_trees::facts::{
    ContentBodyAvailableInput, ContentBodyClaimBinding, ContentBodyConservationFact,
};
use language_semantics::{PermissionClaimIdentity, PermissionProvenance};
use validation::CanonicalPlace;

pub(crate) fn observe_inputs(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    conservation: &ContentBodyConservationFact,
    places: &[LinearPlace],
) -> Result<
    Vec<(
        ContentBodyAvailableInput,
        Vec<facts::PlaceSegment>,
        Vec<ContentBodyClaimBinding>,
    )>,
    &'static str,
> {
    let machine = program
        .machine_by_symbol(conservation.machine_symbol)
        .ok_or("missing owning machine")?;
    let state = program
        .state_by_symbol(conservation.state_symbol)
        .ok_or("missing owning state")?;
    if !facts
        .flow
        .control
        .operands
        .is_valid(conservation.completed_value)
    {
        return Err("missing completion capture");
    }
    let operand = facts
        .flow
        .control
        .operands
        .get(conservation.completed_value);
    let contexts = facts
        .flow
        .semantic_constraint_contexts(operand.constraints)
        .collect::<Vec<_>>();
    let mut inputs = Vec::new();
    let descendants = facts.qualifications.content.descendant_frontier(
        conservation.inputs,
        conservation.machine_symbol,
        conservation.state_symbol,
        operand.statement_index,
        conservation.evaluation,
        arena::Handle::invalid(),
    )?;
    for (transformation, expected, consumed) in descendants.replacements {
        super::content_snapshots::preserve_replacement(
            program,
            facts,
            transformation,
            expected,
            consumed,
        )?;
    }
    for descendant in descendants.bindings {
        let current_binding = facts
            .qualifications
            .content
            .body_claim_bindings
            .get(descendant.binding);
        let place = unique_live_input(places, current_binding.claim_identity)?;
        let current = CanonicalPlace {
            root: place.root,
            segments: place.path.clone(),
        };
        if !crate::flow::place_cases_are_selected(
            program,
            &facts.semantic,
            &contexts,
            conservation.machine_symbol,
            conservation.state_symbol,
            operand.statement_index,
            &current,
        ) {
            return Err("input case is not selected at completion");
        }
        if !crate::checks::contracts::exact_scalar_membership(
            program,
            facts,
            &contexts,
            &current,
            current_binding.domain,
            current_binding.semantic_domain,
        ) {
            return Err("input's exact routed membership is not live at completion");
        }
        super::content_snapshots::preserve_projection(
            program,
            facts,
            descendant.binding,
            super::content_snapshots::Observation::Place {
                at: operand,
                place: current,
            },
        )?;
        inputs.push((
            ContentBodyAvailableInput {
                binding: descendant.binding,
                entry_bindings: Default::default(),
                root: place.root,
                segments: Default::default(),
                provenance: place.provenance.ok_or("missing input provenance")?,
            },
            place.path.clone(),
            descendant
                .entries
                .iter()
                .map(|entry| {
                    facts
                        .qualifications
                        .content
                        .body_claim_bindings
                        .get(*entry)
                        .clone()
                })
                .collect(),
        ));
    }
    if inputs.is_empty() {
        return Err("no owned input");
    }
    crate::checks::borrows::require_unborrowed_completion(
        program,
        facts,
        machine,
        state,
        operand,
        conservation.evaluation,
        &inputs
            .iter()
            .map(|(input, path, _)| CanonicalPlace {
                root: input.root,
                segments: path.clone(),
            })
            .collect::<Vec<_>>(),
    )?;
    Ok(inputs)
}

fn unique_live_input(
    places: &[LinearPlace],
    identity: PermissionClaimIdentity,
) -> Result<&LinearPlace, &'static str> {
    if identity == PermissionClaimIdentity::Unknown {
        return Err("unknown entry claim");
    }
    let mut matching = places.iter().filter(|place| {
        place.claim_identity == Some(identity) && place.live && !place.case_excluded
    });
    let place = matching.next().ok_or("entry claim is no longer live")?;
    if matching.next().is_some() {
        return Err("entry claim has more than one live owner");
    }
    if !place.ever_established
        || place.zero_image
        || !place.root.is_valid()
        || matches!(place.provenance, None | Some(PermissionProvenance::Unknown))
    {
        return Err("live input lacks established custody");
    }
    Ok(place)
}

pub(crate) fn publish(
    facts: &mut CheckFacts,
    conservation: arena::Handle<ContentBodyConservationFact>,
    observed: Vec<(
        ContentBodyAvailableInput,
        Vec<facts::PlaceSegment>,
        Vec<ContentBodyClaimBinding>,
    )>,
) {
    let inputs = observed
        .into_iter()
        .map(|(mut input, path, entries)| {
            input.entry_bindings = facts
                .qualifications
                .content
                .body_claim_bindings
                .insert_many(entries);
            input.segments = facts.flow.ownership.segments.insert_many(path);
            input
        })
        .collect::<Vec<_>>();
    let inputs = facts
        .qualifications
        .content
        .body_available_inputs
        .insert_many(inputs);
    facts
        .qualifications
        .content
        .body_conservations
        .get_mut(conservation)
        .available_inputs = inputs;
}
