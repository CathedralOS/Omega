//! Compute byte spans of already encoded alternatives. The producer and
//! checker retain their separate block-order derivations; both use these exact
//! displacement equations and monotone AArch64 widening bounds.

use super::OptimizedResolvedSelectedFormLayoutError as Error;
use machine_code::layout_requirements::*;
use machine_code::{
    DeferredControlEncodingReason, ResolvedConditionalBranchPredicate, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormMachineDisposition, SelectedFunctionLayoutPolicy,
};
use selected_instructions::{
    MachineAlternativeFamily, MachineSizeKnowledge, SelectedBlockId, SelectedInstructionId,
};
use std::collections::{BTreeMap, BTreeSet};
use target::Architecture;

pub struct ConditionalBranchDescriptor<'a> {
    pub predicate: ResolvedConditionalBranchPredicate, // Branch predicate family encoded by the terminator.
    pub terminator: &'a EncodedLayoutRow, // Underlying conditional-branch instruction row.
    pub when_taken: &'a EncodedLayoutSuccessor, // Successor edge reached when the predicate holds.
    pub when_fallthrough: &'a EncodedLayoutSuccessor, // Successor edge that must sit immediately after the branch.
}

pub struct LayoutPlan {
    pub block_offsets: BTreeMap<SelectedBlockId, u64>, // Function-relative start offset of each block.
    pub block_sizes: BTreeMap<SelectedBlockId, u64>,   // Total byte size of each block.
    pub instruction_offsets: BTreeMap<SelectedInstructionId, u64>, // Function-relative byte offset of each instruction.
    pub function_size: u64, // Total byte size of the function across all blocks.
    pub widened_branches: BTreeSet<SelectedInstructionId>, // AArch64 conditional branches widened to 8-byte skip+branch pairs.
}

/// One function's rows looked up by instruction. Instruction identifiers are
/// sparse, so the rows sit in one slice sorted by identifier and sized to the
/// function, not in a tree or a table spanning every identifier.
pub struct InstructionRows<'rows, Row> {
    rows: Vec<(SelectedInstructionId, &'rows Row)>,
}

impl<'rows, Row> InstructionRows<'rows, Row> {
    /// Refuses the rows when one instruction has two of them, naming it.
    pub fn new(
        rows: impl IntoIterator<Item = (SelectedInstructionId, &'rows Row)>,
    ) -> Result<Self, SelectedInstructionId> {
        let mut rows: Vec<_> = rows.into_iter().collect();
        if !rows.is_sorted_by_key(|(instruction, _)| *instruction) {
            rows.sort_unstable_by_key(|(instruction, _)| *instruction);
        }
        if let Some(pair) = rows.windows(2).find(|pair| pair[0].0 == pair[1].0) {
            return Err(pair[0].0);
        }
        Ok(Self { rows })
    }

    pub fn get(&self, instruction: SelectedInstructionId) -> Option<&'rows Row> {
        self.rows
            .binary_search_by_key(&instruction, |(identity, _)| *identity)
            .ok()
            .map(|position| self.rows[position].1)
    }
}

/// Take one function's pre-layout rows from the program-wide row stream. The
/// stream lists each function's instructions block by block, so every row must
/// name the instruction at its position.
pub fn function_pre_rows<'rows>(
    function: &EncodedLayoutFunction,
    rows: &mut std::slice::Iter<'rows, SelectedFormEncodingRow>,
) -> Result<InstructionRows<'rows, SelectedFormEncodingRow>, Error> {
    let row_count = function
        .blocks
        .iter()
        .map(|block| block.instructions.len() + 1)
        .sum();
    let mut collected = Vec::with_capacity(row_count);
    for block in &function.blocks {
        for instruction in instructions(block) {
            let row = rows
                .next()
                .filter(|row| row.instruction == instruction.id)
                .ok_or(Error::MissingInstruction(instruction.id))?;
            collected.push((instruction.id, row));
        }
    }
    InstructionRows::new(collected).map_err(Error::DuplicateInstruction)
}

