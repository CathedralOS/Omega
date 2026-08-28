use std::fmt::Write;

use omega_optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, Optimization,
    OptimizationExecutionPhase, OptimizationSelectionIdentity, OptimizationSelections,
    OptimizationWorkBudget, PostAllocationOptimizationManifestIdentity,
    PrePhysicalOptimizationManifestIdentity, SelectedLoweringOptimizationCompletionIdentity,
};
use omega_regalloc::ValidatedTerminalSelectedAnalysis;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;

use crate::{
    OptimizedPostAllocationMachineOptimizationError, OptimizedPostAllocationMachinePipelineError,
    OptimizedPostSelectedLoweringHomeCustodyError, OptimizedRegisterHomeCustodyError,
    OptimizedResolvedSelectedFormLayoutError, OptimizedSelectedFormEncodingError,
    OptimizedX86BranchRelaxationError, StagedOptimizedAarch64CbnzFusion,
    StagedOptimizedAarch64CbnzFusionCustodyReceipt,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterSelectedLowering, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, StagedOptimizedX86BranchRelaxation,
    TerminalResolvedSelectedFormLayoutIdentity, TerminalSelectedFormEncodingIdentity,
    TerminalSelectedFunctionLayoutPolicy, TerminalWholeFunctionExitContractError,
    TerminalWholeFunctionExitContractIdentity, TerminalX86BranchRelaxationIdentity,
    ValidatedTerminalWholeFunctionExitContract,
    stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion,
    stage_optimized_post_allocation_machine_plan,
    stage_optimized_post_allocation_machine_plan_after_selected_lowering,
    stage_optimized_resolved_selected_form_layout,
    stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    stage_optimized_x86_branch_relaxation, stage_terminal_whole_function_exit_contract,
    stage_terminal_whole_function_exit_contract_after_aarch64_cbnz_fusion,
    stage_terminal_whole_function_exit_contract_after_x86_branch_relaxation,
    validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody,
    validate_optimized_aarch64_cbnz_fusion_custody,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
    validate_optimized_register_home_custody, validate_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    validate_optimized_x86_branch_relaxation, validate_terminal_whole_function_exit_contract,
    validate_terminal_whole_function_exit_contract_after_aarch64_cbnz_fusion,
    validate_terminal_whole_function_exit_contract_after_x86_branch_relaxation,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGFRM\0\0";
const MANIFEST_VERSION: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationStage {
    ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationScope {
    FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionRelativeOptimizationRealizationStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub instructions: u64,
    pub bytes: u64,
    pub resolved_conditional_branches: u64,
}

/// Structured report at the function-relative selected-form boundary after
/// validating the admitted whole-function frameless exit discipline. It owns
/// no frame, section, symbol, relocation, executable image, installation, or
/// publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRelativeOptimizationRealizationManifest {
    pub identity: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub stage: FunctionRelativeOptimizationRealizationStage,
    pub selections: OptimizationSelectionIdentity,
    pub selected_lowering_selections: OptimizationSelectionIdentity,
    pub selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pub allocation_recovery_selections: OptimizationSelectionIdentity,
    pub post_allocation_machine_selections: OptimizationSelectionIdentity,
    pub function_relative_layout_selections: OptimizationSelectionIdentity,
    pub pre_physical_manifest: PrePhysicalOptimizationManifestIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub pre_allocation_machine_effects:
        omega_machine_optimizer::TerminalPreAllocationMachineEffectIdentity,
    pub post_allocation_machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity,
    pub baseline_pre_layout: TerminalSelectedFormEncodingIdentity,
    pub pre_layout: TerminalSelectedFormEncodingIdentity,
    pub baseline_resolved_layout: TerminalResolvedSelectedFormLayoutIdentity,
    pub resolved_layout: TerminalResolvedSelectedFormLayoutIdentity,
    pub x86_branch_relaxation: Option<TerminalX86BranchRelaxationIdentity>,
    pub aarch64_cbnz_fusion: Option<omega_machine_optimizer::TerminalAarch64CbnzFusionIdentity>,
    pub whole_function_exit_contract: TerminalWholeFunctionExitContractIdentity,
    pub target: NativeTarget,
    pub layout_policy: TerminalSelectedFunctionLayoutPolicy,
    pub scope: FunctionRelativeOptimizationRealizationScope,
    pub statistics: FunctionRelativeOptimizationRealizationStatistics,
    pub frame: FunctionRelativeOptimizationUnavailableData,
    pub machine_emission: FunctionRelativeOptimizationUnavailableData,
    pub section_placement: FunctionRelativeOptimizationUnavailableData,
    pub symbols: FunctionRelativeOptimizationUnavailableData,
    pub object_relocations: FunctionRelativeOptimizationUnavailableData,
    pub executable_image: FunctionRelativeOptimizationUnavailableData,
    pub installation: FunctionRelativeOptimizationUnavailableData,
    pub publication: FunctionRelativeOptimizationUnavailableData,
}

impl FunctionRelativeOptimizationRealizationManifest {
    pub fn recomputed_identity(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        let mut canonical = Vec::new();
        canonical
            .extend_from_slice(b"omega.function-relative-optimization-realization-manifest.v5\0");
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_content(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(MANIFEST_MAGIC);
        encoded.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, FunctionRelativeOptimizationRealizationManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(MANIFEST_MAGIC.len())? != MANIFEST_MAGIC {
            return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != MANIFEST_VERSION {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::UnsupportedVersion(
                    version,
                ),
            );
        }
        let identity =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
            tag => {
                return Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownStage(tag),
                );
            }
        };
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let selected_lowering_selections =
            OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let selected_lowering_completion = match cursor.byte()? {
            0 => None,
            1 => Some(SelectedLoweringOptimizationCompletionIdentity::from_bytes(
                cursor.array()?,
            )),
            tag => {
                return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownSelectedLoweringCompletionStatus(tag));
            }
        };
        let allocation_recovery_selections =
            OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let post_allocation_machine_selections =
            OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let function_relative_layout_selections =
            OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let pre_physical_manifest =
            PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let post_allocation_manifest =
            PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let selected = TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let pre_allocation_machine_effects =
            omega_machine_optimizer::TerminalPreAllocationMachineEffectIdentity::from_bytes(
                cursor.array()?,
            );
        let post_allocation_machine =
            omega_machine_optimizer::TerminalPostAllocationMachineIdentity::from_bytes(
                cursor.array()?,
            );
        let baseline_pre_layout = TerminalSelectedFormEncodingIdentity::from_bytes(cursor.array()?);
        let pre_layout = TerminalSelectedFormEncodingIdentity::from_bytes(cursor.array()?);
        let baseline_resolved_layout =
            TerminalResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
        let resolved_layout =
            TerminalResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
        let x86_branch_relaxation = match cursor.byte()? {
            0 => None,
            1 => Some(TerminalX86BranchRelaxationIdentity::from_bytes(
                cursor.array()?,
            )),
            tag => {
                return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownX86BranchRelaxationStatus(tag));
            }
        };
        let aarch64_cbnz_fusion = match cursor.byte()? {
            0 => None,
            1 => Some(
                omega_machine_optimizer::TerminalAarch64CbnzFusionIdentity::from_bytes(
                    cursor.array()?,
                ),
            ),
            tag => {
                return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownAarch64CbnzFusionStatus(tag));
            }
        };
        let whole_function_exit_contract =
            TerminalWholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let layout_policy = match cursor.byte()? {
            1 => TerminalSelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1,
            tag => {
                return Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownLayoutPolicy(
                        tag,
                    ),
                );
            }
        };
        let scope = match cursor.byte()? {
            1 => FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
            tag => {
                return Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownScope(tag),
                );
            }
        };
        let statistics = FunctionRelativeOptimizationRealizationStatistics {
            functions: u64::from_le_bytes(cursor.array()?),
            blocks: u64::from_le_bytes(cursor.array()?),
            instructions: u64::from_le_bytes(cursor.array()?),
            bytes: u64::from_le_bytes(cursor.array()?),
            resolved_conditional_branches: u64::from_le_bytes(cursor.array()?),
        };
        let unavailable = [
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
        ];
        if cursor.remaining() != 0 {
            return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::TrailingBytes);
        }
        let manifest = Self {
            identity,
            stage,
            selections,
            selected_lowering_selections,
            selected_lowering_completion,
            allocation_recovery_selections,
            post_allocation_machine_selections,
            function_relative_layout_selections,
            pre_physical_manifest,
            post_allocation_manifest,
            selected,
            pre_allocation_machine_effects,
            post_allocation_machine,
            baseline_pre_layout,
            pre_layout,
            baseline_resolved_layout,
            resolved_layout,
            x86_branch_relaxation,
            aarch64_cbnz_fusion,
            whole_function_exit_contract,
            target,
            layout_policy,
            scope,
            statistics,
            frame: unavailable[0],
            machine_emission: unavailable[1],
            section_placement: unavailable[2],
            symbols: unavailable[3],
            object_relocations: unavailable[4],
            executable_image: unavailable[5],
            installation: unavailable[6],
            publication: unavailable[7],
        };
        if manifest.identity != manifest.recomputed_identity() {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::IdentityMismatch,
            );
        }
        Ok(manifest)
    }

    pub fn render_text(&self) -> String {
        let mut output = String::new();
        writeln!(output, "Omega function-relative optimization realization").unwrap();
        writeln!(
            output,
            "stage: validated function-relative selected forms and whole-function exit v1"
        )
        .unwrap();
        writeln!(output, "manifest identity: {}", hex(&self.identity.bytes())).unwrap();
        writeln!(
            output,
            "full named suite: {}",
            hex(&self.selections.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "selected-lowering suite: {}",
            hex(&self.selected_lowering_selections.bytes())
        )
        .unwrap();
        match self.selected_lowering_completion {
            Some(identity) => writeln!(
                output,
                "selected-lowering completion: {}",
                hex(&identity.bytes())
            )
            .unwrap(),
            None => writeln!(output, "selected-lowering completion: not run").unwrap(),
        }
        writeln!(
            output,
            "allocation-recovery suite: {}",
            hex(&self.allocation_recovery_selections.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "post-allocation-machine suite: {}",
            hex(&self.post_allocation_machine_selections.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "function-relative-layout suite: {}",
            hex(&self.function_relative_layout_selections.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "pre-physical manifest: {}",
            hex(&self.pre_physical_manifest.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "post-allocation manifest: {}",
            hex(&self.post_allocation_manifest.bytes())
        )
        .unwrap();
        writeln!(output, "selected CFG: {}", hex(&self.selected.bytes())).unwrap();
        writeln!(
            output,
            "pre-allocation machine effects: {}",
            hex(&self.pre_allocation_machine_effects.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "post-allocation machine: {}",
            hex(&self.post_allocation_machine.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "baseline pre-layout encoding: {}",
            hex(&self.baseline_pre_layout.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "pre-layout encoding: {}",
            hex(&self.pre_layout.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "baseline resolved layout: {}",
            hex(&self.baseline_resolved_layout.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "final resolved layout: {}",
            hex(&self.resolved_layout.bytes())
        )
        .unwrap();
        match self.x86_branch_relaxation {
            Some(identity) => {
                writeln!(output, "x86 branch relaxation: {}", hex(&identity.bytes())).unwrap()
            }
            None => writeln!(output, "x86 branch relaxation: not run").unwrap(),
        }
        match self.aarch64_cbnz_fusion {
            Some(identity) => {
                writeln!(output, "AArch64 CBNZ fusion: {}", hex(&identity.bytes())).unwrap()
            }
            None => writeln!(output, "AArch64 CBNZ fusion: not run").unwrap(),
        }
        writeln!(
            output,
            "whole-function exit contract: {}",
            hex(&self.whole_function_exit_contract.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "target: {}/{} pointers={}/{}",
            architecture_name(self.target.architecture),
            object_format_name(self.target.object_format),
            self.target.pointer_size,
            self.target.pointer_alignment
        )
        .unwrap();
        writeln!(
            output,
            "layout policy: entry-then-zero-fallthrough-then-nonzero-v1"
        )
        .unwrap();
        writeln!(
            output,
            "scope: function-relative-fragments-with-validated-whole-function-exit-v1"
        )
        .unwrap();
        writeln!(output, "functions: {}", self.statistics.functions).unwrap();
        writeln!(output, "blocks: {}", self.statistics.blocks).unwrap();
        writeln!(output, "instructions: {}", self.statistics.instructions).unwrap();
        writeln!(output, "function-relative bytes: {}", self.statistics.bytes).unwrap();
        writeln!(
            output,
            "resolved conditional branches: {}",
            self.statistics.resolved_conditional_branches
        )
        .unwrap();
        writeln!(output, "frame: unavailable").unwrap();
        writeln!(output, "machine emission: unavailable").unwrap();
        writeln!(output, "section placement: unavailable").unwrap();
        writeln!(output, "symbols: unavailable").unwrap();
        writeln!(output, "object relocations: unavailable").unwrap();
        writeln!(output, "executable image: unavailable").unwrap();
        writeln!(output, "installation: unavailable").unwrap();
        writeln!(output, "publication: unavailable").unwrap();
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionRelativeOptimizationRealizationManifest {
    record: FunctionRelativeOptimizationRealizationManifest,
}

impl ValidatedFunctionRelativeOptimizationRealizationManifest {
    pub const fn record(&self) -> &FunctionRelativeOptimizationRealizationManifest {
        &self.record
    }

    #[cfg(test)]
    pub(crate) fn record_mut(&mut self) -> &mut FunctionRelativeOptimizationRealizationManifest {
        &mut self.record
    }
}

#[derive(Debug)]
pub struct StagedSelectedLoweringFunctionRelativeRealization {
    homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: StagedOptimizedPostAllocationMachinePlan,
    encoding: StagedOptimizedSelectedFormEncoding,
    baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<StagedOptimizedX86BranchRelaxation>,
    exit_contract: ValidatedTerminalWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    custody: StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt,
}

impl StagedSelectedLoweringFunctionRelativeRealization {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomesAfterSelectedLowering {
        &self.homes
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }
    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }
    pub const fn baseline_layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.baseline_layout
    }
    pub const fn relaxation(&self) -> Option<&StagedOptimizedX86BranchRelaxation> {
        self.relaxation.as_ref()
    }
    pub fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        final_layout(&self.baseline_layout, self.relaxation.as_ref())
    }
    pub const fn exit_contract(&self) -> &ValidatedTerminalWholeFunctionExitContract {
        &self.exit_contract
    }
    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }
    pub const fn custody(
        &self,
    ) -> &StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(
        &mut self,
    ) -> &mut ValidatedFunctionRelativeOptimizationRealizationManifest {
        &mut self.manifest
    }

    #[cfg(test)]
    pub(crate) fn exit_contract_mut(&mut self) -> &mut ValidatedTerminalWholeFunctionExitContract {
        &mut self.exit_contract
    }
}

/// Function-relative realization reached directly from ordinary register homes
/// when the build selected a function-relative layout optimization but no
/// selected-lowering family. The absence of selected-lowering completion is
/// retained in its manifest and custody rather than synthesized.
#[derive(Debug)]
pub struct StagedFunctionRelativeLayoutOptimizationRealization {
    homes: StagedOptimizedRegisterHomes,
    machine: StagedOptimizedPostAllocationMachinePlan,
    encoding: StagedOptimizedSelectedFormEncoding,
    baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    relaxation: StagedOptimizedX86BranchRelaxation,
    exit_contract: ValidatedTerminalWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    custody: StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt,
}

impl StagedFunctionRelativeLayoutOptimizationRealization {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomes {
        &self.homes
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }
    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }
    pub const fn baseline_layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.baseline_layout
    }
    pub const fn relaxation(&self) -> &StagedOptimizedX86BranchRelaxation {
        &self.relaxation
    }
    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        self.relaxation.layout()
    }
    pub const fn exit_contract(&self) -> &ValidatedTerminalWholeFunctionExitContract {
        &self.exit_contract
    }
    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }
    pub const fn custody(
        &self,
    ) -> &StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
        &self.custody
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(
        &mut self,
    ) -> &mut ValidatedFunctionRelativeOptimizationRealizationManifest {
        &mut self.manifest
    }
}

/// Completed direct-homes realization for the exact AArch64 CBNZ
/// post-allocation transformation. Both baseline and transformed forms remain
/// owned so later custody can prove the four-byte change without treating the
/// transformed layout as baseline authority.
#[derive(Debug)]
pub struct StagedAarch64CbnzFunctionRelativeRealization {
    homes: StagedOptimizedRegisterHomes,
    machine: StagedOptimizedPostAllocationMachinePlan,
    fusion: StagedOptimizedAarch64CbnzFusion,
    baseline_encoding: StagedOptimizedSelectedFormEncoding,
    encoding: StagedOptimizedSelectedFormEncoding,
    baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    layout: StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: ValidatedTerminalWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    custody: StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt,
}

impl StagedAarch64CbnzFunctionRelativeRealization {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomes {
        &self.homes
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }
    pub const fn fusion(&self) -> &StagedOptimizedAarch64CbnzFusion {
        &self.fusion
    }
    pub const fn baseline_encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.baseline_encoding
    }
    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }
    pub const fn baseline_layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.baseline_layout
    }
    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.layout
    }
    pub const fn exit_contract(&self) -> &ValidatedTerminalWholeFunctionExitContract {
        &self.exit_contract
    }
    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> &StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(
        &mut self,
    ) -> &mut ValidatedFunctionRelativeOptimizationRealizationManifest {
        &mut self.manifest
    }

    #[cfg(test)]
    pub(crate) fn exit_contract_mut(&mut self) -> &mut ValidatedTerminalWholeFunctionExitContract {
        &mut self.exit_contract
    }
}

