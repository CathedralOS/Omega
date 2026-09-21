//! Independent validation of constant condition materialization.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! fold's legality from the source records — the named instruction's
//! `MaterializeBoolean*` kind and emitted `[def result]` shape, the flag
//! units it reads resolving through the shared condition-state walk to
//! one compare's published definitions, that compare's compile-time
//! constant operands, and the target's own materialize row the rewritten
//! instruction is built from — then rebuilds the instruction the contract
//! demands and requires the proposal to carry it at the materialization's
//! position. Reinserting the source boolean must restore the complete
//! source by content: every other instruction, register, call, and
//! function included. A producer admission error therefore fails
//! validation even when the proposal is exactly what that producer
//! emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionProvenance, SelectedOperand, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{ConstantBooleanError, ConstantBooleanReceipt, ValidatedConstantBoolean};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::condition_state::{
    ConditionStateError, adjacency, backward_cone, constant_operands, entry_index, instruction_at,
    reaching_event,
};

/// The validator's own reconstruction of the fold the contract permits:
/// the admitted materialization's coordinates and emitted provenance, the
/// boolean result register and folded predicate outcome, the target's
/// materialize row, the resolved-index successor lists the shared walk
/// ran over, and the flag-use count for the measured-step contract. It
/// shares no state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    materialization_index: usize,
    materialization_id: SelectedInstructionId,
    provenance: SelectedInstructionProvenance,
    result: VirtualRegisterId,
    value: bool,
    row: &'source RegisterInstructionConstraint,
    successors: Vec<Vec<usize>>,
    implicit_uses: usize,
}

/// The constant a flag-reading `MaterializeBoolean*` materializes when
/// the compare observed `(left, right)` — the predicates the selected
/// catalog defines over `left - right`, evaluated at compile time. The
/// validator re-decides the table from the instruction kind alone: a
/// wrong predicate inside shared producer code would decide both sides
/// identically, so the fold's decision lives here, not in `admission`.
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

/// The boolean result register of an emitted `[def result]` flag reader,
/// or none: the validator re-decides the emitted shape — one plain
/// definition, no implicit definitions or clobbers, and a nonempty
/// flag-use roster, since a boolean observing no condition state has no
/// constant to evaluate.
fn flag_reader(instruction: &SelectedInstruction) -> Option<VirtualRegisterId> {
    if predicate_outcome(instruction.kind, 0, 0).is_none()
        || instruction.operands.len() != 1
        || instruction.implicit_uses.is_empty()
        || !instruction.implicit_defs.is_empty()
        || !instruction.clobbers.is_empty()
    {
        return None;
    }
    let result = &instruction.operands[0];
    if result.operand != 0
        || result.access != RegisterOperandAccess::Def
        || result.fixed_view.is_some()
        || result.tied_to.is_some()
        || result.early_clobber
    {
        return None;
    }
    Some(result.virtual_register)
}

/// The family's own mapping of a shared condition-state rejection onto
/// its error vocabulary. The producer's `From` implementation lives in
/// `admission`; the validator maps each site itself so a changed producer
/// mapping cannot relabel its refusal.
fn condition_error(error: ConditionStateError) -> ConstantBooleanError {
    match error {
        ConditionStateError::Use => ConstantBooleanError::UnsupportedUse,
        ConditionStateError::Producer => ConstantBooleanError::UnsupportedProducer,
        ConditionStateError::Literal => ConstantBooleanError::UnsupportedLiteral,
    }
}

