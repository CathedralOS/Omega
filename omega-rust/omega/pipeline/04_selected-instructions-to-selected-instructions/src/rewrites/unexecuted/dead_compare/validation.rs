//! Independent validation of dead-compare removal.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! removal's legality from the source records — the named instruction's
//! compare kind and canonical flag-publisher shape against the bound
//! constraint row, the call and memory-access rosters that must not name
//! it, and the forward walk proving every condition-state unit it publishes
//! dies unread on every path — then rebuilds the function the contract
//! demands and requires the proposal to equal it. Restoring the removed
//! compare and the source settlements must reproduce the complete source
//! by content. A producer admission error therefore fails validation even
//! when the proposal is exactly what that producer emitted.
use std::collections::BTreeSet;
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlockId, SelectedFunction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{DeadCompareError, DeadCompareReceipt, ValidatedDeadCompare};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{block_instructions, terminator_successors};

/// The validator's own reconstruction of the removal the contract permits:
/// the admitted compare's coordinates and its block's identity. It shares
/// no state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    function_index: usize,
    block_index: usize,
    block: SelectedBlockId,
    compare_index: usize,
    /// The distinct condition-state units the compare publishes, collected
    /// by the validator's own audit for the measured-step contract.
    units: usize,
}

/// Whether `kind` is one of the three flag publishers whose whole effect is
/// the condition-state surface it defines, with the operand arity the
/// emitted form carries. The validator re-decides the compare grammar from
/// the instruction kind alone.
fn compare_arity(kind: SelectedInstructionKind) -> Option<usize> {
    match kind {
        SelectedInstructionKind::CompareI64 => Some(2),
        SelectedInstructionKind::CompareI64Immediate { .. }
        | SelectedInstructionKind::CompareI64Zero => Some(1),
        _ => None,
    }
}

/// The validator's own dead-unit audit: walk forward from the instruction
/// after the compare until an implicit definition or clobber ends the
/// unit's live range on every path. Any reached implicit use refuses — the
/// compare's publication is observable there. A unit still live past the
/// terminator resumes at the head of each successor block; an edge naming
/// a block the function does not contain leaves the unit's readers
/// unprovable and refuses; a terminator with no successors ends the path
/// with the unit unobserved. `(block, start)` pairs dedupe the walk, so a
/// loop carrying the unit back into the compare's own block re-scans from
/// its head — a reader there observes the compare's earlier execution, and
/// the compare's own definition ends the unit past it.
fn audit_dead_unit(
    function: &SelectedFunction,
    block_index: usize,
    compare_index: usize,
    unit: RegisterUnitId,
) -> Result<(), DeadCompareError> {
    let mut visited = BTreeSet::new();
    let mut frontier = vec![(block_index, compare_index + 1)];
    while let Some((current, start)) = frontier.pop() {
        if !visited.insert((current, start)) {
            continue;
        }
        let block = &function.blocks[current];
        let mut killed = false;
        for instruction in block_instructions(block).skip(start) {
            if instruction.implicit_uses.contains(&unit) {
                return Err(DeadCompareError::UnsupportedUse);
            }
            if instruction.implicit_defs.contains(&unit) || instruction.clobbers.contains(&unit) {
                killed = true;
                break;
            }
        }
        if killed {
            continue;
        }
        for successor in terminator_successors(&block.terminator) {
            let target = function
                .blocks
                .iter()
                .position(|block| block.id == successor.block)
                .ok_or(DeadCompareError::UnsupportedUse)?;
            frontier.push((target, 0));
        }
    }
    Ok(())
}

