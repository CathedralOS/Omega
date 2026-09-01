use omega_optimization_core::{
    NativeOptimizationProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
};
use omega_target::NativeTarget;
use omega_target_operations::BoundaryScalarArgument;
use psi_core::{BoundaryMachineId, MachineId, OperationId};
use psi_terminal::TerminalPsiIdentity;

use crate::NativeSelectedProviderPlanDigest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeByteSpan {
    offset: usize,
    byte_count: usize,
}

impl NativeByteSpan {
    pub const fn from_replayed_parts(offset: usize, byte_count: usize) -> Self {
        Self { offset, byte_count }
    }

    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn byte_count(self) -> usize {
        self.byte_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCompilerBuiltinCatalogIdentity {
    LinuxElfV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedBoundaryOccurrence {
    terminal: TerminalPsiIdentity,
    machine: MachineId,
    operation: OperationId,
    boundary: BoundaryMachineId,
    operation_ordinal: usize,
    identity: OptimizedBoundaryOccurrenceIdentity,
}

impl OptimizedBoundaryOccurrence {
    pub const fn terminal(&self) -> TerminalPsiIdentity {
        self.terminal
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn operation(&self) -> OperationId {
        self.operation
    }

    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }

    pub const fn operation_ordinal(&self) -> usize {
        self.operation_ordinal
    }

    pub const fn identity(&self) -> OptimizedBoundaryOccurrenceIdentity {
        self.identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeIdentityOptimizationProjection {
    terminal: TerminalPsiIdentity,
    boundary_occurrences: Vec<OptimizedBoundaryOccurrence>,
    identity: NativeOptimizationProjectionIdentity,
}

impl NativeIdentityOptimizationProjection {
    pub const fn terminal(&self) -> TerminalPsiIdentity {
        self.terminal
    }

    pub fn boundary_occurrences(&self) -> &[OptimizedBoundaryOccurrence] {
        &self.boundary_occurrences
    }

    pub const fn identity(&self) -> NativeOptimizationProjectionIdentity {
        self.identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryTraitSettlement {
    occurrence: OptimizedBoundaryOccurrence,
    requirement_identity: String,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    catalog: NativeCompilerBuiltinCatalogIdentity,
    execution: omega_target_operations::BoundaryExecutionBinding,
    realization: omega_target_operations::BoundaryRealization,
    scalar_argument: BoundaryScalarArgument,
    identity: [u8; 32],
}

impl BoundaryTraitSettlement {
    pub const fn occurrence(&self) -> &OptimizedBoundaryOccurrence {
        &self.occurrence
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn selected_plan_digest(&self) -> NativeSelectedProviderPlanDigest {
        self.selected_plan_digest
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn catalog(&self) -> NativeCompilerBuiltinCatalogIdentity {
        self.catalog
    }

    pub const fn execution(&self) -> omega_target_operations::BoundaryExecutionBinding {
        self.execution
    }

    pub const fn realization(&self) -> &omega_target_operations::BoundaryRealization {
        &self.realization
    }

    pub const fn scalar_argument(&self) -> BoundaryScalarArgument {
        self.scalar_argument
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub fn into_parts(self) -> BoundaryTraitSettlementParts {
        BoundaryTraitSettlementParts {
            occurrence: self.occurrence,
            requirement_identity: self.requirement_identity,
            selected_plan_digest: self.selected_plan_digest,
            target: self.target,
            catalog: self.catalog,
            execution: self.execution,
            realization: self.realization,
            scalar_argument: self.scalar_argument,
            identity: self.identity,
        }
    }

    pub fn from_replayed_parts(parts: BoundaryTraitSettlementParts) -> Self {
        parts.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalChildParent {
    BoundaryTraitSettlement(BoundaryTraitSettlement),
}

impl PhysicalChildParent {
    pub const fn identity(&self) -> &[u8; 32] {
        match self {
            Self::BoundaryTraitSettlement(settlement) => settlement.identity(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalRelocationDisposition {
    DirectInstructionBytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePhysicalChild {
    parent: PhysicalChildParent,
    projection: NativeOptimizationProjectionIdentity,
    occurrence: OptimizedBoundaryOccurrenceIdentity,
    machine_span: NativeByteSpan,
    object_span: NativeByteSpan,
    final_image_span: NativeByteSpan,
    machine_bytes_digest: [u8; 32],
    object_bytes_digest: [u8; 32],
    final_image_bytes_digest: [u8; 32],
    relocation: PhysicalRelocationDisposition,
    identity: [u8; 32],
}

impl NativePhysicalChild {
    pub const fn parent(&self) -> &PhysicalChildParent {
        &self.parent
    }

    pub const fn projection(&self) -> NativeOptimizationProjectionIdentity {
        self.projection
    }

    pub const fn occurrence(&self) -> OptimizedBoundaryOccurrenceIdentity {
        self.occurrence
    }

    pub const fn machine_span(&self) -> NativeByteSpan {
        self.machine_span
    }

    pub const fn object_span(&self) -> NativeByteSpan {
        self.object_span
    }

    pub const fn final_image_span(&self) -> NativeByteSpan {
        self.final_image_span
    }

    pub const fn relocation(&self) -> PhysicalRelocationDisposition {
        self.relocation
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub fn into_parts(self) -> NativePhysicalChildParts {
        NativePhysicalChildParts {
            parent: self.parent,
            projection: self.projection,
            occurrence: self.occurrence,
            machine_span: self.machine_span,
            object_span: self.object_span,
            final_image_span: self.final_image_span,
            machine_bytes_digest: self.machine_bytes_digest,
            object_bytes_digest: self.object_bytes_digest,
            final_image_bytes_digest: self.final_image_bytes_digest,
            relocation: self.relocation,
            identity: self.identity,
        }
    }

    pub fn from_replayed_parts(parts: NativePhysicalChildParts) -> Self {
        parts.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePhysicalEvidence {
    projection: NativeIdentityOptimizationProjection,
    children: Vec<NativePhysicalChild>,
    identity: [u8; 32],
}

impl NativePhysicalEvidence {
    pub const fn projection(&self) -> &NativeIdentityOptimizationProjection {
        &self.projection
    }

    pub fn children(&self) -> &[NativePhysicalChild] {
        &self.children
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub fn into_parts(self) -> NativePhysicalEvidenceParts {
        NativePhysicalEvidenceParts {
            projection: self.projection,
            children: self.children,
            identity: self.identity,
        }
    }

    pub fn from_replayed_parts(parts: NativePhysicalEvidenceParts) -> Self {
        Self {
            projection: parts.projection,
            children: parts.children,
            identity: parts.identity,
        }
    }
}

#[derive(Debug)]
pub struct BoundaryTraitSettlementParts {
    pub occurrence: OptimizedBoundaryOccurrence,
    pub requirement_identity: String,
    pub selected_plan_digest: NativeSelectedProviderPlanDigest,
    pub target: NativeTarget,
    pub catalog: NativeCompilerBuiltinCatalogIdentity,
    pub execution: omega_target_operations::BoundaryExecutionBinding,
    pub realization: omega_target_operations::BoundaryRealization,
    pub scalar_argument: BoundaryScalarArgument,
    pub identity: [u8; 32],
}

impl From<BoundaryTraitSettlementParts> for BoundaryTraitSettlement {
    fn from(parts: BoundaryTraitSettlementParts) -> Self {
        Self {
            occurrence: parts.occurrence,
            requirement_identity: parts.requirement_identity,
            selected_plan_digest: parts.selected_plan_digest,
            target: parts.target,
            catalog: parts.catalog,
            execution: parts.execution,
            realization: parts.realization,
            scalar_argument: parts.scalar_argument,
            identity: parts.identity,
        }
    }
}

#[derive(Debug)]
pub struct NativePhysicalChildParts {
    pub parent: PhysicalChildParent,
    pub projection: NativeOptimizationProjectionIdentity,
    pub occurrence: OptimizedBoundaryOccurrenceIdentity,
    pub machine_span: NativeByteSpan,
    pub object_span: NativeByteSpan,
    pub final_image_span: NativeByteSpan,
    pub machine_bytes_digest: [u8; 32],
    pub object_bytes_digest: [u8; 32],
    pub final_image_bytes_digest: [u8; 32],
    pub relocation: PhysicalRelocationDisposition,
    pub identity: [u8; 32],
}

#[derive(Debug)]
pub struct NativePhysicalEvidenceParts {
    pub projection: NativeIdentityOptimizationProjection,
    pub children: Vec<NativePhysicalChild>,
    pub identity: [u8; 32],
}

impl From<NativePhysicalChildParts> for NativePhysicalChild {
    fn from(parts: NativePhysicalChildParts) -> Self {
        Self {
            parent: parts.parent,
            projection: parts.projection,
            occurrence: parts.occurrence,
            machine_span: parts.machine_span,
            object_span: parts.object_span,
            final_image_span: parts.final_image_span,
            machine_bytes_digest: parts.machine_bytes_digest,
            object_bytes_digest: parts.object_bytes_digest,
            final_image_bytes_digest: parts.final_image_bytes_digest,
            relocation: parts.relocation,
            identity: parts.identity,
        }
    }
}

pub(super) fn native_byte_span(offset: usize, byte_count: usize) -> NativeByteSpan {
    NativeByteSpan::from_replayed_parts(offset, byte_count)
}

pub(super) fn optimized_boundary_occurrence(
    terminal: TerminalPsiIdentity,
    machine: MachineId,
    operation: OperationId,
    boundary: BoundaryMachineId,
    operation_ordinal: usize,
    identity: OptimizedBoundaryOccurrenceIdentity,
) -> OptimizedBoundaryOccurrence {
    OptimizedBoundaryOccurrence {
        terminal,
        machine,
        operation,
        boundary,
        operation_ordinal,
        identity,
    }
}

pub(super) fn identity_projection(
    terminal: TerminalPsiIdentity,
    boundary_occurrences: Vec<OptimizedBoundaryOccurrence>,
    identity: NativeOptimizationProjectionIdentity,
) -> NativeIdentityOptimizationProjection {
    NativeIdentityOptimizationProjection {
        terminal,
        boundary_occurrences,
        identity,
    }
}

pub(super) fn native_physical_evidence(
    projection: NativeIdentityOptimizationProjection,
    children: Vec<NativePhysicalChild>,
    identity: [u8; 32],
) -> NativePhysicalEvidence {
    NativePhysicalEvidence {
        projection,
        children,
        identity,
    }
}
