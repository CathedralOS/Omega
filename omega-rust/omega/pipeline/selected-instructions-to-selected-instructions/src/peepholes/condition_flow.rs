//! Condition-state resolution over the selected CFG for peephole pairs.
//!
//! Every condition-state pair family's admission and its independent replay
//! both ask the same two questions of the selected function: which
//! condition-state event every execution path to the consumer's flag read
//! last observed — the in-block rule when a definition or clobber precedes
//! the read, otherwise the least-fixpoint entry-event walk over the
//! backward-reachable predecessor cone — and whether the resolved producer's
//! operands are compile-time constant. This module owns that machinery for
//! `peepholes`: the sibling `rewrites::condition_state` walk answers the
//! same questions for the constant-condition rewrites, but stays inside the
//! rewrites subtree, so the pair families' analysis stands alone here and
//! is shared by the descriptor-driven admissions and the descriptor-free
//! replays alike. `terminator_pair` reads at a block's terminator position;
//! `condition_materialization` reads at a mid-block instruction position —
//! both are `read_position` arguments to the same walk.
use std::collections::{BTreeSet, VecDeque};

use register_model::{RegisterInstructionConstraint, RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionKind,
    SelectedSuccessor, SelectedTerminator, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

/// The rejection reasons a condition-state resolution can raise; the
/// admission maps them onto the pair family's error vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConditionFlowError {
    /// A used unit's reaching event is not the single resolved definition:
    /// a clobber or a different instruction's event, several reaching
    /// events, the entry unknown, an eventless path, or a missing operand
    /// position on the resolved producer.
    Use,
    /// A producer operand's register is not defined by a unique clean
    /// `MaterializeI64`.
    Producer,
    /// A materialized literal does not fit the width the form publishes.
    Literal,
}

/// The instruction a terminator carries — a position every traversal of its
/// block observes, and the body-end landing position a destination can
/// name.
pub(super) fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

/// Every successor edge a terminator names, in its physical record order;
/// `HostedExitProcess` and `Return` name none.
pub(super) fn terminator_successors(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    match terminator {
        SelectedTerminator::Jump { successor, .. } => vec![successor],
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => vec![when_less, when_not_less],
        SelectedTerminator::HostedExitProcess { .. } | SelectedTerminator::Return { .. } => {
            Vec::new()
        }
    }
}

/// Every position of a block: its body instructions, then the instruction
/// its terminator carries.
pub(super) fn block_instructions(
    block: &SelectedBlock,
) -> impl Iterator<Item = &SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
}

/// The sixty-four-bit pattern a `MaterializeI64` publishes. The literal
/// must fit the register it is written into: a signed value is its i64
/// pattern, an unsigned value its u64 pattern, and anything wider is a
/// malformed materialization rather than a foldable literal.
fn literal_bits(value: IntegerValue) -> Option<u64> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value).ok().map(|value| value as u64),
        IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
    }
}

/// The bit pattern a `CompareI64Immediate` encodes: the target forms carry
/// an unsigned payload, so any other value is malformed here.
fn immediate_bits(value: IntegerValue) -> Option<u64> {
    match value {
        IntegerValue::Signed(value) => u64::try_from(value).ok(),
        IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
    }
}

