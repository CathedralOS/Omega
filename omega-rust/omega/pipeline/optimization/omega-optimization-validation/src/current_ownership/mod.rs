//! Optimizer module role: executable entrance. Current ownership-frontier reconstruction and independent replay.
//!
//! Entry reconstruction, CFG replay, frontier mutation, cleanup validation,
//! structural placement, and partial-affine residual accounting descend into
//! named leaves. This entrance owns the reconstruction-to-replay join.

use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::{AbstractFunctionResult, AbstractOperation as O};
use omega_optimization_unit::{OptimizationBlock, PsiOptimizationFunction};
use psi_core::{BlockId, ClaimId, MachineId, PlaceId, StructuralTypeId};
use psi_terminal::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralAffineDiscard, StructuralFieldType,
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

use crate::OptimizationUnitValidationError;

mod cleanup;
mod model;
mod mutations;
mod replay;
mod residuals;
mod structural;

use cleanup::*;
use model::*;
use mutations::*;
use residuals::*;
use structural::*;

pub(super) fn validate_current_ownership_frontier(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &OptimizationBlock>,
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<psi_core::BoundaryMachineId, &BoundaryMachineDeclaration>,
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
