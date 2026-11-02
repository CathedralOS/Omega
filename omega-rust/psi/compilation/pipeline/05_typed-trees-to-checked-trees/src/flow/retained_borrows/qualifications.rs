//! A retained handback conserves declared membership from every actual input.
//! Captured constraints prove the input qualification; descriptor snapshots only
//! relate its exact storage aliases. Neither relation manufactures an issuer.

use super::{RetainedBorrowHandback, RetainedBorrowSupply};
use arena::Handle;
use checked_trees::{CheckFacts, FlowContextFacts, FlowControlFacts, FlowStorageSnapshot};
use facts::{FactPlan, PlaceSegment, ProgramPoint};
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;
use validation::{CanonicalPlace, completed_storage::StoredObservation};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RetainedMembership {
    pub place: CanonicalPlace,
    pub domain: SymbolHandle,
    pub instance: SemanticDomainId,
}

/// The full declared duty must hold at every possible original supply. Proof
/// handles may differ between arms; only their exact subject and applied domain
/// are met. Aliases also meet across arms rather than being unioned into grants.
pub(in crate::flow) fn memberships(
    program: &dyn validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &FlowContextFacts,
    control: &FlowControlFacts,
    returned: &RetainedBorrowHandback,
) -> Vec<RetainedMembership> {
    if returned.supplies.is_empty() {
        return Vec::new();
    }
    // One immutable membership selection per actual supply, not one whole
    // context scan per declared array element or projected field.
    let supplies = returned
        .supplies
        .iter()
        .map(|supply| {
            let selected = contexts
                .semantic_constraint_contexts(supply.constraints)
                .collect::<Vec<_>>();
            let evidence = validation::membership_evidence::MembershipEvidence::prepare(
                program, semantic, &selected,
            );
            (supply, selected, evidence)
        })
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for (path, domain, instance) in
        validation::field_domains::declared_result_field_domain_paths(program, returned.reference)
    {
        if !instance.is_valid()
            || !path
                .iter()
                .all(crate::flow::place_segment_has_static_identity)
        {
            continue;
        }
        let mut common: Option<Vec<CanonicalPlace>> = None;
        let supplied = supplies.iter().all(|(supply, source_contexts, evidence)| {
            let aliases = aliases(program, control, returned, supply, &path);
            let proven = crate::checks::contracts::value_provable_domain(program, domain)
                || aliases.iter().any(|place| {
                    crate::flow::place_cases_are_selected(
                        program,
                        semantic,
                        source_contexts,
                        program
                            .machine_holding_state(supply.operand.state_symbol)
                            .map_or(SymbolHandle::invalid(), |machine| machine.symbol),
                        supply.operand.state_symbol,
                        supply.operand.statement_index,
                        place,
                    ) && evidence.contains(place, domain, instance)
                });
            if !proven {
                return false;
            }
            match &mut common {
                Some(common) => common.retain(|place| aliases.contains(place)),
                None => common = Some(aliases),
            }
            true
        });
        if supplied {
            for place in common.unwrap_or_default() {
                crate::collections::push_unique(
                    &mut rows,
                    RetainedMembership {
                        place,
                        domain,
                        instance,
                    },
                );
            }
        }
    }
    rows
}

/// Replay the same source relation independently of the provisional output
/// fact. Exact call/operand and cohort authentication precede membership lookup.
pub(crate) fn replay_memberships(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    point: ProgramPoint,
) -> Vec<RetainedMembership> {
    let ProgramPoint::CallEnsures {
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    } = point
    else {
        return Vec::new();
    };
    let Some((machine, state)) =
        validation::machine_state_by_symbols(program, machine_symbol, state_symbol)
    else {
        return Vec::new();
    };
    let Some(recorded) =
        facts
            .borrow
            .call_at(machine_symbol, state_symbol, statement_index, call_ordinal)
    else {
        return Vec::new();
    };
    let borrowed = facts.borrow.calls.get(recorded);
    let Some(state_flow) = facts.flow.control.state(machine_symbol, state_symbol) else {
        return Vec::new();
    };
    let Some(call) = crate::flow::call_flows(&facts.flow.control, state_flow.calls)
        .iter()
        .find(|call| {
            call.statement_index == statement_index
                && call.call_ordinal == call_ordinal
                && call.target_symbol == borrowed.target_symbol
                && call.authored_expression == borrowed.authored_expression
        })
    else {
        return Vec::new();
    };
    let Some((operands, _)) =
        crate::flow::captured_call_operands(program, &facts.flow, state_flow, call)
    else {
        return Vec::new();
    };
    let Some(sources) = crate::flow::call_phases::ArgumentSources::new(
        program,
        machine_symbol,
        state_symbol,
        borrowed,
        Some(&[]),
        &facts.flow.control,
        operands,
    ) else {
        return Vec::new();
    };
    super::handbacks(
        program,
        &facts.borrow,
        &facts.flow.control,
        machine,
        state,
        borrowed,
        Some(&sources),
    )
    .iter()
    .flat_map(|returned| {
        memberships(
            program,
            &facts.semantic,
            &facts.flow.contexts,
            &facts.flow.control,
            returned,
        )
    })
    .collect()
}