/// Same completed CBNZ realization after a named selected-lowering run.
#[derive(Debug)]
pub struct StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization {
    homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: StagedOptimizedPostAllocationMachinePlan,
    fusion: StagedOptimizedAarch64CbnzFusion,
    baseline_encoding: StagedOptimizedSelectedFormEncoding,
    encoding: StagedOptimizedSelectedFormEncoding,
    baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    layout: StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: ValidatedTerminalWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    custody: StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt,
}

impl StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomesAfterSelectedLowering {
        &self.homes
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }
    pub const fn fusion(&self) -> &StagedOptimizedAarch64CbnzFusion {
        &self.fusion
    }
    pub const fn baseline_encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.baseline_encoding
    }
    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }
    pub const fn baseline_layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.baseline_layout
    }
    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.layout
    }
    pub const fn exit_contract(&self) -> &ValidatedTerminalWholeFunctionExitContract {
        &self.exit_contract
    }
    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }
    pub const fn custody(
        &self,
    ) -> &StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
    source: StagedOptimizedRegisterHomeCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    fusion: StagedOptimizedAarch64CbnzFusionCustodyReceipt,
    exit_contract: TerminalWholeFunctionExitContractIdentity,
    realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
    source: StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    fusion: StagedOptimizedAarch64CbnzFusionCustodyReceipt,
    exit_contract: TerminalWholeFunctionExitContractIdentity,
    realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> StagedOptimizedRegisterHomeCustodyReceipt {
        self.source
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }
    pub const fn fusion(&self) -> StagedOptimizedAarch64CbnzFusionCustodyReceipt {
        self.fusion
    }
    pub const fn exit_contract(&self) -> TerminalWholeFunctionExitContractIdentity {
        self.exit_contract
    }
    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

