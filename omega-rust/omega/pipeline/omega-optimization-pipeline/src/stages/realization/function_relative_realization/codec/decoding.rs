use super::super::model::{
    FunctionRelativeFrameDisposition, FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationStatistics,
};
use super::super::prelude::*;
use super::super::{
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData,
};
use super::cursor::Cursor;
use super::error::FunctionRelativeOptimizationRealizationManifestDecodeError as Error;
use super::post_allocation::decode_optional_custody;
use super::target::decode_target;

pub(super) fn decode_manifest_content(
    cursor: &mut Cursor<'_>,
    identity: FunctionRelativeOptimizationRealizationManifestIdentity,
) -> Result<FunctionRelativeOptimizationRealizationManifest, Error> {
    let stage = match cursor.byte()? {
        1 => FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        tag => return Err(Error::UnknownStage(tag)),
    };
    let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let selected_lowering_selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let selected_lowering_completion = match cursor.byte()? {
        0 => None,
        1 => Some(SelectedLoweringOptimizationCompletionIdentity::from_bytes(
            cursor.array()?,
        )),
        tag => return Err(Error::UnknownSelectedLoweringCompletionStatus(tag)),
    };
    let allocation_recovery_selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let post_allocation_machine_selections =
        OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let function_relative_layout_selections =
        OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let pre_physical_manifest =
        PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?);
    let post_allocation_manifest =
        PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let pre_allocation_machine_effects =
        omega_selected_instructions::PreAllocationMachineEffectIdentity::from_bytes(
            cursor.array()?,
        );
    let post_allocation_machine =
        omega_physical_instructions::PostAllocationMachineIdentity::from_bytes(cursor.array()?);
    let baseline_pre_layout = SelectedFormEncodingIdentity::from_bytes(cursor.array()?);
    let pre_layout = SelectedFormEncodingIdentity::from_bytes(cursor.array()?);
    let baseline_resolved_layout = ResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
    let resolved_layout = ResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
    let x86_branch_relaxation = match cursor.byte()? {
        0 => None,
        1 => Some(X86BranchRelaxationIdentity::from_bytes(cursor.array()?)),
        tag => return Err(Error::UnknownX86BranchRelaxationStatus(tag)),
    };
    let post_allocation_machine_optimization = decode_optional_custody(cursor)?;
    let whole_function_exit_contract =
        WholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
    let target = decode_target(cursor)?;
    let layout_policy = match cursor.byte()? {
        1 => SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1,
        2 => SelectedFunctionLayoutPolicy::SingleEntryBlockV1,
        3 => SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1,
        4 => SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1,
        5 => SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
        tag => return Err(Error::UnknownLayoutPolicy(tag)),
    };
    let scope = match cursor.byte()? {
        1 => FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        tag => return Err(Error::UnknownScope(tag)),
    };
    let statistics = FunctionRelativeOptimizationRealizationStatistics {
        functions: u64::from_le_bytes(cursor.array()?),
        blocks: u64::from_le_bytes(cursor.array()?),
        instructions: u64::from_le_bytes(cursor.array()?),
        bytes: u64::from_le_bytes(cursor.array()?),
        resolved_conditional_branches: u64::from_le_bytes(cursor.array()?),
        structural_unit_functions: u64::from_le_bytes(cursor.array()?),
        structural_unit_blocks: u64::from_le_bytes(cursor.array()?),
        structural_unit_instructions: u64::from_le_bytes(cursor.array()?),
        structural_unit_bytes: u64::from_le_bytes(cursor.array()?),
        unresolved_internal_machine_fixups: u64::from_le_bytes(cursor.array()?),
    };
    let frame = match cursor.byte()? {
        1 => FunctionRelativeFrameDisposition::Unavailable,
        2 => FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 {
            layout: crate::TargetFrameLayoutIdentity::from_bytes(cursor.array()?),
            protocol: crate::TargetFrameProtocolEncodingIdentity::from_bytes(cursor.array()?),
        },
        tag => return Err(Error::UnknownFrameDisposition(tag)),
    };
    let unavailable = [
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
    ];
    let manifest = FunctionRelativeOptimizationRealizationManifest {
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
        post_allocation_machine_optimization,
        whole_function_exit_contract,
        target,
        layout_policy,
        scope,
        statistics,
        frame,
        machine_emission: unavailable[0],
        section_placement: unavailable[1],
        symbols: unavailable[2],
        object_relocations: unavailable[3],
        executable_image: unavailable[4],
        installation: unavailable[5],
        publication: unavailable[6],
    };
    Ok(manifest)
}

fn decode_unavailable(
    cursor: &mut Cursor<'_>,
) -> Result<FunctionRelativeOptimizationUnavailableData, Error> {
    match cursor.byte()? {
        1 => Ok(FunctionRelativeOptimizationUnavailableData::Unavailable),
        tag => Err(Error::UnknownUnavailableStatus(tag)),
    }
}
