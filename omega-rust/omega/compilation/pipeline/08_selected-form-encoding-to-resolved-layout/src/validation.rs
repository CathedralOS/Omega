/*
We independently validate candidate function-relative layouts in stage 08 before publication or cross-stage admission. We verify root identities, re-derive the layout policy and canonical block order through an independent `BTreeMap`-backed traversal (`derive_order`), recompute block and instruction byte spans including monotone AArch64 conditional-branch widening, and check every resolved row's bytes, fixup custody, and target-decoded branch footprints against the pre-layout encoding store and physical register model.
*/

use std::collections::BTreeMap;

use machine_code::{
    DeferredControlEncodingReason, SelectedFormEncodedRowStore, SelectedFormEncoding,
    SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineDisposition,
    resolved_machine_layout_identity as layout_identity,
};
use physical_instructions::{
    PostAllocationMachineFunction, PostAllocationMachineInstruction,
    PostAllocationMachineOptimizationCustody, PostAllocationMachinePlan,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    CurrentSelectedProgram, SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedTerminator,
};
use target::Architecture;

use crate::ordinary::{
    ConditionalBranchDescriptor, branch_size, conditional_branch_descriptor,
    conditional_branch_family, derive_plan, instructions, select,
};
use crate::{
    OptimizedResolvedSelectedFormLayoutError as Error, ResolvedBranchEvidence,
    ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy,
    StagedOptimizedResolvedSelectedFormLayout, declared_size_matches,
};

type PreLayoutRows<'a> = std::slice::Iter<'a, SelectedFormEncodingRow>;

pub(crate) fn validate(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), Error> {
    let selected_plan = selected.plan;
    let machine_plan = machine;
    if pre_layout.selected() != selected.identity
        || pre_layout.machine() != machine.identity
        || selected_plan.target != machine_plan.target
        || selected_plan.target.architecture != physical.model().architecture
        || selected_plan.functions.len() != machine_plan.functions.len()
        || pre_layout.post_allocation_machine_optimization().is_some()
    {
        return Err(Error::RootMismatch);
    }
    let expected_policy = select(selected_plan)?;
    validate_roots(
        selected,
        machine,
        pre_layout,
        None,
        expected_policy,
        artifact,
    )?;
    validate_ordinary(
        selected,
        machine,
        physical,
        pre_layout,
        expected_policy,
        artifact,
    )?;
    validate_identity(artifact)
}

fn validate_roots(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    pre_layout: &SelectedFormEncoding,
    optimization: Option<PostAllocationMachineOptimizationCustody>,
    policy: SelectedFunctionLayoutPolicy,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), Error> {
    if artifact.selected() != selected.identity
        || artifact.machine() != machine.identity
        || artifact.pre_layout() != pre_layout.identity()
        || artifact.post_allocation_machine_optimization() != optimization
        || artifact.target() != selected.plan.target
        || artifact.policy() != policy
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok(())
}

fn validate_identity(artifact: &StagedOptimizedResolvedSelectedFormLayout) -> Result<(), Error> {
    let identity = layout_identity(
        artifact.selected(),
        artifact.machine(),
        artifact.pre_layout(),
        artifact.post_allocation_machine_optimization(),
        artifact.target(),
        artifact.policy(),
        artifact.functions(),
    );
    if artifact.identity() != identity {
        return Err(Error::ArtifactMismatch);
    }
    Ok(())
}