impl StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
        &self.source
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }
    pub const fn fusion(&self) -> StagedOptimizedAarch64CbnzFusionCustodyReceipt {
        self.fusion
    }
    pub const fn exit_contract(&self) -> TerminalWholeFunctionExitContractIdentity {
        self.exit_contract
    }
    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
    source: StagedOptimizedRegisterHomeCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    relaxation: TerminalX86BranchRelaxationIdentity,
    exit_contract: TerminalWholeFunctionExitContractIdentity,
    realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
    pub const fn source(&self) -> StagedOptimizedRegisterHomeCustodyReceipt {
        self.source
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }
    pub const fn relaxation(&self) -> TerminalX86BranchRelaxationIdentity {
        self.relaxation
    }
    pub const fn exit_contract(&self) -> TerminalWholeFunctionExitContractIdentity {
        self.exit_contract
    }
    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
    source: StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    exit_contract: TerminalWholeFunctionExitContractIdentity,
    realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
        &self.source
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }
    pub const fn exit_contract(&self) -> TerminalWholeFunctionExitContractIdentity {
        self.exit_contract
    }
    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationError {
    Homes(OptimizedPostSelectedLoweringHomeCustodyError),
    DirectHomes(OptimizedRegisterHomeCustodyError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    PostAllocationMachineOptimization(OptimizedPostAllocationMachineOptimizationError),
    Encoding(OptimizedSelectedFormEncodingError),
    Layout(OptimizedResolvedSelectedFormLayoutError),
    X86BranchRelaxation(OptimizedX86BranchRelaxationError),
    ExitContract(TerminalWholeFunctionExitContractError),
    MissingFunctionRelativeLayoutOptimization,
    StatisticsOverflow,
    RootMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for FunctionRelativeOptimizationRealizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "function-relative optimization realization failed: {self:?}"
        )
    }
}

