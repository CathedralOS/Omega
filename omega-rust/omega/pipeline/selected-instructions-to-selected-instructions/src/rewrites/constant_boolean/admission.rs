//! Shared admission for constant condition materialization: locate the
//! named `MaterializeBoolean*`, confirm its clean `[def result]` shape,
//! resolve every implicit flag unit it reads to the same compare
//! definition — in-block or across predecessor edges — and prove that
//! compare's operands compile-time constant so the observed predicate is
//! decidable.
use std::collections::{BTreeSet, VecDeque};

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedSuccessor, SelectedTerminator,
    VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

use super::ConstantBooleanError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub block_index: usize,
    pub materialization_index: usize,
    pub materialization_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    /// The boolean result register: the rewritten materialization defines
    /// it unchanged.
    pub result: VirtualRegisterId,
    /// The folded predicate outcome the rewritten `MaterializeI64` carries.
    pub value: bool,
    /// The materialize row the rewritten instruction is built from.
    pub row: &'source RegisterInstructionConstraint,
}

/// Every instruction of `block`, including the one its terminator carries:
/// a register definition there still counts toward the unique-producer
/// rule, and a flag event there still ends a unit's live range.
fn block_instructions(block: &SelectedBlock) -> impl Iterator<Item = &SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
}

fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
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
/// an unsigned twelve-bit payload, so any other value is malformed here.
fn immediate_bits(value: IntegerValue) -> Option<u64> {
    match value {
        IntegerValue::Signed(value) => u64::try_from(value).ok(),
        IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
    }
}

/// The unique-producer guarantee the literal folds use, lifted to a bit
/// pattern: exactly one instruction in the function may define `register`
/// — a `UseDef` rewrite or a terminator-carried definition counts as a
/// second one — and it must be a `MaterializeI64` whose own shape is the
/// emitted `[def]` record with no unit traffic. The literal guarantee then
/// holds wherever the compare could read the register, not only at one
/// site.
fn materialized_bits(
    function: &SelectedFunction,
    register: VirtualRegisterId,
) -> Result<u64, ConstantBooleanError> {
    let mut producers = function.blocks.iter().flat_map(|block| {
        block_instructions(block).filter(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
            })
        })
    });
    let producer = producers
        .next()
        .ok_or(ConstantBooleanError::UnsupportedProducer)?;
    if producers.next().is_some() {
        return Err(ConstantBooleanError::UnsupportedProducer);
    }
    let SelectedInstructionKind::MaterializeI64 { value } = producer.kind else {
        return Err(ConstantBooleanError::UnsupportedProducer);
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
        return Err(ConstantBooleanError::UnsupportedProducer);
    }
    literal_bits(value).ok_or(ConstantBooleanError::UnsupportedLiteral)
}

/// A plain `Use` operand at `position`, carrying no fixed view, tie, or
/// early clobber: the register the flag computation reads must be the
/// operand's own, not a restricted view of it.
fn plain_use(
    instruction: &SelectedInstruction,
    position: usize,
) -> Result<VirtualRegisterId, ConstantBooleanError> {
    let operand = instruction
        .operands
        .get(position)
        .ok_or(ConstantBooleanError::UnsupportedUse)?;
    if operand.operand != position as u16
        || operand.access != RegisterOperandAccess::Use
        || operand.fixed_view.is_some()
        || operand.tied_to.is_some()
        || operand.early_clobber
    {
        return Err(ConstantBooleanError::UnsupportedUse);
    }
    Ok(operand.virtual_register)
}

/// The `(left, right)` bit patterns the compare's published flag state
/// describes — `left - right` in the compare's own direction — admitted
/// only when both are compile-time constant.
fn constant_operands(
    function: &SelectedFunction,
    compare: &SelectedInstruction,
) -> Result<(u64, u64), ConstantBooleanError> {
    match compare.kind {
        SelectedInstructionKind::CompareI64 => {
            if compare.operands.len() != 2 {
                return Err(ConstantBooleanError::UnsupportedUse);
            }
            let left = plain_use(compare, 0)?;
            let right = plain_use(compare, 1)?;
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
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            if compare.operands.len() != 1 {
                return Err(ConstantBooleanError::UnsupportedUse);
            }
            let left = plain_use(compare, 0)?;
            let immediate =
                immediate_bits(immediate).ok_or(ConstantBooleanError::UnsupportedLiteral)?;
            Ok((materialized_bits(function, left)?, immediate))
        }
        SelectedInstructionKind::CompareI64Zero => {
            if compare.operands.len() != 1 {
                return Err(ConstantBooleanError::UnsupportedUse);
            }
            let left = plain_use(compare, 0)?;
            Ok((materialized_bits(function, left)?, 0))
        }
        _ => Err(ConstantBooleanError::UnsupportedUse),
    }
}