fn validate_ordinary(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
    policy: SelectedFunctionLayoutPolicy,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), Error> {
    let selected_plan = selected.plan;
    let machine_plan = machine;
    if artifact.functions().len() != selected_plan.functions.len() {
        return Err(Error::ArtifactMismatch);
    }
    let mut pre_rows = pre_layout.rows().iter();
    for ((selected_function, machine_function), candidate) in selected_plan
        .functions
        .iter()
        .zip(&machine_plan.functions)
        .zip(artifact.functions())
    {
        validate_function(
            selected_plan.target.architecture,
            selected_function,
            machine_function,
            physical,
            policy,
            &mut pre_rows,
            candidate,
            pre_layout.encoded(),
        )?;
    }
    if pre_rows.next().is_some() {
        return Err(Error::RootMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_function(
    architecture: Architecture,
    selected: &SelectedFunction,
    machine: &PostAllocationMachineFunction,
    physical: &ValidatedPhysicalRegisterModel,
    policy: SelectedFunctionLayoutPolicy,
    pre_rows: &mut PreLayoutRows<'_>,
    candidate: &ResolvedSelectedFunctionLayout,
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), Error> {
    if selected.machine != machine.machine
        || selected.machine != candidate.machine
        || selected.blocks.len() != machine.blocks.len()
    {
        return Err(Error::ArtifactMismatch);
    }
    let pre_rows = collect_pre_layout_rows(selected, pre_rows)?;
    let machine_blocks = machine
        .blocks
        .iter()
        .map(|block| (block.block, block))
        .collect::<BTreeMap<_, _>>();
    if machine_blocks.len() != machine.blocks.len() {
        return Err(Error::RootMismatch);
    }
    let ordered = derive_order(selected, policy)?;
    if candidate.blocks.len() != ordered.len() {
        return Err(Error::ArtifactMismatch);
    }
    let layout = derive_plan(architecture, &ordered, &pre_rows)?;
    if candidate.byte_count != layout.function_size {
        return Err(Error::ArtifactMismatch);
    }
    for (block, candidate_block) in ordered.iter().copied().zip(&candidate.blocks) {
        let machine_block = machine_blocks.get(&block.id).ok_or(Error::RootMismatch)?;
        let instruction_count = block.instructions.len() + 1;
        if machine_block.instructions.len() != instruction_count
            || candidate_block.instructions.len() != instruction_count
            || candidate_block.block != block.id
            || candidate_block.offset != layout.block_offsets[&block.id]
            || candidate_block.byte_count != layout.block_sizes[&block.id]
        {
            return Err(Error::ArtifactMismatch);
        }
        let mut instruction_offset = candidate_block.offset;
        for ((instruction, machine_instruction), candidate_row) in instructions(block)
            .zip(&machine_block.instructions)
            .zip(&candidate_block.instructions)
        {
            if machine_instruction.instruction != instruction.id {
                return Err(Error::RootMismatch);
            }
            let pre = pre_rows
                .get(&instruction.id)
                .ok_or(Error::MissingInstruction(instruction.id))?;
            validate_row(
                architecture,
                block,
                instruction,
                machine_instruction,
                pre,
                physical,
                instruction_offset,
                &layout.block_offsets,
                candidate_row,
                layout.widened_branches.contains(&instruction.id),
                encoded,
            )?;
            instruction_offset = instruction_offset
                .checked_add(
                    u64::try_from(candidate_row.bytes.len())
                        .map_err(|_| Error::ArtifactMismatch)?,
                )
                .ok_or(Error::ArtifactMismatch)?;
        }
        if instruction_offset
            != candidate_block
                .offset
                .checked_add(candidate_block.byte_count)
                .ok_or(Error::ArtifactMismatch)?
        {
            return Err(Error::ArtifactMismatch);
        }
    }
    Ok(())
}

fn collect_pre_layout_rows<'a>(
    function: &SelectedFunction,
    rows: &mut PreLayoutRows<'a>,
) -> Result<BTreeMap<SelectedInstructionId, &'a SelectedFormEncodingRow>, Error> {
    let mut collected = BTreeMap::new();
    for block in &function.blocks {
        for instruction in instructions(block) {
            let row = rows
                .next()
                .ok_or(Error::MissingInstruction(instruction.id))?;
            if row.instruction != instruction.id {
                return Err(Error::MissingInstruction(instruction.id));
            }
            if collected.insert(instruction.id, row).is_some() {
                return Err(Error::DuplicateInstruction(instruction.id));
            }
        }
    }
    Ok(collected)
}

fn derive_order(
    function: &SelectedFunction,
    policy: SelectedFunctionLayoutPolicy,
) -> Result<Vec<&SelectedBlock>, Error> {
    let invalid = || Error::UnsupportedFunctionShape(function.machine);
    let mut positions = BTreeMap::new();
    for (position, block) in function.blocks.iter().enumerate() {
        if positions.insert(block.id, position).is_some() {
            return Err(invalid());
        }
    }
    let entry = &function.blocks[*positions.get(&function.entry_block).ok_or_else(invalid)?];
    if policy == SelectedFunctionLayoutPolicy::SingleEntryBlockV1
        && (function.blocks.len() != 1
            || !matches!(entry.terminator, SelectedTerminator::Return { .. }))
    {
        return Err(invalid());
    }
    let mut has_incoming = vec![false; function.blocks.len()];
    for block in &function.blocks {
        if let Some(destination) = fallthrough(block) {
            let position = *positions.get(&destination).ok_or_else(invalid)?;
            if destination == block.id
                || destination == entry.id
                || std::mem::replace(&mut has_incoming[position], true)
            {
                return Err(invalid());
            }
        }
    }
    let mut roots = function
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(position, block)| {
            (!has_incoming[position] && block.id != entry.id).then_some(block)
        });
    let mut visited = vec![false; function.blocks.len()];
    let mut order = Vec::with_capacity(function.blocks.len());
    let mut next = Some(entry);
    while let Some(block) = next {
        let position = positions[&block.id];
        if std::mem::replace(&mut visited[position], true) {
            return Err(invalid());
        }
        order.push(block);
        next = if let Some(destination) = fallthrough(block) {
            Some(&function.blocks[*positions.get(&destination).ok_or_else(invalid)?])
        } else {
            roots.next()
        };
    }
    if order.len() != function.blocks.len() {
        return Err(invalid());
    }
    Ok(order)
}

