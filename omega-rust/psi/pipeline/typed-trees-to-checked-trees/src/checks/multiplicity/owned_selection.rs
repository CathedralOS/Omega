//! Conditional whole-local transfers, keyed by existing source selection nodes.
//! Availability is conservative; residual custody remains in the receipts until
//! the actual death edge. A path constructing a fresh result discards, at the
//! join edge, the one source whose residual slot the result displaces; every
//! other complement stays in the receipt until the actual death edge.
use crate::checks::multiplicity::linear_obligations::LinearPlace;
use crate::checks::multiplicity::linear_obligations::type_reference_is_reference;
use crate::checks::type_multiplicity;
use arena::Handle;
use arena::HandleSpan;
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
use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern, TableMatchArm};
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::TypeReferenceNode;

pub(super) fn record_statement(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    places: &mut [LinearPlace],
) -> Result<bool, Diagnostic> {
    let expressions = statement_expressions(program, statement);
    for (_, receipt) in facts
        .flow
        .ownership
        .owned_selections
        .iter()
        .filter(|(_, receipt)| {
            receipt.state == state.symbol && (receipt.statement_ordinal as usize) < statement_index
        })
    {
        // A linear join consumes one exact claim place, named on every
        // transfer; the rest of the source root's frontier is residual
        // custody that stays live and consumable. Only a use reaching the
        // consumed claim (or a place containing it) sees a dead place.
        let linear_receipt =
            program.type_multiplicity(receipt.type_reference) == Multiplicity::Linear;
        for source in facts
            .flow
            .ownership
            .selection_sources
            .span_or_empty(receipt.sources)
        {
            if !linear_receipt {
                if expressions
                    .iter()
                    .any(|expression| names_symbol(program, *expression, source.symbol))
                    || matches!(statement, StatementNode::Call(call) if call.receiver_root_symbol == source.symbol)
                {
                    return Err(Diagnostic::error(
                        "owned value may have been transferred by an earlier match; it cannot be used here",
                    ));
                }
                continue;
            }
            if linear_source_use_conflicts(
                program,
                facts,
                machine,
                state,
                statement_index,
                statement,
                &expressions,
                receipt,
                source.symbol,
            ) {
                return Err(Diagnostic::error(
                    "linear match transfer consumed this exact claim on every edge; it cannot be used here",
                ));
            }
        }
    }
    let selected = expressions
        .iter()
        .copied()
        .filter(|expression| {
            matches!(
                program.expression_table.expression(*expression),
                ExpressionNode::Match(_)
            ) && has_owned_leaf(program, machine, state, statement_index, *expression)
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok(false);
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let (expression, destination, type_reference) = match statement {
        StatementNode::LocalData(local) if !local.is_mutable => {
            (local.initial_value, local.symbol, local.type_reference)
        }
        StatementNode::Expression(expression) if statement_index + 1 == statements.len() => {
            (*expression, SymbolHandle::invalid(), state.return_type)
        }
        _ => return Err(unsupported()),
    };
    let linear_result = program.type_multiplicity(type_reference) == Multiplicity::Linear;
    // An affine destination may carry linear claims: the join then settles the
    // consumed source claims at the exact destination frontier paths rather
    // than inventing a root claim. The carrier still owes the same finite
    // owned walls as a plain record — no loans, slices, nominal cleanup, or
    // recursive storage.
    let destination_ok = if linear_result {
        validation::has_linear_owned_contents(program, type_reference)
    } else {
        program.type_multiplicity(type_reference) == Multiplicity::Affine
            && validation::has_linear_owned_contents(program, type_reference)
    };
    if !selected.contains(&expression) || !destination_ok {
        return Err(unsupported());
    }
    // The source destination owns one carrier on every selected path. Nested
    // selections recurse into their own authored arms, without expanding paths.
    let mut leaves = Vec::new();
    collect_leaves(
        program,
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
            let facts::PlaceRoot::Symbol(symbol) = root else {
                continue;
            };
            symbol_leaves += 1;
            if !consumed.iter().any(|(candidate, candidate_path)| {
                *candidate == *symbol && *candidate_path == *path
            }) {
                consumed.push((*symbol, path.clone()));
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
    let mut sources = Vec::new();
    for (_, _, root, _, _) in &leaves {
        // A call's structural product roots its own once-evaluated custody;
        // only whole-local roots carry a roster source.
        let facts::PlaceRoot::Symbol(symbol) = *root else {
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
        // A source is either an established immutable local or an immutable
        // owned parameter: parameters carry their authored position as the
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
            if parameter.is_self || parameter.is_const || parameter.is_mutable {
                return Err(Diagnostic::error(
                    "owned match source must be an available whole immutable plain-affine local of the exact result type",
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
            let (source_ordinal, source_local) = statements[..statement_index]
                .iter()
                .enumerate()
                .find_map(|(ordinal, statement)| match statement {
                    StatementNode::LocalData(source) if source.symbol == symbol => {
                        Some((ordinal, source))
                    }
                    _ => None,
                })
                .ok_or_else(unsupported)?;
            if source_local.is_mutable || !source_local.initial_value.is_valid() {
                return Err(Diagnostic::error(
                    "owned match source must be an available whole immutable plain-affine local of the exact result type",
                ));
            }
            // A carrier source has no tracked root place: its linear children
            // are the custody roster. Their provenance is the single
            // establishment event the declaration's write recorded, and any
            // disagreement means the set was not established by one write.
            let provenance = if consumed_paths.is_empty() {
                places
                    .iter()
                    .find(|place| place.symbol == symbol && place.path.is_empty())
                    .ok_or_else(unsupported)?
                    .provenance
            } else {
                let provenances = consumed_paths
                    .iter()
                    .map(|claim_path| {
                        places
                            .iter()
                            .find(|place| place.symbol == symbol && place.path == *claim_path)
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
            .find(|place| place.symbol == symbol && place.path.is_empty());
        let mut consumed = Vec::new();
        for claim_path in &consumed_paths {
            let place = places
                .iter()
                .find(|place| place.symbol == symbol && place.path == *claim_path)
                .ok_or_else(unsupported)?;
            if !place.live
                || !place.ever_established
                || place.conditional
                || place.multiplicity != Multiplicity::Linear
            {
                return Err(Diagnostic::error(
                    "owned match source must be an available immutable owned local or parameter claim of the exact result type",
                ));
            }
            consumed.push(place);
        }
        let source_ok = if consumed.is_empty() {
            root_place.is_some_and(|place| place.multiplicity == Multiplicity::Affine)
        } else {
            true
        } && validation::has_linear_owned_contents(program, source_reference);
        if !source_ok {
            return Err(Diagnostic::error(
                "owned match source must be an available immutable owned local or parameter claim of the exact result type",
            ));
        }
        if consumed.is_empty() {
            let place = root_place.ok_or_else(unsupported)?;
            if !place.live || !place.ever_established || place.conditional {
                return Err(Diagnostic::error(
                    "owned match source must be an available immutable owned local or parameter claim of the exact result type",
                ));
            }
        }
        // A whole-leaf source is selected at its root boundary, so its local
        // type is the result type. A source reached only through projected
        // leaves instead owes the exact moved path recorded on each transfer;
        // the projected leaf type was already checked against the result.
        if leaves.iter().any(|(_, _, leaf_root, path, _)| {
            *leaf_root == facts::PlaceRoot::Symbol(symbol) && path.is_empty()
        }) && program.normalized_type_identity(source_reference)
            != program.normalized_type_identity(type_reference)
        {
            return Err(Diagnostic::error(
                "owned match source must be an available whole immutable plain-affine local of the exact result type",
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
    for (expression, source_arm, root, path, leaf_type) in leaves {
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
                let source = Handle::from_parts(
                    sources.start().arena_index() + ordinal as u32,
                    sources.start().generation(),
                );
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
                            .find(|place| place.symbol == symbol && place.path == claim_path)
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
    let receipt = FlowOwnedSelectionReceipt {
        machine: machine.symbol,
        state: state.symbol,
        statement_ordinal: u32::try_from(statement_index).map_err(|_| unsupported())?,
        expression,
        destination,
        type_reference,
        sources,
        transfers,
        death: PermissionEventSource::StateExit,
    };
    apply_availability(program, &facts.flow.ownership, &receipt, places);
    facts.flow.ownership.owned_selections.append(receipt);
    Ok(true)
}

pub(super) fn apply_availability(
    program: &typed_trees::TypedTrees,
    ownership: &checked_trees::FlowOwnershipFacts,
    receipt: &FlowOwnedSelectionReceipt,
    places: &mut [LinearPlace],
) {
    // `live` here is the all-path availability summary, not disposition. The
    // conditional complement remains owed in the receipt's source roster.
    for source in ownership.selection_sources.span_or_empty(receipt.sources) {
        if let Some(place) = places
            .iter_mut()
            .find(|place| place.symbol == source.symbol && place.path.is_empty())
        {
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
            if let Some(place) = places
                .iter_mut()
                .find(|place| place.symbol == source.symbol && place.path == claim_path)
            {
                place.live = false;
            }
        }
    }
    // A linear join whose edges all move the same consumed claim hands that
    // claim itself to the destination: its identity and provenance survive
    // the transfer. A fresh-per-edge product leaves both edge-dependent.
    let inherited = (program.type_multiplicity(receipt.type_reference) == Multiplicity::Linear)
        .then(|| {
            ownership
                .selection_transfers
                .span_or_empty(receipt.transfers)
                .iter()
                .find_map(|transfer| {
                    if !transfer.source.is_valid() {
                        return None;
                    }
                    ownership
                        .selection_transfer_claims
                        .span_or_empty(transfer.claims)
                        .first()
                        .map(|claim| (claim.claim_identity, claim.provenance))
                })
        })
        .flatten();
    if let Some(place) = places
        .iter_mut()
        .find(|place| place.symbol == receipt.destination && place.path.is_empty())
    {
        place.live = true;
        place.ever_established = true;
        place.provenance = Some(
            inherited
                .map(|(_, provenance)| provenance)
                .unwrap_or(PermissionProvenance::Unknown),
        );
        place.claim_identity = Some(
            inherited
                .map(|(identity, _)| identity)
                .unwrap_or(PermissionClaimIdentity::Unknown),
        );
    }
    // An affine carrier destination has no tracked root place: the join
    // establishes each frontier claim place instead. A claim position
    // inherits the consumed claim's identity and provenance only when every
    // reachable edge supplies the same pair; a distinct source on any arm, a
    // fresh product, or a claimless leaf leaves both edge-dependent.
    if receipt.destination.is_valid() {
        let transfers = ownership
            .selection_transfers
            .span_or_empty(receipt.transfers);
        let fully_sourced = transfers.len() == terminal_arm_values(program, receipt.expression)
            && transfers.iter().all(|transfer| transfer.source.is_valid());
        for (position, place) in places
            .iter_mut()
            .filter(|place| place.symbol == receipt.destination && !place.path.is_empty())
            .enumerate()
        {
            let inherited_claim = fully_sourced.then(|| {
                transfers
                    .iter()
                    .map(|transfer| {
                        ownership
                            .selection_transfer_claims
                            .span_or_empty(transfer.claims)
                            .get(position)
                            .map(|claim| (claim.claim_identity, claim.provenance))
                    })
                    .collect::<Option<Vec<_>>>()
            });
            let unanimous = inherited_claim.flatten().and_then(|positions| {
                (positions.iter().all(|position| *position == positions[0])).then_some(positions[0])
            });
            place.live = true;
            place.ever_established = true;
            place.provenance = Some(
                unanimous
                    .map(|(_, provenance)| provenance)
                    .unwrap_or(PermissionProvenance::Unknown),
            );
            place.claim_identity = Some(
                unanimous
                    .map(|(identity, _)| identity)
                    .unwrap_or(PermissionClaimIdentity::Unknown),
            );
        }
    }
}

/// Whether a statement touches a claim an earlier linear selection already
/// consumed on every edge. The consumed place is the roster source plus the
/// exact transfer path; residual siblings of the source root stay live and
/// consumable, so only overlapping maximal use paths conflict. An
/// unresolvable call receiver conservatively counts as a whole-source use.
#[allow(clippy::too_many_arguments)]
fn linear_source_use_conflicts(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    expressions: &[ExpressionHandle],
    receipt: &FlowOwnedSelectionReceipt,
    source_symbol: SymbolHandle,
) -> bool {
    // The consumed places are the claim set each transfer names, not just the
    // moved leaf path: a claim row is the exact discharged place, while a
    // claimless transfer still guards its leaf path (a projected affine leaf
    // keeps residual siblings off the moved child).
    let consumed_paths = facts
        .flow
        .ownership
        .selection_transfers
        .span_or_empty(receipt.transfers)
        .iter()
        .filter(|transfer| transfer.source.is_valid())
        .filter(|transfer| {
            facts
                .flow
                .ownership
                .selection_sources
                .get(transfer.source)
                .symbol
                == source_symbol
        })
        .flat_map(|transfer| {
            let claims = facts
                .flow
                .ownership
                .selection_transfer_claims
                .span_or_empty(transfer.claims);
            if claims.is_empty() {
                vec![
                    facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(transfer.path)
                        .to_vec(),
                ]
            } else {
                claims
                    .iter()
                    .map(|claim| {
                        facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(claim.path)
                            .to_vec()
                    })
                    .collect()
            }
        })
        .collect::<Vec<_>>();
    if consumed_paths.is_empty() {
        return false;
    }
    let mut used_paths: Vec<Vec<facts::PlaceSegment>> = expressions
        .iter()
        .filter_map(|expression| {
            crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                *expression,
            )
        })
        .filter(|place| place.root == facts::PlaceRoot::Symbol(source_symbol))
        .map(|place| place.segments)
        .collect();
    if let StatementNode::Call(call) = statement
        && call.receiver_root_symbol == source_symbol
    {
        match crate::flow::canonical_receiver_place_for_call_site(
            program,
            machine.symbol,
            state.symbol,
            &crate::semantic_calls::CallSite::Statement(call),
            statement_index,
        ) {
            Some(place) if place.root == facts::PlaceRoot::Symbol(source_symbol) => {
                used_paths.push(place.segments)
            }
            _ => return true,
        }
    }
    // The inner `Name` nodes of a projection are strict prefixes of the
    // outermost use; keep only maximal touched paths so reading a residual
    // sibling does not count as using the consumed whole.
    let used_paths: Vec<Vec<facts::PlaceSegment>> = used_paths
        .iter()
        .filter(|path| {
            !used_paths
                .iter()
                .any(|other| other.len() > path.len() && other.starts_with(path.as_slice()))
        })
        .cloned()
        .collect();
    used_paths.iter().any(|used| {
        consumed_paths
            .iter()
            .any(|consumed| place_paths_overlap(used, consumed))
    })
}

/// Two canonical paths overlap when one reaches inside the other or both
/// name the same place; a dynamic index cannot be statically excluded from a
/// fixed element claim, so it conflicts on the element axis.
pub(super) fn place_paths_overlap(
    used: &[facts::PlaceSegment],
    consumed: &[facts::PlaceSegment],
) -> bool {
    used.iter().zip(consumed.iter()).all(|(used, consumed)| {
        used == consumed
            || (matches!(used, facts::PlaceSegment::Index { .. })
                && matches!(
                    consumed,
                    facts::PlaceSegment::FixedIndex { .. }
                        | facts::PlaceSegment::Index { .. }
                        | facts::PlaceSegment::FixedRange { .. }
                ))
    })
}

/// Whether an owned match source still owes loan-closure evidence: either a
/// syntactic borrow the loan ledger never recorded (an escape without a
/// trackable loan, for example persistent machine storage) or a recorded loan
/// on the source root still live at this statement's entry. The flow builder
/// drops a loan from the entry constraint set once its last use passes, so a
/// once-borrowed source whose loans all closed joins like any other owner; a
/// live loan keeps the explicit rejection instead of guessing at referent
/// custody across the conditional move.
fn source_needs_loan_closure(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    prior_statements: &[StatementNode],
    symbol: SymbolHandle,
) -> bool {
    let borrowed = prior_statements.iter().any(|statement| {
        statement_expressions(program, statement).iter().any(|expression| {
            matches!(program.expression_table.expression(*expression), ExpressionNode::Borrow(borrow)
                if expression_names(program, borrow.target, symbol))
        })
    });
    let recorded = facts
        .borrow
        .loans
        .iter()
        .any(|(_, loan)| loan.root_symbol == symbol);
    if borrowed && !recorded {
        return true;
    }
    if !recorded {
        return false;
    }
    let statement_fact = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, flow_state)| {
            (flow_state.machine_symbol == machine.symbol && flow_state.state_symbol == state.symbol)
                .then_some(flow_state)
        })
        .and_then(|flow_state| facts.flow.state_statement(flow_state, statement_index));
    match statement_fact {
        Some(statement_fact) => facts
            .flow
            .borrow_loan_constraints(statement_fact.entry_constraints)
            .any(|loan| facts.borrow.loans.get(loan).root_symbol == symbol),
        // Without the statement's entry constraint set, fall back to the
        // loan's recorded last use, the same boundary the expiry filter uses.
        None => facts.borrow.loans.iter().any(|(_, loan)| {
            loan.root_symbol == symbol && loan.last_use_statement_index >= statement_index
        }),
    }
}

fn unsupported() -> Diagnostic {
    Diagnostic::error(
        "owned match requires one existing whole plain-affine or linear-owned local per path and an immutable local or return destination; projections, calls and borrowed custody require additional ownership evidence",
    )
}

fn has_owned_leaf(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => reachable_arms(program, dispatch.arms)
            .iter()
            .any(|(_, arm)| has_owned_leaf(program, machine, state, statement_index, arm.value)),
        ExpressionNode::Name(_) => {
            validation::expression_result_type_reference(program, machine, state, expression)
                .is_some_and(|reference| {
                    program.type_multiplicity(reference) != Multiplicity::Unrestricted
                        && (validation::affine_owned_value_source(program, expression, reference)
                            .is_some()
                            || validation::linear_owned_value_source(
                                program, expression, reference,
                            )
                            .is_some())
                })
        }
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            projected_leaf_source(program, machine, state, statement_index, expression).is_some()
        }
        // A record arm moving existing children carries its leaves inside the
        // constructor: a projected field is the same moved-child leaf the
        // arm-level rule admits, so the roster has to engage for it too.
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| has_owned_leaf(program, machine, state, statement_index, field.value)),
        _ => false,
    }
}

/// The exact canonical projection for one `member`/`indexed` leaf, when the
/// leaf selects a plain affine child of a live plain affine owner. A borrowed
/// prefix would escape the root's custody, so any reference at or below the
/// root rejects the projection instead of guessing referent ownership. The
/// root may be a whole local or a call's structural product; the product is
/// its own once-evaluated owner and carries no roster source.
fn projected_leaf_source(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<(facts::PlaceRoot, Vec<facts::PlaceSegment>)> {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    ) {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    )?;
    if place.segments.is_empty()
        || !place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
            )
        })
    {
        return None;
    }
    match place.root {
        facts::PlaceRoot::Symbol(symbol) => {
            if program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.symbol == symbol && parameter.is_self)
                || symbol == machine.symbol
                || !matches!(
                    program.symbols.get(symbol).kind,
                    symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
                )
            {
                return None;
            }
        }
        facts::PlaceRoot::Expression(root)
            if matches!(
                program.expression_table.expression(root),
                ExpressionNode::Call(_)
            ) => {}
        _ => return None,
    }
    // The leaf admits either an affine plain-owned child (the original rule)
    // or a linear child under an affine root: the moved claim then keeps its
    // exact path while the root's residual frontier stays live on every edge.
    // A linear root would put the claim at the whole place, which a projected
    // path cannot split, so the root itself must be affine.
    let leaf_reference = crate::flow::canonical_place_type_reference(
        program,
        state.symbol,
        statement_index,
        &place,
    )?;
    let leaf_multiplicity = type_multiplicity(program, leaf_reference);
    if !matches!(
        leaf_multiplicity,
        Multiplicity::Affine | Multiplicity::Linear
    ) {
        return None;
    }
    let linear_leaf = leaf_multiplicity == Multiplicity::Linear;
    // A temporary root (a call's structural product) can drop an affine
    // residual sibling, but a linear residual has no surviving owner place;
    // a projected linear child may only leave a named local or parameter.
    if linear_leaf && !matches!(place.root, facts::PlaceRoot::Symbol(_)) {
        return None;
    }
    let contents_ok = |type_reference| {
        if linear_leaf {
            validation::has_linear_owned_contents(program, type_reference)
        } else {
            validation::has_plain_owned_contents(program, type_reference)
        }
    };
    let place_type_ok = |segments: &[facts::PlaceSegment], expected: Multiplicity| {
        crate::flow::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &crate::flow::CanonicalPlace {
                root: place.root,
                segments: segments.to_vec(),
            },
        )
        .is_some_and(|type_reference| {
            matches!(
                program.type_reference_table.type_reference(type_reference),
                TypeReferenceNode::Named { .. }
                    | TypeReferenceNode::Generic { .. }
                    | TypeReferenceNode::FixedArray { .. }
            ) && type_multiplicity(program, type_reference) == expected
                && contents_ok(type_reference)
        })
    };
    if !place_type_ok(&place.segments, leaf_multiplicity)
        || !place_type_ok(&[], Multiplicity::Affine)
    {
        return None;
    }
    // Root and every proper prefix must be owned, never a reference: moving a
    // projected child through a borrow would imply referent custody.
    if (0..place.segments.len()).any(|length| {
        crate::flow::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &crate::flow::CanonicalPlace {
                root: place.root,
                segments: place.segments[..length].to_vec(),
            },
        )
        .is_some_and(|reference| type_reference_is_reference(program, reference))
    }) {
        return None;
    }
    Some((place.root, place.segments))
}

fn collect_leaves(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    source_arm: Handle<TableMatchArm>,
    leaves: &mut Vec<(
        ExpressionHandle,
        Handle<TableMatchArm>,
        facts::PlaceRoot,
        Vec<facts::PlaceSegment>,
        TypeReferenceHandle,
    )>,
) -> Result<(), Diagnostic> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) if source_arm.is_valid() => {
            match validation::affine_owned_value_source(program, expression, type_reference)
                .or_else(|| {
                    validation::linear_owned_value_source(program, expression, type_reference)
                }) {
                Some(symbol) => {
                    leaves.push((
                        expression,
                        source_arm,
                        facts::PlaceRoot::Symbol(symbol),
                        Vec::new(),
                        type_reference,
                    ));
                    Ok(())
                }
                None => fresh_leaf(program, machine, state, expression, type_reference),
            }
        }
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) if source_arm.is_valid() => {
            match projected_leaf_source(program, machine, state, statement_index, expression) {
                Some((root, path)) => {
                    let leaf_type = crate::flow::canonical_place_type_reference(
                        program,
                        state.symbol,
                        statement_index,
                        &crate::flow::CanonicalPlace {
                            root,
                            segments: path.clone(),
                        },
                    )
                    .ok_or_else(unsupported)?;
                    if program.normalized_type_identity(leaf_type)
                        != program.normalized_type_identity(type_reference)
                    {
                        return Err(unsupported());
                    }
                    leaves.push((expression, source_arm, root, path, leaf_type));
                    Ok(())
                }
                _ => Err(unsupported()),
            }
        }
        ExpressionNode::StructLiteral(_) if source_arm.is_valid() => {
            // A record arm is fresh construction whose fields may each move
            // an exact owned child leaf. Moved fields join the roster so the
            // edge residual schedule names the precise complement; every
            // other field keeps the unrestricted-operand rule.
            let fields = fresh_leaf_shape(program, machine, state, expression, type_reference)?;
            for field in fields {
                collect_field_leaves(
                    program,
                    machine,
                    state,
                    statement_index,
                    field,
                    source_arm,
                    leaves,
                )?;
            }
            Ok(())
        }
        ExpressionNode::Call(call) if source_arm.is_valid() => {
            // A call's structural product is a fresh arm value: it carries no
            // roster leaf and no transfer, but its argument operands obey the
            // same hidden-ownership rules as fresh construction fields, and a
            // receiver or non-unrestricted parameter would move custody the
            // receipt cannot name.
            if call.receiver.is_valid() || !call_moves_no_ownership(program, call) {
                return Err(unsupported());
            }
            let operands = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .flat_map(|argument| expression_nodes(program, *argument))
                .collect::<Vec<_>>();
            hidden_ownership_operands(
                program,
                machine,
                state,
                &operands,
                "owned match call arguments require unrestricted operands without hidden ownership transfers",
            )
        }
        ExpressionNode::Match(dispatch) => {
            // Selection predicates cannot introduce ownership effects hidden
            // outside the selected value graph. Scalar calls need their own
            // captured effect/loan sequence before joining this route, so a
            // predicate call stays rejected even when its arguments move no
            // ownership.
            let mut operands = expression_nodes(program, dispatch.subject);
            for (_, arm) in reachable_arms(program, dispatch.arms) {
                if let MatchPattern::Value(pattern) = arm.pattern {
                    operands.extend(expression_nodes(program, pattern));
                }
            }
            if operands.iter().any(|operand| {
                matches!(
                    program.expression_table.expression(*operand),
                    ExpressionNode::Call(_) | ExpressionNode::Borrow(_) | ExpressionNode::Atomic(_)
                )
            }) {
                return Err(unsupported());
            }
            let tag_operands = case_tag_operands(program, machine, state, &operands);
            if operands.iter().any(|operand| {
                matches!(
                    program.expression_table.expression(*operand),
                    ExpressionNode::Name(_)
                ) && !tag_operands.contains(operand)
                    && validation::expression_result_type_reference(
                        program, machine, state, *operand,
                    )
                    .is_none_or(|reference| {
                        program.type_multiplicity(reference) != Multiplicity::Unrestricted
                    })
            }) {
                return Err(Diagnostic::error(
                    "owned match predicates require unrestricted operands without hidden ownership transfers",
                ));
            }
            for (source_arm, arm) in reachable_arms(program, dispatch.arms) {
                collect_leaves(
                    program,
                    machine,
                    state,
                    statement_index,
                    arm.value,
                    type_reference,
                    source_arm,
                    leaves,
                )?;
            }
            Ok(())
        }
        _ => Err(unsupported()),
    }
}