impl std::error::Error for FunctionRelativeOptimizationRealizationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownSelectedLoweringCompletionStatus(u8),
    UnknownX86BranchRelaxationStatus(u8),
    UnknownAarch64CbnzFusionStatus(u8),
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    UnknownLayoutPolicy(u8),
    UnknownScope(u8),
    UnknownUnavailableStatus(u8),
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for FunctionRelativeOptimizationRealizationManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid function-relative realization manifest: {self:?}"
        )
    }
}

impl std::error::Error for FunctionRelativeOptimizationRealizationManifestDecodeError {}

pub fn stage_selected_lowering_function_relative_realization(
    homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
) -> Result<
    StagedSelectedLoweringFunctionRelativeRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    validate_optimized_register_home_after_selected_lowering_custody(&homes)
        .map_err(FunctionRelativeOptimizationRealizationError::Homes)?;
    let machine = stage_optimized_post_allocation_machine_plan_after_selected_lowering(&homes)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    let run = homes.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let optimized = selected_stage.optimized_target().optimized();
    let selections = optimized.selections();
    let budget = optimized.budget_per_pass();
    let (encoding, baseline_layout, relaxation, exit_contract, manifest) = match run.steps().last()
    {
        Some(step) => {
            build_realization(step.fold(), &homes, &machine, physical, selections, budget)?
        }
        None => build_realization(
            selected_stage.selected(),
            &homes,
            &machine,
            physical,
            selections,
            budget,
        )?,
    };
    let custody = custody_receipt(&homes, &machine, &exit_contract, &manifest);
    Ok(StagedSelectedLoweringFunctionRelativeRealization {
        homes,
        machine,
        encoding,
        baseline_layout,
        relaxation,
        exit_contract,
        manifest,
        custody,
    })
}

pub fn validate_selected_lowering_function_relative_realization_custody(
    staged: &StagedSelectedLoweringFunctionRelativeRealization,
) -> Result<
    StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    validate_optimized_register_home_after_selected_lowering_custody(&staged.homes)
        .map_err(FunctionRelativeOptimizationRealizationError::Homes)?;
    let replayed_machine =
        validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
            &staged.homes,
            &staged.machine,
        )
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    if &replayed_machine != staged.machine.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let run = staged.homes.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let optimized = selected_stage.optimized_target().optimized();
    let selections = optimized.selections();
    match run.steps().last() {
        Some(step) => {
            validate_realization_artifacts(
                step.fold(),
                &staged.machine,
                physical,
                &staged.encoding,
                &staged.baseline_layout,
                staged.relaxation.as_ref(),
                &staged.exit_contract,
                selections,
            )?;
        }
        None => {
            validate_realization_artifacts(
                selected_stage.selected(),
                &staged.machine,
                physical,
                &staged.encoding,
                &staged.baseline_layout,
                staged.relaxation.as_ref(),
                &staged.exit_contract,
                selections,
            )?;
        }
    }
    let replayed = expected_manifest(
        &staged.homes,
        &staged.machine,
        &staged.encoding,
        &staged.baseline_layout,
        staged.relaxation.as_ref(),
        &staged.exit_contract,
    )?;
    if replayed.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = custody_receipt(
        &staged.homes,
        &staged.machine,
        &staged.exit_contract,
        &replayed,
    );
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

