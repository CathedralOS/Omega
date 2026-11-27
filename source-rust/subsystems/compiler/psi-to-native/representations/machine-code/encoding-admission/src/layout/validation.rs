use machine_code::layout_requirements::*;
/*
We validate retained function-relative layouts when explicitly requested, not on the producer's ordinary construction path. We verify root identities, re-derive canonical block order, recompute byte spans including AArch64 branch widening, and check row bytes, fixups and decoded branch footprints against the pre-layout store and physical register model.
*/

use std::collections::BTreeMap;

use machine_code::{
    DeferredControlEncodingReason, ResolvedMachineLayout, SelectedFormEncodedRowStore,
    SelectedFormEncoding, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition, resolved_machine_layout_identity as layout_identity,
};
use post_allocation_machine::PostAllocationMachineOptimizationCustody;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::SelectedBlockId;
use target::Architecture;

use super::geometry::{
    ConditionalBranchDescriptor, branch_size, conditional_branch_descriptor,
    conditional_branch_family, derive_plan, function_pre_rows, instructions, select,
};
use super::{
    OptimizedResolvedSelectedFormLayoutError as Error, ResolvedBranchEvidence,
    ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy, declared_size_matches,
};

type PreLayoutRows<'a> = std::slice::Iter<'a, SelectedFormEncodingRow>;

pub(crate) fn validate(
    selected: &EncodedLayoutRequirements,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
    artifact: &ResolvedMachineLayout,
    allow_x86_short: bool,
) -> Result<(), Error> {
    let selected_plan = selected;
    let machine = selected;
    let machine_plan = machine;
    if pre_layout.selected() != machine.selected
        || pre_layout.machine() != machine.machine
        || selected_plan.target != machine_plan.target
        || selected_plan.target.architecture != physical.model().architecture
        || selected_plan.functions.len() != machine_plan.alternatives.len()
        || pre_layout.post_allocation_machine_optimization().is_some()
    {
        return Err(Error::RootMismatch);
    }
    let expected_policy = select(&selected_plan.functions)?;
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
        allow_x86_short,
    )?;
    validate_identity(artifact)
}

fn validate_roots(
    selected: &EncodedLayoutRequirements,
    machine: &EncodedLayoutRequirements,
    pre_layout: &SelectedFormEncoding,
    optimization: Option<PostAllocationMachineOptimizationCustody>,
    policy: SelectedFunctionLayoutPolicy,
    artifact: &ResolvedMachineLayout,
) -> Result<(), Error> {
    if artifact.selected() != machine.selected
        || artifact.machine() != machine.machine
        || artifact.pre_layout() != pre_layout.identity()
        || artifact.post_allocation_machine_optimization() != optimization
        || artifact.target() != selected.target
        || artifact.policy() != policy
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok(())
}