fn fallthrough(block: &SelectedBlock) -> Option<SelectedBlockId> {
    conditional_branch_descriptor(block).map(|descriptor| descriptor.when_fallthrough.block)
}

#[allow(clippy::too_many_arguments)]
fn validate_row(
    architecture: Architecture,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    pre: &SelectedFormEncodingRow,
    physical: &ValidatedPhysicalRegisterModel,
    expected_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    candidate: &ResolvedSelectedFormRow,
    widened: bool,
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), Error> {
    if candidate.instruction != instruction.id
        || candidate.alternative != pre.alternative
        || candidate.alternative != machine.alternative.key
        || candidate.offset != expected_offset
    {
        return Err(Error::ArtifactMismatch);
    }
    let bytes = encoded.bytes(pre.code);
    match (&pre.machine_disposition, &pre.state) {
        (SelectedFormMachineDisposition::RetainedV1, SelectedFormEncodingState::Encoded) => {
            if candidate.bytes != bytes
                || candidate.branch.is_some()
                || candidate.internal_machine_fixup.is_some()
                || candidate.normalized_foreign_call_fixup.is_some()
                || candidate.image_address_fixup.is_some()
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedInternalMachineCall { fixup },
        ) => {
            if !matches!(
                instruction.kind,
                selected_instructions::selected_call_pattern!(
                    target: selected_instructions::SelectedCallTarget::Machine(callee),
                    ..
                ) if callee == fixup.callee
            ) || candidate.bytes != bytes
                || candidate.branch.is_some()
                || candidate.internal_machine_fixup() != Some(**fixup)
                || candidate.normalized_foreign_call_fixup.is_some()
                || candidate.image_address_fixup.is_some()
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedNormalizedForeignCall { fixup },
        ) => {
            if !matches!(
                instruction.kind,
                SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal }
                    if boundary == fixup.boundary && ordinal == fixup.ordinal
            ) || candidate.bytes != bytes
                || candidate.branch.is_some()
                || candidate.internal_machine_fixup.is_some()
                || candidate.normalized_foreign_call_fixup() != Some(fixup.as_ref())
                || candidate.image_address_fixup.is_some()
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedImageAddress { fixup },
        ) => {
            if !matches!(
                instruction.kind,
                SelectedInstructionKind::ImageAddress { reference } if fixup.reference == reference
            ) || candidate.bytes != bytes
                || candidate.branch.is_some()
                || candidate.internal_machine_fixup.is_some()
                || candidate.normalized_foreign_call_fixup.is_some()
                || candidate.image_address_fixup() != Some(**fixup)
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            },
        ) => validate_branch(
            architecture,
            block,
            instruction,
            expected_offset,
            block_offsets,
            machine,
            physical,
            candidate,
            widened,
        ),
        _ => Err(Error::ArtifactMismatch),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_branch(
    architecture: Architecture,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    candidate: &ResolvedSelectedFormRow,
    widened: bool,
) -> Result<(), Error> {
    if let SelectedTerminator::Jump {
        instruction: jump,
        successor,
    } = &block.terminator
    {
        if jump.id != instruction.id {
            return Err(Error::ArtifactMismatch);
        }
        let Some(ResolvedBranchEvidence::Jump(actual)) = candidate.branch.as_deref() else {
            return Err(Error::ArtifactMismatch);
        };
        let target = *block_offsets
            .get(&successor.block)
            .ok_or(Error::ArtifactMismatch)?;
        let effects = match architecture {
            Architecture::X86_64 => {
                let end = instruction_offset
                    .checked_add(5)
                    .ok_or(Error::ArtifactMismatch)?;
                let displacement = checked_delta(target, end)?;
                if displacement != actual.byte_displacement {
                    return Err(Error::ArtifactMismatch);
                }
                isa_x86_64::validate_x86_64_selected_jump_form(
                    physical,
                    machine.alternative.key,
                    displacement,
                    &candidate.bytes,
                )
                .map_err(|_| Error::ArtifactMismatch)?
                .footprint()
                .encoded
                .clone()
            }
            Architecture::Aarch64 => {
                let displacement = checked_delta(target, instruction_offset)?;
                if displacement != actual.byte_displacement {
                    return Err(Error::ArtifactMismatch);
                }
                isa_aarch64::validate_aarch64_selected_jump_form(
                    physical,
                    machine.alternative.key,
                    displacement,
                    &candidate.bytes,
                )
                .map_err(|_| Error::ArtifactMismatch)?
                .footprint()
                .encoded
                .clone()
            }
        };
        if actual.source_block != block.id
            || actual.target_edge != successor.psi_edge
            || actual.target_block != successor.block
            || actual.target_offset != target
            || actual.decoded_effects != effects
            || effects != machine.alternative.encoded
            || !declared_size_matches(machine.alternative.size, candidate.bytes.len() as u64)
        {
            return Err(Error::ArtifactMismatch);
        }
        return Ok(());
    }
    let Some(descriptor) = conditional_branch_descriptor(block) else {
        return Err(Error::ArtifactMismatch);
    };
    let ConditionalBranchDescriptor {
        predicate,
        terminator,
        when_taken,
        when_fallthrough,
    } = descriptor;
    if terminator.id != instruction.id {
        return Err(Error::ArtifactMismatch);
    }
    if machine.alternative.key.family != conditional_branch_family(predicate) {
        return Err(Error::ArtifactMismatch);
    }
    let taken_offset = *block_offsets
        .get(&when_taken.block)
        .ok_or(Error::ArtifactMismatch)?;
    let fallthrough_offset = *block_offsets
        .get(&when_fallthrough.block)
        .ok_or(Error::ArtifactMismatch)?;
    let branch_size = if widened {
        8
    } else {
        branch_size(architecture)
    };
    let branch_end = instruction_offset
        .checked_add(branch_size)
        .ok_or(Error::ArtifactMismatch)?;
    if fallthrough_offset != branch_end {
        return Err(Error::ArtifactMismatch);
    }
    let displacement = match architecture {
        Architecture::X86_64 => checked_delta(taken_offset, branch_end)?,
        Architecture::Aarch64 => checked_delta(taken_offset, instruction_offset)?,
    };
    let (register_reads, effects) = match architecture {
        Architecture::X86_64 => {
            let decoded = match predicate {
                ResolvedConditionalBranchPredicate::NonZeroV1 => {
                    isa_x86_64::validate_x86_64_selected_nonzero_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                        &candidate.bytes,
                    )
                }
                ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                    isa_x86_64::validate_x86_64_selected_u64_less_than_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                        &candidate.bytes,
                    )
                }
                ResolvedConditionalBranchPredicate::I64LessThanV1 => {
                    isa_x86_64::validate_x86_64_selected_i64_less_than_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                        &candidate.bytes,
                    )
                }
            }
            .map_err(|_| Error::ArtifactMismatch)?;
            (
                decoded.footprint().register_reads.clone(),
                decoded.footprint().encoded.clone(),
            )
        }
        Architecture::Aarch64 => {
            let form = if widened {
                isa_aarch64::Aarch64ConditionalBranchForm::WidenedPairV1
            } else {
                isa_aarch64::Aarch64ConditionalBranchForm::CanonicalWordV1
            };
            let decoded = isa_aarch64::validate_aarch64_selected_conditional_branch_form(
                physical,
                machine.alternative.key,
                conditional_branch_family(predicate),
                form,
                displacement,
                &candidate.bytes,
            )
            .map_err(|_| Error::ArtifactMismatch)?;
            (
                decoded.footprint().register_reads.clone(),
                decoded.footprint().encoded.clone(),
            )
        }
    };
    if effects != machine.alternative.encoded {
        return Err(Error::ArtifactMismatch);
    }
    if u64::try_from(candidate.bytes.len()).ok() != Some(branch_size)
        || !declared_size_matches(machine.alternative.size, branch_size)
    {
        return Err(Error::ArtifactMismatch);
    }
    let evidence = ResolvedConditionalBranchEvidence {
        predicate,
        source_block: block.id,
        when_taken_edge: when_taken.psi_edge,
        when_taken_block: when_taken.block,
        when_taken_offset: taken_offset,
        when_fallthrough_edge: when_fallthrough.psi_edge,
        when_fallthrough_block: when_fallthrough.block,
        when_fallthrough_offset: fallthrough_offset,
        byte_displacement: displacement,
        decoded_register_reads: register_reads,
        decoded_effects: effects,
    };
    if candidate.branch.as_deref() != Some(&ResolvedBranchEvidence::Conditional(evidence)) {
        return Err(Error::ArtifactMismatch);
    }
    Ok(())
}

