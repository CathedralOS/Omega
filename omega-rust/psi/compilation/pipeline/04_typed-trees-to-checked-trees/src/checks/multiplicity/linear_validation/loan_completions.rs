//! Normal exclusive calls return storage, not the old value's identity. The
//! shared mutation owner supplies exact storage writes and the signature's
//! exclusive authority ceiling, including retained aliases and carried loans.
//! Completion publishes the possible claim paths of the returned region;
//! actual stored case guards select its live subset during execution.

use super::super::linear_obligations::{ClaimIdentityAllocator, LinearPlace};
use checked_trees::{CheckFacts, FlowCallFact, FlowLoanCompletionFact, FlowPermissionEventFact};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
};
use symbols::SymbolHandle;

pub(crate) fn regions_for_calls(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    calls: &[FlowCallFact],
    frames: Option<&validation::CallFrameResolver<'_>>,
    summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<Vec<(PermissionEventSource, Vec<validation::CanonicalPlace>)>, diagnostics::Diagnostic>
{
    calls
        .iter()
        .map(|call| {
            let source = PermissionEventSource::Call {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_symbol: call.target_symbol,
            };
            let caller = program
                .machine_by_symbol(machine)
                .ok_or_else(|| diagnostics::Diagnostic::error("loan completion lost its caller"))?;
            // Internal transitions do not suspend a caller frame. External
            // completion abandons it; neither can establish returned custody.
            if validation::terminal_callable_target(program, caller, call.target_symbol).is_none()
                || validation::boundary_requirement_completion(program, call.target_symbol)
                    != semantic_vocabulary::BoundaryCompletion::NormalReturn
            {
                return Ok((source, Vec::new()));
            }
            let borrowed = checked_trees::BorrowCallFact {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                authored_expression: call.authored_expression,
                reference_slot_effects: Default::default(),
                completed_reference_loans: Default::default(),
                receiver_symbol: call.receiver_symbol,
                target_symbol: call.target_symbol,
                has_receiver: call.has_receiver,
                accesses: call.accesses,
            };
            let ceiling = crate::flow::signature_ceiling_regions(
                program,
                machine,
                state,
                Some(&facts.borrow),
                &borrowed,
                frames,
            )
            .ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
                    "exclusive call completion of `{}` in `{}` state `{}` at statement {} call {} has no representable caller storage authority",
                    program.symbols.display_path(call.target_symbol,"::"), caller.name,
                    program.state_by_symbol(state).map_or("<unknown>",|state| state.name.as_str()), call.statement_index,call.call_ordinal,
                ))
            })?;
            if ceiling.is_empty() {
                return Ok((source, Vec::new()));
            }
            let writes = crate::flow::call_mutated_regions(
                program,
                machine,
                state,
                &facts.borrow,
                &borrowed,
                summaries,
                frames,
            )
            .unwrap_or_else(|| ceiling.clone());
            let mut regions = Vec::new();
            for write in writes {
                for authority in &ceiling {
                    let region = if authority.contains(&write) {
                        &write
                    } else if write.contains(authority) {
                        authority
                    } else {
                        continue;
                    };
                    let Some(mut region) = region.clone().into_declared() else {
                        continue;
                    };
                    // A write summary may retain an index expression even
                    // when substitution closed it. Resolve only genuinely
                    // closed coordinates, never a current caller guess.
                    for segment in &mut region.segments {
                        if let facts::PlaceSegment::Index { expression } = *segment
                            && let Some(index) = program
                                .closed_integer_expression_value_in(expression, machine)
                                .and_then(|value| value.to_u64())
                                .and_then(|value| usize::try_from(value).ok())
                        {
                            *segment = facts::PlaceSegment::FixedIndex { index };
                        }
                    }
                    if let Some(unresolved) = region.segments.iter().position(|segment| {
                        !matches!(segment, facts::PlaceSegment::Field { .. }
                            | facts::PlaceSegment::Case { .. }
                            | facts::PlaceSegment::FixedIndex { .. })
                    }) {
                        // Runtime selectors are may-write coordinates, not
                        // claim identities. Rejoin their containing collection
                        // only if this call lent that complete collection.
                        // An ancestor outside the exclusive ceiling would
                        // resurrect unrelated moved siblings and is forbidden.
                        region.segments.truncate(unresolved);
                        if authority.anchor().root != region.root
                            || !region.segments.starts_with(&authority.anchor().segments)
                        {
                            return Err(diagnostics::Diagnostic::error(
                                "loan completion has no static region within its exclusive authority",
                            ));
                        }
                    }
                    if !regions.iter().any(|existing| existing == &region) {
                        regions.push(region);
                    }
                }
            }
            // A parent region subsumes its descendants. Restore each owned
            // catalogue once even when several declared writes overlap it.
            let candidates = regions.clone();
            regions.retain(|region| {
                !candidates.iter().any(|other| {
                    other != region && other.root == region.root
                        && region.segments.starts_with(&other.segments)
                })
            });
            // A content write returns the existing owner's loan, not a
            // replacement Extent value. Callee borrowed-content restoration
            // remains obligatory; it cannot mint the caller's owner claim.
            // Only declared storage replacement has a new payload roster.
            Ok((source, regions))
        })
        .collect()
}