pub fn stage_function_relative_layout_optimization_realization(
    homes: StagedOptimizedRegisterHomes,
) -> Result<
    StagedFunctionRelativeLayoutOptimizationRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    validate_optimized_register_home_custody(
        homes.legality_stage(),
        homes.homes(),
        homes.post_allocation_manifest(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::DirectHomes)?;
    let machine = stage_optimized_post_allocation_machine_plan(&homes)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let optimized = selected_stage.optimized_target().optimized();
    let selections = optimized.selections();
    if !rel8_selected(selections)? {
        return Err(
            FunctionRelativeOptimizationRealizationError::MissingFunctionRelativeLayoutOptimization,
        );
    }
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let baseline_layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let relaxation = stage_optimized_x86_branch_relaxation(
        selected,
        &machine,
        physical,
        &encoding,
        &baseline_layout,
        optimized.budget_per_pass(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::X86BranchRelaxation)?;
    let exit_contract = stage_terminal_whole_function_exit_contract_after_x86_branch_relaxation(
        selected,
        &machine,
        physical,
        &encoding,
        &baseline_layout,
        &relaxation,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_direct_manifest(
        &homes,
        &machine,
        &encoding,
        &baseline_layout,
        &relaxation,
        &exit_contract,
    )?;
    let custody = direct_custody_receipt(&homes, &machine, &relaxation, &exit_contract, &manifest);
    Ok(StagedFunctionRelativeLayoutOptimizationRealization {
        homes,
        machine,
        encoding,
        baseline_layout,
        relaxation,
        exit_contract,
        manifest,
        custody,
    })
}

pub fn validate_function_relative_layout_optimization_realization_custody(
    staged: &StagedFunctionRelativeLayoutOptimizationRealization,
) -> Result<
    StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let source = validate_optimized_register_home_custody(
        staged.homes.legality_stage(),
        staged.homes.homes(),
        staged.homes.post_allocation_manifest(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::DirectHomes)?;
    if source != staged.homes.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&staged.homes, &staged.machine)
            .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    if &machine != staged.machine.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let selected_stage = staged
        .homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let selections = selected_stage.optimized_target().optimized().selections();
    if !rel8_selected(selections)? {
        return Err(
            FunctionRelativeOptimizationRealizationError::MissingFunctionRelativeLayoutOptimization,
        );
    }
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.baseline_layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_optimized_x86_branch_relaxation(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.relaxation,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::X86BranchRelaxation)?;
    validate_terminal_whole_function_exit_contract_after_x86_branch_relaxation(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.relaxation,
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_direct_manifest(
        &staged.homes,
        &staged.machine,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.relaxation,
        &staged.exit_contract,
    )?;
    if manifest.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = direct_custody_receipt(
        &staged.homes,
        &staged.machine,
        &staged.relaxation,
        &staged.exit_contract,
        &manifest,
    );
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

pub fn stage_aarch64_cbnz_function_relative_realization(
    homes: StagedOptimizedRegisterHomes,
    machine: StagedOptimizedPostAllocationMachinePlan,
    fusion: StagedOptimizedAarch64CbnzFusion,
) -> Result<
    StagedAarch64CbnzFunctionRelativeRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    validate_optimized_register_home_custody(
        homes.legality_stage(),
        homes.homes(),
        homes.post_allocation_manifest(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::DirectHomes)?;
    validate_optimized_post_allocation_machine_plan_custody(&homes, &machine)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    validate_optimized_aarch64_cbnz_fusion_custody(&homes, &machine, &fusion)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)?;
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let (baseline_encoding, encoding, baseline_layout, layout, exit_contract) =
        build_cbnz_artifacts(selected, &machine, physical, &fusion)?;
    let manifest = expected_direct_cbnz_manifest(
        &homes,
        &machine,
        &fusion,
        &baseline_encoding,
        &encoding,
        &baseline_layout,
        &layout,
        &exit_contract,
    )?;
    let custody = StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
        source: homes.custody(),
        machine: machine.custody().clone(),
        fusion: fusion.custody(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    };
    Ok(StagedAarch64CbnzFunctionRelativeRealization {
        homes,
        machine,
        fusion,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
        manifest,
        custody,
    })
}

pub fn validate_aarch64_cbnz_function_relative_realization_custody(
    staged: &StagedAarch64CbnzFunctionRelativeRealization,
) -> Result<
    StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let source = validate_optimized_register_home_custody(
        staged.homes.legality_stage(),
        staged.homes.homes(),
        staged.homes.post_allocation_manifest(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::DirectHomes)?;
    if source != staged.homes.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&staged.homes, &staged.machine)
            .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    if &machine != staged.machine.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let fusion = validate_optimized_aarch64_cbnz_fusion_custody(
        &staged.homes,
        &staged.machine,
        &staged.fusion,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)?;
    if fusion != staged.fusion.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let selected_stage = staged
        .homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    validate_cbnz_artifacts(
        selected_stage.selected(),
        &staged.machine,
        selected_stage.register_environment().physical(),
        &staged.fusion,
        &staged.baseline_encoding,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout,
        &staged.exit_contract,
    )?;
    let manifest = expected_direct_cbnz_manifest(
        &staged.homes,
        &staged.machine,
        &staged.fusion,
        &staged.baseline_encoding,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout,
        &staged.exit_contract,
    )?;
    if manifest.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
        source,
        machine,
        fusion,
        exit_contract: staged.exit_contract.identity(),
        realization: manifest.record.identity,
    };
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

pub fn stage_selected_lowering_aarch64_cbnz_function_relative_realization(
    homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: StagedOptimizedPostAllocationMachinePlan,
    fusion: StagedOptimizedAarch64CbnzFusion,
) -> Result<
    StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    validate_optimized_register_home_after_selected_lowering_custody(&homes)
        .map_err(FunctionRelativeOptimizationRealizationError::Homes)?;
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        &homes, &machine,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody(
        &homes, &machine, &fusion,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)?;
    let run = homes.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let artifacts = match run.steps().last() {
        Some(step) => build_cbnz_artifacts(step.fold(), &machine, physical, &fusion)?,
        None => build_cbnz_artifacts(selected_stage.selected(), &machine, physical, &fusion)?,
    };
    let (baseline_encoding, encoding, baseline_layout, layout, exit_contract) = artifacts;
    let manifest = expected_selected_lowering_cbnz_manifest(
        &homes,
        &machine,
        &fusion,
        &baseline_encoding,
        &encoding,
        &baseline_layout,
        &layout,
        &exit_contract,
    )?;
    let custody = StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
        source: homes.custody().clone(),
        machine: machine.custody().clone(),
        fusion: fusion.custody(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    };
    Ok(
        StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization {
            homes,
            machine,
            fusion,
            baseline_encoding,
            encoding,
            baseline_layout,
            layout,
            exit_contract,
            manifest,
            custody,
        },
    )
}

pub fn validate_selected_lowering_aarch64_cbnz_function_relative_realization_custody(
    staged: &StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization,
) -> Result<
    StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let source = validate_optimized_register_home_after_selected_lowering_custody(&staged.homes)
        .map_err(FunctionRelativeOptimizationRealizationError::Homes)?;
    if &source != staged.homes.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let machine = validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        &staged.homes,
        &staged.machine,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    if &machine != staged.machine.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let fusion = validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody(
        &staged.homes,
        &staged.machine,
        &staged.fusion,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)?;
    if fusion != staged.fusion.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let run = staged.homes.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    match run.steps().last() {
        Some(step) => validate_cbnz_artifacts(
            step.fold(),
            &staged.machine,
            physical,
            &staged.fusion,
            &staged.baseline_encoding,
            &staged.encoding,
            &staged.baseline_layout,
            &staged.layout,
            &staged.exit_contract,
        )?,
        None => validate_cbnz_artifacts(
            selected_stage.selected(),
            &staged.machine,
            physical,
            &staged.fusion,
            &staged.baseline_encoding,
            &staged.encoding,
            &staged.baseline_layout,
            &staged.layout,
            &staged.exit_contract,
        )?,
    }
    let manifest = expected_selected_lowering_cbnz_manifest(
        &staged.homes,
        &staged.machine,
        &staged.fusion,
        &staged.baseline_encoding,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout,
        &staged.exit_contract,
    )?;
    if manifest.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
        source,
        machine,
        fusion,
        exit_contract: staged.exit_contract.identity(),
        realization: manifest.record.identity,
    };
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn build_cbnz_artifacts<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    fusion: &StagedOptimizedAarch64CbnzFusion,
) -> Result<
    (
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedResolvedSelectedFormLayout,
        StagedOptimizedResolvedSelectedFormLayout,
        ValidatedTerminalWholeFunctionExitContract,
    ),
    FunctionRelativeOptimizationRealizationError,
