//! Reconstructed ownership-frontier membership for relocated affine
//! scalar-case, empty-record, and structural-call results.
//!
//! An affine `EstablishScalarCase`, `EstablishRecord`, or `CallStructural`
//! relocated out of a
//! cyclic member re-times
//! where its result is owned: the seed established a fresh place inside the
//! member block and discarded it on every dispatch edge, while the
//! transformed unit produces the one persistent place in the preheader,
//! keeps it live across member-internal edges, and disposes it at component
//! exits and member returns. The verifier's frontier catalog remains the
//! sole authority for every place and every site the move does not touch;
//! for the relocated place the catalog must be re-expressed through the
//! transformed custody so the retained-edge affine-authority replay sees the
//! same ownership the current-custody replay executes. Both consumers run
//! this one reconstruction — the realization when it attaches the
//! transformed catalog and the context projection when it independently
//! re-derives the catalog the transformed unit must carry — so the delta is
//! checked evidence, never a trusted attachment. The derivation is
//! deliberately narrow: only an affine case result whose `psi_operation`
//! moved away from its seed block can gain or lose membership anywhere, so
//! validating the verified seed itself never rewrites a single fact.

use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    OwnershipFrontierFact, OwnershipFrontierOwnedPlace, OwnershipFrontierSite,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};

/// The affine `EstablishScalarCase`, `EstablishRecord`, and `CallStructural`
/// results whose
/// position in `unit` moved
/// away from the block the seed established them in — the only places whose
/// frontier membership a relocation may re-time. A node's seed home is the
/// block containing its `psi_operation` in the immutable Terminal module; a
/// moved producer is the signature the relocation freeze already
/// proved against the seed. Validating the verified seed itself always
/// yields an empty map, so the seed catalog is never touched.
pub(crate) fn relocated_scalar_case_result_places(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> BTreeMap<MachineId, BTreeSet<PlaceId>> {
    let mut seed_home: BTreeMap<(MachineId, OperationId), BlockId> = BTreeMap::new();
    for machine in &input.context().module().machines {
        for block in &machine.blocks {
            for operation in &block.operations {
                seed_home.insert((machine.id, operation.id), block.id);
            }
        }
    }
    let mut relocated: BTreeMap<MachineId, BTreeSet<PlaceId>> = BTreeMap::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for node in &block.nodes {
                let (psi_operation, result) = match &node.operation {
                    O::EstablishScalarCase {
                        psi_operation,
                        result,
                        ..
                    }
                    | O::EstablishRecord {
                        psi_operation,
                        result,
                        ..
                    }
                    | O::CallStructural {
                        psi_operation,
                        result,
                        ..
                    } => (*psi_operation, result),
                    _ => continue,
                };
                if result.multiplicity != terminal_psi::StructuralMultiplicity::Affine {
                    continue;
                }
                if seed_home.get(&(function.machine, psi_operation)) == Some(&block.id) {
                    continue;
                }
                relocated
                    .entry(function.machine)
                    .or_default()
                    .insert(result.place);
            }
        }
    }
    relocated
}

