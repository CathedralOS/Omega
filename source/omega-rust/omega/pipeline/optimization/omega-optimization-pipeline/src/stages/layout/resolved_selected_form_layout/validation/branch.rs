use std::collections::BTreeMap;

use omega_isa_aarch64::{
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_nonzero_branch_form,
};
use omega_isa_x86_64::validate_x86_64_selected_nonzero_branch_form;
use omega_machine_optimizer::{
    Aarch64CbnzFusionAction, PostAllocationMachineInstruction, QualifiedPhysicalRead,
};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{
    MachineEncodedEffects, MachineSizeKnowledge, SelectedBlock, SelectedBlockId,
    SelectedInstruction, SelectedTerminator,
};
use omega_target::Architecture;

use super::super::{
    OptimizedResolvedSelectedFormLayoutError, ResolvedConditionalBranchEvidence,
    ResolvedSelectedFormRow,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    architecture: Architecture,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    fused: Option<(&QualifiedPhysicalRead, &Aarch64CbnzFusionAction)>,
    candidate: &ResolvedSelectedFormRow,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let SelectedTerminator::ConditionalBranch {
        instruction: terminator,
        when_nonzero,
        when_zero,
    } = &block.terminator
    else {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    };
    if terminator.id != instruction.id {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    let nonzero_offset = *block_offsets
        .get(&when_nonzero.block)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
    let zero_offset = *block_offsets
        .get(&when_zero.block)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
    let branch_size = branch_size(architecture);
    let branch_end = instruction_offset
        .checked_add(branch_size)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
    if zero_offset != branch_end {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    let displacement = match architecture {
        Architecture::X86_64 => checked_delta(nonzero_offset, branch_end)?,
        Architecture::Aarch64 => checked_delta(nonzero_offset, instruction_offset)?,
    };
    let (register_reads, effects) = decode(
        architecture,
        physical,
        machine,
        fused,
        displacement,
        &candidate.bytes,
    )?;
    if let Some((source_read, action)) = fused {
        validate_fused_footprint(
            block,
            source_read,
            action,
            physical,
            &register_reads,
            &effects,
            &machine.alternative.encoded,
        )?;
    } else if effects != machine.alternative.encoded {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    if u64::try_from(candidate.bytes.len()).ok() != Some(branch_size)
        || !declared_size_matches(machine.alternative.size, branch_size)
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    let evidence = ResolvedConditionalBranchEvidence {
        source_block: block.id,
        when_nonzero_edge: when_nonzero.psi_edge,
        when_nonzero_block: when_nonzero.block,
        when_nonzero_offset: nonzero_offset,
        when_zero_edge: when_zero.psi_edge,
        when_zero_block: when_zero.block,
        when_zero_offset: zero_offset,
        byte_displacement: displacement,
        decoded_register_reads: register_reads,
        decoded_effects: effects,
    };
    if candidate.branch.as_deref() != Some(&evidence) {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    Ok(())
}

fn decode(
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
    machine: &PostAllocationMachineInstruction,
    fused: Option<(&QualifiedPhysicalRead, &Aarch64CbnzFusionAction)>,
    displacement: i64,
    bytes: &[u8],
) -> Result<
    (
        Vec<omega_register_model::RegisterViewId>,
        MachineEncodedEffects,
    ),
    OptimizedResolvedSelectedFormLayoutError,
> {
    let footprint = match (architecture, fused) {
        (Architecture::Aarch64, Some((source_read, _))) => {
            let decoded = validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                physical,
                source_read.view,
                displacement,
                bytes,
            )
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
            return Ok((
                decoded.footprint().register_reads.clone(),
                decoded.footprint().encoded.clone(),
            ));
        }
        (Architecture::X86_64, Some(_)) => {
            return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
        }
        (Architecture::X86_64, None) => {
            let decoded = validate_x86_64_selected_nonzero_branch_form(
                physical,
                machine.alternative.key,
                displacement,
                bytes,
            )
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
            return Ok((
                decoded.footprint().register_reads.clone(),
                decoded.footprint().encoded.clone(),
            ));
        }
        (Architecture::Aarch64, None) => validate_aarch64_selected_nonzero_branch_form(
            physical,
            machine.alternative.key,
            displacement,
            bytes,
        )
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?
        .footprint()
        .clone(),
    };
    Ok((footprint.register_reads, footprint.encoded))
}

#[allow(clippy::too_many_arguments)]
fn validate_fused_footprint(
    block: &SelectedBlock,
    source_read: &QualifiedPhysicalRead,
    action: &Aarch64CbnzFusionAction,
    physical: &ValidatedPhysicalRegisterModel,
    register_reads: &[omega_register_model::RegisterViewId],
    effects: &MachineEncodedEffects,
    original: &MachineEncodedEffects,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == source_read.view)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
    let SelectedTerminator::ConditionalBranch {
        when_nonzero,
        when_zero,
        ..
    } = &block.terminator
    else {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    };
    if register_reads != [source_read.view]
        || source_read.units != view.units
        || &action.source_read != source_read
        || action.when_nonzero_edge != when_nonzero.psi_edge
        || action.when_nonzero_block != when_nonzero.block
        || action.when_zero_edge != when_zero.psi_edge
        || action.when_zero_block != when_zero.block
        || !effects.external_operand_reads.is_empty()
        || !effects.external_operand_writes.is_empty()
        || effects.implicit_unit_uses != action.pc_units
        || effects.implicit_unit_defs != action.pc_units
        || !effects.implicit_unit_clobbers.is_empty()
        || effects
            .implicit_unit_uses
            .iter()
            .any(|unit| action.nzcv_units.contains(unit))
        || effects.memory != original.memory
        || effects.stack != original.stack
        || effects.trap != original.trap
        || effects.control != original.control
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    Ok(())
}

fn declared_size_matches(knowledge: MachineSizeKnowledge, actual: u64) -> bool {
    match knowledge {
        MachineSizeKnowledge::ExactBytes(expected) => u64::from(expected) == actual,
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => {
            actual >= u64::from(minimum_bytes)
                && maximum_bytes.is_none_or(|maximum| actual <= u64::from(maximum))
        }
    }
}

fn branch_size(architecture: Architecture) -> u64 {
    match architecture {
        Architecture::X86_64 => 6,
        Architecture::Aarch64 => 4,
    }
}

fn checked_delta(target: u64, base: u64) -> Result<i64, OptimizedResolvedSelectedFormLayoutError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
}
