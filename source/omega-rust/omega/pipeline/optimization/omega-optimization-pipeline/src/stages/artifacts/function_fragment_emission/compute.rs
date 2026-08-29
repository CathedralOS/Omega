use omega_isa_x86_64::{
    X86_64StructuralUnitInternalControlFixupKind, X86_64StructuralUnitInternalControlFixupState,
};
use omega_machine_code::{
    FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentConditionalBranchEvidence,
    FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan,
    FunctionFragmentInstructionSpan, FunctionFragmentInternalMachineFixup,
    FunctionFragmentInternalMachineFixupKind, FunctionFragmentInternalMachineFixupState,
    FunctionFragmentSuccessorProvenance, StructuralUnitCallFragmentSpan,
    StructuralUnitFunctionFragment, StructuralUnitFunctionFragmentBlockSpan,
};
use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    OptimizationSelections,
};
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstruction, SelectedTerminator,
};

use crate::{
    ResolvedSelectedFormRow, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
    StagedPostAllocationMachineFunctionRelativeSource,
};

use super::error::FunctionFragmentEmissionError;
use super::model::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionSourceKind,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData, ValidatedFunctionFragmentEmissionManifest,
};
use super::source::{
    active_resident_rematerialization, StagedOptimizedFunctionFragmentEmissionSource,
};

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<
    (
        FunctionFragmentEmissionPlan,
        ValidatedFunctionFragmentEmissionManifest,
    ),
    FunctionFragmentEmissionError,