/// The constant a flag-reading `MaterializeBoolean*` materializes when the
/// compare observed `(left, right)` — the predicates the selected catalog
/// defines over `left - right`, evaluated at compile time.
fn predicate_outcome(kind: SelectedInstructionKind, left: u64, right: u64) -> Option<bool> {
    match kind {
        SelectedInstructionKind::MaterializeBooleanEqual => Some(left == right),
        SelectedInstructionKind::MaterializeBooleanU64LessThan => Some(left < right),
        SelectedInstructionKind::MaterializeBooleanI64LessThan => {
            Some((left as i64) < (right as i64))
        }
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => Some(left <= right),
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            Some((left as i64) <= (right as i64))
        }
        _ => None,
    }
}

/// A condition-state event located by block and instruction-stream
/// position — `position == block.instructions.len()` names the
/// terminator's carried instruction — so identity never relies on
/// instruction-id uniqueness.
type EventSite = (usize, usize);

/// What one path's last observed condition-state event for a flag unit can
/// be while the unit still reaches the materialization.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ReachingEvent {
    /// The path carried no recorded event for the unit: only the entry
    /// block seeds this, because condition state at function entry is not
    /// the compare's.
    Unknown,
    /// The path's last event is the instruction at the site.
    At(EventSite),
}

/// The instruction at `site` — a body instruction, or the terminator's
/// carried instruction when the position is the stream's last.
fn instruction_at(function: &SelectedFunction, site: EventSite) -> &SelectedInstruction {
    let block = &function.blocks[site.0];
    if site.1 == block.instructions.len() {
        terminator_instruction(&block.terminator)
    } else {
        &block.instructions[site.1]
    }
}

/// Every successor edge of `terminator`, whatever its role: condition
/// state is physical, so a flag unit flows across semantic, edge-transfer,
/// and case-dispatch continuations alike. Crossing an edge is transparent
/// to it — a successor record's `bindings` move registers, its
/// `structural_bindings` move storage slots, its `structural_case`
/// payloads move registers or storage, and its `fuel` carries charge
/// counts — no successor field can name a `RegisterUnitId`, so no edge
/// transport or roster can add, drop, or alter a flag event.
fn successor_edges(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    match terminator {
        SelectedTerminator::Jump { successor, .. } => vec![successor],
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        }
        | SelectedTerminator::ConditionalBranchU64LessThan {
            when_less: when_nonzero,
            when_not_less: when_zero,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less: when_nonzero,
            when_not_less: when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        SelectedTerminator::HostedExitProcess { .. } | SelectedTerminator::Return { .. } => {
            Vec::new()
        }
    }
}

