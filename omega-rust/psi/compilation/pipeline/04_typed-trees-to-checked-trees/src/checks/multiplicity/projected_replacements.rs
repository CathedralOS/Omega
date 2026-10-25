//! Replacement of a captured owned subtree after RHS evaluation has joined.
//!
//! Source multiplicity and stored descriptor cleanup are independent: copying
//! a shared reference creates no affine source claim, but replacing its slot
//! still ends that exact runtime carrier. Publish its current custody here,
//! beside ordinary affine replacement, rather than reviving an entry loan or
//! guessing which RHS spelling last wrote the slot. Cleanup consumers reuse the
//! captured destination and retain the containing home.

use super::cleanup_dispositions::RemainingCustody;
use super::linear_obligations::LinearPlace;
use super::linear_obligations::type_reference_is_reference;
use crate::checks::type_multiplicity;
use checked_trees::{
    CheckFacts, ContextualCleanupDisposition, ContextualCleanupFacts, FlowPermissionEventFact,
};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceNode;

/// A projected store replaces one owned subtree, independently of
/// whether that subtree was extracted earlier. It does not establish or
/// consume the containing root's linear/affine claim.
#[allow(clippy::too_many_arguments)]
pub(super) fn append_replacement(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    statement: &typed_trees::statement::StatementNode,
    places: &[LinearPlace],
    remaining: &super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
    cleanup: &mut ContextualCleanupFacts,
) -> Result<Vec<super::cleanup_dispositions::StorageEstablishment>, diagnostics::Diagnostic> {
    let Some(assignment) = statement.as_assignment() else {
        return Ok(Vec::new());
    };
    let Some(place) = validation::canonical_place_from_expression_in_state(
        program,
        state,
        statement_index,
        assignment.target,
    ) else {
        return Ok(Vec::new());
    };
    // A range selects several owned subtrees, not one array-typed subtree.
    // Publish each selected occurrence through the ordinary custody owner so
    // its exclusions, nominal deaths and re-establishment stay authoritative.
    let destinations = if let Some(indexed) =
        program.expression_table.indexed_node(assignment.target)
        && program.expression_table.range_node(indexed.index).is_some()
    {
        let (Some(machine_definition), Some(state_definition)) = (
            program.machine_by_symbol(machine),
            program.state_by_symbol(state),
        ) else {
            return Ok(Vec::new());
        };
        let mut bounds = validation::ImmutableBoundLookup::new(program);
        let Some(shape) = validation::array_window_shape(
            program,
            machine_definition,
            state_definition,
            assignment.target,
            &mut bounds,
        ) else {
            return Ok(Vec::new());
        };
        let (Some(start), Some(end), Some(collection)) = (
            shape.start,
            shape.end,
            validation::canonical_place_from_expression_in_state(
                program,
                state,
                statement_index,
                indexed.collection,
            ),
        ) else {
            return Ok(Vec::new());
        };
        let Some(collection_type) = validation::canonical_place_type_reference(
            program,
            state,
            statement_index,
            &collection,
        )
        .and_then(|reference| validation::unwrapped_type_reference(program, reference)) else {
            return Ok(Vec::new());
        };
        let TypeReferenceNode::FixedArray {
            length: typed_trees::types::FixedArrayLength::Literal(length),
            ..
        } = program.type_reference_table.type_reference(collection_type)
        else {
            return Ok(Vec::new());
        };
        // Bounds validation owns the diagnostic. Invalid geometry creates no
        // death events and cannot cause unbounded enumeration outside storage.
        if start > end || end > *length {
            return Ok(Vec::new());
        }
        (start..end)
            .map(|index| {
                let mut segments = collection.segments.clone();
                segments.push(facts::PlaceSegment::FixedIndex { index });
                validation::CanonicalPlace {
                    root: collection.root,
                    segments,
                }
            })
            .collect()
    } else {
        vec![place]
    };
    Ok(destinations
        .into_iter()
        .map(|place| {
            append_place_replacement(
                program,
                facts,
                machine,
                state,
                statement_index,
                place,
                places,
                remaining,
                permissions,
                cleanup,
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn append_place_replacement(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    place: validation::CanonicalPlace,
    places: &[LinearPlace],
    remaining: &super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
    cleanup: &mut ContextualCleanupFacts,
) -> Result<Option<super::cleanup_dispositions::StorageEstablishment>, diagnostics::Diagnostic> {
    let Some(symbol) = place.root.as_symbol() else {
        return Ok(None);
    };
    // This occurrence replaces the captured destination, not a statically
    // named claim. Runtime selectors retain that same path in the death and
    // establishment events; cleanup later uses the assignment's captured
    // coordinates rather than evaluating the selectors again. Ownership,
    // reference crossings and residual custody below decide eligibility.
    let owner = places.iter().find(|owner| {
        owner.root == facts::PlaceRoot::Symbol(symbol)
            && owner.path.is_empty()
            && (owner.live || remaining.has_storage_frontier(place.root))
            && owner.multiplicity == Multiplicity::Affine
    });
    // Strict prefixes own the selected slot. The endpoint may itself be a
    // stored descriptor; crossing it instead would write someone else's value.
    for length in 0..place.segments.len() {
        let prefix = validation::CanonicalPlace {
            root: place.root,
            segments: place.segments[..length].to_vec(),
        };
        let Some(reference) =
            validation::canonical_place_type_reference(program, state, statement_index, &prefix)
        else {
            return Ok(None);
        };
        if type_reference_is_reference(program, reference) {
            return Ok(None);
        }
    }
    let Some(endpoint) =
        validation::canonical_place_type_reference(program, state, statement_index, &place)
    else {
        return Ok(None);
    };
    let descriptor =
        validation::reference_result_custody::stored_loan_parts(program, endpoint).is_some();
    let affine = type_multiplicity(program, endpoint) == Multiplicity::Affine && !descriptor;
    if descriptor && place.segments.is_empty() {
        return Ok(None);
    }
    if descriptor {
        if append_descriptor_death(
            program,
            facts,
            cleanup,
            machine,
            state,
            statement_index,
            &place,
            endpoint,
            remaining,
        )
        .is_none()
        {
            return Ok(None);
        }
    } else if (owner.is_none() && !remaining.has_storage_frontier(place.root))
        || type_reference_is_reference(program, endpoint)
        || type_multiplicity(program, endpoint) == Multiplicity::Linear
    {
        return Ok(None);
    }
    if !affine && !descriptor && !remaining.has_storage_frontier(place.root) {
        // Copy-field stores carry no disposal event in a complete value.
        // They matter here only as establishment of reserved construction
        // storage; ordinary scalar write semantics stay with their owner.
        return Ok(None);
    }
    let discarded = if affine {
        remaining.disposal_paths(program, place.root, &place.segments)
    } else {
        Vec::new()
    };
    let source = language_semantics::PermissionEventSource::Statement { statement_index };
    let segments = facts
        .flow
        .ownership
        .segments
        .insert_many(place.segments.iter().copied());
    for path in discarded {
        let discarded = validation::CanonicalPlace {
            root: place.root,
            segments: path,
        };
        let Some(reference) =
            validation::canonical_place_type_reference(program, state, statement_index, &discarded)
        else {
            return Err(diagnostics::Diagnostic::error(
                "replacement cleanup has no exact discarded type",
            ));
        };
        // The ordinary cleanup owner retains exact live claims, nominal hooks,
        // and excluded subtrees. A receiving overwrite cannot replace that
        // authority with a claim count or a structural partition guess.
        if !super::linear_validation::append_owned_region_cleanup_permission_events(
            program,
            reference,
            discarded.root,
            &discarded.segments,
            places,
            remaining,
            FlowPermissionEventFact {
                expression: Default::default(),
                evaluation: checked_trees::FlowPermissionEvaluation::Statement,
                machine_symbol: machine,
                state_symbol: state,
                source,
                kind: PermissionEventKind::AffineDrop,
                multiplicity: Multiplicity::Affine,
                access: PermissionAccess::Owned,
                claim_identity: PermissionClaimIdentity::Unknown,
                provenance: PermissionProvenance::Unknown,
                root: place.root,
                segments: Default::default(),
                obligation_live: false,
            },
            &mut facts.flow.ownership.segments,
            permissions,
        ) {
            return Err(diagnostics::Diagnostic::error(
                "replacement cannot dispose its remaining owned region",
            ));
        }
    }
    // Return the same receiving occurrence to the cleanup owner. Projected
    // affine regions do not have their own tracked linear-claim row.
    let establishment = super::cleanup_dispositions::StorageEstablishment {
        event_ordinal: permissions.len(),
        destination: place,
    };
    permissions.push(FlowPermissionEventFact {
        expression: Default::default(),
        evaluation: checked_trees::FlowPermissionEvaluation::Statement,
        machine_symbol: machine,
        state_symbol: state,
        source,
        kind: PermissionEventKind::Establish,
        multiplicity: if affine {
            Multiplicity::Affine
        } else {
            Multiplicity::Unrestricted
        },
        access: PermissionAccess::Owned,
        claim_identity: PermissionClaimIdentity::Unknown,
        provenance: PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: state,
            source,
        },
        root: establishment.destination.root,
        segments,
        obligation_live: false,
    });
    Ok(Some(establishment))
}

/// A stored loan's death ends a carrier, not an affine source obligation.
/// The current custody is already joined after all RHS calls. In particular,
/// an exclusive call may have replaced the ingress descriptor; its old loan
/// identity is neither needed nor valid authority for this slot's current death.
#[allow(clippy::too_many_arguments)]
fn append_descriptor_death(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    cleanup: &mut ContextualCleanupFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    endpoint: typed_trees::types::TypeReferenceHandle,
    remaining: &RemainingCustody,
) -> Option<()> {
    let declaration = program.state_by_symbol(state)?;
    let symbol = place.root.as_symbol()?;
    if let Some(parameter) = program.state_parameter_by_symbol(declaration, symbol) {
        if !validation::reference_result_custody::stored_carrier_ingress_is_complete(
            program,
            facts,
            machine,
            state,
            symbol,
            parameter.type_reference,
        ) {
            return None;
        }
    } else {
        let (ordinal, local) = program.state_local_declaration(declaration, symbol)?;
        if ordinal >= statement_index
            || (!local.initial_value.is_valid() && !remaining.has_storage_frontier(place.root))
        {
            return None;
        }
    }
    let assignment = program
        .statement_table
        .statement_at(declaration, statement_index)?
        .as_assignment()?;
    let identity = validation::stored_structural_type_identity(program, endpoint, &[], &[]);
    if !identity.is_valid()
        || identity
            != validation::stored_structural_type_identity(
                program,
                assignment.receiving_type,
                &[],
                &[],
            )
    {
        return None;
    }
    if !remaining
        .removed_paths(program, place.root, &place.segments)
        .is_empty()
    {
        return None;
    }
    let mut disposition = ContextualCleanupDisposition {
        machine,
        state,
        source: language_semantics::PermissionEventSource::Statement { statement_index },
        root: place.root,
        disposed_type: endpoint,
        path: facts
            .flow
            .ownership
            .segments
            .insert_many(place.segments.iter().copied()),
        ..Default::default()
    };
    // Absence of an exclusion does not establish a sum payload. Prove every
    // containing tag at the actual pre-store point, not at statement entry.
    {
        let evidence = crate::flow::CleanupCaseEvidence::new(program, facts, &disposition)?;
        if evidence.reachability(&place.segments)? != validation::CleanupCaseReachability::Selected
        {
            return None;
        }
        // Canonical selector identity is not disjointness: entries[i] may be
        // the selected entries[j]. Authenticate exclusions before either a
        // descriptor death OR a vacancy establishment can be published.
        for excluded in remaining.removed_paths(program, place.root, &[]) {
            if place.segments.starts_with(excluded) || excluded.starts_with(&place.segments) {
                return None;
            }
            if crate::checks::storage_path_compatibility(
                program,
                crate::checks::StorageSelectorFacts {
                    flow: &facts.flow,
                    semantic: &facts.semantic,
                },
                &place.segments,
                excluded,
            )
            .0 && !facts
                .flow
                .control
                .state(machine, state)
                .zip(evidence.contexts().as_ref())
                .is_some_and(|(state, contexts)| {
                    crate::checks::captured_storage_paths_are_disjoint(
                        program,
                        facts,
                        state,
                        contexts,
                        &place.segments,
                        excluded,
                    )
                })
            {
                return None;
            }
        }
    }
    // Only an actual construction vacancy skips disposal. A moved ancestor or
    // an inactive payload cannot be turned into receiving-storage authority.
    if !remaining.is_present(place.root, &place.segments) {
        return remaining
            .is_construction_vacancy(place.root, &place.segments)
            .then_some(());
    }
    let exclusions = remaining
        .removed_regions(program, place.root, &[])
        .into_iter()
        .map(
            |(path, cause)| checked_trees::ContextualCleanupExcludedRegion {
                root: place.root,
                segments: cleanup.segments.insert_many(path.iter().copied()),
                cause,
            },
        )
        .collect::<Vec<_>>();
    let inactive_cases = remaining
        .inactive_case_paths(place.root, &[])
        .into_iter()
        .map(|path| checked_trees::ContextualCleanupExcludedRegion {
            root: place.root,
            segments: cleanup.segments.insert_many(path.iter().copied()),
            cause: checked_trees::ContextualCleanupExclusionCause::InactiveCase,
        })
        .collect::<Vec<_>>();
    disposition.exclusions = cleanup.exclusions.insert_many(exclusions);
    disposition.inactive_cases = cleanup.exclusions.insert_many(inactive_cases);
    cleanup.dispositions.append(disposition);
    Some(())
}