/// Reconstruct the legality of removing `compare` from first principles:
/// locate the instruction, verify the canonical flag-publisher shape
/// against the bound constraint row and the roster, refuse any call or
/// memory-access row that names it, and audit every published unit's death
/// on the validator's own forward walk. Nothing in this audit reads the
/// producer's admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, DeadCompareError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(DeadCompareError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(DeadCompareError::SourceMismatch)?;
    let (block_index, compare_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == compare)
                .map(|compare_index| (block_index, compare_index))
        })
        .ok_or(DeadCompareError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let compare_instruction = &block.instructions[compare_index];
    // The removed instruction must be one of the three compare forms in
    // the emitted flag-publisher shape: every operand a plain `Use` at its
    // canonical position, no implicit uses the removal would silently
    // drop, and a nonempty published surface — a compare defining no
    // condition state at all is malformed, not dead.
    let arity =
        compare_arity(compare_instruction.kind).ok_or(DeadCompareError::UnsupportedInstruction)?;
    if compare_instruction.operands.len() != arity
        || !compare_instruction.implicit_uses.is_empty()
        || compare_instruction
            .implicit_defs
            .iter()
            .chain(compare_instruction.clobbers.iter())
            .next()
            .is_none()
        || compare_instruction
            .operands
            .iter()
            .enumerate()
            .any(|(position, operand)| {
                operand.operand != position as u16
                    || operand.access != RegisterOperandAccess::Use
                    || operand.fixed_view.is_some()
                    || operand.tied_to.is_some()
                    || operand.early_clobber
            })
    {
        return Err(DeadCompareError::UnsupportedInstruction);
    }
    // The compare's own constraint row must declare the same all-`use`
    // operand shape at the classes the roster assigns, and the same
    // implicit surface the instruction carries: a row that disagrees would
    // change what removing the instruction means.
    let row = environment
        .constraint(compare_instruction.constraint)
        .ok_or(DeadCompareError::ConstraintMismatch)?;
    if row.operands.len() != arity
        || row.implicit_uses != compare_instruction.implicit_uses
        || row.implicit_defs != compare_instruction.implicit_defs
        || row.clobbers != compare_instruction.clobbers
    {
        return Err(DeadCompareError::ConstraintMismatch);
    }
    for (position, operand) in compare_instruction.operands.iter().enumerate() {
        let register = function
            .virtual_registers
            .iter()
            .find(|register| register.id == operand.virtual_register)
            .ok_or(DeadCompareError::ConstraintMismatch)?;
        if row.operands[position].operand != operand.operand
            || row.operands[position].access != RegisterOperandAccess::Use
            || row.operands[position].class != register.class
        {
            return Err(DeadCompareError::ConstraintMismatch);
        }
    }
    // Removing the compare orphans anything that names it: call contracts
    // and memory-access rows bind instructions by identity, and neither
    // may claim the compare.
    if function
        .calls
        .iter()
        .any(|call| call.instruction == compare)
        || function
            .memory_accesses
            .iter()
            .any(|access| access.instruction == compare)
    {
        return Err(DeadCompareError::UnsupportedInstruction);
    }
    // Every condition-state unit the compare publishes must die unread:
    // each walks forward from the compare until a redefinition or clobber
    // kills it on every path, refusing the first reached reader. The
    // validator collects and audits the same set on its own, so a producer
    // that admitted a live publication cannot pass validation.
    let units: BTreeSet<RegisterUnitId> = compare_instruction
        .implicit_defs
        .iter()
        .chain(compare_instruction.clobbers.iter())
        .copied()
        .collect();
    for &unit in &units {
        audit_dead_unit(function, block_index, compare_index, unit)?;
    }
    Ok(Reconstructed {
        function,
        function_index,
        block_index,
        block: block.id,
        compare_index,
        units: units.len(),
    })
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: one step per block plus one per instruction
/// across the plan, a second scan of the reconstructed function's blocks
/// together with its roster, call, and access rows, then the dead-unit
/// audit at two block traversals plus the successor edges per published
/// flag unit.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    units: usize,
) -> Result<u64, DeadCompareError> {
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(DeadCompareError::IdentityOverflow)?;
    let edge_count = function
        .blocks
        .iter()
        .map(|block| terminator_successors(&block.terminator).len())
        .sum::<usize>();
    let dead_audit = units
        .checked_mul(
            function_scan
                .checked_mul(2)
                .and_then(|scan| scan.checked_add(edge_count))
                .ok_or(DeadCompareError::IdentityOverflow)?,
        )
        .ok_or(DeadCompareError::IdentityOverflow)?;
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
                .checked_add(function_scan)?
                .checked_add(function.virtual_registers.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.memory_accesses.len())
        })
        .and_then(|total| total.checked_add(dead_audit))
        .ok_or(DeadCompareError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| DeadCompareError::IdentityOverflow)
}