/// The unique-producer guarantee the operand resolution uses, lifted to a
/// bit pattern: exactly one instruction in the function may define
/// `register` — a `UseDef` rewrite or a terminator-carried definition
/// counts as a second one — and it must be a `MaterializeI64` whose own
/// shape is the emitted `[def]` record with no unit traffic. The literal
/// guarantee then holds wherever the producer could read the register, not
/// only at one site.
pub(super) fn materialized_bits(
    function: &SelectedFunction,
    register: VirtualRegisterId,
) -> Result<u64, ConditionFlowError> {
    let mut producers = function.blocks.iter().flat_map(|block| {
        block_instructions(block).filter(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
            })
        })
    });
    let producer = producers.next().ok_or(ConditionFlowError::Producer)?;
    if producers.next().is_some() {
        return Err(ConditionFlowError::Producer);
    }
    let SelectedInstructionKind::MaterializeI64 { value } = producer.kind else {
        return Err(ConditionFlowError::Producer);
    };
    if producer.operands.len() != 1
        || producer.operands[0].operand != 0
        || producer.operands[0].access != RegisterOperandAccess::Def
        || producer.operands[0].virtual_register != register
        || producer.operands[0].fixed_view.is_some()
        || producer.operands[0].tied_to.is_some()
        || producer.operands[0].early_clobber
        || !producer.implicit_uses.is_empty()
        || !producer.implicit_defs.is_empty()
        || !producer.clobbers.is_empty()
    {
        return Err(ConditionFlowError::Producer);
    }
    literal_bits(value).ok_or(ConditionFlowError::Literal)
}

/// A plain `Use` operand at `position`, carrying no fixed view, tie, or
/// early clobber: the register the flag computation reads must be the
/// operand's own, not a restricted view of it.
fn plain_use(
    instruction: &SelectedInstruction,
    position: usize,
) -> Result<VirtualRegisterId, ConditionFlowError> {
    let operand = instruction
        .operands
        .get(position)
        .ok_or(ConditionFlowError::Use)?;
    if operand.operand != position as u16
        || operand.access != RegisterOperandAccess::Use
        || operand.fixed_view.is_some()
        || operand.tied_to.is_some()
        || operand.early_clobber
    {
        return Err(ConditionFlowError::Use);
    }
    Ok(operand.virtual_register)
}

/// How a producer record resolves the bits a consumer predicate decides
/// on — `left - right` in the compare's own direction.
///
/// Each variant binds one producer kind's operand grammar: a descriptor
/// names the grammar rather than leaving admission to infer it from operand
/// counts, and the replays restate the same grammar from the producer's
/// kind alone. The grammars are producer-side facts shared by every
/// condition-state pair family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConditionOperandResolution {
    /// `CompareI64`: two plain `Use` operands. Each register resolves to the
    /// bits its unique in-function `MaterializeI64` producer publishes — a
    /// `UseDef` or a terminator-carried write makes it a second producer and
    /// refuses — or both operands name the same register, where `x - x`
    /// fixes the state at `(0, 0)` whatever produced `x`.
    TwoRegisterOperands,
    /// `CompareI64Immediate`: operand 0 resolves as under
    /// [`TwoRegisterOperands`](Self::TwoRegisterOperands); the right operand
    /// is the literal the kind's `immediate` field carries.
    RegisterAndKindImmediate,
    /// `CompareI64Zero`: operand 0 resolves as under
    /// [`TwoRegisterOperands`](Self::TwoRegisterOperands); the right operand
    /// is the kind's zero bound.
    RegisterAndZeroBound,
}

/// The `(left, right)` bit patterns the producer's published flag state
/// describes — `left - right` in the compare's own direction — under the
/// operand grammar `resolution` declares for the producer kind.
pub(super) fn resolved_operands(
    function: &SelectedFunction,
    producer: &SelectedInstruction,
    resolution: ConditionOperandResolution,
) -> Result<(u64, u64), ConditionFlowError> {
    match resolution {
        ConditionOperandResolution::TwoRegisterOperands => {
            let SelectedInstructionKind::CompareI64 = producer.kind else {
                return Err(ConditionFlowError::Use);
            };
            if producer.operands.len() != 2 {
                return Err(ConditionFlowError::Use);
            }
            let left = plain_use(producer, 0)?;
            let right = plain_use(producer, 1)?;
            // `register - register` is zero on every lane: the flag state
            // is constant whatever the register holds, so no producer is
            // needed for either side.
            if left == right {
                return Ok((0, 0));
            }
            Ok((
                materialized_bits(function, left)?,
                materialized_bits(function, right)?,
            ))
        }
        ConditionOperandResolution::RegisterAndKindImmediate => {
            let SelectedInstructionKind::CompareI64Immediate { immediate } = producer.kind else {
                return Err(ConditionFlowError::Use);
            };
            if producer.operands.len() != 1 {
                return Err(ConditionFlowError::Use);
            }
            let left = plain_use(producer, 0)?;
            let immediate = immediate_bits(immediate).ok_or(ConditionFlowError::Literal)?;
            Ok((materialized_bits(function, left)?, immediate))
        }
        ConditionOperandResolution::RegisterAndZeroBound => {
            let SelectedInstructionKind::CompareI64Zero = producer.kind else {
                return Err(ConditionFlowError::Use);
            };
            if producer.operands.len() != 1 {
                return Err(ConditionFlowError::Use);
            }
            let left = plain_use(producer, 0)?;
            Ok((materialized_bits(function, left)?, 0))
        }
    }
}