fn aliases(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    returned: &RetainedBorrowHandback,
    supply: &RetainedBorrowSupply,
    path: &[PlaceSegment],
) -> Vec<CanonicalPlace> {
    let mut active = Vec::new();
    observation_aliases(
        program,
        control,
        supply.operand.storage_snapshot,
        &returned.place,
        returned.reference,
        path,
        &mut active,
    )
}

type AliasQuery = (CanonicalPlace, TypeReferenceHandle, Vec<PlaceSegment>);

/// Cross only actual stored reference bindings at the original immutable cut.
/// The typed projection owner decides fields/cases/indices. No initializer,
/// current descriptor, lifetime root or pointer address supplies an alias.
fn observation_aliases(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    snapshot: Handle<FlowStorageSnapshot>,
    root: &CanonicalPlace,
    reference: TypeReferenceHandle,
    path: &[PlaceSegment],
    active: &mut Vec<AliasQuery>,
) -> Vec<CanonicalPlace> {
    let key = (root.clone(), reference, path.to_vec());
    if active.contains(&key) {
        return Vec::new();
    }
    active.push(key);
    let mut output = crate::flow::project_captured_view_place(root.clone(), path)
        .into_iter()
        .collect::<Vec<_>>();
    for prefix_length in 0..=path.len() {
        let Some(selected) = validation::project_type_reference_from_segments(
            program,
            reference,
            &path[..prefix_length],
        ) else {
            continue;
        };
        let Some(shell) = validation::reference_shell(program, selected) else {
            continue;
        };
        if !shell.access.is_readable() {
            continue;
        }
        let Some(slot) =
            crate::flow::project_captured_view_place(root.clone(), &path[..prefix_length])
        else {
            continue;
        };
        let Some(stored) = validation::completed_storage::at_snapshot(
            program,
            control,
            snapshot,
            &slot,
            StoredObservation::Binding,
        ) else {
            continue;
        };
        let Some(captures) = stored.captures().filter(|captures| !captures.is_empty()) else {
            continue;
        };
        let mut common: Option<Vec<CanonicalPlace>> = None;
        for capture in captures {
            if !control.operands.is_valid(capture.capture) {
                common = Some(Vec::new());
                break;
            }
            let source = control.operands.get(capture.capture);
            let places = validation::project_type_reference_from_segments(
                program,
                capture.reference,
                &capture.projection,
            )
            .and_then(|selected| validation::reference_shell(program, selected))
            .filter(|actual| {
                let identity = program.normalized_type_identity(actual.referee);
                actual.access.is_readable()
                    && identity.is_valid()
                    && identity == program.normalized_type_identity(shell.referee)
            })
            .and_then(|_| {
                crate::flow::operand_values::captured_operand_reference_places_with_formation(
                    program,
                    control,
                    source,
                    capture.reference,
                    &capture.projection,
                )
            });
            let candidates = match places {
                Some(places) if !places.is_empty() => {
                    let mut candidates: Option<Vec<CanonicalPlace>> = None;
                    for place in places {
                        let aliases = observation_aliases(
                            program,
                            control,
                            snapshot,
                            &place.place,
                            shell.referee,
                            &path[prefix_length..],
                            active,
                        );
                        match &mut candidates {
                            Some(common) => common.retain(|place| aliases.contains(place)),
                            None => candidates = Some(aliases),
                        }
                    }
                    candidates.unwrap_or_default()
                }
                _ => Vec::new(),
            };
            match &mut common {
                Some(common) => common.retain(|place| candidates.contains(place)),
                None => common = Some(candidates),
            }
        }
        for alias in common.unwrap_or_default() {
            crate::collections::push_unique(&mut output, alias);
        }
    }
    active.pop();
    output
}