/// Build the function the contract demands from the validator's own
/// record: the compare leaves its block and boundary settlements shift
/// over the removed ordinal. The producer's `apply` is not consulted;
/// both sides derive the same function from the source alone.
fn expect(reconstructed: &Reconstructed<'_>) -> Result<SelectedFunction, DeadCompareError> {
    let mut expected = reconstructed.function.clone();
    let block = expected
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(DeadCompareError::SourceMismatch)?;
    if block.id != reconstructed.block {
        return Err(DeadCompareError::SourceMismatch);
    }
    // Positions at or before the removed ordinal name instructions that
    // stay put; every later position — including the after-body position —
    // shifts one ordinal earlier. Settlements name hosted boundary
    // operations, never flag units or registers, so the removal itself is
    // invisible to them.
    let body = block.instructions.len();
    expected.boundary_settlements = expected
        .boundary_settlements
        .into_iter()
        .map(|mut settlement| {
            if settlement.block == reconstructed.block {
                let position = settlement.instruction_index as usize;
                if position > body {
                    return Err(DeadCompareError::SourceMismatch);
                }
                if position > reconstructed.compare_index {
                    settlement.instruction_index = u32::try_from(position - 1)
                        .map_err(|_| DeadCompareError::IdentityOverflow)?;
                }
            }
            Ok(settlement)
        })
        .collect::<Result<Vec<_>, _>>()?;
    block.instructions.remove(reconstructed.compare_index);
    Ok(expected)
}

