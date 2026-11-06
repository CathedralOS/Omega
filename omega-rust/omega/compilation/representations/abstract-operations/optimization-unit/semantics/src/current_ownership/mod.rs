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

use optimization_unit::{
    OptimizationBlock, OwnershipFrontierFact, OwnershipFrontierLiveClaim,
    OwnershipFrontierOwnedPlace, OwnershipFrontierPartialCustody, OwnershipFrontierSnapshot,
    PsiOptimizationFunction,
};
use semantic_vocabulary::{BlockId, ClaimId, MachineId, PlaceId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeDeclaration,
};

use crate::OptimizationUnitValidationError;
use crate::unit_validation::references::LiveReference;

pub(crate) mod address_joins;
mod block_parameters;
mod call_accesses;
mod cleanup;
pub(crate) use call_accesses::{CallAccesses, argument_storage_is_closed};
mod continuation;
mod mutations;
pub(crate) mod references;
mod replay;
mod residuals;
mod storage_regions;
pub(crate) use storage_regions::storage_cannot_retain_loan;
mod structural;
pub(crate) use structural::{
    constructor_arguments, partial_affine_root_is_drained, partial_affine_root_type,
    projected_paths_are_fully_consumed,
};

pub(crate) use block_parameters::parameter_establishment_order;
use block_parameters::{consume_edge_sources, establish_edge_parameters};

pub(super) fn validate_current_ownership_frontier_collecting(
    psi: terminal_psi::TerminalPsiIdentity,
    identities: &terminal_psi::TerminalIdentityTable,
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<
        semantic_vocabulary::BoundaryMachineId,
        &BoundaryMachineDeclaration,
    >,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    call_accesses: &mut CallAccesses,
    facts: &mut Vec<OwnershipFrontierFact>,
) -> Result<
    Vec<terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement>,
    OptimizationUnitValidationError,
> {
    let entry = reconstruct_entry_ownership(function, structural_types)?;
    let mut outputs = reference_output_builder(function, structural_types, &entry.live_references)?;
    replay::validate_current_ownership_cfg(
        function,
        identities,
        dynamic_dispatch,
        blocks,
        predecessors,
        functions,
        boundary_machines,
        structural_types,
        call_accesses,
        entry,
        psi,
        facts,
        &mut outputs,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn reconstruct_reference_outputs(
    psi: terminal_psi::TerminalPsiIdentity,
    identities: &terminal_psi::TerminalIdentityTable,
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<
        semantic_vocabulary::BoundaryMachineId,
        &BoundaryMachineDeclaration,
    >,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    call_accesses: &mut CallAccesses,
) -> Result<crate::reference_outputs::ReferenceBodyCompletion, OptimizationUnitValidationError> {
    let entry = reconstruct_entry_ownership(function, structural_types)?;
    let mut outputs = reference_output_builder(function, structural_types, &entry.live_references)?;
    let mut facts = Vec::new();
    let compatibility_requirements = replay::validate_current_ownership_cfg(
        function,
        identities,
        dynamic_dispatch,
        blocks,
        predecessors,
        functions,
        boundary_machines,
        structural_types,
        call_accesses,
        entry,
        psi,
        &mut facts,
        &mut outputs,
    )?;
    // The replay returns only after complete CFG joins and return coverage.
    // A pure returned reference can have an independently checked interpretation
    // even when this optional stored-output graph cannot represent its parent.
    let unconditional = compatibility_requirements.is_empty();
    Ok(crate::reference_outputs::ReferenceBodyCompletion {
        unconditional,
        outputs: outputs
            .seal(function, structural_types, compatibility_requirements)
            .map_err(str::to_owned),
    })
}

fn reference_output_builder(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    entry: &[LiveReference],
) -> Result<crate::reference_outputs::ReferenceOutputBuilder, OptimizationUnitValidationError> {
    let declarations = structural_types
        .values()
        .copied()
        .cloned()
        .collect::<Vec<_>>();
    crate::reference_outputs::ReferenceOutputBuilder::new(function, &declarations, entry).map_err(
        |reason| OptimizationUnitValidationError::CurrentReferenceCustodyViolation {
            machine: function.machine,
            block: function.entry,
            node: None,
            reason,
        },
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

impl CurrentOwnership {
    fn snapshot(&self) -> OwnershipFrontierSnapshot {
        OwnershipFrontierSnapshot {
            claims: self
                .claims
                .iter()
                .map(|(claim, live)| OwnershipFrontierLiveClaim {
                    claim: *claim,
                    input: live.input,
                    path: live.path.clone(),
                    multiplicity: live.multiplicity,
                })
                .collect(),
            owned_places: self
                .owned_places
                .iter()
                .map(|(place, multiplicity)| OwnershipFrontierOwnedPlace {
                    place: *place,
                    multiplicity: *multiplicity,
                })
                .collect(),
            partial_custody: self
                .partial_custody_paths
                .iter()
                .map(|(place, moved_paths)| OwnershipFrontierPartialCustody {
                    place: *place,
                    moved_paths: moved_paths.iter().cloned().collect(),
                })
                .collect(),
        }
    }
}

fn reconstruct_entry_ownership(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<CurrentOwnership, OptimizationUnitValidationError> {
    let mut claims = BTreeMap::<ClaimId, LiveClaim>::new();
    for claim in &function.entry_claim_declarations {
        let parameter = function
            .structural_parameter(claim.input)
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
        let parameter = function.structural_parameter(claim.input.root);
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