/// One record field's contribution to the roster: an exact projected child
/// moves as a leaf, while every other field shape stays on the fresh-construction
/// operand rules. Whole names are not field leaves: a field's carrier is its
/// declared member type, not the source root's whole-place type, so a whole
/// name at field position cannot name the moved custody the roster records.
fn collect_field_leaves(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    field: ExpressionHandle,
    source_arm: Handle<TableMatchArm>,
    leaves: &mut Vec<(
        ExpressionHandle,
        Handle<TableMatchArm>,
        facts::PlaceRoot,
        Vec<facts::PlaceSegment>,
        TypeReferenceHandle,
    )>,
) -> Result<(), Diagnostic> {
    let fresh = |field| {
        hidden_ownership_operands(
            program,
            machine,
            state,
            &expression_nodes(program, field),
            "owned match record field operands require unrestricted operands without hidden ownership transfers",
        )
    };
    match program.expression_table.expression(field) {
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            match projected_leaf_source(program, machine, state, statement_index, field) {
                Some((root, path)) => {
                    let leaf_type = crate::flow::canonical_place_type_reference(
                        program,
                        state.symbol,
                        statement_index,
                        &crate::flow::CanonicalPlace {
                            root,
                            segments: path.clone(),
                        },
                    )
                    .ok_or_else(unsupported)?;
                    if Some(leaf_type)
                        != validation::expression_result_type_reference(
                            program, machine, state, field,
                        )
                    {
                        return Err(unsupported());
                    }
                    leaves.push((field, source_arm, root, path, leaf_type));
                    Ok(())
                }
                None => fresh(field),
            }
        }
        ExpressionNode::StructLiteral(literal) if literal.case_symbol.is_none() => {
            let reference =
                validation::expression_result_type_reference(program, machine, state, field)
                    .ok_or_else(unsupported)?;
            let fields = fresh_leaf_shape(program, machine, state, field, reference)?;
            for nested in fields {
                collect_field_leaves(
                    program,
                    machine,
                    state,
                    statement_index,
                    nested,
                    source_arm,
                    leaves,
                )?;
            }
            Ok(())
        }
        _ => fresh(field),
    }
}

