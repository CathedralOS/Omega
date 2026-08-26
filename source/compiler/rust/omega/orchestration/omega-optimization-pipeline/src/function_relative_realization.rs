use std::fmt::Write;

use omega_optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use omega_regalloc::ValidatedTerminalSelectedAnalysis;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;

use crate::{
    OptimizedPostAllocationMachinePipelineError, OptimizedPostSelectedLoweringHomeCustodyError,
    OptimizedResolvedSelectedFormLayoutError, OptimizedSelectedFormEncodingError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomesAfterSelectedLowering, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, TerminalResolvedSelectedFormLayoutIdentity,
    TerminalSelectedFormEncodingIdentity, TerminalSelectedFunctionLayoutPolicy,
    stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_post_allocation_machine_plan_after_selected_lowering,
    stage_optimized_resolved_selected_form_layout,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
    validate_optimized_resolved_selected_form_layout,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGFRM\0\0";
const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationStage {
    ValidatedFunctionRelativeSelectedFormsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationScope {
    FunctionRelativeFragmentsV1,
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

/// Structured report at the exact function-relative selected-form boundary.
/// It owns no section, symbol, relocation, executable image, installation, or
/// publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRelativeOptimizationRealizationManifest {
    pub identity: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub stage: FunctionRelativeOptimizationRealizationStage,
    pub selections: OptimizationSelectionIdentity,
    pub selected_lowering_selections: OptimizationSelectionIdentity,
    pub selected_lowering_completion: SelectedLoweringOptimizationCompletionIdentity,
    pub pre_physical_manifest: PrePhysicalOptimizationManifestIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub pre_allocation_machine_effects:
        omega_machine_optimizer::TerminalPreAllocationMachineEffectIdentity,
    pub post_allocation_machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity,
    pub pre_layout: TerminalSelectedFormEncodingIdentity,
    pub resolved_layout: TerminalResolvedSelectedFormLayoutIdentity,
    pub target: NativeTarget,
    pub layout_policy: TerminalSelectedFunctionLayoutPolicy,
    pub scope: FunctionRelativeOptimizationRealizationScope,
    pub statistics: FunctionRelativeOptimizationRealizationStatistics,
    pub frame: FunctionRelativeOptimizationUnavailableData,
    pub whole_function_exit_contract: FunctionRelativeOptimizationUnavailableData,
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
            .extend_from_slice(b"omega.function-relative-optimization-realization-manifest.v1\0");
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
            1 => FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsV1,
            tag => {
                return Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownStage(tag),
                );
            }
        };
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let selected_lowering_selections =
            OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let selected_lowering_completion =
            SelectedLoweringOptimizationCompletionIdentity::from_bytes(cursor.array()?);
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
        let pre_layout = TerminalSelectedFormEncodingIdentity::from_bytes(cursor.array()?);
        let resolved_layout =
            TerminalResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
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
            1 => FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsV1,
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
            pre_physical_manifest,
            post_allocation_manifest,
            selected,
            pre_allocation_machine_effects,
            post_allocation_machine,
            pre_layout,
            resolved_layout,
            target,
            layout_policy,
            scope,
            statistics,
            frame: unavailable[0],
            whole_function_exit_contract: unavailable[1],
            machine_emission: unavailable[2],
            section_placement: unavailable[3],
            symbols: unavailable[4],
            object_relocations: unavailable[5],
            executable_image: unavailable[6],
            installation: unavailable[7],
            publication: unavailable[8],
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
            "stage: validated function-relative selected forms v1"
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
        writeln!(
            output,
            "selected-lowering completion: {}",
            hex(&self.selected_lowering_completion.bytes())
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
            "pre-layout encoding: {}",
            hex(&self.pre_layout.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "resolved layout: {}",
            hex(&self.resolved_layout.bytes())
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
        writeln!(output, "scope: function-relative-fragments-v1").unwrap();
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
        writeln!(output, "whole-function exit contract: unavailable").unwrap();
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
    layout: StagedOptimizedResolvedSelectedFormLayout,
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
    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.layout
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
    source: StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
        &self.source
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }
    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationError {
    Homes(OptimizedPostSelectedLoweringHomeCustodyError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    Encoding(OptimizedSelectedFormEncodingError),
    Layout(OptimizedResolvedSelectedFormLayoutError),
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
    let (encoding, layout, manifest) = match run.steps().last() {
        Some(step) => build_realization(step.fold(), &homes, &machine, physical)?,
        None => build_realization(selected_stage.selected(), &homes, &machine, physical)?,
    };
    let custody = custody_receipt(&homes, &machine, &manifest);
    Ok(StagedSelectedLoweringFunctionRelativeRealization {
        homes,
        machine,
        encoding,
        layout,
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
    match run.steps().last() {
        Some(step) => {
            validate_realization_artifacts(
                step.fold(),
                &staged.machine,
                physical,
                &staged.encoding,
                &staged.layout,
            )?;
        }
        None => {
            validate_realization_artifacts(
                selected_stage.selected(),
                &staged.machine,
                physical,
                &staged.encoding,
                &staged.layout,
            )?;
        }
    }
    let replayed = expected_manifest(
        &staged.homes,
        &staged.machine,
        &staged.encoding,
        &staged.layout,
    )?;
    if replayed.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = custody_receipt(&staged.homes, &staged.machine, &replayed);
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn build_realization<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    homes: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
) -> Result<
    (
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedResolvedSelectedFormLayout,
        ValidatedFunctionRelativeOptimizationRealizationManifest,
    ),
    FunctionRelativeOptimizationRealizationError,
> {
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, machine, physical)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout(selected, machine, physical, &encoding)
            .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let manifest = expected_manifest(homes, machine, &encoding, &layout)?;
    Ok((encoding, layout, manifest))
}

fn validate_realization_artifacts<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    validate_optimized_layout_independent_selected_form_encoding(
        selected, machine, physical, encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(selected, machine, physical, encoding, layout)
        .map_err(FunctionRelativeOptimizationRealizationError::Layout)
}

fn expected_manifest(
    homes: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let run = homes.selected_lowering_run();
    let completion = run.custody();
    let post = homes.post_allocation_manifest().record();
    if post.selected_lowering_completion != Some(completion.identity())
        || post.selected != completion.final_selected()
        || post.target != layout.target()
        || machine.machine().receipt().post_allocation_manifest() != post.identity
        || machine.machine().receipt().selected() != completion.final_selected()
        || encoding.selected() != completion.final_selected()
        || encoding.machine() != machine.machine().receipt().identity()
        || layout.selected() != completion.final_selected()
        || layout.machine() != machine.machine().receipt().identity()
        || layout.pre_layout() != encoding.identity()
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let statistics = statistics(layout)?;
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let mut record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage:
            FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsV1,
        selections: completion.selections(),
        selected_lowering_selections: completion.selected_lowering_selections(),
        selected_lowering_completion: completion.identity(),
        pre_physical_manifest: completion.source().manifest(),
        post_allocation_manifest: post.identity,
        selected: completion.final_selected(),
        pre_allocation_machine_effects: machine.effects().effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        pre_layout: encoding.identity(),
        resolved_layout: layout.identity(),
        target: layout.target(),
        layout_policy: layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsV1,
        statistics,
        frame: unavailable,
        whole_function_exit_contract: unavailable,
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

fn statistics(
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

fn custody_receipt(
    homes: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
    StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
        source: homes.custody().clone(),
        machine: machine.custody().clone(),
        realization: manifest.record.identity,
    }
}

fn encode_manifest_content(manifest: &FunctionRelativeOptimizationRealizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsV1 => 1,
    });
    for identity in [
        manifest.selections.bytes(),
        manifest.selected_lowering_selections.bytes(),
        manifest.selected_lowering_completion.bytes(),
        manifest.pre_physical_manifest.bytes(),
        manifest.post_allocation_manifest.bytes(),
        manifest.selected.bytes(),
        manifest.pre_allocation_machine_effects.bytes(),
        manifest.post_allocation_machine.bytes(),
        manifest.pre_layout.bytes(),
        manifest.resolved_layout.bytes(),
    ] {
        canonical.extend_from_slice(&identity);
    }
    encode_target(&mut canonical, manifest.target);
    canonical.push(match manifest.layout_policy {
        TerminalSelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 => 1,
    });
    canonical.push(match manifest.scope {
        FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsV1 => 1,
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
        manifest.whole_function_exit_contract,
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
