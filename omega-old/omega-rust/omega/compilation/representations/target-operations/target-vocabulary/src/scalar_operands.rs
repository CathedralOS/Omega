//! Scalar and call operands shared by the current native forms.

pub use crate::TargetDescriptorOrigin as LegalizedDescriptorOrigin;
use calling_conventions::{CallPlan, ValuePlacement};
use native_vocabulary::provenance::ValueDefinitionSite;
use semantic_vocabulary::{
    BoundaryMachineId, ClosedScalarType, ObligationId, StructuralFieldId, ValueId,
};
use terminal_psi::CrashRouteBucket;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalizedValueDefinition {
    pub value: ValueId,
    pub scalar_type: ClosedScalarType,
    pub definition_site: ValueDefinitionSite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalizedRuntimeIndexOperand {
    pub operand: native_vocabulary::values::AbstractResult,
    pub stride: u32,
    pub extent: Option<u64>,
    pub obligation: semantic_vocabulary::ObligationId,
    pub accepted_fact: optimization_core::AcceptedObligationFactIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarCall {
    pub descriptor_origins: Vec<LegalizedDescriptorOrigin>, // Conservative backing union for returned descriptors.
    pub access_bindings: Vec<crate::TargetAccessCallBinding>,
    pub source: crate::NativeCallOrigin,
    pub target: crate::NativeCallTarget,
    pub call_plan: CallPlan,
    pub arguments: Vec<LegalizedScalarArgument>,
    pub result_placement: Option<ValuePlacement>,
    pub structural_result: Option<terminal_psi::StructuralOperationResult>,
    pub claim_transfers: Vec<terminal_psi::ClaimTransfer>,
    pub requirement_obligations: Vec<ObligationId>,
    pub crash_continuations: Vec<CrashRouteBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarArgument {
    Scalar {
        source: ValueId,
        placement: ValuePlacement,
    },
    Structural {
        semantic: terminal_psi::StructuralArgument,
        target: crate::TargetStructuralArgument,
        indices: Vec<LegalizedRuntimeIndexOperand>, // One joined row per `RuntimeIndex` segment in `semantic.path`, in path order.
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedNormalizedForeignCall {
    pub boundary: BoundaryMachineId,
    pub provider_execution: crate::ExternalExecutionBinding,
    pub binding: crate::NormalizedForeignCallBinding,
    pub scalar_arguments: Vec<crate::NormalizedForeignScalarArgument>,
    pub structural_arguments: Vec<crate::NormalizedForeignStructuralArgument>,
    pub structural_indices: Vec<Vec<LegalizedRuntimeIndexOperand>>, // Exact bound operands, in structural argument order and then path order.
    pub result: crate::TargetBoundaryResult,
    pub callbacks: Vec<crate::TargetNativeCallbackArgument>,
    pub descriptor_input: Option<crate::DescriptorInputGuard>, // Target producer's count guard for a descriptor-reading import.
}

impl LegalizedRuntimeIndexOperand {
    /// Compares the selector's semantic identity, geometry and proof key across
    /// the target handoff. The target operand's ABI location is checked by its
    /// own source-correspondence rules, not by this semantic comparison.
    pub fn matches_target(&self, target: &crate::TargetStructuralRuntimeIndex) -> bool {
        self.operand.value == target.operand.source_value()
            && self.operand.scalar_type == target.operand.scalar_type()
            && self.stride == target.stride
            && self.extent == target.extent
            && self.obligation == target.obligation
    }
}

impl LegalizedScalarArgument {
    pub fn scalar_source(&self) -> Option<ValueId> {
        match self {
            Self::Scalar { source, .. } => Some(*source),
            Self::Structural { .. } => None,
        }
    }
    pub fn placement(&self) -> &ValuePlacement {
        match self {
            Self::Scalar { placement, .. } => placement,
            Self::Structural { target, .. } => &target.destination,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalizedStructuralCasePayload {
    pub field: StructuralFieldId,
    pub field_byte_offset: u32,
    pub parameter: crate::LegalizedValueDefinition, // Target-block scalar arrival defined after observing the selected case payload.
}