fn validate_identity(artifact: &ResolvedMachineLayout) -> Result<(), Error> {
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
    selected: &EncodedLayoutRequirements,
    machine: &EncodedLayoutRequirements,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
    policy: SelectedFunctionLayoutPolicy,
    artifact: &ResolvedMachineLayout,
    allow_x86_short: bool,
) -> Result<(), Error> {
    let selected_plan = selected;
    let machine_plan = machine;
    if artifact.functions().len() != selected_plan.functions.len() {
        return Err(Error::ArtifactMismatch);
    }
    let mut pre_rows = pre_layout.rows().iter();
    for ((selected_function, machine_function), candidate) in selected_plan
        .functions
        .iter()
        .zip(&machine_plan.alternatives)
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
            allow_x86_short,
        )?;
    }
    if pre_rows.next().is_some() {
        return Err(Error::RootMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn validate_function(
    architecture: Architecture,
    selected: &EncodedLayoutFunction,
    machine: &EncodedFunctionAlternatives,
    physical: &ValidatedPhysicalRegisterModel,
    policy: SelectedFunctionLayoutPolicy,
    pre_rows: &mut PreLayoutRows<'_>,
    candidate: &ResolvedSelectedFunctionLayout,
    encoded: &SelectedFormEncodedRowStore,
    allow_x86_short: bool,
) -> Result<(), Error> {
    if selected.machine != machine.machine
        || selected.machine != candidate.machine
        || selected.blocks.len() != machine.blocks.len()
    {
        return Err(Error::ArtifactMismatch);
    }
    let pre_rows = function_pre_rows(selected, pre_rows)?;
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
    let layout = if allow_x86_short && architecture == Architecture::X86_64 {
        current_x86_spans(&ordered, candidate)?
    } else {
        derive_plan(architecture, &ordered, &pre_rows)?
    };
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
                .get(instruction.id)
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
                allow_x86_short,
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

// A relaxed x86 layout admits two exact conditional widths. Accumulate its
// actual row widths, then independently decode every row and verify each edge
// against these offsets. This checks the current result without retaining or
// reconstructing the earlier near-branch layout.
fn current_x86_spans(
    ordered: &[&EncodedLayoutBlock],
    candidate: &ResolvedSelectedFunctionLayout,
) -> Result<super::geometry::LayoutPlan, Error> {
    let mut layout = super::geometry::LayoutPlan {
        block_offsets: BTreeMap::new(),
        block_sizes: BTreeMap::new(),
        instruction_offsets: BTreeMap::new(),
        function_size: 0,
        widened_branches: std::collections::BTreeSet::new(),
    };
    for (block, claimed) in ordered.iter().zip(&candidate.blocks) {
        if claimed.block != block.id || claimed.instructions.len() != block.instructions.len() + 1 {
            return Err(Error::ArtifactMismatch);
        }
        let start = layout.function_size;
        layout.block_offsets.insert(block.id, start);
        for (instruction, row) in instructions(block).zip(&claimed.instructions) {
            if row.instruction != instruction.id {
                return Err(Error::ArtifactMismatch);
            }
            layout.function_size = layout
                .function_size
                .checked_add(u64::try_from(row.bytes.len()).map_err(|_| Error::OffsetOverflow)?)
                .ok_or(Error::OffsetOverflow)?;
        }
        layout
            .block_sizes
            .insert(block.id, layout.function_size - start);
    }
    Ok(layout)
}

pub fn derive_order(
    function: &EncodedLayoutFunction,
    policy: SelectedFunctionLayoutPolicy,
) -> Result<Vec<&EncodedLayoutBlock>, Error> {
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
            || !matches!(entry.terminator, EncodedLayoutTerminator::Return { .. }))
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

fn fallthrough(block: &EncodedLayoutBlock) -> Option<SelectedBlockId> {
    conditional_branch_descriptor(block).map(|descriptor| descriptor.when_fallthrough.block)
}

#[allow(clippy::too_many_arguments)]
pub fn validate_row(
    architecture: Architecture,
    block: &EncodedLayoutBlock,
    instruction: &EncodedLayoutRow,
    machine: &EncodedAlternativeRequirement,
    pre: &SelectedFormEncodingRow,
    physical: &ValidatedPhysicalRegisterModel,
    expected_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    candidate: &ResolvedSelectedFormRow,
    widened: bool,
    encoded: &SelectedFormEncodedRowStore,
    allow_x86_short: bool,
) -> Result<(), Error> {
    let short_branch = allow_x86_short
        && architecture == Architecture::X86_64
        && candidate.bytes.len() == 2
        && conditional_branch_descriptor(block)
            .is_some_and(|branch| branch.terminator.id == instruction.id);
    let mut expected_alternative = machine.alternative.key;
    if short_branch
        && (machine.alternative.key.variant != 0
            || !declared_size_matches(machine.alternative.size, 6))
    {
        return Err(Error::ArtifactMismatch);
    }
    if short_branch
        && !matches!(
            instruction.kind,
            EncodedLayoutOperation::ConditionalBranchNonZero
        )
    {
        expected_alternative.variant = 1;
    }
    if candidate.instruction != instruction.id
        || pre.alternative != machine.alternative.key
        || candidate.alternative != expected_alternative
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
                EncodedLayoutOperation::Call { callee: Some(callee) } if callee == fixup.callee
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
                EncodedLayoutOperation::NormalizedForeignCall { boundary, ordinal }
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
                EncodedLayoutOperation::ImageAddress { reference } if fixup.reference == reference
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
                ..
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
            short_branch,
        ),
        _ => Err(Error::ArtifactMismatch),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_branch(
    architecture: Architecture,
    block: &EncodedLayoutBlock,
    instruction: &EncodedLayoutRow,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &EncodedAlternativeRequirement,
    physical: &ValidatedPhysicalRegisterModel,
    candidate: &ResolvedSelectedFormRow,
    widened: bool,
    short_branch: bool,
) -> Result<(), Error> {
    if candidate.internal_machine_fixup.is_some()
        || candidate.normalized_foreign_call_fixup.is_some()
        || candidate.image_address_fixup.is_some()
    {
        return Err(Error::ArtifactMismatch);
    }
    if let EncodedLayoutTerminator::Jump {
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
    let branch_size = if short_branch {
        2
    } else if widened {
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
                    let validate = if short_branch {
                        isa_x86_64::validate_x86_64_selected_short_nonzero_branch_form
                    } else {
                        isa_x86_64::validate_x86_64_selected_nonzero_branch_form
                    };
                    validate(
                        physical,
                        candidate.alternative,
                        displacement,
                        &candidate.bytes,
                    )
                }
                ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                    isa_x86_64::validate_x86_64_selected_u64_less_than_branch_form(
                        physical,
                        candidate.alternative,
                        displacement,
                        &candidate.bytes,
                    )
                }
                ResolvedConditionalBranchPredicate::I64LessThanV1 => {
                    isa_x86_64::validate_x86_64_selected_i64_less_than_branch_form(
                        physical,
                        candidate.alternative,
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
        || (!short_branch && !declared_size_matches(machine.alternative.size, branch_size))
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