/// The unit-surface contract every condition-resolved rewrite republishes,
/// shared by the pair families: the flag uses retire — the compile-time
/// decision replaces the observation — while every non-flag unit the
/// consumer record used must stay read on the rewritten row, the rewritten
/// row republishes the consumer's implicit definitions and clobbers
/// verbatim, and it may read no unit the consumer did not read. The
/// terminator family's `Jump` row carries the control unit forward under
/// this contract; the materialization family's `MaterializeI64` row reads
/// nothing, so its consumer may carry flag uses only.
pub(super) fn resolved_unit_surface(
    flag_uses: &[RegisterUnitId],
    plain_uses: &[RegisterUnitId],
    consumer: &SelectedInstruction,
    rewritten: &RegisterInstructionConstraint,
) -> bool {
    !flag_uses.is_empty()
        && plain_uses
            .iter()
            .all(|unit| rewritten.implicit_uses.contains(unit))
        && rewritten.implicit_defs == consumer.implicit_defs
        && rewritten.clobbers == consumer.clobbers
        && rewritten
            .implicit_uses
            .iter()
            .all(|unit| consumer.implicit_uses.contains(unit))
}

/// A condition-state event located by block and instruction-stream
/// position — `position == block.instructions.len()` names the
/// terminator's carried instruction — so identity never relies on
/// instruction-id uniqueness.
pub(super) type EventSite = (usize, usize);

/// What one path's last observed condition-state event for a flag unit can
/// be while the unit still reaches the read position.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ReachingEvent {
    /// The path carried no recorded event for the unit: only the entry
    /// block seeds this, because condition state at function entry is not
    /// the producer's.
    Unknown,
    /// The path's last event is the instruction at the site.
    At(EventSite),
}

/// The instruction at `site` — a body instruction, or the terminator's
/// carried instruction when the position is the stream's last.
pub(super) fn instruction_at(function: &SelectedFunction, site: EventSite) -> &SelectedInstruction {
    let block = &function.blocks[site.0];
    if site.1 == block.instructions.len() {
        terminator_instruction(&block.terminator)
    } else {
        &block.instructions[site.1]
    }
}

/// The block-indexed predecessor and successor adjacency of `function`.
/// An edge naming a block the function does not contain participates in
/// neither: it cannot carry a path into any block the walk visits.
pub(super) fn adjacency(function: &SelectedFunction) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let block_index = |id: SelectedBlockId| function.blocks.iter().position(|block| block.id == id);
    let successors: Vec<Vec<usize>> = function
        .blocks
        .iter()
        .map(|block| {
            terminator_successors(&block.terminator)
                .iter()
                .filter_map(|successor| block_index(successor.block))
                .collect()
        })
        .collect();
    let mut predecessors = vec![Vec::new(); function.blocks.len()];
    for (source, targets) in successors.iter().enumerate() {
        for &target in targets {
            predecessors[target].push(source);
        }
    }
    (successors, predecessors)
}

/// The index of `function`'s entry block, whose unseeded condition state
/// bounds every path the walk considers.
pub(super) fn entry_index(function: &SelectedFunction) -> Option<usize> {
    function
        .blocks
        .iter()
        .position(|block| block.id == function.entry_block)
}