> {
    match source {
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(realization) => {
            let selected = realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected();
            compute_from(
                source,
                selected,
                realization.layout(),
                realization.manifest().record(),
            )
        }
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8AfterSelectedLowering(
            realization,
        ) => {
            let run = realization.homes().selected_lowering_run();
            let selected_stage = run
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage();
            match run.steps().last() {
                Some(step) => compute_from(
                    source,
                    step.fold(),
                    realization.layout(),
                    realization.manifest().record(),
                ),
                None => compute_from(
                    source,
                    selected_stage.selected(),
                    realization.layout(),
                    realization.manifest().record(),
                ),
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(realization) => {
            match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => {
                    let selected = homes
                        .legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage()
                        .selected();
                    compute_from(
                        source,
                        selected,
                        realization.layout(),
                        realization.manifest().record(),
                    )
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    let run = homes.selected_lowering_run();
                    let selected_stage = run
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage();
                    match run.steps().last() {
                        Some(step) => compute_from(
                            source,
                            step.fold(),
                            realization.layout(),
                            realization.manifest().record(),
                        ),
                        None => compute_from(
                            source,
                            selected_stage.selected(),
                            realization.layout(),
                            realization.manifest().record(),
                        ),
                    }
                }
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::ActiveResidentRematerialization(
            realization,
        ) => {
            let rematerialization = active_resident_rematerialization(realization);
            compute_from(
                source,
                rematerialization.rematerialization(),
                realization.source().layout(),
                realization.manifest().record(),
            )
        }
        StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(realization) => {
            let selected = realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected();
            compute_from(
                source,
                selected,
                realization.layout(),
                realization.manifest().record(),
            )
        }
        StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(realization) => {
            compute_structural_unit(source, realization)
        }
    }
}

fn compute_from(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
    selected: &impl ValidatedSelectedAnalysis,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    source_manifest: &crate::FunctionRelativeOptimizationRealizationManifest,
) -> Result<
    (
        FunctionFragmentEmissionPlan,
        ValidatedFunctionFragmentEmissionManifest,
    ),
    FunctionFragmentEmissionError,
> {
    let selected_plan = selected.selected_plan();
    let expected_allocation_recovery = match source {
        StagedOptimizedFunctionFragmentEmissionSource::ActiveResidentRematerialization(_) => {
            OptimizationSelections::new([
                omega_optimization_core::Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ])
            .expect("the closed rematerialization source kind has one valid selection")
            .identity()
        }
        _ => OptimizationSelections::default().identity(),
    };
    if selected.selected_identity() != layout.selected()
        || selected_plan.target != layout.target()
        || selected_plan.functions.len() != layout.functions().len()
        || !selected_plan.structural_unit_functions.is_empty()
        || !layout.structural_unit_functions().is_empty()
        || source_manifest.selected != selected.selected_identity()
        || source_manifest.resolved_layout != layout.identity()
        || source_manifest.allocation_recovery_selections != expected_allocation_recovery
        || matches!(
            source,
            StagedOptimizedFunctionFragmentEmissionSource::ActiveResidentRematerialization(_)
        ) && source_manifest.selections != expected_allocation_recovery
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for selected_function in &selected_plan.functions {
        let resolved = layout
            .functions()
            .iter()
            .find(|function| function.machine == selected_function.machine)
            .ok_or(FunctionFragmentEmissionError::MissingFunction(
                selected_function.machine,
            ))?;
        functions.push(emit_function(selected_function, resolved)?);
    }
    let mut fragments = FunctionFragmentEmissionPlan {
        identity: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
        psi: selected_plan.psi,
        fuel_schedule: selected_plan.fuel_schedule,
        selected: selected.selected_identity(),
        target: selected_plan.target,
        entry: selected_plan.entry,
        functions,
        structural_unit_functions: Vec::new(),
    };
    fragments.identity = fragments.recomputed_identity();
    let statistics = statistics(&fragments)?;
    let kind = source_kind(source);
    let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
    let mut record = FunctionFragmentEmissionManifest {
        identity: FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"pending"),
        stage: FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1,
        source_kind: kind,
        source_realization: source_manifest.identity,
        selections: source_manifest.selections,
        psi: fragments.psi,
        fuel_schedule: fragments.fuel_schedule,
        selected: fragments.selected,
        post_allocation_manifest: source_manifest.post_allocation_manifest,
        post_allocation_machine: source_manifest.post_allocation_machine,
        final_pre_layout: source_manifest.pre_layout,
        final_resolved_layout: source_manifest.resolved_layout,
        whole_function_exit_contract: source_manifest.whole_function_exit_contract,
        fragments: fragments.identity,
        target: fragments.target,
        statistics,
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok((
        fragments,
        ValidatedFunctionFragmentEmissionManifest { record },
    ))
}

fn compute_structural_unit(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
    realization: &StagedOptimizedStructuralUnitFunctionRelativeRealization,
) -> Result<
    (
        FunctionFragmentEmissionPlan,
        ValidatedFunctionFragmentEmissionManifest,
    ),
    FunctionFragmentEmissionError,
> {
    let selected_plan = source.selected_plan();
    let layout = realization.layout();
    let source_manifest = realization.manifest().record();
    if !selected_plan.functions.is_empty()
        || !layout.functions().is_empty()
        || selected_plan.structural_unit_functions.len() != layout.structural_unit_functions().len()
        || selected_plan.structural_unit_functions.is_empty()
        || selected_plan.target != layout.target()
        || source_manifest.selected != layout.selected()
        || source_manifest.resolved_layout != layout.identity()
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }

    let mut structural_unit_functions =
        Vec::with_capacity(selected_plan.structural_unit_functions.len());
    for (selected, resolved) in selected_plan
        .structural_unit_functions
        .iter()
        .zip(layout.structural_unit_functions())
    {
        if selected.machine != resolved.machine
            || selected.entry_block != resolved.block
            || resolved.offset != 0
        {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        let call = match (&selected.call, &resolved.call) {
            (None, None) => None,
            (Some(selected_call), Some(resolved_call)) => {
                if selected_call.id != resolved_call.instruction
                    || selected_call.operation != resolved_call.operation
                    || selected_call.callee != resolved_call.callee
                {
                    return Err(FunctionFragmentEmissionError::RootMismatch);
                }
                let fixup = resolved_call.fixup;
                let kind = match fixup.kind {
                    X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1 => FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                };
                let state = match fixup.state {
                    X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1 => {
                        FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
                    }
                };
                let base = resolved_call.offset;
                Some(StructuralUnitCallFragmentSpan {
                    instruction: resolved_call.instruction,
                    operation: resolved_call.operation,
                    callee: resolved_call.callee,
                    offset: base,
                    bytes: resolved_call.bytes.clone(),
                    provenance: selected_call.provenance.clone(),
                    fixup: FunctionFragmentInternalMachineFixup {
                        kind,
                        state,
                        callee: fixup.callee,
                        opcode_function_offset: base
                            .checked_add(u64::from(fixup.opcode_byte_offset))
                            .ok_or(FunctionFragmentEmissionError::OffsetOverflow)?,
                        field_function_offset: base
                            .checked_add(u64::from(fixup.field_byte_offset))
                            .ok_or(FunctionFragmentEmissionError::OffsetOverflow)?,
                        next_instruction_function_offset: base
                            .checked_add(u64::from(fixup.next_instruction_byte_offset))
                            .ok_or(FunctionFragmentEmissionError::OffsetOverflow)?,
                        field_byte_width: fixup.field_byte_width,
                        addend: fixup.addend,
                    },
                })
            }
            _ => return Err(FunctionFragmentEmissionError::RootMismatch),
        };
        let returned = &resolved.return_instruction;
        let selected_return = &selected.terminator.instruction;
        if selected_return.id != returned.instruction {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        let return_instruction = FunctionFragmentInstructionSpan {
            instruction: returned.instruction,
            alternative: returned.alternative,
            offset: returned.offset,
            bytes: returned.bytes.clone(),
            branch: None,
            provenance: selected_return.provenance.clone(),
            control: FunctionFragmentControlProvenance::Return {
                psi_return_edge: selected.terminator.psi_return_edge,
            },
        };
        let mut bytes = Vec::new();
        if let Some(call) = &call {
            if u64::try_from(bytes.len())
                .map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?
                != call.offset
            {
                return Err(FunctionFragmentEmissionError::RootMismatch);
            }
            bytes.extend_from_slice(&call.bytes);
        }
        if u64::try_from(bytes.len()).map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?
            != return_instruction.offset
        {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        bytes.extend_from_slice(&return_instruction.bytes);
        if u64::try_from(bytes.len()).map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?
            != resolved.byte_count
        {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        structural_unit_functions.push(StructuralUnitFunctionFragment {
            machine: selected.machine,
            attachment: selected.attachment,
            provenance: selected.provenance.clone(),
            byte_count: resolved.byte_count,
            bytes,
            block: StructuralUnitFunctionFragmentBlockSpan {
                block: resolved.block,
                offset: resolved.offset,
                byte_count: resolved.byte_count,
                call,
                return_instruction,
            },
        });
    }

    let mut fragments = FunctionFragmentEmissionPlan {
        identity: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
        psi: selected_plan.psi,
        fuel_schedule: selected_plan.fuel_schedule,
        selected: source_manifest.selected,
        target: selected_plan.target,
        entry: selected_plan.entry,
        functions: Vec::new(),
        structural_unit_functions,
    };
    fragments.identity = fragments.recomputed_identity();
    let statistics = statistics(&fragments)?;
    let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
    let mut record = FunctionFragmentEmissionManifest {
        identity: FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"pending"),
        stage: if statistics.unresolved_internal_machine_fixups == 0 {
            FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1
        } else {
            FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1
        },
        source_kind: FunctionFragmentEmissionSourceKind::StructuralUnitV1,
        source_realization: source_manifest.identity,
        selections: source_manifest.selections,
        psi: fragments.psi,
        fuel_schedule: fragments.fuel_schedule,
        selected: fragments.selected,
        post_allocation_manifest: source_manifest.post_allocation_manifest,
        post_allocation_machine: source_manifest.post_allocation_machine,
        final_pre_layout: source_manifest.pre_layout,
        final_resolved_layout: source_manifest.resolved_layout,
        whole_function_exit_contract: source_manifest.whole_function_exit_contract,
        fragments: fragments.identity,
        target: fragments.target,
        statistics,
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok((
        fragments,
        ValidatedFunctionFragmentEmissionManifest { record },
    ))
}

fn emit_function(
    selected: &SelectedFunction,
    resolved: &crate::ResolvedSelectedFunctionLayout,
) -> Result<FunctionFragment, FunctionFragmentEmissionError> {
    let mut bytes = Vec::new();
    let mut blocks = Vec::with_capacity(resolved.blocks.len());
    for resolved_block in &resolved.blocks {
        let block_start = u64::try_from(bytes.len())
            .map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?;
        if block_start != resolved_block.offset {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        let selected_block = selected
            .blocks
            .iter()
            .find(|block| block.id == resolved_block.block)
            .ok_or(FunctionFragmentEmissionError::MissingBlock(
                resolved_block.block,
            ))?;
        let mut instructions = Vec::with_capacity(resolved_block.instructions.len());
        for row in &resolved_block.instructions {
            let row_offset = u64::try_from(bytes.len())
                .map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?;
            if row_offset != row.offset {
                return Err(FunctionFragmentEmissionError::RootMismatch);
            }
            let instruction = selected_instruction(selected_block, row)?;
            let control = control_provenance(selected_block, instruction.id);
            bytes.extend_from_slice(&row.bytes);
            instructions.push(FunctionFragmentInstructionSpan {
                instruction: row.instruction,
                alternative: row.alternative,
                offset: row.offset,
                bytes: row.bytes.clone(),
                branch: row.branch.as_deref().map(|branch| {
                    Box::new(FunctionFragmentConditionalBranchEvidence {
                        source_block: branch.source_block,
                        when_nonzero_edge: branch.when_nonzero_edge,
                        when_nonzero_block: branch.when_nonzero_block,
                        when_nonzero_offset: branch.when_nonzero_offset,
                        when_zero_edge: branch.when_zero_edge,
                        when_zero_block: branch.when_zero_block,
                        when_zero_offset: branch.when_zero_offset,
                        byte_displacement: branch.byte_displacement,
                        decoded_register_reads: branch.decoded_register_reads.clone(),
                        decoded_effects: branch.decoded_effects.clone(),
                    })
                }),
                provenance: instruction.provenance.clone(),
                control,
            });
        }
        let block_end = u64::try_from(bytes.len())
            .map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?;
        if block_end.checked_sub(block_start) != Some(resolved_block.byte_count) {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        blocks.push(FunctionFragmentBlockSpan {
            block: resolved_block.block,
            offset: resolved_block.offset,
            byte_count: resolved_block.byte_count,
            instructions,
        });
    }
    let byte_count =
        u64::try_from(bytes.len()).map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?;
    if byte_count != resolved.byte_count {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    Ok(FunctionFragment {
        machine: selected.machine,
        attachment: selected.attachment,
        provenance: selected.provenance.clone(),
        byte_count,
        bytes,
        blocks,
    })
}

fn selected_instruction<'a>(
    block: &'a SelectedBlock,
    row: &ResolvedSelectedFormRow,
) -> Result<&'a SelectedInstruction, FunctionFragmentEmissionError> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .find(|instruction| instruction.id == row.instruction)
        .ok_or(FunctionFragmentEmissionError::MissingInstruction(
            row.instruction,
        ))
}

fn control_provenance(
    block: &SelectedBlock,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> FunctionFragmentControlProvenance {
    match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            instruction: branch,
            when_nonzero,
            when_zero,
        } if branch.id == instruction => FunctionFragmentControlProvenance::ConditionalBranch {
            when_nonzero: FunctionFragmentSuccessorProvenance {
                psi_edge: when_nonzero.psi_edge,
                block: when_nonzero.block,
                source_target: when_nonzero.source_target,
                bindings: when_nonzero.bindings.clone(),
                fuel: when_nonzero.fuel.clone(),
            },
            when_zero: FunctionFragmentSuccessorProvenance {
                psi_edge: when_zero.psi_edge,
                block: when_zero.block,
                source_target: when_zero.source_target,
                bindings: when_zero.bindings.clone(),
                fuel: when_zero.fuel.clone(),
            },
        },
        SelectedTerminator::Return {
            instruction: returned,
            psi_return_edge,
        } if returned.id == instruction => FunctionFragmentControlProvenance::Return {
            psi_return_edge: *psi_return_edge,
        },
        _ => FunctionFragmentControlProvenance::None,
    }
}

fn statistics(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<FunctionFragmentEmissionStatistics, FunctionFragmentEmissionError> {
    let mut result = FunctionFragmentEmissionStatistics {
        functions: u64::try_from(fragments.functions.len())
            .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
        ..FunctionFragmentEmissionStatistics::default()
    };
    for function in &fragments.functions {
        result.bytes = result
            .bytes
            .checked_add(function.byte_count)
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        result.blocks = result
            .blocks
            .checked_add(
                u64::try_from(function.blocks.len())
                    .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
            )
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        for block in &function.blocks {
            result.instruction_spans = result
                .instruction_spans
                .checked_add(
                    u64::try_from(block.instructions.len())
                        .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
                )
                .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
            for row in &block.instructions {
                result.zero_byte_instruction_spans += u64::from(row.bytes.is_empty());
                result.resolved_conditional_branches += u64::from(row.branch.is_some());
                let mut fuel = row.provenance.fuel.len();
                if let FunctionFragmentControlProvenance::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                } = &row.control
                {
                    fuel = fuel
                        .checked_add(when_nonzero.fuel.len())
                        .and_then(|fuel| fuel.checked_add(when_zero.fuel.len()))
                        .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
                }
                result.logical_fuel_settlements = result
                    .logical_fuel_settlements
                    .checked_add(
                        u64::try_from(fuel)
                            .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
                    )
                    .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
            }
        }
    }
    result.structural_unit_functions = u64::try_from(fragments.structural_unit_functions.len())
        .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?;
    for function in &fragments.structural_unit_functions {
        result.structural_unit_blocks = result
            .structural_unit_blocks
            .checked_add(1)
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        result.structural_unit_bytes = result
            .structural_unit_bytes
            .checked_add(function.byte_count)
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        result.structural_unit_instruction_spans = result
            .structural_unit_instruction_spans
            .checked_add(1 + u64::from(function.block.call.is_some()))
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        result.structural_logical_fuel_settlements = result
            .structural_logical_fuel_settlements
            .checked_add(
                u64::try_from(function.block.return_instruction.provenance.fuel.len())
                    .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
            )
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        if let Some(call) = &function.block.call {
            result.unresolved_internal_machine_fixups = result
                .unresolved_internal_machine_fixups
                .checked_add(1)
                .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
            result.structural_logical_fuel_settlements = result
                .structural_logical_fuel_settlements
                .checked_add(
                    u64::try_from(call.provenance.fuel.len())
                        .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
                )
                .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        }
    }
    Ok(result)
}

fn source_kind(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> FunctionFragmentEmissionSourceKind {
    match source {
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(_)
        | StagedOptimizedFunctionFragmentEmissionSource::X86Rel8AfterSelectedLowering(_) => {
            FunctionFragmentEmissionSourceKind::X86Rel8V1
        }
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(realization) => {
            FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                optimization: realization.optimization().optimization(),
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::ActiveResidentRematerialization(_) => {
            FunctionFragmentEmissionSourceKind::ActiveResidentImmediateU64MultiUseRematerializationV1
        }
        StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(_) => {
            FunctionFragmentEmissionSourceKind::UnitBaselineV1
        }
        StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(_) => {
            FunctionFragmentEmissionSourceKind::StructuralUnitV1
        }
    }
}