fn checked_delta(target: u64, base: u64) -> Result<i64, Error> {
    i64::try_from(i128::from(target) - i128::from(base)).map_err(|_| Error::ArtifactMismatch)
}

#[cfg(test)]
mod order_tests {
    crate::order_suite::order_suite!(super::derive_order);
}

#[cfg(test)]
mod tests {
    use isa_aarch64::{
        aarch64_physical_register_model, encode_aarch64_selected_conditional_branch_form,
        encode_aarch64_selected_jump_form,
    };
    use machine_code::{
        DeferredControlEncodingReason, SelectedFormDecodedFootprint, SelectedFormEncodingRow,
        SelectedFormEncodingState, SelectedFormMachineDisposition,
    };
    use physical_instructions::{
        PostAllocationMachineBlock, PostAllocationMachineFunction, PostAllocationMachineInstruction,
    };
    use selected_instructions::{
        MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
        MachineAlternativeKey, MachineLatencyKnowledge, MachineSizeKnowledge, SelectedBlock,
        SelectedFunction, SelectedInstruction, SelectedInstructionKind, SelectedTerminator,
    };
    use semantic_vocabulary::EdgeId;
    use target::Architecture;

    use super::validate_function;
    use crate::order_suite::{block_with, branch, function_from_blocks, instruction, successor};
    use crate::ordinary::layout;
    use crate::{
        ResolvedBranchEvidence, ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy,
    };