/// A fresh arm constructs the exact result type and contributes no source and
/// no transfer; its field operands obey the same no-hidden-ownership rules as
/// selection predicates.
fn fresh_leaf(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
) -> Result<(), Diagnostic> {
    let fields = fresh_leaf_shape(program, machine, state, expression, type_reference)?;
    let operands = fields
        .into_iter()
        .flat_map(|value| expression_nodes(program, value))
        .collect::<Vec<_>>();
    hidden_ownership_operands(
        program,
        machine,
        state,
        &operands,
        "owned match fresh fields require unrestricted operands without hidden ownership transfers",
    )
}

/// The fresh-construction shape every literal or case arm must keep: the
/// authored expression declares the exact result type, its contents satisfy
/// the multiplicity's owned-storage walls, and the field value list is handed
/// back for the caller's per-field disposition.
fn fresh_leaf_shape(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
) -> Result<Vec<ExpressionHandle>, Diagnostic> {
    let (reference, fields) =
        if let Some(constructor) = validation::scalar_case_constructor(program, expression) {
            (
                constructor.type_reference,
                constructor
                    .fields
                    .into_iter()
                    .map(|(_, value, _)| value)
                    .collect::<Vec<_>>(),
            )
        } else if let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(expression)
            && literal.case_symbol.is_none()
        {
            let reference =
                validation::expression_result_type_reference(program, machine, state, expression)
                    .ok_or_else(unsupported)?;
            (
                reference,
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value)
                    .collect(),
            )
        } else {
            return Err(unsupported());
        };
    let contents_ok = if program.type_multiplicity(type_reference) == Multiplicity::Linear {
        validation::has_linear_owned_contents(program, reference)
    } else {
        validation::has_plain_owned_contents(program, reference)
    };
    if !contents_ok
        || program.normalized_type_identity(reference)
            != program.normalized_type_identity(type_reference)
    {
        return Err(unsupported());
    }
    Ok(fields)
}