/// Rewrite `facts`' owned-place membership for every place in `relocated`:
/// each catalog site owns the place exactly when the unit's current custody —
/// reconstructed by [`case_result_owned_sites`] — carries it there. A fact's
/// identity is canonical over its snapshot, so every touched row is rebuilt
/// through [`OwnershipFrontierFact::new`]; sites and places the move does
/// not affect keep their seed spelling byte-exact.
pub(crate) fn rewrite_relocated_case_result_frontiers(
    facts: &mut [OwnershipFrontierFact],
    unit: &PsiOptimizationUnit,
    relocated: &BTreeMap<MachineId, BTreeSet<PlaceId>>,
) {
    for (machine, places) in relocated {
        let Some(function) = unit
            .functions
            .iter()
            .find(|function| function.machine == *machine)
        else {
            continue;
        };
        for place in places {
            let owned = case_result_owned_sites(function, *place);
            for fact in facts.iter_mut().filter(|fact| fact.machine == *machine) {
                let owns = owned.contains(&fact.site);
                let has = fact
                    .snapshot
                    .owned_places
                    .iter()
                    .any(|owned_place| owned_place.place == *place);
                if owns == has {
                    continue;
                }
                let mut snapshot = fact.snapshot.clone();
                if owns {
                    snapshot.owned_places.push(OwnershipFrontierOwnedPlace {
                        place: *place,
                        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    });
                    snapshot
                        .owned_places
                        .sort_by_key(|owned_place| owned_place.place);
                } else {
                    snapshot
                        .owned_places
                        .retain(|owned_place| owned_place.place != *place);
                }
                *fact = OwnershipFrontierFact::new(fact.psi, *machine, fact.site, snapshot);
            }
        }
    }
}

