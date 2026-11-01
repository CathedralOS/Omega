//! One possible completed value and its simultaneous authenticated aliases.
//! Alternatives remain separate obligations, including candidates with the same
//! selection condition. Geometry is useful only with matching type and epoch.

use super::{BoundObservation, CanonicalPlace, CheckFacts, FlowOperandFact};
use crate::flow::{CapturedValue, CapturedValueChoice};
use facts::BooleanPredicateOperand;
use typed_trees::types::TypeReferenceHandle;
use validation::CheckingInput;
use validation::canonical_place_type_reference;
use validation::owned_projection_type_reference;
use validation::project_type_reference_from_segments;
use validation::reference_shell;

pub(in crate::checks::contracts) struct Alias<'facts> {
    pub place: CanonicalPlace,
    pub epoch: &'facts FlowOperandFact,
}

pub(in crate::checks::contracts) struct CapturedBinding<'facts> {
    pub position: usize,
    pub choices: Vec<CapturedValueChoice>,
    pub aliases: Vec<Alias<'facts>>,
}

impl<'facts> CapturedBinding<'facts> {
    pub fn unresolved(position: usize, choices: Vec<CapturedValueChoice>) -> Self {
        Self {
            position,
            choices,
            aliases: Vec::new(),
        }
    }

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

pub(in crate::checks::contracts) fn retain_bindings<'facts>(
    program: &dyn CheckingInput,
    facts: &'facts CheckFacts,
    captured: &CapturedValue<'facts>,
    epoch: &'facts FlowOperandFact,
    bindings: &[BoundObservation],
    first_position: usize,
    retained: &mut Vec<CapturedBinding<'facts>>,
) {
    for (offset, binding) in bindings.iter().enumerate() {
        let producer = producer_alias(program, facts, captured, epoch, binding);
        let mut candidate =
            CapturedBinding::observation(first_position + offset, binding.place.clone(), epoch);
        candidate.choices = captured.choices.clone();
        if let Some((observed, alias, source_type)) = &producer
            && binding.place == *observed
            && binding.compatible_type(program, *source_type)
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
    program: &dyn CheckingInput,
    facts: &'facts CheckFacts,
    captured: &CapturedValue<'facts>,
    epoch: &'facts FlowOperandFact,
    binding: &BoundObservation,
) -> Option<(CanonicalPlace, Alias<'facts>, TypeReferenceHandle)> {
    let stable_copy = |reference, projection: &crate::flow::LiteralExpressionProjection| {
        match &binding.operand {
            BooleanPredicateOperand::Scalar(_) => {
                project_type_reference_from_segments(program, reference, &projection.remaining)
                    .is_some_and(|selected| {
                        crate::flow::operand_binds_stable_copy(program, selected)
                    })
            }
            BooleanPredicateOperand::CaseSubject(_) => {
                // A tag is part of the owned carrier even when its payload
                // contains live views. Never cross a reference to freeze it.
                owned_projection_type_reference(program, reference, &projection.remaining)
                    .is_some_and(|selected| {
                        reference_shell(program, selected).is_none()
                            && program.closed_data_definition(selected).is_some()
                    })
            }
        }
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
    // later actuals observe the copy, not a new read of the descriptor. The
    // producer may be the input descriptor before the call wrote its referent;
    // that old content is not the read's value. Its recorded descriptor graph
    // supplies the address, while the read supplies post-receiver predicates.
    // Without that cut, a live view still owes content preservation through the
    // actual's epoch. Binding-prefix checks below remain independent.
    let producer_epoch = captured.producer_epoch.unwrap_or(captured.operand);
    if crate::flow::operand_binds_live_view(program, captured.type_reference)
        && captured.producer_epoch.is_none()
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
        if let Some(read) = captured.producer_epoch {
            !crate::flow::operand_bindings_agree(
                program,
                &facts.flow,
                captured.operand.row(),
                read.row(),
                prefix,
            )
        } else {
            !crate::flow::operand_entry_binding_origin_is_stable(
                program,
                &facts.flow,
                captured.operand.row(),
                prefix,
            )
        }
    }) {
        return None;
    }
    let Some(source_type) = canonical_place_type_reference(
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
        source_type,
    ))
}

/// One read of an alternative of a completed selection: the read epoch that
/// detached its contents and the storage place it observed.
pub(in crate::checks::contracts) struct AlternativeRead<'facts> {
    choice: CapturedValueChoice,
    epoch: &'facts FlowOperandFact,
    place: CanonicalPlace,
}

/// Record every single-choice read among these completed values. Values whose
/// contents were not detached by a read, or that span several selections, add
/// nothing.
pub(in crate::checks::contracts) fn collect_alternative_reads<'facts>(
    program: &dyn CheckingInput,
    facts: &'facts CheckFacts,
    values: &[CapturedValue<'facts>],
    reads: &mut Vec<AlternativeRead<'facts>>,
) {
    for value in values {
        let (Some(epoch), [choice]) = (value.producer_epoch, value.choices.as_slice()) else {
            continue;
        };
        let Some(places) = crate::flow::captured_value_places(program, &facts.flow, value) else {
            continue;
        };
        for place in places {
            reads.push(AlternativeRead {
                choice: *choice,
                epoch: epoch.row(),
                place: place.projected,
            });
        }
    }
}

/// A binding on current storage equals, on each predecessor, the one read that
/// predecessor reached when every path reaches a read of an alternative of one
/// selection and preserves its contents (`fact_validity.md`: each predecessor
/// supplies its own reaching value). The binding then has one capture per
/// alternative under that alternative's condition; coverage decides the rest.
/// Several reads under one alternative, or reads from different selections,
/// leave the binding unconditional.
pub(in crate::checks::contracts) fn retain_alternative_reads<'facts>(
    program: &dyn CheckingInput,
    facts: &'facts CheckFacts,
    storage: arena::Handle<checked_trees::FlowStorageSnapshot>,
    bindings: &[BoundObservation],
    reads: &[AlternativeRead<'facts>],
    retained: &mut Vec<CapturedBinding<'facts>>,
) {
    for (position, binding) in bindings.iter().enumerate() {
        let mut own = retained
            .iter()
            .filter(|capture| capture.position == position);
        if own.clone().next().is_none() || own.any(|capture| !capture.choices.is_empty()) {
            continue;
        }
        let candidates = reads
            .iter()
            .filter(|read| read.place == binding.place)
            .collect::<Vec<_>>();
        let Some(first) = candidates.first() else {
            continue;
        };
        if candidates
            .iter()
            .any(|read| read.choice.selection != first.choice.selection)
            || candidates.iter().enumerate().any(|(index, read)| {
                candidates[..index]
                    .iter()
                    .any(|earlier| earlier.choice.alternative == read.choice.alternative)
            })
        {
            continue;
        }
        let cuts = candidates
            .iter()
            .map(|read| read.epoch.storage_snapshot)
            .collect::<Vec<_>>();
        if !validation::completed_storage::contents_preserved_from(
            program,
            &facts.flow.control,
            storage,
            &binding.place,
            &cuts,
        ) {
            continue;
        }
        retained.retain(|capture| capture.position != position);
        retained.extend(candidates.into_iter().map(|read| CapturedBinding {
            position,
            choices: vec![read.choice],
            aliases: vec![Alias {
                place: binding.place.clone(),
                epoch: read.epoch,
            }],
        }));
    }
}
