//! Bind selected result paths to the occurrences actually transferred.
//!
//! The ordinary evaluator owns effect order and captures each source before
//! its move on that arm. Static leaf discovery supplies result geometry, not
//! availability. This receipt joins those arrivals without borrowing an old
//! declaration's provenance or executing predicates a second time.

use super::{leaves::*, operands::*};
use super::{place_paths_overlap, source_needs_loan_closure, stored_contents, unsupported};
use crate::checks::multiplicity::linear_obligations::LinearPlace;
use arena::Handle;
use checked_trees::CheckFacts;
use checked_trees::{
    FlowOwnedSelectionClaim, FlowOwnedSelectionReceipt, FlowOwnedSelectionSource,
    FlowOwnedSelectionTransfer,
};
use diagnostics::Diagnostic;
use language_semantics::Multiplicity;
use language_semantics::PermissionClaimIdentity;
use language_semantics::PermissionEventSource;
use language_semantics::PermissionProvenance;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

pub(in crate::checks::multiplicity) fn record_statement(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    places: &mut [LinearPlace],
    evaluated_observations: &[super::super::linear_validation::ObservedCustody],
    claim_identities: &mut super::super::linear_obligations::ClaimIdentityAllocator,
) -> Result<bool, Diagnostic> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let (expression, destination, type_reference) = match statement {
        StatementNode::LocalData(local) if !local.is_mutable => {
            (local.initial_value, local.symbol, local.type_reference)
        }
        StatementNode::Expression(expression) if statement_index + 1 == statements.len() => {
            (*expression, SymbolHandle::invalid(), state.return_type)
        }
        _ => return Ok(false),
    };
    // This receipt binds the receiving owner's result selection. Operand
    // selections belong to the shared occurrence schedule: treating a nested
    // match as this destination would give it the enclosing call's result
    // type and skip that call's other ownership events.
    if !expression.is_valid()
        || !matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Match(_)
        )
        || !has_owned_leaf(program, places, machine, state, statement_index, expression)
    {
        return Ok(false);
    }
    let linear_result = program.type_multiplicity(type_reference) == Multiplicity::Linear;
    // An affine destination may carry linear claims: the join then settles the
    // consumed source claims at the exact destination frontier paths rather
    // than inventing a root claim. The carrier still owes the same finite
    // owned walls as a plain record. Shared-view members move their descriptor
    // storage, while loan checking retains their backing and lifetime relation.
    let destination_ok = matches!(
        validation::reference_result_custody::binding_multiplicity(program, type_reference),
        Multiplicity::Affine | Multiplicity::Linear
    ) && stored_contents(program, type_reference);
    if !destination_ok {
        return Err(unsupported());
    }
    // The source destination owns one carrier on every selected path. Nested
    // selections recurse into their own authored arms, without expanding paths.
    let mut leaves = Vec::new();
    collect_leaves(
        program,
        places,
        machine,
        state,
        statement_index,
        expression,
        type_reference,
        Handle::invalid(),
        &mut leaves,
    )?;
    if leaves.is_empty() {
        return Err(unsupported());
    }
    // Two leaves on the same arm over one root name places the arm's edge
    // moves independently; overlapping paths consume the same owned storage
    // twice. Distinct arms are separate edges and may each reach the same
    // path.
    for (index, (_, arm, root, path, _)) in leaves.iter().enumerate() {
        for (_, other_arm, other_root, other_path, _) in &leaves[index + 1..] {
            if arm == other_arm && root == other_root && place_paths_overlap(path, other_path) {
                return Err(Diagnostic::error(
                    "owned match record fields must name disjoint moved places; overlapping paths on one arm consume the same owned storage twice",
                ));
            }
        }
    }
    // A linear join may not drop custody: every predecessor edge must agree on
    // the live ownership frontier, so every reachable arm has to move the same
    // tracked place into the result. Distinct sources, a source on only some
    // arms, or different projected paths under one root leave an obligation
    // live on one edge and dead on another, which the join cannot reconcile.
    // Fresh per-edge products carry no tracked place and join vacuously.
    if linear_result {
        let terminals = terminal_arm_values(program, expression);
        let mut consumed: Vec<(SymbolHandle, Vec<facts::PlaceSegment>)> = Vec::new();
        let mut symbol_leaves = 0usize;
        for (_, _, root, path, _) in &leaves {
            let Some(symbol) = root.as_symbol() else {
                continue;
            };
            symbol_leaves += 1;
            if !consumed
                .iter()
                .any(|(candidate, candidate_path)| *candidate == symbol && *candidate_path == *path)
            {
                consumed.push((symbol, path.clone()));
            }
        }
        if consumed.len() > 1 || (!consumed.is_empty() && symbol_leaves != terminals) {
            return Err(Diagnostic::error(
                "linear match custody requires every reachable arm to move the same live source place; a distinct source, a fresh arm, or a different projected path leaves an obligation live on only some edges",
            ));
        }
    }
    // Every arm-value leaf carries the result type, while a record-field leaf
    // carries its own field type: the consumed claim frontier is per leaf. A
    // whole affine carrier contributes its complete linear claim frontier, a
    // linear leaf contributes itself, and a plain leaf contributes nothing.
    let leaf_frontier = |leaf_type: TypeReferenceHandle| {
        crate::checks::multiplicity::linear_validation::linear_claim_frontier(program, leaf_type)
    };
    // A record's affine shell does not make its contained claims discardable.
    // Each exact consumed claim must move on every reachable edge, irrespective
    // of which result field receives it. Swaps preserve that set; omitting a
    // source on one arm does not. Duplicate moves on one arm were rejected above.
    let terminal_count = terminal_arm_values(program, expression);
    let mut claim_edges = Vec::new();
    for (_, arm, root, path, leaf_type) in &leaves {
        for claim in leaf_frontier(*leaf_type) {
            let mut claim_path = path.clone();
            claim_path.extend(claim.path);
            if !claim_edges
                .iter()
                .any(|(candidate_root, candidate_path, candidate_arm)| {
                    candidate_root == root && candidate_path == &claim_path && candidate_arm == arm
                })
            {
                claim_edges.push((*root, claim_path, *arm));
            }
        }
    }
    for (root, path, _) in &claim_edges {
        if claim_edges
            .iter()
            .filter(|(candidate_root, candidate_path, _)| {
                candidate_root == root && candidate_path == path
            })
            .count()
            != terminal_count
        {
            return Err(Diagnostic::error(
                "linear match custody requires every reachable arm to move the same live source claims; a missing move leaves an obligation live on only some edges",
            ));
        }
    }
    let mut sources = Vec::new();
    let mut source_arrivals = Vec::new();
    for (leaf_expression, source_arm, root, leaf_path, leaf_type) in &leaves {
        // A call's structural product roots its own once-evaluated custody;
        // only whole-local roots carry a roster source.
        let Some(symbol) = root.as_symbol() else {
            continue;
        };
        let places =
            evaluated_source_places(*leaf_expression, *root, leaf_path, evaluated_observations)?;
        // The exact claims this source's leaves consume, in canonical frontier
        // order: each claim path is the leaf's moved path extended by the
        // claim's position below the leaf. Distinct claim paths can never
        // overlap inside one frontier, so overlapping-but-distinct entries
        // here mean the leaf set disagrees with itself.
        let mut consumed_paths: Vec<Vec<facts::PlaceSegment>> = Vec::new();
        {
            for template in &leaf_frontier(*leaf_type) {
                let mut claim_path = leaf_path.clone();
                claim_path.extend_from_slice(&template.path);
                if consumed_paths.contains(&claim_path) {
                    continue;
                }
                if consumed_paths
                    .iter()
                    .any(|existing| place_paths_overlap(existing, &claim_path))
                {
                    return Err(Diagnostic::error(
                        "owned match transfers must name disjoint exact claim places",
                    ));
                }
                consumed_paths.push(claim_path);
            }
        }
        // A source is established storage in the current permission context.
        // Mutability grants reassignment, not copying or weaker move rules.
        // Parameters keep their authored position as the source ordinal;
        // provenance comes from the actual evaluated occurrence below.
        // A carrier source has no tracked root place: its linear children
        // are the custody roster. Their provenance is the single
        // establishment event the declaration's write recorded, and any
        // disagreement means the set was not established by one write.
        let provenance = if consumed_paths.is_empty() {
            places
                .iter()
                .find(|place| {
                    place.root == facts::PlaceRoot::Symbol(symbol) && place.path.is_empty()
                })
                .ok_or_else(unsupported)?
                .provenance
        } else {
            let provenances = consumed_paths
                .iter()
                .map(|claim_path| {
                    places
                        .iter()
                        .find(|place| {
                            place.root == facts::PlaceRoot::Symbol(symbol)
                                && place.path == *claim_path
                        })
                        .and_then(|place| place.provenance)
                        .unwrap_or(PermissionProvenance::Unknown)
                })
                .collect::<Vec<_>>();
            if provenances
                .iter()
                .any(|provenance| *provenance != provenances[0])
            {
                return Err(Diagnostic::error(
                    "owned match source claims must share one establishment provenance",
                ));
            }
            Some(provenances[0])
        };
        let parameter = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == symbol);
        let (source_ordinal, source_reference, provenance, origin_selection) = if let Some((
            position,
            parameter,
        )) = parameter
        {
            if parameter.is_const {
                return Err(Diagnostic::error(
                    "owned match source must be an available whole plain-affine local of the exact result type",
                ));
            }
            (
                position,
                parameter.type_reference,
                provenance.unwrap_or(PermissionProvenance::Unknown),
                Handle::invalid(),
            )
        } else {
            let (source_ordinal, source_local) = program
                .state_local_declaration(state, symbol)
                .filter(|&(ordinal, _)| ordinal < statement_index)
                .ok_or_else(unsupported)?;
            let origin_selection = facts
                .flow
                .ownership
                .owned_selection_at(state.symbol, source_ordinal as u32)
                .filter(|(_, receipt)| {
                    receipt.destination == symbol
                        && if consumed_paths.is_empty() {
                            provenance
                                == Some(PermissionProvenance::Established {
                                    machine_symbol: machine.symbol,
                                    state_symbol: state.symbol,
                                    source: PermissionEventSource::Statement {
                                        statement_index: source_ordinal,
                                    },
                                })
                        } else {
                            consumed_paths.iter().all(|path| {
                                places
                                    .iter()
                                    .find(|place| place.root == *root && place.path == *path)
                                    .is_some_and(|place| {
                                        facts
                                            .flow
                                            .ownership
                                            .selection_result_claims
                                            .span_or_empty(receipt.result_claims)
                                            .iter()
                                            .any(|claim| {
                                                facts
                                                    .flow
                                                    .ownership
                                                    .segments
                                                    .span_or_empty(claim.path)
                                                    == *path
                                                    && Some(claim.claim_identity)
                                                        == place.claim_identity
                                                    && Some(claim.provenance) == place.provenance
                                            })
                                    })
                            })
                        }
                })
                .map(|(handle, _)| handle)
                .unwrap_or_default();
            (
                source_ordinal,
                source_local.type_reference,
                provenance.unwrap_or(PermissionProvenance::Unknown),
                origin_selection,
            )
        };
        // The roster still roots at the whole place symbol, but the exact
        // claims the transfer consumes decide availability: a whole affine
        // carrier discharges its complete frontier, a linear leaf moves the
        // claim at its own path, and a claim-free leaf answers with its root
        // place. Every consumed claim must be a live, established,
        // unconditional linear place.
        let root_place = places
            .iter()
            .find(|place| place.root == facts::PlaceRoot::Symbol(symbol) && place.path.is_empty());
        let mut consumed = Vec::new();
        for claim_path in &consumed_paths {
            let place = places
                .iter()
                .find(|place| {
                    place.root == facts::PlaceRoot::Symbol(symbol) && place.path == *claim_path
                })
                .ok_or_else(unsupported)?;
            if !place.live
                || !place.ever_established
                || place.conditional
                || place.multiplicity != Multiplicity::Linear
            {
                return Err(Diagnostic::error(
                    "owned match source must be an available owned local or parameter claim of the exact result type",
                ));
            }
            consumed.push(place);
        }
        let source_ok = if consumed.is_empty() {
            root_place.is_some_and(|place| place.multiplicity == Multiplicity::Affine)
        } else {
            true
        } && stored_contents(program, source_reference);
        if !source_ok {
            return Err(Diagnostic::error(
                "owned match source must be an available owned local or parameter claim of the exact result type",
            ));
        }
        if consumed.is_empty() {
            let place = root_place.ok_or_else(unsupported)?;
            if !place.live || !place.ever_established || place.conditional {
                return Err(Diagnostic::error(
                    "owned match source must be an available owned local or parameter claim of the exact result type",
                ));
            }
        }
        // A whole source must match the receiving leaf's type, not necessarily
        // the enclosing selection's type: construction may move it beneath a
        // field in a different carrier. Projected leaves already checked their
        // exact moved path's type when the leaf roster was collected.
        if leaves.iter().any(|(_, _, leaf_root, path, leaf_type)| {
            *leaf_root == facts::PlaceRoot::Symbol(symbol)
                && path.is_empty()
                && program.normalized_type_identity(source_reference)
                    != program.normalized_type_identity(*leaf_type)
        }) {
            return Err(Diagnostic::error(
                "owned match source must be an available whole plain-affine local of the exact result type",
            ));
        }
        // A source whose custody was borrowed joins the selection once every
        // recorded loan on its root has already closed. A loan still live at
        // this edge would need per-edge closure evidence the receipt cannot
        // express, and a borrow the ledger never recorded cannot vouch for
        // closure either, so both keep the conservative rejection.
        if source_needs_loan_closure(
            program,
            facts,
            machine,
            state,
            statement_index,
            &statements[..statement_index],
            symbol,
        ) {
            return Err(Diagnostic::error(
                "owned match source with borrowed custody requires selected loan-closure evidence",
            ));
        }
        if !origin_selection.is_valid() && provenance == PermissionProvenance::Unknown {
            return Err(Diagnostic::error(
                "owned match source has no exact incoming origin",
            ));
        }
        // The transfer names one claim identity only when its consumed set
        // consumes exactly one; a whole carrier's claim set has no single
        // identity, and the per-claim rows on each transfer carry the exact
        // consumed evidence instead.
        let claim_identity = if let [claim] = consumed.as_slice() {
            claim
                .claim_identity
                .unwrap_or(PermissionClaimIdentity::Unknown)
        } else {
            PermissionClaimIdentity::Unknown
        };
        let source = FlowOwnedSelectionSource {
            symbol,
            statement_ordinal: u32::try_from(source_ordinal).map_err(|_| unsupported())?,
            retains_storage: true,
        };
        let source_ordinal =
            if let Some(ordinal) = sources.iter().position(|candidate| *candidate == source) {
                ordinal
            } else {
                let ordinal = sources.len();
                sources.push(source);
                ordinal
            };
        source_arrivals.push((
            *leaf_expression,
            *source_arm,
            *root,
            leaf_path.clone(),
            source_ordinal,
            provenance,
            claim_identity,
            origin_selection,
        ));
    }
    // Every terminal edge moving the whole current occupant leaves no skipped
    // carrier in this home. Predicate effects may have installed different
    // occupants: their distinct claim evidence remains on the transfers, not
    // this physical-home roster.
    for (source_ordinal, source) in sources.iter_mut().enumerate() {
        let mut whole_arms = Vec::new();
        for (_, arm, _, path, arrival, _, _, _) in &source_arrivals {
            if *arrival == source_ordinal && path.is_empty() && !whole_arms.contains(arm) {
                whole_arms.push(*arm);
            }
        }
        source.retains_storage = whole_arms.len() != terminal_count;
    }
    // Reverse establishment order: locals in descending statement order, then
    // parameters in descending authored position, since parameters are
    // established at state entry before every statement local.
    let mut sources = sources.into_iter().enumerate().collect::<Vec<_>>();
    sources.sort_by_key(|(_, source)| {
        let is_local = program
            .state_parameters(state)
            .iter()
            .all(|parameter| parameter.symbol != source.symbol);
        std::cmp::Reverse((is_local, source.statement_ordinal))
    });
    let source_ordinals = sources
        .iter()
        .map(|(ordinal, _)| *ordinal)
        .collect::<Vec<_>>();
    let sources = facts
        .flow
        .ownership
        .selection_sources
        .insert_many(sources.into_iter().map(|(_, source)| source));
    let mut transfers = Vec::new();
    let selection_expression = expression;
    for (expression, source_arm, root, path, leaf_type) in leaves {
        let result_path = super::results::operand_result_path(
            program,
            selection_expression,
            source_arm,
            expression,
            type_reference,
        )?;
        let (source, claims, provenance, claim_identity, origin_selection) = match root {
            facts::PlaceRoot::Symbol(symbol) => {
                let places =
                    evaluated_source_places(expression, root, &path, evaluated_observations)?;
                let (source_ordinal, provenance, claim_identity, origin_selection) =
                    source_arrivals
                        .iter()
                        .find_map(
                            |(
                                candidate,
                                arm,
                                candidate_root,
                                candidate_path,
                                ordinal,
                                provenance,
                                claim_identity,
                                origin_selection,
                            )| {
                                (*candidate == expression
                                    && *arm == source_arm
                                    && *candidate_root == root
                                    && *candidate_path == path)
                                    .then_some((
                                        *ordinal,
                                        *provenance,
                                        *claim_identity,
                                        *origin_selection,
                                    ))
                            },
                        )
                        .ok_or_else(|| {
                            Diagnostic::error("selected source lost its evaluated occurrence")
                        })?;
                let ordinal = source_ordinals
                    .iter()
                    .position(|candidate| *candidate == source_ordinal)
                    .ok_or_else(unsupported)?;
                let source = sources.handle_at(ordinal as u32);
                // Each leaf consumes the leaf type's whole linear frontier
                // under its own moved path: one row per claim, each naming
                // the consumed place's exact identity and provenance.
                let claims = leaf_frontier(leaf_type)
                    .iter()
                    .map(|template| {
                        let mut claim_path = path.clone();
                        claim_path.extend_from_slice(&template.path);
                        let place = places
                            .iter()
                            .find(|place| {
                                place.root == facts::PlaceRoot::Symbol(symbol)
                                    && place.path == claim_path
                            })
                            .ok_or_else(unsupported)?;
                        Ok(FlowOwnedSelectionClaim {
                            path: facts
                                .flow
                                .ownership
                                .segments
                                .insert_many(claim_path.iter().copied()),
                            claim_identity: place
                                .claim_identity
                                .unwrap_or(PermissionClaimIdentity::Unknown),
                            provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                (source, claims, provenance, claim_identity, origin_selection)
            }
            // A call-product root is identified by the transfer expression's
            // authored place, not a roster source.
            facts::PlaceRoot::Expression(root_expression)
                if matches!(
                    program.expression_table.expression(root_expression),
                    ExpressionNode::Call(_)
                ) =>
            {
                (
                    Handle::invalid(),
                    Vec::new(),
                    PermissionProvenance::Unknown,
                    PermissionClaimIdentity::Unknown,
                    Handle::invalid(),
                )
            }
            _ => return Err(unsupported()),
        };
        transfers.push(FlowOwnedSelectionTransfer {
            expression,
            source_arm,
            source,
            provenance,
            claim_identity,
            origin_selection,
            path: facts.flow.ownership.segments.insert_many(path),
            result_path: facts.flow.ownership.segments.insert_many(result_path),
            claims: facts
                .flow
                .ownership
                .selection_transfer_claims
                .insert_many(claims),
        });
    }
    let transfers = facts
        .flow
        .ownership
        .selection_transfers
        .insert_many(transfers);
    let mut receipt = FlowOwnedSelectionReceipt {
        machine: machine.symbol,
        state: state.symbol,
        statement_ordinal: u32::try_from(statement_index).map_err(|_| unsupported())?,
        expression,
        destination,
        type_reference,
        sources,
        transfers,
        result_claims: Default::default(),
        produced_claims: Default::default(),
        death: PermissionEventSource::StateExit,
    };
    receipt.produced_claims = super::results::record_productions(
        program,
        &mut facts.flow.ownership,
        &receipt,
        machine,
        state,
        places,
        claim_identities,
    )?;
    receipt.result_claims = super::results::bind_results(
        program,
        &mut facts.flow.ownership,
        &receipt,
        claim_identities,
    )?;
    facts.flow.ownership.owned_selections.append(receipt);
    Ok(true)
}

/// The evaluator captured this operand before its move or reborrow, after
/// preceding calls and conditional pattern effects on the same route. The
/// source spelling and the joined post-expression inventory cannot replace it.
fn evaluated_source_places<'evaluation>(
    expression: typed_trees::expression::ExpressionHandle,
    root: facts::PlaceRoot,
    path: &[facts::PlaceSegment],
    observations: &'evaluation [super::super::linear_validation::ObservedCustody],
) -> Result<&'evaluation [LinearPlace], Diagnostic> {
    let mut arrivals = observations.iter().filter(|observation| {
        observation.expression == expression
            && observation.place.root == root
            && observation.place.segments == path
    });
    let (Some(arrival), None) = (arrivals.next(), arrivals.next()) else {
        return Err(Diagnostic::error(
            "selected source has no unique evaluated operand arrival",
        ));
    };
    Ok(&arrival.places)
}
