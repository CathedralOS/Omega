//! One possible completed value and its simultaneous authenticated aliases.
//! Alternatives remain separate obligations, including candidates with the same
//! selection condition. Geometry is useful only with matching type and epoch.

use super::{BoundObservation, CanonicalPlace, CheckFacts, FlowOperandFact};
use crate::flow::{CapturedValue, CapturedValueChoice};

pub(super) struct Alias<'facts> {
    pub place: CanonicalPlace,
    pub epoch: &'facts FlowOperandFact,
}

pub(super) struct CapturedBinding<'facts> {
    pub position: usize,
    pub choices: Vec<CapturedValueChoice>,
    pub aliases: Vec<Alias<'facts>>,
}

impl<'facts> CapturedBinding<'facts> {
    pub fn observation(
        position: usize,
        place: CanonicalPlace,
        epoch: &'facts FlowOperandFact,
    ) -> Self {
        Self {
            position,
            choices: Vec::new(),
            aliases: vec![Alias { place, epoch }],
        }
    }
}

pub(super) fn retain_bindings<'facts>(
    program: &dyn validation::CheckingInput,
    facts: &'facts CheckFacts,
    captured: &CapturedValue<'facts>,
    epoch: &'facts FlowOperandFact,
    bindings: &[BoundObservation],
    first_position: usize,
    retained: &mut Vec<CapturedBinding<'facts>>,
) {
    let producer = producer_alias(program, facts, captured, epoch);
    for (offset, binding) in bindings.iter().enumerate() {
        let mut candidate =
            CapturedBinding::observation(first_position + offset, binding.place.clone(), epoch);
        candidate.choices = captured.choices.clone();
        if let Some((observed, alias, primitive)) = &producer
            && binding.place == *observed
            && binding.operand.primitive_type() == Some(*primitive)
        {
            candidate.aliases.push(Alias {
                place: alias.place.clone(),
                epoch: alias.epoch,
            });
        }
        retained.push(candidate);
    }
}

fn producer_alias<'facts>(
    program: &dyn validation::CheckingInput,
    facts: &'facts CheckFacts,
    captured: &CapturedValue<'facts>,
    epoch: &'facts FlowOperandFact,
) -> Option<(
    CanonicalPlace,
    Alias<'facts>,
    typed_trees::types::PrimitiveType,
)> {
    let stable_copy = |reference, projection: &crate::flow::LiteralExpressionProjection| {
        validation::project_type_reference_from_segments(program, reference, &projection.remaining)
            .is_some_and(|selected| crate::flow::operand_binds_stable_copy(program, selected))
    };
    if !stable_copy(captured.observation_type, &captured.observation_projection)
        || (captured.producer_epoch.is_none()
            && !stable_copy(captured.type_reference, &captured.projection))
    {
        return None;
    }
    let Some(observed) = crate::flow::captured_observation_places(program, &facts.flow, captured)
    else {
        return None;
    };
    let Some(produced) = crate::flow::captured_value_places(program, &facts.flow, captured) else {
        return None;
    };
    let ([observed], [produced]) = (observed.as_slice(), produced.as_slice()) else {
        return None;
    };
    // A typed receiving read may detach contents from a producer that remains
    // a live descriptor. Its recorded read epoch owns that source identity;
    // later actuals observe the copy, not a new read of the descriptor. Without
    // that cut, a live view still owes preservation through the actual's epoch.
    let producer_epoch = captured.producer_epoch.unwrap_or(captured.operand);
    if crate::flow::operand_binds_live_view(program, captured.type_reference)
        && !crate::flow::operand_observations_agree(
            program,
            &facts.flow,
            captured.operand.row(),
            captured.producer_epoch.map_or(epoch, |read| read.row()),
            &produced.projected,
        )
    {
        return None;
    }
    // Geometry alone does not identify the observed value. The source's
    // reference prefixes were evaluated before its selectors: a selector may
    // write payload without redirecting them, but replacing a descriptor makes
    // the completed scalar unavailable as an alias of the current source slot.
    // Keep its original local snapshot binding even when this optional relation
    // cannot be established.
    let Some(prefixes) = super::super::capture_coordinates::borrowed_bindings(
        program,
        captured.operand.row(),
        &captured.projection.remaining,
    ) else {
        return None;
    };
    if prefixes.iter().any(|prefix| {
        !crate::flow::operand_entry_binding_origin_is_stable(
            program,
            &facts.flow,
            captured.operand.row(),
            prefix,
        )
    }) {
        return None;
    }
    let Some(source_type) = validation::canonical_place_type_reference(
        program,
        captured.operand.state_symbol,
        captured.operand.statement_index,
        &produced.projected,
    ) else {
        return None;
    };
    Some((
        observed.projected.clone(),
        Alias {
            place: produced.projected.clone(),
            epoch: producer_epoch.row(),
        },
        program.primitive_type_reference(source_type)?,
    ))
}