/// A call moves no selection custody when it carries no receiver and every
/// target parameter is unrestricted: its product is then fresh owned storage
/// and its arguments cannot smuggle an owned leaf the receipt does not name.
/// An unresolvable target conservatively counts as a transfer.
fn call_moves_no_ownership(
    program: &typed_trees::TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    !call.receiver.is_valid()
        && crate::semantic_calls::call_target_parameters(program, call.target_symbol).is_some_and(
            |parameters| {
                parameters.iter().all(|parameter| {
                    program.type_multiplicity(parameter.type_reference)
                        == Multiplicity::Unrestricted
                })
            },
        )
}

/// The hidden-ownership operand rule shared by fresh arm constructions and
/// fresh call arguments. A call operand stays admissible only while it moves
/// no custody itself; borrows, atomics, custody-moving calls, and named owned
/// operands all keep their explicit rejections.
fn hidden_ownership_operands(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    operands: &[ExpressionHandle],
    message: &str,
) -> Result<(), Diagnostic> {
    if operands.iter().any(
        |operand| match program.expression_table.expression(*operand) {
            ExpressionNode::Call(call) => !call_moves_no_ownership(program, call),
            ExpressionNode::Borrow(_) | ExpressionNode::Atomic(_) => true,
            _ => false,
        },
    ) {
        return Err(unsupported());
    }
    let tag_operands = case_tag_operands(program, machine, state, operands);
    if operands.iter().any(|operand| {
        matches!(
            program.expression_table.expression(*operand),
            ExpressionNode::Name(_)
        ) && !tag_operands.contains(operand)
            && validation::expression_result_type_reference(program, machine, state, *operand)
                .is_none_or(|reference| {
                    program.type_multiplicity(reference) != Multiplicity::Unrestricted
                })
    }) {
        return Err(Diagnostic::error(message));
    }
    Ok(())
}