> {
    let baseline_encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, machine, physical)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let baseline_layout = stage_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        &baseline_encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
            selected, machine, physical, fusion,
        )
        .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let layout = stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
        selected, machine, physical, &encoding, fusion,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let exit_contract = stage_terminal_whole_function_exit_contract_after_aarch64_cbnz_fusion(
        selected, machine, physical, &encoding, fusion, &layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    Ok((
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
    ))
}

#[allow(clippy::too_many_arguments)]
fn validate_cbnz_artifacts<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        machine,
        physical,
        baseline_encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        baseline_encoding,
        baseline_layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
        selected, machine, physical, fusion, encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
        selected, machine, physical, encoding, fusion, layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_terminal_whole_function_exit_contract_after_aarch64_cbnz_fusion(
        selected,
        machine,
        physical,
        encoding,
        fusion,
        layout,
        exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)
}

fn build_realization<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    homes: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<
    (
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedResolvedSelectedFormLayout,
        Option<StagedOptimizedX86BranchRelaxation>,
        ValidatedTerminalWholeFunctionExitContract,
        ValidatedFunctionRelativeOptimizationRealizationManifest,
    ),
    FunctionRelativeOptimizationRealizationError,
> {
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, machine, physical)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let baseline_layout =
        stage_optimized_resolved_selected_form_layout(selected, machine, physical, &encoding)
            .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let relaxation = stage_selected_relaxation(
        selected,
        machine,
        physical,
        &encoding,
        &baseline_layout,
        selections,
        budget,
    )?;
    let exit_contract = stage_exit_contract(
        selected,
        machine,
        physical,
        &encoding,
        &baseline_layout,
        relaxation.as_ref(),
    )?;
    let manifest = expected_manifest(
        homes,
        machine,
        &encoding,
        &baseline_layout,
        relaxation.as_ref(),
        &exit_contract,
    )?;
    Ok((
        encoding,
        baseline_layout,
        relaxation,
        exit_contract,
        manifest,
    ))
}

#[allow(clippy::too_many_arguments)]
fn validate_realization_artifacts<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    validate_optimized_layout_independent_selected_form_encoding(
        selected, machine, physical, encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_selected_relaxation(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
        relaxation,
        selections,
    )?;
    validate_exit_contract(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
        relaxation,
        exit_contract,
    )
}

fn rel8_selected(
    selections: &OptimizationSelections,
) -> Result<bool, FunctionRelativeOptimizationRealizationError> {
    let phase = selections.for_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    match phase.as_slice() {
        [] => Ok(false),
        [Optimization::X86RelaxConditionalBranchesToRel8V1] => Ok(true),
        _ => Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    }
}

fn stage_selected_relaxation<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<Option<StagedOptimizedX86BranchRelaxation>, FunctionRelativeOptimizationRealizationError>
{
    if !rel8_selected(selections)? {
        return Ok(None);
    }
    stage_optimized_x86_branch_relaxation(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
        budget,
    )
    .map(Some)
    .map_err(FunctionRelativeOptimizationRealizationError::X86BranchRelaxation)
}

fn validate_selected_relaxation<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    match (rel8_selected(selections)?, relaxation) {
        (false, None) => Ok(()),
        (true, Some(relaxation)) => validate_optimized_x86_branch_relaxation(
            selected,
            machine,
            physical,
            encoding,
            baseline_layout,
            relaxation,
        )
        .map_err(FunctionRelativeOptimizationRealizationError::X86BranchRelaxation),
        _ => Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    }
}

fn stage_exit_contract<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
) -> Result<ValidatedTerminalWholeFunctionExitContract, FunctionRelativeOptimizationRealizationError>
{
    match relaxation {
        Some(relaxation) => {
            stage_terminal_whole_function_exit_contract_after_x86_branch_relaxation(
                selected,
                machine,
                physical,
                encoding,
                baseline_layout,
                relaxation,
            )
        }
        None => stage_terminal_whole_function_exit_contract(
            selected,
            machine,
            physical,
            encoding,
            baseline_layout,
        ),
    }
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)
}

fn validate_exit_contract<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    match relaxation {
        Some(relaxation) => {
            validate_terminal_whole_function_exit_contract_after_x86_branch_relaxation(
                selected,
                machine,
                physical,
                encoding,
                baseline_layout,
                relaxation,
                exit_contract,
            )
        }
        None => validate_terminal_whole_function_exit_contract(
            selected,
            machine,
            physical,
            encoding,
            baseline_layout,
            exit_contract,
        ),
    }
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)
}

fn final_layout<'layout>(
    baseline_layout: &'layout StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&'layout StagedOptimizedX86BranchRelaxation>,
) -> &'layout StagedOptimizedResolvedSelectedFormLayout {
    relaxation
        .map(StagedOptimizedX86BranchRelaxation::layout)
        .unwrap_or(baseline_layout)
}

fn validate_relaxation_manifest_roots(
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    match (rel8_selected(selections)?, relaxation) {
        (false, None) => Ok(()),
        (true, Some(relaxation))
            if relaxation.source() == baseline_layout.identity()
                && relaxation.output() == relaxation.layout().identity() =>
        {
            Ok(())
        }
        _ => Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    }
}

#[allow(clippy::too_many_arguments)]
fn expected_direct_cbnz_manifest(
    homes: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selections = selected_stage.optimized_target().optimized().selections();
    let post = homes.post_allocation_manifest().record();
    expected_cbnz_manifest(
        selections,
        OptimizationSelections::default().identity(),
        None,
        homes.custody().manifest(),
        post.identity,
        post.selected,
        post.target,
        machine,
        fusion,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
    )
}

#[allow(clippy::too_many_arguments)]
fn expected_selected_lowering_cbnz_manifest(
    homes: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let run = homes.selected_lowering_run();
    let completion = run.custody();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selections = selected_stage.optimized_target().optimized().selections();
    let post = homes.post_allocation_manifest().record();
    expected_cbnz_manifest(
        selections,
        selections
            .for_phase(OptimizationExecutionPhase::SelectedLowering)
            .identity(),
        Some(completion.identity()),
        completion.source().manifest(),
        post.identity,
        post.selected,
        post.target,
        machine,
        fusion,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
    )
}