pub fn select(functions: &[EncodedLayoutFunction]) -> Result<SelectedFunctionLayoutPolicy, Error> {
    let single_entry_count = functions
        .iter()
        .filter(|function| is_single_entry(function))
        .count();
    if single_entry_count == functions.len() {
        Ok(SelectedFunctionLayoutPolicy::SingleEntryBlockV1)
    } else if single_entry_count == 0
        && functions.iter().all(|function| {
            matches!(
                function.blocks.first().map(|block| &block.terminator),
                Some(EncodedLayoutTerminator::ConditionalBranch { .. })
            )
        })
    {
        Ok(SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1)
    } else if single_entry_count == 0
        && functions.iter().all(|function| {
            matches!(
                function.blocks.first().map(|block| &block.terminator),
                Some(
                    EncodedLayoutTerminator::ConditionalBranchU64LessThan { .. }
                        | EncodedLayoutTerminator::ConditionalBranchI64LessThan { .. }
                )
            )
        })
    {
        Ok(SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1)
    } else if let Some(function) = functions
        .iter()
        .find(|function| !is_admitted_canonical_shape(function))
    {
        // Name the function that has no canonical shape, not whichever
        // function happens to sit at a counted position.
        Err(Error::UnsupportedFunctionShape(function.machine))
    } else {
        Ok(SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1)
    }
}

pub fn derive_plan(
    architecture: Architecture,
    blocks: &[&EncodedLayoutBlock],
    pre_rows: &InstructionRows<SelectedFormEncodingRow>,
) -> Result<LayoutPlan, Error> {
    let mut widened = BTreeSet::new();
    loop {
        let plan = derive_once(architecture, blocks, pre_rows, &widened)?;
        if architecture != Architecture::Aarch64 {
            return Ok(plan);
        }
        let mut grown = widened;
        for block in blocks {
            let Some(descriptor) = conditional_branch_descriptor(block) else {
                continue;
            };
            if grown.contains(&descriptor.terminator.id) {
                continue;
            }
            let missing = || Error::MissingInstruction(descriptor.terminator.id);
            let row = pre_rows.get(descriptor.terminator.id).ok_or_else(missing)?;
            if !widening_candidate(descriptor.terminator, row) {
                continue;
            }
            let taken_offset = *plan
                .block_offsets
                .get(&descriptor.when_taken.block)
                .ok_or_else(missing)?;
            let instruction_offset = *plan
                .instruction_offsets
                .get(&descriptor.terminator.id)
                .ok_or_else(missing)?;
            let displacement = byte_displacement(taken_offset, instruction_offset)?;
            if !isa_aarch64::AARCH64_CONDITIONAL_BRANCH_BYTE_REACH.contains(&displacement) {
                grown.insert(descriptor.terminator.id);
            }
        }
        if grown == plan.widened_branches {
            return Ok(plan);
        }
        widened = grown;
    }
}

fn derive_once(
    architecture: Architecture,
    blocks: &[&EncodedLayoutBlock],
    pre_rows: &InstructionRows<SelectedFormEncodingRow>,
    widened: &BTreeSet<SelectedInstructionId>,
) -> Result<LayoutPlan, Error> {
    let mut block_offsets = BTreeMap::new();
    let mut block_sizes = BTreeMap::new();
    let mut instruction_offsets = BTreeMap::new();
    let mut offset = 0_u64;
    for block in blocks {
        block_offsets.insert(block.id, offset);
        let start = offset;
        for instruction in instructions(block) {
            instruction_offsets.insert(instruction.id, offset);
            let pre = pre_rows
                .get(instruction.id)
                .ok_or(Error::MissingInstruction(instruction.id))?;
            offset = offset
                .checked_add(instruction_size(architecture, instruction, pre, widened)?)
                .ok_or(Error::OffsetOverflow)?;
        }
        block_sizes.insert(block.id, offset - start);
    }
    Ok(LayoutPlan {
        block_offsets,
        block_sizes,
        instruction_offsets,
        function_size: offset,
        widened_branches: widened.clone(),
    })
}

fn widening_candidate(instruction: &EncodedLayoutRow, row: &SelectedFormEncodingRow) -> bool {
    matches!(
        instruction.kind,
        EncodedLayoutOperation::ConditionalBranchNonZero
            | EncodedLayoutOperation::ConditionalBranchU64LessThan
            | EncodedLayoutOperation::ConditionalBranchI64LessThan
    ) && matches!(
        row.machine_disposition,
        SelectedFormMachineDisposition::RetainedV1
    ) && matches!(
        row.state,
        SelectedFormEncodingState::DeferredControl {
            reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            ..
        }
    )
}