    const PAD_BEYOND_IMM19: usize = (1 << 18) + 4;

    fn jump(source: u32, destination: u32) -> SelectedTerminator {
        SelectedTerminator::Jump {
            instruction: instruction(source, SelectedInstructionKind::Jump),
            successor: successor(destination),
        }
    }

    fn returned(source: u32) -> SelectedTerminator {
        SelectedTerminator::Return {
            instruction: instruction(source, SelectedInstructionKind::ReturnUnit),
            psi_return_edge: EdgeId::new(u64::from(source) + 1).unwrap(),
        }
    }

    fn block(id: u32, terminator: SelectedTerminator) -> SelectedBlock {
        block_with(id, Vec::new(), terminator)
    }

    fn physical() -> register_model::ValidatedPhysicalRegisterModel {
        register_model::validate_physical_register_model(aarch64_physical_register_model()).unwrap()
    }

    fn branch_alternative() -> MachineAlternative {
        let physical = physical();
        let key = MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchNonZero,
            variant: 0,
        };
        let encoded = encode_aarch64_selected_conditional_branch_form(
            &physical,
            key,
            MachineAlternativeFamily::ConditionalBranchNonZero,
            isa_aarch64::Aarch64ConditionalBranchForm::CanonicalWordV1,
            8,
        )
        .unwrap();
        MachineAlternative {
            key,
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(8),
            },
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: encoded.footprint().encoded.clone(),
        }
    }

    fn jump_alternative() -> MachineAlternative {
        let physical = physical();
        let key = MachineAlternativeKey {
            family: MachineAlternativeFamily::Jump,
            variant: 0,
        };
        let encoded = encode_aarch64_selected_jump_form(&physical, key, 4).unwrap();
        MachineAlternative {
            key,
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(4),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: encoded.footprint().encoded.clone(),
        }
    }

    fn plain_alternative() -> MachineAlternative {
        MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::CopyI64,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(4),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: branch_alternative().encoded,
        }
    }

    fn row_alternative(
        instruction: &SelectedInstruction,
        alternatives: &Alternatives,
    ) -> MachineAlternativeKey {
        alternatives.of(instruction).key
    }

    fn footprint(alternatives: &Alternatives) -> SelectedFormDecodedFootprint {
        SelectedFormDecodedFootprint {
            register_reads: Vec::new(),
            register_writes: Vec::new(),
            implicit_defs: Vec::new(),
            implicit_clobbers: Vec::new(),
            encoded: alternatives.plain.encoded.clone(),
        }
    }

    fn encoded_row(
        instruction: &SelectedInstruction,
        footprint: &SelectedFormDecodedFootprint,
        encoded: &mut machine_code::SelectedFormEncodedRowStore,
    ) -> SelectedFormEncodingRow {
        let (code, interned) = encoded.intern(&[0; 4], footprint.clone());
        SelectedFormEncodingRow {
            instruction: instruction.id,
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::CopyI64,
                variant: 0,
            },
            machine_disposition: SelectedFormMachineDisposition::RetainedV1,
            state: SelectedFormEncodingState::Encoded,
            address: None,
            code,
            footprint: interned,
        }
    }

    fn deferred_row(
        instruction: &SelectedInstruction,
        alternatives: &Alternatives,
    ) -> SelectedFormEncodingRow {
        SelectedFormEncodingRow {
            instruction: instruction.id,
            alternative: row_alternative(instruction, alternatives),
            machine_disposition: SelectedFormMachineDisposition::RetainedV1,
            state: SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            },
            address: None,
            code: machine_code::SelectedFormEncodedByteSpan::default(),
            footprint: machine_code::SelectedFormDecodedFootprintHandle::default(),
        }
    }

    fn terminator_row(terminator: &SelectedTerminator) -> &SelectedInstruction {
        selected_instructions::terminator_instruction(terminator)
    }

    fn machine_row(
        instruction: &SelectedInstruction,
        alternative: &MachineAlternative,
    ) -> PostAllocationMachineInstruction {
        PostAllocationMachineInstruction {
            instruction: instruction.id,
            alternative: alternative.clone(),
            operands: Vec::new(),
            address: None,
            implicit_unit_uses: Vec::new(),
            implicit_unit_defs: Vec::new(),
            implicit_unit_clobbers: Vec::new(),
            unit_uses: Vec::new(),
            unit_defs: Vec::new(),
            unit_clobbers: Vec::new(),
        }
    }

    fn pre_layout_rows(
        function: &SelectedFunction,
        footprint: &SelectedFormDecodedFootprint,
        alternatives: &Alternatives,
        encoded: &mut machine_code::SelectedFormEncodedRowStore,
    ) -> Vec<SelectedFormEncodingRow> {
        let mut rows = Vec::new();
        for block in &function.blocks {
            for instruction in &block.instructions {
                rows.push(encoded_row(instruction, footprint, encoded));
            }
            let terminator = terminator_row(&block.terminator);
            rows.push(match &block.terminator {
                SelectedTerminator::ConditionalBranch { .. }
                | selected_instructions::conditional_branch_less_pair_pattern!(..)
                | SelectedTerminator::Jump { .. } => deferred_row(terminator, alternatives),
                _ => encoded_row(terminator, footprint, encoded),
            });
        }
        rows
    }

    fn machine_function(
        function: &SelectedFunction,
        alternatives: &Alternatives,
    ) -> PostAllocationMachineFunction {
        PostAllocationMachineFunction {
            machine: function.machine,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            blocks: function
                .blocks
                .iter()
                .map(|block| PostAllocationMachineBlock {
                    block: block.id,
                    instructions: block
                        .instructions
                        .iter()
                        .chain(std::iter::once(terminator_row(&block.terminator)))
                        .map(|instruction| machine_row(instruction, alternatives.of(instruction)))
                        .collect(),
                })
                .collect(),
        }
    }

    struct Alternatives {
        branch: MachineAlternative,
        jump: MachineAlternative,
        plain: MachineAlternative,
    }

    impl Alternatives {
        fn of(&self, instruction: &SelectedInstruction) -> &MachineAlternative {
            match instruction.kind {
                SelectedInstructionKind::ConditionalBranchNonZero => &self.branch,
                SelectedInstructionKind::Jump => &self.jump,
                _ => &self.plain,
            }
        }
    }

    fn produce_and_admit(selected: &SelectedFunction) -> ResolvedSelectedFunctionLayout {
        let alternatives = Alternatives {
            branch: branch_alternative(),
            jump: jump_alternative(),
            plain: plain_alternative(),
        };
        let footprint = footprint(&alternatives);
        let mut encoded = machine_code::SelectedFormEncodedRowStore::new();
        let rows = pre_layout_rows(selected, &footprint, &alternatives, &mut encoded);
        let machine = machine_function(selected, &alternatives);
        let pre_rows: std::collections::BTreeMap<_, _> =
            rows.iter().map(|row| (row.instruction, row)).collect();
        let machine_rows: std::collections::BTreeMap<_, _> = machine
            .instructions()
            .map(|instruction| (instruction.instruction, instruction))
            .collect();
        let produced = layout(
            Architecture::Aarch64,
            selected,
            &pre_rows,
            &machine_rows,
            &physical(),
            SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
            &encoded,
        )
        .unwrap();
        let mut iter = rows.iter();
        validate_function(
            Architecture::Aarch64,
            selected,
            &machine,
            &physical(),
            SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
            &mut iter,
            &produced,
            &encoded,
        )
        .unwrap();
        assert!(iter.next().is_none());
        produced
    }

    #[test]
    fn taken_edge_beyond_imm19_widens_to_inverted_skip_plus_unconditional_branch() {
        let padding: Vec<SelectedInstruction> = (0..PAD_BEYOND_IMM19 as u32)
            .map(|index| instruction(1000 + index, SelectedInstructionKind::CopyI64))
            .collect();
        let selected = function_from_blocks(vec![
            block(0, branch(0, 2, 1)),
            block_with(1, padding, jump(1, 2)),
            block(2, returned(2)),
        ]);
        let layout = produce_and_admit(&selected);
        let row = &layout.blocks[0].instructions[0];
        assert_eq!(row.bytes.len(), 8);
        let skip = u32::from_le_bytes(row.bytes[0..4].try_into().unwrap());
        assert_eq!(skip, 0x5400_0000 | (2 << 5));
        let target = u32::from_le_bytes(row.bytes[4..8].try_into().unwrap());
        assert_eq!(target & 0xfc00_0000, 0x1400_0000);
        let branch_offset = layout.blocks[0].instructions[0].offset;
        let taken_offset = layout.blocks[2].offset;
        let decoded = i64::from(((target & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
        assert_eq!(
            decoded,
            i64::try_from(taken_offset).unwrap() - i64::try_from(branch_offset).unwrap() - 4
        );
        assert!(
            i64::try_from(taken_offset).unwrap() - i64::try_from(branch_offset).unwrap() > 1 << 20
        );
        let Some(ResolvedBranchEvidence::Conditional(evidence)) =
            layout.blocks[0].instructions[0].branch.as_deref()
        else {
            panic!("conditional branch must carry conditional evidence")
        };
        assert_eq!(
            evidence.byte_displacement,
            i64::try_from(taken_offset).unwrap() - i64::try_from(branch_offset).unwrap()
        );
        let jump_row = layout.blocks[1].instructions.last().unwrap();
        assert_eq!(jump_row.bytes.len(), 4);
        assert_eq!(
            u32::from_le_bytes(jump_row.bytes[..4].try_into().unwrap()) & 0xfc00_0000,
            0x1400_0000
        );
    }

    #[test]
    fn backward_taken_edge_beyond_imm19_widens() {
        let padding: Vec<SelectedInstruction> = (0..PAD_BEYOND_IMM19 as u32)
            .map(|index| instruction(2000 + index, SelectedInstructionKind::CopyI64))
            .collect();
        let selected = function_from_blocks(vec![
            block(0, jump(0, 1)),
            block_with(1, padding, branch(1, 0, 2)),
            block(2, returned(2)),
        ]);
        let layout = produce_and_admit(&selected);
        let row = layout.blocks[1].instructions.last().unwrap();
        assert_eq!(row.bytes.len(), 8);
        let skip = u32::from_le_bytes(row.bytes[0..4].try_into().unwrap());
        assert_eq!(skip, 0x5400_0000 | (2 << 5));
        let target = u32::from_le_bytes(row.bytes[4..8].try_into().unwrap());
        let decoded = i64::from(((target & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
        assert_eq!(decoded, -(i64::try_from(row.offset).unwrap()) - 4);
    }

    #[test]
    fn in_range_branch_keeps_the_canonical_four_byte_form() {
        let padding: Vec<SelectedInstruction> = (0..4u32)
            .map(|index| instruction(3000 + index, SelectedInstructionKind::CopyI64))
            .collect();
        let selected = function_from_blocks(vec![
            block(0, branch(0, 2, 1)),
            block_with(1, padding, jump(1, 2)),
            block(2, returned(2)),
        ]);
        let layout = produce_and_admit(&selected);
        let row = &layout.blocks[0].instructions[0];
        assert_eq!(row.bytes.len(), 4);
        let word = u32::from_le_bytes(row.bytes[..4].try_into().unwrap());
        assert_eq!(word & 0xff00_001f, 0x5400_0001);
    }
}