pub(crate) fn restore(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    source: PermissionEventSource,
    regions: &[validation::CanonicalPlace],
    places: &mut [LinearPlace],
    identities: &mut ClaimIdentityAllocator,
    events: &mut Vec<FlowPermissionEventFact>,
) -> Result<(), diagnostics::Diagnostic> {
    for region in regions {
        // A write inside a nominal claim does not change its ownership home.
        // A write of the home or its ancestor may replace its payload roster.
        // Borrowed ingress has no owned places here; the ultimate owner's
        // invocation joins its own namespace when the loan returns.
        let selected = places
            .iter()
            .enumerate()
            .filter_map(|(position, place)| {
                (place.root == region.root
                    && place.ever_established
                    && place.path.starts_with(&region.segments)
                    && place.multiplicity == Multiplicity::Linear)
                    .then_some(position)
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            continue;
        }
        let PermissionEventSource::Call {
            statement_index, ..
        } = source
        else {
            return Err(diagnostics::Diagnostic::error(
                "loan completion is not attached to a call",
            ));
        };
        let reference =
            validation::canonical_place_type_reference(program, state, statement_index, region)
                .ok_or_else(|| {
                    diagnostics::Diagnostic::error("loan completion has no exact owned region type")
                })?;
        facts
            .flow
            .ownership
            .loan_completions
            .insert(FlowLoanCompletionFact {
                machine,
                state,
                source,
                root: region.root,
                segments: facts
                    .flow
                    .ownership
                    .segments
                    .insert_many(region.segments.iter().copied()),
                multiplicity: program.type_multiplicity(reference),
            });
        for position in selected {
            let place = &mut places[position];
            let identity = identities.mint(machine, state, source);
            let provenance =
                super::permission_production::established_provenance(machine, state, source);
            place.live = true;
            place.case_excluded = false;
            place.claim_identity = Some(identity);
            place.provenance = Some(provenance);
            events.push(FlowPermissionEventFact {
                expression: Default::default(),
                machine_symbol: machine,
                state_symbol: state,
                source,
                evaluation: checked_trees::FlowPermissionEvaluation::Statement,
                kind: PermissionEventKind::Restore,
                multiplicity: place.multiplicity,
                access: PermissionAccess::Owned,
                claim_identity: identity,
                provenance,
                root: place.root,
                segments: facts
                    .flow
                    .ownership
                    .segments
                    .insert_many(place.path.iter().copied()),
                obligation_live: true,
            });
        }
    }
    Ok(())
}