/// The block-indexed predecessor and successor adjacency of `function`.
/// An edge naming a block the function does not contain participates in
/// neither: it cannot carry a path into any block the walk visits.
fn adjacency(function: &SelectedFunction) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let block_index = |id: SelectedBlockId| function.blocks.iter().position(|block| block.id == id);
    let successors: Vec<Vec<usize>> = function
        .blocks
        .iter()
        .map(|block| {
            successor_edges(&block.terminator)
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

/// Resolve one implicit flag use to the condition-state event every path
/// reaching the materialization last observed, as a `(block, stream
/// position)` site.
///
/// The in-block rule stands when an event precedes the materialization:
/// linear body order makes it the last event on every path through the
/// position. With no in-block event the resolution crosses block
/// boundaries. A block's own last event — body or terminator-carried — is
/// what its exit edges carry, while a block holding no event for the unit
/// passes its entry set through; the entry block's set contains `Unknown`
/// because the condition state there is not the compare's; and every other
/// predecessorless block contributes nothing because no path reaches it.
/// The set equations are union-monotone, so the walk iterates the least
/// fixpoint over the backward-reachable cone and resolves only when the
/// materialization block's entry set is the single compare event — a
/// clobber, a different instruction's definition, `Unknown` among the
/// reaching events, or an empty set all refuse.
fn reaching_event(
    function: &SelectedFunction,
    entry: usize,
    successors: &[Vec<usize>],
    cone: &[bool],
    block_index: usize,
    materialization_index: usize,
    unit: RegisterUnitId,
) -> Result<EventSite, ConstantBooleanError> {
    let block = &function.blocks[block_index];
    if let Some(position) = block.instructions[..materialization_index]
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
    if entry_events[block_index].len() == 1
        && let Some(&ReachingEvent::At(site)) = entry_events[block_index].iter().next()
    {
        return Ok(site);
    }
    Err(ConstantBooleanError::UnsupportedUse)
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, ConstantBooleanError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConstantBooleanError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ConstantBooleanError::SourceMismatch)?;
    let (block_index, materialization_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == materialization)
                .map(|materialization_index| (block_index, materialization_index))
        })
        .ok_or(ConstantBooleanError::SourceMismatch)?;
    let materialization_instruction =
        &function.blocks[block_index].instructions[materialization_index];
    // The materialization must be the emitted `[def result]` flag reader:
    // one plain definition, no implicit definitions or clobbers, and a
    // nonempty flag-use roster — a boolean observing no condition state has
    // no constant to evaluate.
    if predicate_outcome(materialization_instruction.kind, 0, 0).is_none()
        || materialization_instruction.operands.len() != 1
        || materialization_instruction.implicit_uses.is_empty()
        || !materialization_instruction.implicit_defs.is_empty()
        || !materialization_instruction.clobbers.is_empty()
    {
        return Err(ConstantBooleanError::UnsupportedInstruction);
    }
    let result_operand = &materialization_instruction.operands[0];
    if result_operand.operand != 0
        || result_operand.access != RegisterOperandAccess::Def
        || result_operand.fixed_view.is_some()
        || result_operand.tied_to.is_some()
        || result_operand.early_clobber
    {
        return Err(ConstantBooleanError::UnsupportedInstruction);
    }
    let result = result_operand.virtual_register;
    let result_register = function
        .virtual_registers
        .iter()
        .find(|entry| entry.id == result)
        .ok_or(ConstantBooleanError::UnsupportedUse)?;
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition state
    // bounds every path, and mark the cone of blocks that can reach the
    // materialization's block — only their entry sets can feed the result.
    let (successors, predecessors) = adjacency(function);
    let entry = function
        .blocks
        .iter()
        .position(|block| block.id == function.entry_block)
        .ok_or(ConstantBooleanError::SourceMismatch)?;
    let mut cone = vec![false; function.blocks.len()];
    cone[block_index] = true;
    let mut frontier = vec![block_index];
    while let Some(current) = frontier.pop() {
        for &predecessor in &predecessors[current] {
            if !cone[predecessor] {
                cone[predecessor] = true;
                frontier.push(predecessor);
            }
        }
    }
    // Every flag unit the materialization reads must reach from the same
    // compare: on every execution path to the materialization, the last
    // event touching each used unit is that one instruction, and the unit
    // is among its published definitions — a clobber there would leave the
    // observed value unknown. Other readers of the same units are
    // unaffected: the compare stays.
    let mut compare_site = None;
    for unit in &materialization_instruction.implicit_uses {
        let event = reaching_event(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            materialization_index,
            *unit,
        )?;
        if compare_site.is_some_and(|site| site != event) {
            return Err(ConstantBooleanError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    let compare_site = compare_site.ok_or(ConstantBooleanError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if materialization_instruction
        .implicit_uses
        .iter()
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(ConstantBooleanError::UnsupportedUse);
    }
    let (left, right) = constant_operands(function, compare_instruction)?;
    let value = predicate_outcome(materialization_instruction.kind, left, right)
        .ok_or(ConstantBooleanError::UnsupportedInstruction)?;
    // The rewritten instruction is the target's own materialize row: a
    // single `[def]` at the boolean result's class carrying no unit
    // traffic — the fold drops the flag uses, and any implicit definition
    // or clobber the row added would publish unit state the source
    // instruction never had.
    let row = environment
        .constraint(environment.selected_keys().materialize_i64)
        .ok_or(ConstantBooleanError::ConstraintMismatch)?;
    if row.operands.len() != 1
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Def
        || row.operands[0].class != result_register.class
        || row.operands[0].fixed_view.is_some()
        || row.operands[0].tied_to.is_some()
        || row.operands[0].early_clobber
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(ConstantBooleanError::ConstraintMismatch);
    }
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    // Producer scans walk the whole function once per compared register —
    // two at most. The flag walk's shared setup resolves every edge's
    // target index, fills the predecessor lists, and marks the backward
    // cone. Each used unit then scans the materialization block's prefix
    // and — on an in-block miss — every block's stream for its last event,
    // after which entry-set propagation requeues a block only while its
    // set grows: a set holds at most one element per event site plus the
    // unknown marker, so pops stay under `blocks × (elements + 1)` and
    // each pop visits its out-edges — no more than the widest terminator's
    // — at a bounded union cost.
    let elements = function_scan
        .checked_add(1)
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(ConstantBooleanError::IdentityOverflow)?,
        )
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(ConstantBooleanError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let reach_scan = materialization_instruction
        .implicit_uses
        .len()
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            total
                .checked_add(function_scan.checked_mul(2)?)?
                .checked_add(reach_scan)
        })
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ConstantBooleanError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ConstantBooleanError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        materialization_index,
        materialization_id: materialization,
        provenance: materialization_instruction.provenance.clone(),
        result,
        value,
        row,
    })
}

/// The one-instruction proposal shape shared with replay: the target's own
/// materialize row supplies the operand interface while the boolean's
/// result register, instruction identity, and provenance stay with the
/// result. The folded constant is the predicate outcome the retained
/// compare publishes.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: admitted.materialization_id,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(u8::from(admitted.value))),
        },
        constraint: admitted.row.key,
        operands: admitted
            .row
            .operands
            .iter()
            .zip([admitted.result])
            .map(
                |(operand, register)| selected_instructions::SelectedOperand {
                    operand: operand.operand,
                    virtual_register: register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                },
            )
            .collect(),
        implicit_uses: admitted.row.implicit_uses.clone(),
        implicit_defs: admitted.row.implicit_defs.clone(),
        clobbers: admitted.row.clobbers.clone(),
        provenance: admitted.provenance.clone(),
    }
}