#[allow(clippy::too_many_arguments)]
fn expected_cbnz_manifest(
    selections: &OptimizationSelections,
    selected_lowering_selections: OptimizationSelectionIdentity,
    selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pre_physical_manifest: PrePhysicalOptimizationManifestIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    selected: TerminalSelectedInstructionPlanIdentity,
    target: NativeTarget,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let selected_lowering_phase =
        selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    let post_phase = selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let layout_phase = selections.for_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    if selected_lowering_selections != selected_lowering_phase.identity()
        || selected_lowering_completion.is_some() == selected_lowering_phase.is_empty()
        || post_phase.as_slice() != [Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1]
        || !layout_phase.is_empty()
        || fusion.custody().selections() != selections.identity()
        || fusion.custody().post_allocation_machine_selections() != post_phase.identity()
        || machine.machine().receipt().post_allocation_manifest() != post_allocation_manifest
        || machine.machine().receipt().selected() != selected
        || baseline_encoding.selected() != selected
        || baseline_encoding.machine() != machine.machine().receipt().identity()
        || baseline_encoding.machine_optimization().is_some()
        || encoding.selected() != selected
        || encoding.machine() != machine.machine().receipt().identity()
        || encoding.machine_optimization().is_none_or(|custody| {
            custody.selections() != selections.identity()
                || custody.post_allocation_machine_selections() != post_phase.identity()
                || custody.fusion() != fusion.fusion().receipt().identity()
        })
        || baseline_layout.pre_layout() != baseline_encoding.identity()
        || baseline_layout.machine_optimization().is_some()
        || layout.pre_layout() != encoding.identity()
        || layout.machine_optimization() != encoding.machine_optimization()
        || baseline_layout.target() != target
        || layout.target() != target
        || exit_contract.contract().selected != selected
        || exit_contract.contract().post_allocation_manifest != post_allocation_manifest
        || exit_contract.contract().post_allocation_machine
            != machine.machine().receipt().identity()
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout != layout.identity()
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let baseline_bytes = function_relative_statistics(baseline_layout)?.bytes;
    let final_statistics = function_relative_statistics(layout)?;
    let expected_shrink = u64::try_from(fusion.custody().action_count())
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)?;
    if baseline_bytes.checked_sub(final_statistics.bytes) != Some(expected_shrink) {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let mut record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(b"pending"),
        stage: FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections,
        selected_lowering_completion,
        allocation_recovery_selections: selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .identity(),
        post_allocation_machine_selections: post_phase.identity(),
        function_relative_layout_selections: layout_phase.identity(),
        pre_physical_manifest,
        post_allocation_manifest,
        selected,
        pre_allocation_machine_effects: machine.effects().effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        baseline_pre_layout: baseline_encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: baseline_layout.identity(),
        resolved_layout: layout.identity(),
        x86_branch_relaxation: None,
        aarch64_cbnz_fusion: Some(fusion.fusion().receipt().identity()),
        whole_function_exit_contract: exit_contract.identity(),
        target,
        layout_policy: layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics: final_statistics,
        frame: unavailable,
        machine_emission: unavailable,
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionRelativeOptimizationRealizationManifest { record })
}

fn expected_manifest(
    homes: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let run = homes.selected_lowering_run();
    let completion = run.custody();
    let selections = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections();
    let selected_lowering_selections = selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering)
        .identity();
    let function_relative_layout_selections = selections
        .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
        .identity();
    let post_allocation_machine_selections = selections
        .for_phase(OptimizationExecutionPhase::PostAllocationMachine)
        .identity();
    let post = homes.post_allocation_manifest().record();
    if completion.selections() != selections.identity()
        || completion.selected_lowering_selections() != selected_lowering_selections
        || post.selected_lowering_completion != Some(completion.identity())
        || post.selected != completion.final_selected()
        || post.target != baseline_layout.target()
        || machine.machine().receipt().post_allocation_manifest() != post.identity
        || machine.machine().receipt().selected() != completion.final_selected()
        || encoding.selected() != completion.final_selected()
        || encoding.machine() != machine.machine().receipt().identity()
        || baseline_layout.selected() != completion.final_selected()
        || baseline_layout.machine() != machine.machine().receipt().identity()
        || baseline_layout.pre_layout() != encoding.identity()
        || exit_contract.contract().selected != completion.final_selected()
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine
            != machine.machine().receipt().identity()
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout
            != final_layout(baseline_layout, relaxation).identity()
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    validate_relaxation_manifest_roots(baseline_layout, relaxation, selections)?;
    let final_layout = final_layout(baseline_layout, relaxation);
    let statistics = function_relative_statistics(final_layout)?;
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let mut record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage:
            FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections,
        selected_lowering_completion: Some(completion.identity()),
        allocation_recovery_selections: selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .identity(),
        post_allocation_machine_selections,
        function_relative_layout_selections,
        pre_physical_manifest: completion.source().manifest(),
        post_allocation_manifest: post.identity,
        selected: completion.final_selected(),
        pre_allocation_machine_effects: machine.effects().effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        baseline_pre_layout: encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: baseline_layout.identity(),
        resolved_layout: final_layout.identity(),
        x86_branch_relaxation: relaxation.map(StagedOptimizedX86BranchRelaxation::identity),
        aarch64_cbnz_fusion: None,
        whole_function_exit_contract: exit_contract.identity(),
        target: baseline_layout.target(),
        layout_policy: baseline_layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics,
        frame: unavailable,
        machine_emission: unavailable,
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionRelativeOptimizationRealizationManifest { record })
}

fn expected_direct_manifest(
    homes: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: &StagedOptimizedX86BranchRelaxation,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    let selections = optimized.selections();
    let selected_lowering_selections = selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering)
        .identity();
    let function_relative_layout_selections = selections
        .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
        .identity();
    let post_allocation_machine_selections = selections
        .for_phase(OptimizationExecutionPhase::PostAllocationMachine)
        .identity();
    if !selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering)
        .is_empty()
        || !rel8_selected(selections)?
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let source = homes.custody();
    let selected = source.selected();
    let post = homes.post_allocation_manifest().record();
    if post.selected_lowering_completion.is_some()
        || post.selected != selected
        || post.target != baseline_layout.target()
        || machine.machine().receipt().post_allocation_manifest() != post.identity
        || machine.machine().receipt().selected() != selected
        || encoding.selected() != selected
        || encoding.machine() != machine.machine().receipt().identity()
        || baseline_layout.selected() != selected
        || baseline_layout.machine() != machine.machine().receipt().identity()
        || baseline_layout.pre_layout() != encoding.identity()
        || relaxation.source() != baseline_layout.identity()
        || relaxation.output() != relaxation.layout().identity()
        || exit_contract.contract().selected != selected
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine
            != machine.machine().receipt().identity()
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout != relaxation.layout().identity()
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let mut record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage:
            FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections,
        selected_lowering_completion: None,
        allocation_recovery_selections: selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .identity(),
        post_allocation_machine_selections,
        function_relative_layout_selections,
        pre_physical_manifest: source.manifest(),
        post_allocation_manifest: post.identity,
        selected,
        pre_allocation_machine_effects: machine.effects().effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        baseline_pre_layout: encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: baseline_layout.identity(),
        resolved_layout: relaxation.layout().identity(),
        x86_branch_relaxation: Some(relaxation.identity()),
        aarch64_cbnz_fusion: None,
        whole_function_exit_contract: exit_contract.identity(),
        target: baseline_layout.target(),
        layout_policy: baseline_layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics: function_relative_statistics(relaxation.layout())?,
        frame: unavailable,
        machine_emission: unavailable,
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionRelativeOptimizationRealizationManifest { record })
}