pub fn byte_displacement(target: u64, base: u64) -> Result<i64, Error> {
    i64::try_from(i128::from(target) - i128::from(base)).map_err(|_| Error::OffsetOverflow)
}

pub fn instructions(block: &EncodedLayoutBlock) -> impl Iterator<Item = &EncodedLayoutRow> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(block.terminator.instruction()))
}

fn instruction_size(
    architecture: Architecture,
    instruction: &EncodedLayoutRow,
    row: &SelectedFormEncodingRow,
    widened: &BTreeSet<SelectedInstructionId>,
) -> Result<u64, Error> {
    match &row.machine_disposition {
        SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(instruction.kind, EncodedLayoutOperation::CompareI64Zero)
                && matches!(row.state, SelectedFormEncodingState::Encoded)
            {
                return Ok(0);
            }
            return unexpected(instruction.id);
        }
        SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 { .. } => {
            if architecture == Architecture::Aarch64
                && let SelectedFormEncodingState::DeferredControl { alternatives, .. } = &row.state
            {
                return u64::try_from(alternatives.baseline.bytes.len())
                    .map_err(|_| Error::OffsetOverflow);
            }
            return unexpected(instruction.id);
        }
        SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(instruction.kind, EncodedLayoutOperation::CopyI64)
                && matches!(row.state, SelectedFormEncodingState::Encoded)
            {
                return Ok(0);
            }
            return unexpected(instruction.id);
        }
        SelectedFormMachineDisposition::RetainedV1 => {}
    }
    match &row.state {
        SelectedFormEncodingState::Encoded => Ok(row.code.byte_count),
        SelectedFormEncodingState::UnresolvedInternalMachineCall { .. } => {
            if !matches!(instruction.kind, EncodedLayoutOperation::Call { .. }) {
                return unexpected(instruction.id);
            }
            Ok(row.code.byte_count)
        }
        SelectedFormEncodingState::UnresolvedNormalizedForeignCall { .. } => {
            if !matches!(
                instruction.kind,
                EncodedLayoutOperation::NormalizedForeignCall { .. }
            ) {
                return unexpected(instruction.id);
            }
            Ok(row.code.byte_count)
        }
        SelectedFormEncodingState::UnresolvedImageAddress { .. } => {
            if !matches!(
                instruction.kind,
                EncodedLayoutOperation::ImageAddress { .. }
            ) {
                return unexpected(instruction.id);
            }
            Ok(row.code.byte_count)
        }
        SelectedFormEncodingState::DeferredControl {
            reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            alternatives,
        } => {
            if row.instruction != instruction.id {
                return Err(Error::MissingInstruction(instruction.id));
            }
            let form = if widened.contains(&instruction.id) {
                alternatives
                    .widened
                    .as_ref()
                    .ok_or(Error::UnexpectedEncodingState(instruction.id))?
            } else {
                &alternatives.baseline
            };
            u64::try_from(form.bytes.len()).map_err(|_| Error::OffsetOverflow)
        }
    }
}

pub const fn branch_size(architecture: Architecture) -> u64 {
    match architecture {
        Architecture::X86_64 => 6,
        Architecture::Aarch64 => 4,
    }
}

pub fn unexpected<T>(instruction: SelectedInstructionId) -> Result<T, Error> {
    Err(Error::UnexpectedEncodingState(instruction))
}

pub fn conditional_branch_descriptor(
    block: &EncodedLayoutBlock,
) -> Option<ConditionalBranchDescriptor<'_>> {
    match &block.terminator {
        EncodedLayoutTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => Some(ConditionalBranchDescriptor {
            predicate: ResolvedConditionalBranchPredicate::NonZeroV1,
            terminator: instruction,
            when_taken: when_nonzero,
            when_fallthrough: when_zero,
        }),
        EncodedLayoutTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => Some(ConditionalBranchDescriptor {
            predicate: ResolvedConditionalBranchPredicate::U64LessThanV1,
            terminator: instruction,
            when_taken: when_less,
            when_fallthrough: when_not_less,
        }),
        EncodedLayoutTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => Some(ConditionalBranchDescriptor {
            predicate: ResolvedConditionalBranchPredicate::I64LessThanV1,
            terminator: instruction,
            when_taken: when_less,
            when_fallthrough: when_not_less,
        }),
        EncodedLayoutTerminator::Jump { .. }
        | EncodedLayoutTerminator::Stop { .. }
        | EncodedLayoutTerminator::Return { .. } => None,
    }
}

