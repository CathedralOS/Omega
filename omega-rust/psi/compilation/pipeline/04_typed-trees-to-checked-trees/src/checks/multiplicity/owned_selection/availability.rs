//! availability slice of owned_selection.

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
    for (_, _, root, _, _) in &leaves {
        // A call's structural product roots its own once-evaluated custody;
        // only whole-local roots carry a roster source.
        let Some(symbol) = root.as_symbol() else {
            continue;
        };
        if sources
            .iter()
            .any(|source: &FlowOwnedSelectionSource| source.symbol == symbol)
        {
            continue;
        }
        // The exact claims this source's leaves consume, in canonical frontier
        // order: each claim path is the leaf's moved path extended by the
        // claim's position below the leaf. Distinct claim paths can never
        // overlap inside one frontier, so overlapping-but-distinct entries
        // here mean the leaf set disagrees with itself.
        let mut consumed_paths: Vec<Vec<facts::PlaceSegment>> = Vec::new();
        for (_, _, leaf_root, leaf_path, leaf_type) in &leaves {
            if *leaf_root != facts::PlaceRoot::Symbol(symbol) {
                continue;
            }
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
        // Parameters carry their authored position as the
        // source ordinal and their state-entry establishment as provenance.
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
                PermissionProvenance::Established {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    source: PermissionEventSource::StateEntry,
                },
                Handle::invalid(),
            )
        } else {
            let (source_ordinal, source_local) = program
                .state_local_declaration(state, symbol)
                .filter(|&(ordinal, _)| ordinal < statement_index)
                .ok_or_else(unsupported)?;
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
            let origin_selection = facts
                .flow
                .ownership
                .owned_selection_at(state.symbol, source_ordinal as u32)
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
        // The source row names one claim identity only when the transfer set
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
        sources.push(FlowOwnedSelectionSource {
            symbol,
            statement_ordinal: u32::try_from(source_ordinal).map_err(|_| unsupported())?,
            retains_storage: leaves
                .iter()
                .filter(|(_, _, root, path, _)| {
                    *root == facts::PlaceRoot::Symbol(symbol) && path.is_empty()
                })
                .count()
                != terminal_count,
            provenance,
            claim_identity,
            origin_selection,
        });
    }
    // Reverse establishment order: locals in descending statement order, then
    // parameters in descending authored position, since parameters are
    // established at state entry before every statement local.
    sources.sort_by_key(|source| {
        let is_local = program
            .state_parameters(state)
            .iter()
            .all(|parameter| parameter.symbol != source.symbol);
        std::cmp::Reverse((is_local, source.statement_ordinal))
    });
    let sources = facts.flow.ownership.selection_sources.insert_many(sources);
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
        let (source, claims) = match root {
            facts::PlaceRoot::Symbol(symbol) => {
                let ordinal = facts
                    .flow
                    .ownership
                    .selection_sources
                    .span_or_empty(sources)
                    .iter()
                    .position(|source| source.symbol == symbol)
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
                (source, claims)
            }
            // A call-product root is identified by the transfer expression's
            // authored place, not a roster source.
            facts::PlaceRoot::Expression(root_expression)
                if matches!(
                    program.expression_table.expression(root_expression),
                    ExpressionNode::Call(_)
                ) =>
            {
                (Handle::invalid(), Vec::new())
            }
            _ => return Err(unsupported()),
        };
        transfers.push(FlowOwnedSelectionTransfer {
            expression,
            source_arm,
            source,
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
    apply_availability(&facts.flow.ownership, &receipt, places);
    facts.flow.ownership.owned_selections.append(receipt);
    Ok(true)
}

pub(in crate::checks::multiplicity) fn apply_availability(
    ownership: &checked_trees::FlowOwnershipFacts,
    receipt: &FlowOwnedSelectionReceipt,
    places: &mut [LinearPlace],
) {
    // `live` here is the all-path availability summary, not disposition. The
    // conditional complement remains owed in the receipt's source roster.
    for source in ownership.selection_sources.span_or_empty(receipt.sources) {
        if let Some(place) = places.iter_mut().find(|place| {
            place.root == facts::PlaceRoot::Symbol(source.symbol) && place.path.is_empty()
        }) {
            place.live = false;
        }
    }
    // Each transfer names the exact claims it consumes on its edge. Uniform
    // selection means every named claim is gone on every path, so the precise
    // `(symbol, path)` place is dead even when the source root survives as
    // residual custody — the affine root lookup above only covers whole-place
    // sources.
    for transfer in ownership
        .selection_transfers
        .span_or_empty(receipt.transfers)
    {
        if !transfer.source.is_valid() {
            continue;
        }
        let source = ownership.selection_sources.get(transfer.source);
        for claim in ownership
            .selection_transfer_claims
            .span_or_empty(transfer.claims)
        {
            let claim_path = ownership.segments.span_or_empty(claim.path);
            if let Some(place) = places.iter_mut().find(|place| {
                place.root == facts::PlaceRoot::Symbol(source.symbol) && place.path == claim_path
            }) {
                place.live = false;
            }
        }
    }
    // Bind by destination path, never source catalogue position. An invariant
    // source keeps its name; differing arms use the receipt's joined name,
    // whose alternatives remain on the exact selected transfer graph.
    if receipt.destination.is_valid() {
        for place in places
            .iter_mut()
            .filter(|place| place.root == facts::PlaceRoot::Symbol(receipt.destination))
        {
            let binding = ownership
                .selection_result_claims
                .span_or_empty(receipt.result_claims)
                .iter()
                .find(|claim| ownership.segments.span_or_empty(claim.path) == place.path);
            place.live = true;
            place.case_excluded = false;
            place.ever_established = true;
            place.provenance = binding.map(|claim| claim.provenance);
            place.claim_identity = binding.map(|claim| claim.claim_identity);
        }
    }
}