pub(crate) fn function_relative_statistics(
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<
    FunctionRelativeOptimizationRealizationStatistics,
    FunctionRelativeOptimizationRealizationError,
> {
    let count = |value: usize| {
        u64::try_from(value)
            .map_err(|_| FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
    };
    let functions = count(layout.functions().len())?;
    let blocks = layout
        .functions()
        .iter()
        .try_fold(0_u64, |total, function| {
            total
                .checked_add(count(function.blocks.len())?)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    let instructions = layout
        .functions()
        .iter()
        .flat_map(|function| &function.blocks)
        .try_fold(0_u64, |total, block| {
            total
                .checked_add(count(block.instructions.len())?)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    let bytes = layout
        .functions()
        .iter()
        .try_fold(0_u64, |total, function| {
            total
                .checked_add(function.byte_count)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    let resolved_conditional_branches = layout
        .functions()
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction.branch.is_some())
        .try_fold(0_u64, |total, _| {
            total
                .checked_add(1)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    Ok(FunctionRelativeOptimizationRealizationStatistics {
        functions,
        blocks,
        instructions,
        bytes,
        resolved_conditional_branches,
    })
}

pub(crate) fn seal_function_relative_manifest(
    mut record: FunctionRelativeOptimizationRealizationManifest,
) -> ValidatedFunctionRelativeOptimizationRealizationManifest {
    record.identity = record.recomputed_identity();
    ValidatedFunctionRelativeOptimizationRealizationManifest { record }
}

fn custody_receipt(
    homes: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
    StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
        source: homes.custody().clone(),
        machine: machine.custody().clone(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    }
}

fn direct_custody_receipt(
    homes: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    relaxation: &StagedOptimizedX86BranchRelaxation,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
    StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
        source: homes.custody(),
        machine: machine.custody().clone(),
        relaxation: relaxation.identity(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    }
}

fn encode_manifest_content(manifest: &FunctionRelativeOptimizationRealizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1 => 1,
    });
    canonical.extend_from_slice(&manifest.selections.bytes());
    canonical.extend_from_slice(&manifest.selected_lowering_selections.bytes());
    match manifest.selected_lowering_completion {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    canonical.extend_from_slice(&manifest.allocation_recovery_selections.bytes());
    for identity in [
        manifest.post_allocation_machine_selections.bytes(),
        manifest.function_relative_layout_selections.bytes(),
        manifest.pre_physical_manifest.bytes(),
        manifest.post_allocation_manifest.bytes(),
        manifest.selected.bytes(),
        manifest.pre_allocation_machine_effects.bytes(),
        manifest.post_allocation_machine.bytes(),
        manifest.baseline_pre_layout.bytes(),
        manifest.pre_layout.bytes(),
        manifest.baseline_resolved_layout.bytes(),
        manifest.resolved_layout.bytes(),
    ] {
        canonical.extend_from_slice(&identity);
    }
    match manifest.x86_branch_relaxation {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    match manifest.aarch64_cbnz_fusion {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    canonical.extend_from_slice(&manifest.whole_function_exit_contract.bytes());
    encode_target(&mut canonical, manifest.target);
    canonical.push(match manifest.layout_policy {
        TerminalSelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 => 1,
    });
    canonical.push(match manifest.scope {
        FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1 => 1,
    });
    for value in [
        manifest.statistics.functions,
        manifest.statistics.blocks,
        manifest.statistics.instructions,
        manifest.statistics.bytes,
        manifest.statistics.resolved_conditional_branches,
    ] {
        canonical.extend_from_slice(&value.to_le_bytes());
    }
    for unavailable in [
        manifest.frame,
        manifest.machine_emission,
        manifest.section_placement,
        manifest.symbols,
        manifest.object_relocations,
        manifest.executable_image,
        manifest.installation,
        manifest.publication,
    ] {
        canonical.push(match unavailable {
            FunctionRelativeOptimizationUnavailableData::Unavailable => 1,
        });
    }
    canonical
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    encode_usize(bytes, target.pointer_size);
    encode_usize(bytes, target.pointer_alignment);
}

const fn architecture_name(architecture: Architecture) -> &'static str {
    match architecture {
        Architecture::Aarch64 => "aarch64",
        Architecture::X86_64 => "x86_64",
    }
}

const fn object_format_name(object_format: ObjectFormat) -> &'static str {
    match object_format {
        ObjectFormat::Elf => "elf",
        ObjectFormat::MachO => "macho",
        ObjectFormat::Coff => "coff",
    }
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, FunctionRelativeOptimizationRealizationManifestDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownArchitecture(
                    tag,
                ),
            );
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownObjectFormat(
                    tag,
                ),
            );
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?)).map_err(|_| {
        FunctionRelativeOptimizationRealizationManifestDecodeError::TargetLayoutOverflow
    })?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?)).map_err(|_| {
        FunctionRelativeOptimizationRealizationManifestDecodeError::TargetLayoutOverflow
    })?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

fn decode_unavailable(
    cursor: &mut Cursor<'_>,
) -> Result<
    FunctionRelativeOptimizationUnavailableData,
    FunctionRelativeOptimizationRealizationManifestDecodeError,
> {
    match cursor.byte()? {
        1 => Ok(FunctionRelativeOptimizationUnavailableData::Unavailable),
        tag => Err(
            FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownUnavailableStatus(
                tag,
            ),
        ),
    }
}

fn encode_usize(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("function-relative realization value fits u64")
            .to_le_bytes(),
    );
}

struct Cursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> Cursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'encoded [u8], FunctionRelativeOptimizationRealizationManifestDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], FunctionRelativeOptimizationRealizationManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, FunctionRelativeOptimizationRealizationManifestDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}