/// Reconstruct the legality of folding `materialization` from first
/// principles: locate the instruction, verify the emitted flag-reader
/// shape and the result register, resolve every used unit's reaching
/// event on the module's shared condition-state walk and require the one
/// compare site whose published definitions cover them all, prove the
/// compare's operands compile-time constant, and check the target's
/// materialize row against the contract the rewrite assumes. Nothing in
/// this audit reads the producer's admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, ConstantBooleanError> {
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
    let result = flag_reader(materialization_instruction)
        .ok_or(ConstantBooleanError::UnsupportedInstruction)?;
    let result_register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == result)
        .ok_or(ConstantBooleanError::UnsupportedUse)?;
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition
    // state bounds every path, and mark the cone of blocks that can reach
    // the materialization's block — only their entry sets can feed the
    // result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(ConstantBooleanError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every flag unit the materialization reads must reach from the same
    // compare: on every execution path to the materialization, the last
    // event touching each used unit is that one instruction, and the unit
    // is among its published definitions — a clobber there would leave
    // the observed value unknown. The validator resolves each unit's
    // reaching event on its own, so a producer that admitted divergent
    // sites or an uncovered unit cannot pass validation.
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
        )
        .map_err(condition_error)?;
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
    let (left, right) =
        constant_operands(function, compare_instruction).map_err(condition_error)?;
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
    Ok(Reconstructed {
        function,
        block_index,
        materialization_index,
        materialization_id: materialization,
        provenance: materialization_instruction.provenance.clone(),
        result,
        value,
        row,
        successors,
        implicit_uses: materialization_instruction.implicit_uses.len(),
    })
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: one step per block plus one per instruction
/// across the plan, two scans of the reconstructed function's blocks for
/// the producer audit, and the shared walk's setup and fixpoint bound at
/// one per used flag unit.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    implicit_uses: usize,
) -> Result<u64, ConstantBooleanError> {
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
    // The shared walk's setup resolves every edge's target index, fills
    // the predecessor lists, and marks the backward cone. Each used unit
    // scans the materialization block's prefix and — on an in-block
    // miss — every block's stream for its last event, after which
    // entry-set propagation requeues a block only while its set grows:
    // a set holds at most one element per event site plus the unknown
    // marker, so pops stay under `blocks × (elements + 1)` and each pop
    // visits its out-edges at a bounded union cost.
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
    let reach_scan = implicit_uses
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
    u64::try_from(steps).map_err(|_| ConstantBooleanError::IdentityOverflow)
}

/// Build the instruction the contract demands from the validator's own
/// record: the target's materialize row supplies the operand interface
/// while the boolean's result register, instruction identity, and
/// provenance stay with the result. The folded constant is the predicate
/// outcome the retained compare publishes. The producer's `rewritten`
/// constructor is not consulted; both sides derive the same instruction
/// from the source alone.
fn expected(reconstructed: &Reconstructed<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: reconstructed.materialization_id,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(u8::from(reconstructed.value))),
        },
        constraint: reconstructed.row.key,
        operands: reconstructed
            .row
            .operands
            .iter()
            .zip([reconstructed.result])
            .map(|(operand, register)| SelectedOperand {
                operand: operand.operand,
                virtual_register: register,
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: reconstructed.row.implicit_uses.clone(),
        implicit_defs: reconstructed.row.implicit_defs.clone(),
        clobbers: reconstructed.row.clobbers.clone(),
        provenance: reconstructed.provenance.clone(),
    }
}

