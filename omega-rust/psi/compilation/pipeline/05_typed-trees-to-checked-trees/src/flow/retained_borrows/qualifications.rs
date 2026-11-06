//! A consumed holder can still owe guarantees to its original lenders.
//! Collect those old obligations from every captured input. Their earlier
//! validity and complete borrower closure do not prove validity after writes;
//! the call preservation owner must establish that separately.

use super::{RetainedBorrowHandback, RetainedBorrowSupply};
use arena::Handle;
use checked_trees::{FlowContextFacts, FlowControlFacts, FlowStorageSnapshot};
use facts::{FactPlan, PlaceSegment};
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;
use validation::{CanonicalPlace, completed_storage::StoredObservation};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RetainedDuty {
    pub place: CanonicalPlace,
    pub domain: SymbolHandle,
    pub instance: SemanticDomainId,
}

/// Every original supplier must agree on the declared obligation and subject.
/// This selects duties for the write-preservation judgment, never current
/// membership facts. In particular, being able to prove a predicate from a
/// value does not mean that the callee left this value satisfying it.
pub(in crate::flow) fn declared_duties(
    program: &dyn validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &FlowContextFacts,
    control: &FlowControlFacts,
    returned: &RetainedBorrowHandback,
) -> Vec<RetainedDuty> {
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
            let lender_domains = validation::canonical_owned_place_type_reference(
                program,
                supply.operand.state_symbol,
                supply.operand.statement_index,
                &returned.place,
            )
            .map(|reference| {
                validation::field_domains::declared_type_domain_rows(program, reference)
                    .into_iter()
                    .filter(|row| !row.transfer && row.path.is_empty())
                    .map(|row| (row.domain_symbol, row.semantic_domain))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
            (supply, selected, evidence, lender_domains)
        })
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    // The returned type is already the selected referent. Its own root
    // qualification is a duty just like qualifications on its children.
    for row in validation::field_domains::declared_type_domain_rows(program, returned.reference)
        .into_iter()
        .filter(|row| !row.transfer)
    {
        let (path, domain, instance) = (row.path, row.domain_symbol, row.semantic_domain);
        if !instance.is_valid()
            || !path
                .iter()
                .all(crate::flow::place_segment_has_static_identity)
        {
            continue;
        }
        // Lending a declared qualified value preserves its owner's duty. An
        // extra qualification on the helper's input can expire instead, so
        // its formal type alone cannot restore the root's membership.
        if path.is_empty()
            && !supplies
                .iter()
                .all(|(_, _, _, lender)| lender.contains(&(domain, instance)))
        {
            continue;
        }
        let mut common: Option<Vec<CanonicalPlace>> = None;
        let supplied = supplies
            .iter()
            .all(|(supply, source_contexts, evidence, _)| {
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
                crate::shared::collections::push_unique(
                    &mut rows,
                    RetainedDuty {
                        place,
                        domain,
                        instance,
                    },
                );
            }
        }
    }
    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
        let supplies = returned
            .supplies
            .iter()
            .map(|supply| {
                (
                    supply.operand.state_symbol,
                    supply.operand.statement_index,
                    supply.operand.expression,
                    supply.operand.type_reference,
                    supply.operand.storage_snapshot,
                )
            })
            .collect::<Vec<_>>();
        eprintln!(
            "CHECKSITE retained-handback-duties subject={:?} reference={:?} supplies={supplies:?} rows={rows:?}",
            returned.place, returned.reference
        );
    }
    rows
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
            crate::shared::collections::push_unique(&mut output, alias);
        }
    }
    active.pop();
    output
}