/// Reinsert the compare and the source settlements: undoing the
/// validator's expected edit must restore the complete source by content —
/// every other instruction, register, call, and function included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
) -> Result<SelectedInstructionPlan, DeadCompareError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(reconstructed.function_index)
        .ok_or(DeadCompareError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(DeadCompareError::ReplayMismatch)?;
    if block.id != reconstructed.block || block.instructions.len() < reconstructed.compare_index {
        return Err(DeadCompareError::ReplayMismatch);
    }
    block.instructions.insert(
        reconstructed.compare_index,
        reconstructed.function.blocks[reconstructed.block_index].instructions
            [reconstructed.compare_index]
            .clone(),
    );
    function.boundary_settlements = reconstructed.function.boundary_settlements.clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs
/// the removal's preconditions from the source, requires the proposal to
/// equal the function its own record produces, and restores the complete
/// source by content — every other instruction, register, call, and
/// settlement included. The producer's `admission::admit` is never
/// consulted, so a wrong legality decision fails here even when the
/// proposal matches the edit the producer emitted.
pub fn validate_dead_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedDeadCompare, DeadCompareError> {
    let reconstructed = reconstruct(source, function_index, compare, environment)?;
    if measured_steps(
        source.selected_plan(),
        reconstructed.function,
        reconstructed.units,
    )? > budget.validation_steps()
    {
        return Err(DeadCompareError::WorkBudgetExceeded);
    }
    if proposed.functions.get(function_index) != Some(&expect(&reconstructed)?) {
        return Err(DeadCompareError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(DeadCompareError::ReplayMismatch);
    }
    Ok(ValidatedDeadCompare {
        receipt: DeadCompareReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}

#[cfg(test)]
mod independence_tests {
    use std::sync::Arc;

    use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
    use optimization_unit::ValueDefinitionSite;
    use register_environment::baseline_target_register_environment;
    use register_model::RegisterInstructionConstraint;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
        SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ScalarType,
        ValueId,
    };
    use target::NativeTarget;
    use target_operations_to_selected_instructions::selected_instruction_plan_identity;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        DeadCompareError, DeadCompareReceipt, ValidatedDeadCompare, validate_dead_compare,
    };

    const LOAD: SelectedInstructionId = SelectedInstructionId(2);
    const COMPARE: SelectedInstructionId = SelectedInstructionId(3);
    const READER: SelectedInstructionId = SelectedInstructionId(4);
    const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);

    const POINTER: VirtualRegisterId = VirtualRegisterId(0);
    const INPUT: VirtualRegisterId = VirtualRegisterId(1);
    const OUTPUT: VirtualRegisterId = VirtualRegisterId(2);

    fn instruction(
        id: SelectedInstructionId,
        kind: SelectedInstructionKind,
        row: &RegisterInstructionConstraint,
        registers: &[VirtualRegisterId],
    ) -> SelectedInstruction {
        SelectedInstruction {
            id,
            kind,
            constraint: row.key,
            operands: row
                .operands
                .iter()
                .zip(registers)
                .map(|(operand, register)| SelectedOperand {
                    operand: operand.operand,
                    virtual_register: *register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                })
                .collect(),
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance: Default::default(),
        }
    }

    fn u64_scalar() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
    }

    /// `r1 = load8 r0; compare r1, r1; r2 = materialize_boolean_equal;
    /// return` — the materialization's implicit uses read the flag units
    /// the compare publishes, so the compare is live and its removal is a
    /// legality error. A producer that admitted it anyway would publish a
    /// plan whose compare is gone while the reader still observes the
    /// vanished publication; the validator's own audit must refuse the
    /// legality with `UnsupportedUse`, not merely diff the proposal.
    /// Feeding that forged proposal is the observable proof that
    /// validation no longer relies on the producer's admission routine.
    #[test]
    fn validator_refuses_a_proposal_its_own_audit_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let load_row = environment.constraint(keys.load8.unwrap()).unwrap();
        let compare_row = environment.constraint(keys.compare_i64).unwrap();
        let reader_row = environment.constraint(keys.materialize_boolean).unwrap();
        let return_row = environment.constraint(keys.return_unit).unwrap();
        let class = compare_row.operands[0].class;
        let machine = MachineId::new(1).unwrap();
        let function = SelectedFunction {
            machine,
            attachment: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: vec![
                VirtualRegister {
                    id: POINTER,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::EntryParameter {
                        source_value: ValueId::new(1).unwrap(),
                        parameter_index: 0,
                    },
                    definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: INPUT,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: LOAD,
                        source_value: ValueId::new(2).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: OUTPUT,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: READER,
                        source_value: ValueId::new(3).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
            ],
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![
                    instruction(
                        LOAD,
                        SelectedInstructionKind::Load8 { byte_offset: 0 },
                        load_row,
                        &[POINTER, INPUT],
                    ),
                    instruction(
                        COMPARE,
                        SelectedInstructionKind::CompareI64,
                        compare_row,
                        &[INPUT, INPUT],
                    ),
                    instruction(
                        READER,
                        SelectedInstructionKind::MaterializeBooleanEqual,
                        reader_row,
                        &[OUTPUT],
                    ),
                ],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        TERMINAL,
                        SelectedInstructionKind::ReturnUnit,
                        return_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(2).unwrap(),
                },
            }],
        };
        let plan = SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target,
            entry: machine,
            functions: vec![function].into(),
        };
        let identity = selected_instruction_plan_identity(&plan);
        let source = ValidatedDeadCompare {
            receipt: DeadCompareReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: plan.fuel_schedule,
            },
            transformed: Arc::new(plan),
        };
        // The proposal a defective producer would emit: the compare removed
        // while the materialization still reads the flags it published.
        let mut proposed = source.transformed().clone();
        proposed.functions[0].blocks[0].instructions.remove(1);
        let budget = OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap();
        assert_eq!(
            validate_dead_compare(&source, 0, COMPARE, &environment, budget, proposed).unwrap_err(),
            DeadCompareError::UnsupportedUse
        );
    }
}