/// Reinsert the source boolean: undoing the validator's expected edit
/// must restore the complete source by content — every other
/// instruction, register, call, and settlement included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
    function_index: usize,
) -> Result<SelectedInstructionPlan, ConstantBooleanError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(function_index)
        .ok_or(ConstantBooleanError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(ConstantBooleanError::ReplayMismatch)?;
    if block.instructions.len() <= reconstructed.materialization_index {
        return Err(ConstantBooleanError::ReplayMismatch);
    }
    block.instructions[reconstructed.materialization_index] = reconstructed.function.blocks
        [reconstructed.block_index]
        .instructions[reconstructed.materialization_index]
        .clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs
/// the fold's preconditions from the source, requires the proposed
/// instruction at the materialization's position to equal the instruction
/// its own record produces, and restores the complete source by content —
/// every other instruction, register, call, and settlement included. The
/// producer's `admission::admit` is never consulted, so a wrong legality
/// decision fails here even when the proposal matches the edit the
/// producer emitted.
pub fn validate_constant_boolean_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConstantBoolean, ConstantBooleanError> {
    let reconstructed = reconstruct(source, function_index, materialization, environment)?;
    if measured_steps(
        source.selected_plan(),
        reconstructed.function,
        &reconstructed.successors,
        reconstructed.implicit_uses,
    )? > budget.validation_steps()
    {
        return Err(ConstantBooleanError::WorkBudgetExceeded);
    }
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .and_then(|block| block.instructions.get(reconstructed.materialization_index))
        != Some(&expected(&reconstructed))
    {
        return Err(ConstantBooleanError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed, function_index)? != *source.selected_plan() {
        return Err(ConstantBooleanError::ReplayMismatch);
    }
    Ok(ValidatedConstantBoolean {
        receipt: ConstantBooleanReceipt {
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
    use register_environment::baseline_target_register_environment;
    use register_model::RegisterInstructionConstraint;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
        SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
        ScalarType, ValueId,
    };
    use target::NativeTarget;
    use target_operations_to_selected_instructions::selected_instruction_plan_identity;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        ConstantBooleanError, ConstantBooleanReceipt, ValidatedConstantBoolean,
        validate_constant_boolean_fold,
    };

    const MATERIALIZE_A: SelectedInstructionId = SelectedInstructionId(2);
    const MATERIALIZE_AGAIN: SelectedInstructionId = SelectedInstructionId(3);
    const MATERIALIZE_B: SelectedInstructionId = SelectedInstructionId(4);
    const COMPARE: SelectedInstructionId = SelectedInstructionId(5);
    const BOOLEAN: SelectedInstructionId = SelectedInstructionId(6);
    const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);

    const LITA: VirtualRegisterId = VirtualRegisterId(0);
    const LITB: VirtualRegisterId = VirtualRegisterId(1);
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

    /// `ra = materialize 3; ra = materialize 3 again; rb = materialize 5;
    /// compare ra, rb; rout = boolean; return` — the compare's left
    /// operand has two producers, so its value is not pinned
    /// function-wide and the fold's constant was never proven. A
    /// producer that admitted it anyway would publish a plan whose
    /// boolean is rewritten to a materialize the source never
    /// established; the validator's own operand audit must refuse with
    /// `UnsupportedProducer`, not merely diff the proposal. Feeding that
    /// forged proposal is the observable proof that validation no longer
    /// relies on the producer's admission routine.
    #[test]
    fn validator_refuses_a_proposal_its_own_audit_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let materialize_row = environment.constraint(keys.materialize_i64).unwrap();
        let compare_row = environment.constraint(keys.compare_i64).unwrap();
        let boolean_row = environment.constraint(keys.materialize_boolean).unwrap();
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
                    id: LITA,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: MATERIALIZE_A,
                        source_value: ValueId::new(2).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: LITB,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: MATERIALIZE_B,
                        source_value: ValueId::new(3).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: OUTPUT,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: BOOLEAN,
                        source_value: ValueId::new(4).unwrap(),
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
                        MATERIALIZE_A,
                        SelectedInstructionKind::MaterializeI64 {
                            value: IntegerValue::Unsigned(3),
                        },
                        materialize_row,
                        &[LITA],
                    ),
                    instruction(
                        MATERIALIZE_AGAIN,
                        SelectedInstructionKind::MaterializeI64 {
                            value: IntegerValue::Unsigned(3),
                        },
                        materialize_row,
                        &[LITA],
                    ),
                    instruction(
                        MATERIALIZE_B,
                        SelectedInstructionKind::MaterializeI64 {
                            value: IntegerValue::Unsigned(5),
                        },
                        materialize_row,
                        &[LITB],
                    ),
                    instruction(
                        COMPARE,
                        SelectedInstructionKind::CompareI64,
                        compare_row,
                        &[LITA, LITB],
                    ),
                    instruction(
                        BOOLEAN,
                        SelectedInstructionKind::MaterializeBooleanEqual,
                        boolean_row,
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
        let source = ValidatedConstantBoolean {
            receipt: ConstantBooleanReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: plan.fuel_schedule,
            },
            transformed: Arc::new(plan),
        };
        // The proposal a defective producer would emit: the boolean
        // folded to a constant materialize even though the compare's
        // operand was never proven compile-time constant — the literal
        // the fold publishes was never established.
        let mut proposed = source.transformed().clone();
        proposed.functions[0].blocks[0].instructions[4] = SelectedInstruction {
            id: BOOLEAN,
            kind: SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0),
            },
            constraint: materialize_row.key,
            operands: materialize_row
                .operands
                .iter()
                .zip([OUTPUT])
                .map(|(operand, register)| SelectedOperand {
                    operand: operand.operand,
                    virtual_register: register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                })
                .collect(),
            implicit_uses: materialize_row.implicit_uses.clone(),
            implicit_defs: materialize_row.implicit_defs.clone(),
            clobbers: materialize_row.clobbers.clone(),
            provenance: Default::default(),
        };
        let budget = OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap();
        assert_eq!(
            validate_constant_boolean_fold(&source, 0, BOOLEAN, &environment, budget, proposed)
                .unwrap_err(),
            ConstantBooleanError::UnsupportedProducer
        );
    }
}