/// The blocks that can reach `target` through `predecessors` — the cone
/// whose entry sets alone can feed the read position's reaching events.
pub(super) fn backward_cone(predecessors: &[Vec<usize>], target: usize) -> Vec<bool> {
    let mut cone = vec![false; predecessors.len()];
    cone[target] = true;
    let mut frontier = vec![target];
    while let Some(current) = frontier.pop() {
        for &predecessor in &predecessors[current] {
            if !cone[predecessor] {
                cone[predecessor] = true;
                frontier.push(predecessor);
            }
        }
    }
    cone
}

/// Resolve one implicit flag use to the condition-state event every path
/// reaching `read_position` in `block_index` last observed, as a `(block,
/// stream position)` site. `read_position == block.instructions.len()`
/// names the terminator's read.
///
/// The in-block rule stands when an event precedes the read position:
/// linear body order makes it the last event on every path through the
/// position. With no in-block event the resolution crosses block
/// boundaries. A block's own last event — body or terminator-carried — is
/// what its exit edges carry, while a block holding no event for the unit
/// passes its entry set through; the entry block's set contains `Unknown`
/// because the condition state there is not the producer's; and every other
/// predecessorless block contributes nothing because no path reaches it.
/// The set equations are union-monotone, so the walk iterates the least
/// fixpoint over the backward-reachable cone and resolves only when the
/// read block's entry set is the single producer event — a clobber, a
/// different instruction's definition, `Unknown` among the reaching
/// events, or an empty set all refuse.
pub(super) fn reaching_event(
    function: &SelectedFunction,
    entry: usize,
    successors: &[Vec<usize>],
    cone: &[bool],
    block_index: usize,
    read_position: usize,
    unit: RegisterUnitId,
) -> Result<EventSite, ConditionFlowError> {
    let block = &function.blocks[block_index];
    if let Some(position) = block.instructions[..read_position]
        .iter()
        .rposition(|instruction| {
            instruction.implicit_defs.contains(&unit) || instruction.clobbers.contains(&unit)
        })
    {
        return Ok((block_index, position));
    }
    // The last condition-state event for `unit` in each block's full
    // stream — body then terminator — or none where the block leaves the
    // unit untouched and passes its entry state through.
    let last_event: Vec<Option<EventSite>> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            block_instructions(block)
                .enumerate()
                .filter(|(_, instruction)| {
                    instruction.implicit_defs.contains(&unit)
                        || instruction.clobbers.contains(&unit)
                })
                .map(|(position, _)| (index, position))
                .last()
        })
        .collect();
    let mut entry_events = vec![BTreeSet::new(); function.blocks.len()];
    entry_events[entry].insert(ReachingEvent::Unknown);
    // Propagate each cone block's contribution — its own last event, or its
    // entry set when it holds none — to its successors until the least
    // fixpoint. A block's set holds at most one element per event site
    // plus the unknown marker, so it requeues only while it still grows.
    let mut pending: VecDeque<usize> = (0..function.blocks.len()).filter(|&b| cone[b]).collect();
    while let Some(current) = pending.pop_front() {
        let contribution = match last_event[current] {
            Some(site) => BTreeSet::from([ReachingEvent::At(site)]),
            None => entry_events[current].clone(),
        };
        if contribution.is_empty() {
            continue;
        }
        for &successor in &successors[current] {
            if !cone[successor] {
                continue;
            }
            let before = entry_events[successor].len();
            entry_events[successor].extend(contribution.iter().copied());
            if entry_events[successor].len() != before {
                pending.push_back(successor);
            }
        }
    }
    let resolved = entry_events[block_index].clone();
    if resolved.len() != 1 {
        return Err(ConditionFlowError::Use);
    }
    match resolved.into_iter().next() {
        Some(ReachingEvent::At(site)) => Ok(site),
        _ => Err(ConditionFlowError::Use),
    }
}
