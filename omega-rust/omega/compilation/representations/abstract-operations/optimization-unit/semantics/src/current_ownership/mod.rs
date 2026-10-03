//! Optimizer module role: executable entrance. Current ownership-frontier reconstruction and independent replay.
//!
//! Entry reconstruction, CFG replay, frontier mutation, cleanup validation,
//! structural placement, and partial-affine residual accounting descend into
//! named leaves. This entrance owns the reconstruction-to-replay join.
//!
//! This frontier tracks disposal obligations, not every available value.
//! Function validation first checks structural producer dominance and exact
//! source/result contracts, so unrestricted roots need no owned-place entry.
//! Claims remain independent obligations even when their root is unrestricted;
//! omitting a root must not bypass claim replay or weaken CFG joins.

use std::collections::{BTreeMap, BTreeSet};

use optimization_unit::{OptimizationBlock, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, ClaimId, MachineId, PlaceId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeDeclaration,
};

use crate::OptimizationUnitValidationError;
use crate::unit_validation::references::LiveReference;

mod address_joins;
mod block_parameters;
mod call_accesses;
mod cleanup;
pub(crate) use call_accesses::{CallAccesses, argument_storage_is_closed};
mod continuation;
mod mutations;
mod references;
mod replay;
mod residuals;
mod storage_regions;
mod structural;
pub(crate) use structural::partial_affine_root_type;

use block_parameters::bind_owned_parameters;
pub(crate) use block_parameters::parameter_establishment_order;
use continuation::apply_edge_partial_affine_discards;
pub(crate) use continuation::valid_partial_continuation_complement;

#[cfg(test)]
pub(super) fn validate_current_ownership_frontier(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &OptimizationBlock>,
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<
        semantic_vocabulary::BoundaryMachineId,
        &BoundaryMachineDeclaration,
    >,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    call_accesses: &CallAccesses,
) -> Result<(), OptimizationUnitValidationError> {
    let requirements = validate_current_ownership_frontier_collecting(
        function,
        blocks,
        successors,
        functions,
        boundary_machines,
        structural_types,
        call_accesses,
    )?;
    if let Some(row) = requirements.first() {
        let terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument {
            source, ..
        } = &row.demand
        else {
            unreachable!("derived storage demand");
        };
        return Err(
            OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed {
                machine: function.machine,
                block: row.position.0,
                node: row.position.1,
                place: *source,
            },
        );
    }
    Ok(())
}

pub(super) fn validate_current_ownership_frontier_collecting(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &OptimizationBlock>,
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<
        semantic_vocabulary::BoundaryMachineId,
        &BoundaryMachineDeclaration,
    >,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    call_accesses: &CallAccesses,
) -> Result<
    Vec<terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement>,
    OptimizationUnitValidationError,
> {
    let entry = reconstruct_entry_ownership(function, structural_types)?;
    replay::validate_current_ownership_cfg(
        function,
        blocks,
        successors,
        functions,
        boundary_machines,
        structural_types,
        call_accesses,
        entry,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveClaim {
    input: Option<PlaceId>,
    path: Vec<StructuralPathSegment>,
    multiplicity: Option<StructuralMultiplicity>,
}

/// Executable ownership reconstructed from current operations and signatures.
/// Immutable source snapshots and cached `OwnershipEvent` rows are not read.
/// `live_references` replays each outstanding loan's carrier location, root
/// origin, and parent; joins compare it exactly like the owned roster.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CurrentOwnership {
    claims: BTreeMap<ClaimId, LiveClaim>,
    owned_places: BTreeMap<PlaceId, StructuralMultiplicity>,
    partial_custody_paths: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>>,
    live_references: Vec<LiveReference>,
}

fn reconstruct_entry_ownership(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<CurrentOwnership, OptimizationUnitValidationError> {
    let mut claims = BTreeMap::<ClaimId, LiveClaim>::new();
    for claim in &function.entry_claim_declarations {
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input)
            .expect("structural signature validation precedes current ownership replay");
        claims.insert(
            claim.claim,
            LiveClaim {
                input: Some(claim.input),
                path: claim.path.clone(),
                multiplicity: Some(if claim.path.is_empty() {
                    parameter.multiplicity
                } else {
                    StructuralMultiplicity::Linear
                }),
            },
        );
    }
    for claim in &function.content_entry_claims {
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input.root);
        claims.entry(claim.claim).or_insert(LiveClaim {
            input: parameter.map(|_| claim.input.root),
            path: Vec::new(),
            multiplicity: parameter.map(|parameter| parameter.multiplicity),
        });
    }
    Ok(CurrentOwnership {
        claims,
        owned_places: function
            .structural_parameters
            .iter()
            .filter_map(|parameter| {
                (parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                    .then_some((parameter.place, parameter.multiplicity))
            })
            .collect(),
        partial_custody_paths: BTreeMap::new(),
        live_references: references::entry_references(function, structural_types)?,
    })
}

pub(crate) use address_joins::validate_compatibility as validate_descriptor_compatibility;
