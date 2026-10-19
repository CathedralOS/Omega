//! Bind body conservation to custody at the value's actual completion.
//!
//! The permission producer supplies its current selected frontier. Observing
//! entry identity or eventual exit availability is insufficient: the input
//! must still be live, routed, unchanged and unborrowed at this completion. These rows
//! remain evidence inputs, not parent-consumption or child-establishment acts.

use super::linear_obligations::LinearPlace;
use checked_trees::CheckFacts;
use checked_trees::facts::{ContentBodyAvailableInput, ContentBodyConservationFact};
use language_semantics::{PermissionClaimIdentity, PermissionProvenance};
use validation::CanonicalPlace;

pub(crate) fn observe_inputs(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    conservation: &ContentBodyConservationFact,
    places: &[LinearPlace],
) -> Result<Vec<(ContentBodyAvailableInput, Vec<facts::PlaceSegment>)>, &'static str> {
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
    for binding_handle in conservation.inputs.handles() {
        let binding = facts
            .qualifications
            .content
            .body_claim_bindings
            .get(binding_handle);
        let place = unique_live_input(places, binding.claim_identity)?;
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
            binding.domain,
            binding.semantic_domain,
        ) {
            return Err("input's exact routed membership is not live at completion");
        }
        let projection = facts
            .qualifications
            .content
            .plans
            .iter()
            .find(|projection| {
                projection.domain == binding.domain
                    && projection.semantic_domain == binding.semantic_domain
                    && projection.machine == binding.projection_machine
            })
            .ok_or("missing exact owner projection")?;
        let observations = crate::checks::content::projection_observations(&projection.expression)
            .ok_or("unresolved projection dependencies")?;
        let entry_path = facts
            .flow
            .ownership
            .segments
            .span_or_empty(binding.segments);
        for observation in observations {
            let mut observed = current.clone();
            observed.segments.extend_from_slice(&observation);
            let mut expected = CanonicalPlace {
                root: facts::PlaceRoot::Symbol(binding.parameter_symbol),
                segments: entry_path.to_vec(),
            };
            expected.segments.extend_from_slice(&observation);
            if !validation::completed_value_preserves_entry_projection(
                program,
                machine,
                state,
                operand.statement_index,
                operand.expression,
                &observed,
                &expected,
            ) {
                return Err("input content does not retain the theorem's entry revision");
            }
        }
        inputs.push((
            ContentBodyAvailableInput {
                binding: binding_handle,
                root: place.root,
                segments: Default::default(),
                provenance: place.provenance.ok_or("missing input provenance")?,
            },
            place.path.clone(),
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
            .map(|(input, path)| CanonicalPlace {
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
    observed: Vec<(ContentBodyAvailableInput, Vec<facts::PlaceSegment>)>,
) {
    let inputs = observed
        .into_iter()
        .map(|(mut input, path)| {
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
