use omega_isa_x86_64::{
    X86_64SelectedStructuralUnitCallFootprint, X86_64StructuralUnitInternalControlFixup,
};
use omega_machine_optimizer::{Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity};
use omega_optimization_core::Optimization;
use omega_selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, SelectedBlockId, SelectedInstructionId,
};
use omega_target::NativeTarget;
use psi_core::{EdgeId, MachineId, OperationId};

use crate::{
    PostAllocationMachineOptimizationCustody, SelectedFormEncodingIdentity,
    SelectedFormMachineOptimizationCustody, SelectedFormMovnOptimizationCustody,
};

use super::identity::layout_identity;

/// Required-stage baseline layout for the currently admitted three-block
/// conditional. This is a visible policy identity, not an optimization level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedFunctionLayoutPolicy {
    EntryThenZeroFallthroughThenNonzeroV1,
    EntryThenNotLessFallthroughThenLessV1,
    SingleEntryBlockV1,
    /// A separate zero-VReg structural roster. Every function has one entry
    /// block containing either `ReturnUnit`, or one unresolved whole-root
    /// `CallUnit` template followed by `ReturnUnit`.
    StructuralUnitCallThenReturnSingleEntryBlockV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedConditionalBranchPredicate {
    NonZeroV1,
    U64LessThanV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResolvedSelectedFormLayoutIdentity(pub(super) [u8; 32]);

impl ResolvedSelectedFormLayoutIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConditionalBranchEvidence {
    pub predicate: ResolvedConditionalBranchPredicate,
    pub source_block: SelectedBlockId,
    pub when_taken_edge: EdgeId,
    pub when_taken_block: SelectedBlockId,
    pub when_taken_offset: u64,
    pub when_fallthrough_edge: EdgeId,
    pub when_fallthrough_block: SelectedBlockId,
    pub when_fallthrough_offset: u64,
    /// x86-64 measures from instruction end; AArch64 measures from the branch
    /// word address. The target decoder independently checks this convention.
    pub byte_displacement: i64,
    pub decoded_register_reads: Vec<omega_register_model::RegisterViewId>,
    pub decoded_effects: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedFormRow {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub branch: Option<Box<ResolvedConditionalBranchEvidence>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedBlockLayout {
    pub block: SelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub instructions: Vec<ResolvedSelectedFormRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedFunctionLayout {
    pub machine: MachineId,
    pub byte_count: u64,
    pub blocks: Vec<ResolvedSelectedBlockLayout>,
}

/// Function-relative custody for the canonical structural Unit call template.
/// The bytes deliberately retain their zero rel32 placeholder; `fixup` remains
/// unresolved until whole-text placement knows both MachineId coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStructuralUnitCallLayout {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub footprint: Box<X86_64SelectedStructuralUnitCallFootprint>,
    pub fixup: X86_64StructuralUnitInternalControlFixup,
}

/// Exact one-block function-relative span for the bounded structural Unit
/// route. A caller is 89 unresolved call bytes plus one `C3`; a leaf is the
/// single `C3` byte. This carrier grants neither section placement nor
/// executable-byte authority while `call.fixup` remains unresolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStructuralUnitFunctionLayout {
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub call: Option<ResolvedStructuralUnitCallLayout>,
    pub return_instruction: ResolvedSelectedFormRow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedResolvedSelectedFormLayout {
    pub(super) selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    pub(super) machine: omega_machine_optimizer::PostAllocationMachineIdentity,
    pub(super) pre_layout: SelectedFormEncodingIdentity,
    pub(super) post_allocation_machine_optimization:
        Option<PostAllocationMachineOptimizationCustody>,
    pub(super) target: NativeTarget,
    pub(super) policy: SelectedFunctionLayoutPolicy,
    pub(super) identity: ResolvedSelectedFormLayoutIdentity,
    pub(super) functions: Vec<ResolvedSelectedFunctionLayout>,
    pub(super) structural_unit_functions: Vec<ResolvedStructuralUnitFunctionLayout>,
}

impl StagedOptimizedResolvedSelectedFormLayout {
    pub const fn selected(&self) -> omega_selected_instructions::SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        self.machine
    }

    pub const fn pre_layout(&self) -> SelectedFormEncodingIdentity {
        self.pre_layout
    }

    pub const fn machine_optimization(&self) -> Option<SelectedFormMachineOptimizationCustody> {
        match self.post_allocation_machine_optimization {
            Some(custody)
                if matches!(
                    custody.optimization(),
                    Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
                ) =>
            {
                Some(SelectedFormMachineOptimizationCustody::from_parts(
                    custody.selections(),
                    custody.post_allocation_machine_selections(),
                    Aarch64CbnzFusionIdentity::from_bytes(custody.artifact_identity()),
                ))
            }
            _ => None,
        }
    }

    pub const fn movn_optimization(&self) -> Option<SelectedFormMovnOptimizationCustody> {
        match self.post_allocation_machine_optimization {
            Some(custody)
                if matches!(
                    custody.optimization(),
                    Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                ) =>
            {
                Some(SelectedFormMovnOptimizationCustody::from_parts(
                    custody.selections(),
                    custody.post_allocation_machine_selections(),
                    Aarch64MovnMaterializationIdentity::from_bytes(custody.artifact_identity()),
                ))
            }
            _ => None,
        }
    }

    pub const fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.post_allocation_machine_optimization
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn policy(&self) -> SelectedFunctionLayoutPolicy {
        self.policy
    }

    pub const fn identity(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.identity
    }

    pub fn functions(&self) -> &[ResolvedSelectedFunctionLayout] {
        &self.functions
    }

    pub fn structural_unit_functions(&self) -> &[ResolvedStructuralUnitFunctionLayout] {
        &self.structural_unit_functions
    }

    /// Rebuild this same resolved-layout representation after a separately
    /// validated, function-relative byte-layout transformation. This helper
    /// recomputes content identity but grants no authority to perform or
    /// validate the transformation itself.
    pub(crate) fn with_replayed_functions(
        &self,
        functions: Vec<ResolvedSelectedFunctionLayout>,
    ) -> Self {
        let identity = layout_identity(
            self.selected,
            self.machine,
            self.pre_layout,
            self.post_allocation_machine_optimization,
            self.target,
            self.policy,
            &functions,
            &self.structural_unit_functions,
        );
        Self {
            selected: self.selected,
            machine: self.machine,
            pre_layout: self.pre_layout,
            post_allocation_machine_optimization: self.post_allocation_machine_optimization,
            target: self.target,
            policy: self.policy,
            identity,
            functions,
            structural_unit_functions: self.structural_unit_functions.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn functions_mut(&mut self) -> &mut [ResolvedSelectedFunctionLayout] {
        &mut self.functions
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn structural_unit_functions_mut(
        &mut self,
    ) -> &mut [ResolvedStructuralUnitFunctionLayout] {
        &mut self.structural_unit_functions
    }
}