/// An exact case-membership comparison observes the subject's tag: authored
/// `subject in Type::Case` lowers to an equality whose right operand is a
/// symbol-stamped case reference with no result type and whose left operand is
/// read, not transferred. Both operands are exempt from the
/// unrestricted-operand rule; calls, borrows and atomic subjects still reject
/// separately, and operands of genuine authored equality are unchanged.
fn case_tag_operands(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    operands: &[ExpressionHandle],
) -> Vec<ExpressionHandle> {
    operands
        .iter()
        .copied()
        .flat_map(
            |operand| match program.expression_table.expression(operand) {
                ExpressionNode::Binary(binary)
                    if validation::has_exact_case_membership_meaning(
                        program,
                        machine,
                        Some(state),
                        operand,
                        binary,
                    ) =>
                {
                    vec![binary.left, binary.right]
                }
                _ => Vec::new(),
            },
        )
        .collect()
}

fn names_symbol(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    matches!(program.expression_table.expression(expression), ExpressionNode::Name(path) if path.symbol == symbol)
}

fn expression_names(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    expression_nodes(program, expression)
        .iter()
        .any(|expression| names_symbol(program, *expression, symbol))
}

fn statement_expressions(
    program: &typed_trees::TypedTrees,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    let roots = match statement {
        StatementNode::LocalData(local) => vec![local.initial_value],
        StatementNode::Expression(expression) => vec![*expression],
        StatementNode::Assignment(assignment) => vec![assignment.target, assignment.value],
        StatementNode::Call(call) => program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec(),
        StatementNode::RootBinding(binding) => {
            let mut roots = vec![binding.receiver];
            if binding.implementation_operand.is_valid() {
                roots.push(binding.implementation_operand);
            }
            roots
        }
        StatementNode::AssemblyFact(fact) => vec![fact.expression],
        StatementNode::Transition(transition) => {
            let mut roots = Vec::new();
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                roots.push(guard);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    typed_trees::statement::TransitionTargetNode::Value(value) => {
                        roots.push(*value)
                    }
                    typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => roots
                        .extend_from_slice(program.expression_table.expression_handles(*arguments)),
                    _ => {}
                }
            }
            roots
        }
    };
    roots
        .into_iter()
        .flat_map(|root| expression_nodes(program, root))
        .collect()
}