/// The frontier sites where `place` is owned under `function`'s current
/// custody, reconstructed as the greatest fixpoint of the verifier's
/// frontier rules restricted to that one place. An operation exit owns it
/// when its entry does or the operation produces it; an operation entry
/// chains the block's node order; an edge entry owns it when the source
/// block's tail does; a successor edge's exit owns it when the entry does
/// and the edge's disposal rosters — trivial discards, residual discards,
/// and owned structural-binding arguments — do not spell it (inherited
/// provenance rows perform no discard work, so their exit custody equals
/// their entry custody, matching the affine-authority replay); a block
/// entry owns it when every in-edge exit does — the must-own join — with
/// the function entry seeded unowned since an operation result cannot be
/// live at function entry. Initializing every site owned and shrinking to
/// stability computes the greatest fixpoint the verifier's own
/// path-sensitive reconstruction derives.
fn case_result_owned_sites(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> BTreeSet<OwnershipFrontierSite> {
    struct BlockPlan {
        /// `(operation, produces)` for each node carrying an `Operation`
        /// provenance row, in block order.
        operations: Vec<(OperationId, bool)>,
    }

    let mut plans: BTreeMap<BlockId, BlockPlan> = BTreeMap::new();
    // Every provenance edge row maps to its source block and canonical edge;
    // only row zero may perform the edge's discard work.
    let mut edge_rows: BTreeMap<EdgeId, (BlockId, EdgeId, bool)> = BTreeMap::new();
    let mut edge_disposes: BTreeSet<EdgeId> = BTreeSet::new();
    let mut in_edges: BTreeMap<BlockId, Vec<EdgeId>> = BTreeMap::new();
    let mut sites: Vec<OwnershipFrontierSite> = Vec::new();
    for block in &function.blocks {
        let mut plan = BlockPlan {
            operations: Vec::new(),
        };
        sites.push(OwnershipFrontierSite::BlockEntry(block.id));
        for node in &block.nodes {
            if let Some(PsiProvenance::Operation(operation)) = node.provenance.first() {
                plan.operations.push((
                    *operation,
                    crate::validation::produced_place_root(&node.operation) == Some(place),
                ));
                sites.push(OwnershipFrontierSite::OperationEntry(*operation));
                sites.push(OwnershipFrontierSite::OperationExit(*operation));
            }
            for edge in &node.successors {
                in_edges.entry(edge.target).or_default().push(edge.psi_edge);
                if edge.trivial_affine_discards.contains(&place)
                    || edge
                        .residual_affine_discards
                        .iter()
                        .any(|discard| discard.place == place)
                    || edge.structural_bindings.iter().any(|binding| {
                        binding.argument.place == place
                            && binding.argument.path.is_empty()
                            && binding.argument.access == terminal_psi::StructuralAccess::Owned
                    })
                {
                    edge_disposes.insert(edge.psi_edge);
                }
                // The edge's own Psi identity is provenance row zero; keep
                // its sites even if a forged roster ever left the row out.
                edge_rows.insert(edge.psi_edge, (block.id, edge.psi_edge, true));
                sites.push(OwnershipFrontierSite::EdgeEntry(edge.psi_edge));
                sites.push(OwnershipFrontierSite::EdgeExit(edge.psi_edge));
                for source in edge.provenance.iter().skip(1) {
                    let PsiProvenance::Edge(source_edge) = *source else {
                        continue;
                    };
                    edge_rows.insert(source_edge, (block.id, edge.psi_edge, false));
                    sites.push(OwnershipFrontierSite::EdgeEntry(source_edge));
                    sites.push(OwnershipFrontierSite::EdgeExit(source_edge));
                }
            }
            match &node.operation {
                // Return/crash edges retain entry state only — the custody
                // arriving at the terminator, before its cleanup actions.
                O::Return { psi_edge, .. }
                | O::ReturnUnit { psi_edge, .. }
                | O::ReturnStructural { psi_edge, .. }
                | O::Crash { psi_edge, .. } => {
                    edge_rows.insert(*psi_edge, (block.id, *psi_edge, false));
                    sites.push(OwnershipFrontierSite::EdgeEntry(*psi_edge));
                }
                _ => {}
            }
        }
        plans.insert(block.id, plan);
    }

    let mut operation_position: BTreeMap<OperationId, (BlockId, usize)> = BTreeMap::new();
    for (block, plan) in &plans {
        for (index, (operation, _)) in plan.operations.iter().enumerate() {
            operation_position.insert(*operation, (*block, index));
        }
    }
    // The custody state at a block's tail: its last operation's exit, or the
    // block entry when the block holds no operation.
    let block_tail = |block: &BlockId| -> OwnershipFrontierSite {
        plans
            .get(block)
            .and_then(|plan| plan.operations.last())
            .map(|(operation, _)| OwnershipFrontierSite::OperationExit(*operation))
            .unwrap_or(OwnershipFrontierSite::BlockEntry(*block))
    };

    let mut owned: BTreeMap<OwnershipFrontierSite, bool> =
        sites.iter().map(|site| (*site, true)).collect();
    // The rules are monotone over `owned`, so iterating from all-owned to
    // stability computes the greatest fixpoint in at most `sites` rounds.
    for _ in 0..=sites.len() {
        let mut changed = false;
        for site in &sites {
            let next = match *site {
                OwnershipFrontierSite::BlockEntry(block) => {
                    block != function.entry
                        && in_edges.get(&block).is_some_and(|entries| {
                            entries
                                .iter()
                                .all(|edge| owned[&OwnershipFrontierSite::EdgeExit(*edge)])
                        })
                }
                OwnershipFrontierSite::OperationEntry(operation) => operation_position
                    .get(&operation)
                    .is_some_and(|(block, index)| {
                        if *index == 0 {
                            owned[&OwnershipFrontierSite::BlockEntry(*block)]
                        } else {
                            let previous = plans[block].operations[*index - 1].0;
                            owned[&OwnershipFrontierSite::OperationExit(previous)]
                        }
                    }),
                OwnershipFrontierSite::OperationExit(operation) => operation_position
                    .get(&operation)
                    .is_some_and(|(block, index)| {
                        owned[&OwnershipFrontierSite::OperationEntry(operation)]
                            || plans[block].operations[*index].1
                    }),
                OwnershipFrontierSite::EdgeEntry(edge) => edge_rows
                    .get(&edge)
                    .is_some_and(|(source, _, _)| owned[&block_tail(source)]),
                OwnershipFrontierSite::EdgeExit(edge) => {
                    edge_rows
                        .get(&edge)
                        .is_some_and(|(source, canonical, row_zero)| {
                            owned[&block_tail(source)]
                                && !(*row_zero && edge_disposes.contains(canonical))
                        })
                }
            };
            if owned[site] != next {
                owned.insert(*site, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    owned
        .into_iter()
        .filter_map(|(site, owned)| owned.then_some(site))
        .collect()
}