pub const fn conditional_branch_family(
    predicate: ResolvedConditionalBranchPredicate,
) -> MachineAlternativeFamily {
    match predicate {
        ResolvedConditionalBranchPredicate::NonZeroV1 => {
            MachineAlternativeFamily::ConditionalBranchNonZero
        }
        ResolvedConditionalBranchPredicate::U64LessThanV1 => {
            MachineAlternativeFamily::ConditionalBranchU64LessThan
        }
        ResolvedConditionalBranchPredicate::I64LessThanV1 => {
            MachineAlternativeFamily::ConditionalBranchI64LessThan
        }
    }
}

pub fn declared_size_matches(knowledge: MachineSizeKnowledge, actual: u64) -> bool {
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

pub fn is_admitted_canonical_shape(
    function: &machine_code::layout_requirements::EncodedLayoutFunction,
) -> bool {
    !function.blocks.is_empty() && function.block(function.entry_block).is_some()
}

pub fn is_single_entry(
    function: &machine_code::layout_requirements::EncodedLayoutFunction,
) -> bool {
    let Some(block) = function
        .blocks
        .first()
        .filter(|_| function.blocks.len() == 1)
    else {
        return false;
    };
    function.entry_block == block.id
        && matches!(
            block.terminator,
            machine_code::layout_requirements::EncodedLayoutTerminator::Return { .. }
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::MachineId;

    fn returning(machine: u64, block: u32) -> EncodedLayoutFunction {
        EncodedLayoutFunction {
            machine: MachineId::new(machine).unwrap(),
            entry_block: SelectedBlockId(block),
            blocks: vec![EncodedLayoutBlock {
                id: SelectedBlockId(block),
                instructions: Vec::new(),
                terminator: EncodedLayoutTerminator::Return {
                    instruction: EncodedLayoutRow {
                        id: SelectedInstructionId(block),
                        kind: EncodedLayoutOperation::Ordinary,
                    },
                },
            }],
        }
    }

    // A function with no blocks has no canonical shape. The refusal must name
    // that function, wherever it sits relative to admitted functions.
    #[test]
    fn unsupported_shape_names_the_function_without_a_canonical_shape() {
        let empty = EncodedLayoutFunction {
            machine: MachineId::new(7).unwrap(),
            entry_block: SelectedBlockId(9),
            blocks: Vec::new(),
        };
        for functions in [
            vec![empty.clone(), returning(1, 1)],
            vec![returning(1, 1), empty.clone()],
            vec![returning(1, 1), returning(2, 2), empty.clone()],
        ] {
            assert_eq!(
                select(&functions),
                Err(Error::UnsupportedFunctionShape(empty.machine))
            );
        }
        assert_eq!(
            select(&[returning(1, 1), returning(2, 2)]),
            Ok(SelectedFunctionLayoutPolicy::SingleEntryBlockV1)
        );
    }

    // Sparse identifiers given out of order still resolve to their own rows;
    // an identifier with two rows is refused by name.
    #[test]
    fn instruction_rows_find_sparse_rows_and_refuse_duplicates() {
        let (low, mid, high) = ("low", "mid", "high");
        let rows = InstructionRows::new([
            (SelectedInstructionId(900), &high),
            (SelectedInstructionId(3), &low),
            (SelectedInstructionId(40), &mid),
        ])
        .unwrap();
        assert_eq!(rows.get(SelectedInstructionId(3)), Some(&low));
        assert_eq!(rows.get(SelectedInstructionId(40)), Some(&mid));
        assert_eq!(rows.get(SelectedInstructionId(900)), Some(&high));
        assert_eq!(rows.get(SelectedInstructionId(41)), None);
        assert_eq!(
            InstructionRows::new([
                (SelectedInstructionId(5), &low),
                (SelectedInstructionId(5), &mid),
            ])
            .err(),
            Some(SelectedInstructionId(5))
        );
    }
}