fn expression_nodes(
    program: &typed_trees::TypedTrees,
    root: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    let mut pending = vec![root];
    let mut nodes = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || nodes.contains(&expression) {
            continue;
        }
        nodes.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                pending.push(dispatch.subject);
                for (_, arm) in reachable_arms(program, dispatch.arms) {
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Atomic(atomic) => pending.extend([atomic.value, atomic.result]),
            ExpressionNode::ArrayLiteral(values) => {
                pending.extend_from_slice(program.expression_table.expression_handles(*values))
            }
            ExpressionNode::Call(call) => {
                pending.push(call.receiver);
                pending
                    .extend_from_slice(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Range(range) => pending.extend([range.start, range.end]),
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            _ => {}
        }
    }
    nodes
}

/// Count the reachable complete arm paths through nested selections: every
/// non-`match` arm value is one edge the linear frontier must agree on.
fn terminal_arm_values(program: &typed_trees::TypedTrees, expression: ExpressionHandle) -> usize {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => reachable_arms(program, dispatch.arms)
            .iter()
            .map(|(_, arm)| terminal_arm_values(program, arm.value))
            .sum(),
        _ => 1,
    }
}

/// Ordered first-match reachability. The source type checker still visits all
/// authored arms. Only literal equality and complete Boolean/wildcard coverage
/// remove ownership alternatives here; no predicate theorem is invented.
pub(super) fn reachable_arms(
    program: &typed_trees::TypedTrees,
    arms: HandleSpan<TableMatchArm>,
) -> Vec<(Handle<TableMatchArm>, &TableMatchArm)> {
    let mut selected = Vec::new();
    let mut patterns = Vec::new();
    let mut booleans = [false; 2];
    for (ordinal, arm) in program.expression_table.match_arms(arms).iter().enumerate() {
        let mut covered = false;
        match arm.pattern {
            MatchPattern::Wildcard => covered = true,
            MatchPattern::Value(pattern) => {
                let node = program.expression_table.expression(pattern);
                let duplicate = patterns.iter().any(|previous| {
                    match (program.expression_table.expression(*previous), node) {
                        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => {
                            left == right
                        }
                        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left
                            .value_bignum()
                            .zip(right.value_bignum())
                            .is_some_and(|(left, right)| left == right),
                        _ => false,
                    }
                });
                if duplicate {
                    continue;
                }
                patterns.push(pattern);
                if let ExpressionNode::Boolean(value) = node {
                    booleans[usize::from(*value)] = true;
                    covered = booleans == [true, true];
                }
            }
        }
        let handle = Handle::from_parts(
            arms.start().arena_index() + ordinal as u32,
            arms.start().generation(),
        );
        selected.push((handle, arm));
        if covered {
            break;
        }
    }
    selected
}
