use machine_code::{
    X86_64SelectedStructuralUnitCallFootprint, X86_64StructuralUnitInternalControlFixup,
};
use optimization_core::{Optimization, OptimizationSelectionIdentity};
use physical_instructions::PostAllocationMachineIdentity;
use physical_instructions::{Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity};
use register_model::{RegisterUnitId, RegisterViewId};
use selected_instructions::{MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionId};
use semantic_vocabulary::{MachineId, OperationId};

use crate::PostAllocationMachineOptimizationCustody;

pub use machine_code::{
    SelectedFormEncodingIdentity, SelectedFormInternalMachineFixup,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredControlEncodingReason {
    RequiresResolvedBranchLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormDecodedFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub implicit_clobbers: Vec<RegisterUnitId>,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedFormEncodingState {
    Encoded {
        bytes: Vec<u8>,
        footprint: Box<SelectedFormDecodedFootprint>,
    },
    DeferredControl {
        reason: DeferredControlEncodingReason,
    },
    UnresolvedInternalMachineCall {
        bytes: Vec<u8>,
        footprint: Box<SelectedFormDecodedFootprint>,
        fixup: SelectedFormInternalMachineFixup,
    },
}

/// Closed rule-neutral disposition consumed by generic encoding and layout.
/// Rule-local plans remain the authority; this value is only their exact row
/// projection under authenticated optimization custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedFormMachineDisposition {
    RetainedV1,
    Aarch64ElidedCompareI64ZeroV1 {
        consumer: SelectedInstructionId,
    },
    Aarch64FusedBranchNonZeroToCbnzV1 {
        compare: SelectedInstructionId,
        source_read: post_allocation_machine_to_post_allocation_machine::QualifiedPhysicalRead,
    },
    Aarch64ElidedSameViewCopyI64V1 {
        consumer: SelectedInstructionId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormEncodingRow {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub machine_disposition: SelectedFormMachineDisposition,
    pub state: SelectedFormEncodingState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitCallEncodingRow {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub bytes: Vec<u8>,
    pub footprint: Box<X86_64SelectedStructuralUnitCallFootprint>,
    pub fixup: X86_64StructuralUnitInternalControlFixup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitFunctionEncoding {
    pub machine: MachineId,
    pub block: selected_instructions::SelectedBlockId,
    pub call: Option<SelectedStructuralUnitCallEncodingRow>,
    pub return_instruction: SelectedFormEncodingRow,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectedFormEncodingCounts {
    pub ordinary_encoded: u64,
    pub ordinary_deferred_control: u64,
    pub ordinary_encoded_call_templates: u64,
    pub ordinary_deferred_internal_control: u64,
    pub ordinary_internal_fixups: u64,
    pub structural_encoded_call_templates: u64,
    pub structural_encoded_returns: u64,
    pub structural_deferred_internal_control: u64,
    pub structural_internal_fixups: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedFormMachineOptimizationCustody {
    pub(super) selections: OptimizationSelectionIdentity,
    pub(super) post_allocation_machine_selections: OptimizationSelectionIdentity,
    pub(super) fusion: Aarch64CbnzFusionIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedFormMovnOptimizationCustody {
    pub(super) selections: OptimizationSelectionIdentity,
    pub(super) post_allocation_machine_selections: OptimizationSelectionIdentity,
    pub(super) materialization: Aarch64MovnMaterializationIdentity,
}

impl SelectedFormMovnOptimizationCustody {
    /// Reconstruct custody decoded from an independently validated layout artifact.
    pub const fn from_parts(
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        materialization: Aarch64MovnMaterializationIdentity,
    ) -> Self {
        Self {
            selections,
            post_allocation_machine_selections,
            materialization,
        }
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }

    pub const fn materialization(self) -> Aarch64MovnMaterializationIdentity {
        self.materialization
    }
}

impl SelectedFormMachineOptimizationCustody {
    /// Reconstruct custody decoded from an independently validated layout artifact.
    pub const fn from_parts(
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        fusion: Aarch64CbnzFusionIdentity,
    ) -> Self {
        Self {
            selections,
            post_allocation_machine_selections,
            fusion,
        }
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }

    pub const fn fusion(self) -> Aarch64CbnzFusionIdentity {
        self.fusion
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedSelectedFormEncoding {
    pub(super) selected: selected_instructions::SelectedInstructionPlanIdentity,
    pub(super) machine: PostAllocationMachineIdentity,
    pub(super) post_allocation_machine_optimization:
        Option<PostAllocationMachineOptimizationCustody>,
    pub(super) identity: SelectedFormEncodingIdentity,
    pub(super) rows: std::sync::Arc<Vec<SelectedFormEncodingRow>>,
    pub(super) structural_unit_functions:
        std::sync::Arc<Vec<SelectedStructuralUnitFunctionEncoding>>,
    pub(super) counts: SelectedFormEncodingCounts,
}

impl StagedOptimizedSelectedFormEncoding {
    pub const fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> PostAllocationMachineIdentity {
        self.machine
    }

    pub const fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.post_allocation_machine_optimization
    }

    /// Compatibility projection for layout routes that still name CBNZ.
    pub const fn machine_optimization(&self) -> Option<SelectedFormMachineOptimizationCustody> {
        match self.post_allocation_machine_optimization {
            Some(custody)
                if matches!(
                    custody.optimization(),
                    Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
                ) =>
            {
                Some(SelectedFormMachineOptimizationCustody {
                    selections: custody.selections(),
                    post_allocation_machine_selections: custody
                        .post_allocation_machine_selections(),
                    fusion: Aarch64CbnzFusionIdentity::from_bytes(custody.artifact_identity()),
                })
            }
            _ => None,
        }
    }

    /// Compatibility projection for layout routes that still name MOVN.
    pub const fn movn_optimization(&self) -> Option<SelectedFormMovnOptimizationCustody> {
        match self.post_allocation_machine_optimization {
            Some(custody)
                if matches!(
                    custody.optimization(),
                    Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                ) =>
            {
                Some(SelectedFormMovnOptimizationCustody {
                    selections: custody.selections(),
                    post_allocation_machine_selections: custody
                        .post_allocation_machine_selections(),
                    materialization: Aarch64MovnMaterializationIdentity::from_bytes(
                        custody.artifact_identity(),
                    ),
                })
            }
            _ => None,
        }
    }

    pub const fn identity(&self) -> SelectedFormEncodingIdentity {
        self.identity
    }

    pub fn rows(&self) -> &[SelectedFormEncodingRow] {
        &self.rows
    }

    pub fn structural_unit_functions(&self) -> &[SelectedStructuralUnitFunctionEncoding] {
        &self.structural_unit_functions
    }

    pub const fn counts(&self) -> SelectedFormEncodingCounts {
        self.counts
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn rows_mut(&mut self) -> &mut [SelectedFormEncodingRow] {
        std::sync::Arc::make_mut(&mut self.rows).as_mut_slice()
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn structural_unit_functions_mut(
        &mut self,
    ) -> &mut [SelectedStructuralUnitFunctionEncoding] {
        std::sync::Arc::make_mut(&mut self.structural_unit_functions).as_mut_slice()
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn counts_mut(&mut self) -> &mut SelectedFormEncodingCounts {
        &mut self.counts
    }
}
