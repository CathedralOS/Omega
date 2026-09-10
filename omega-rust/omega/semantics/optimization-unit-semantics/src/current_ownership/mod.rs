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

use abstract_operations::{AbstractFunctionResult, AbstractOperation as O};
use optimization_unit::{OptimizationBlock, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, ClaimId, MachineId, PlaceId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralAffineDiscard, StructuralFieldType,
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

use crate::OptimizationUnitValidationError;

mod block_parameters;
mod cleanup;
mod continuation;
mod model;
mod mutations;
mod replay;
mod residuals;
mod structural;

use block_parameters::bind_owned_parameters;
pub(crate) use block_parameters::parameter_establishment_order;
use cleanup::*;
pub(crate) use continuation::valid_partial_continuation_complement;
use continuation::{apply_edge_partial_affine_discards, validate_partial_continuation_roster};
use model::*;
use mutations::*;
use residuals::*;
use structural::*;

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
) -> Result<(), OptimizationUnitValidationError> {
    let entry = model::reconstruct_entry_ownership(function);
    replay::validate_current_ownership_cfg(
        function,
        blocks,
        successors,
        functions,
        boundary_machines,
        structural_types,
        entry,
    )
}
