//! Independent validation of boundary-decided predicate materialization.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! fold's legality from the source records — the named instruction's
//! ordering `MaterializeBoolean*` kind and emitted `[def result]` shape,
//! the flag units it reads resolving through the shared condition-state
//! walk to one compare's published definitions, the compare's resolved
//! operand poles, and — for a decided predicate — the target's own
//! materialize row the rewritten instruction is built from — then
//! rebuilds the instruction the contract demands and requires the
//! proposal to carry it at the materialization's position. Reinserting
//! the source boolean must restore the complete source by content: every
//! other instruction, register, call, and function included. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.
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

use super::{BoundaryBooleanError, BoundaryBooleanReceipt, ValidatedBoundaryBoolean};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::unexecuted::condition_state::{
    ConditionStateError, adjacency, backward_cone, boundary_operands, entry_index, instruction_at,
    reaching_event,
};

/// What the pole operand forces on the reader's predicate over
/// `left - right`, in the validator's own record: the same two outcomes
/// the contract defines, re-decided here so a wrong boundary decision
/// inside the producer's `outcome` cannot pass validation.
#[derive(Clone, Copy)]
enum Outcome {
    /// The predicate is decided for every surviving operand value: the
    /// reader becomes the target's own `MaterializeI64` of the constant.
    Constant(bool),
    /// The predicate holds exactly at the pole, coinciding with the
    /// equality condition on the same published state: the reader keeps
    /// its shape and only its kind becomes `MaterializeBooleanEqual`.
    Equal,
}

/// The validator's own reconstruction of the fold the contract permits:
/// the admitted materialization's coordinates and emitted provenance, the
/// boolean result register and decided outcome, the target's materialize
/// row a constant fold is built from, the resolved-index successor lists
/// the shared walk ran over, and the flag-use count for the measured-step
/// contract. It shares no state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    materialization_index: usize,
    materialization_id: SelectedInstructionId,
    provenance: SelectedInstructionProvenance,
    result: VirtualRegisterId,
    outcome: Outcome,
    /// The materialize row the constant-folded instruction is built from —
    /// present only for `Outcome::Constant`; the equality collapse reuses
    /// the reader's own row and surface verbatim.
    row: Option<&'source RegisterInstructionConstraint>,
    successors: Vec<Vec<usize>>,
    implicit_uses: usize,
}

/// The four ordering boolean kinds a pole can decide. `MaterializeBooleanEqual`
/// is absent: `left == right` varies with every unknown side, so no single
/// operand pole can fix it. The validator re-decides the emitted kinds
/// from the instruction kind alone — a wrong admission inside shared
/// producer code would decide both sides identically.
fn decidable(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::MaterializeBooleanU64LessThan
            | SelectedInstructionKind::MaterializeBooleanI64LessThan
            | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
            | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
    )
}

/// The boundary decision over the compare's resolved operand poles. The
/// strict orderings fold only at the far pole — a strict less-than against
/// the domain minimum, or from the domain maximum, can never hold — and the
/// remaining pole cases collapse to a negation (`x != bound`) that no
/// boolean materialization kind expresses. The inclusive orderings are
/// tautologies at the near pole and equalities at the far one. Constant
/// outcomes take precedence where two rules could fire: `(0, 0)` under
/// `U64LessOrEqual` is decided `true` rather than rewritten to the equality
/// reader that would compute the same value. This table is the fold's
/// legality decision: it lives here, not in `admission`, because a wrong
/// boundary rule in shared producer code would certify the proposal its
/// own bug emitted.
fn pole_outcome(
    kind: SelectedInstructionKind,
    left: Option<u64>,
    right: Option<u64>,
) -> Option<Outcome> {
    match kind {
        SelectedInstructionKind::MaterializeBooleanU64LessThan => {
            // `x <u 0` never holds; `u64::MAX <u x` never holds. The
            // remaining poles are `0 <u x` ≡ `x != 0` and `x <u MAX` ≡
            // `x != MAX`, negations no materialization kind selects.
            if right == Some(0) || left == Some(u64::MAX) {
                Some(Outcome::Constant(false))
            } else {
                None
            }
        }
        SelectedInstructionKind::MaterializeBooleanI64LessThan => {
            // `x <s i64::MIN` never holds; `i64::MAX <s x` never holds.
            if right == Some(i64::MIN as u64) || left == Some(i64::MAX as u64) {
                Some(Outcome::Constant(false))
            } else {
                None
            }
        }
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => {
            // `0 <=u x` and `x <=u u64::MAX` always hold; `x <=u 0` and
            // `u64::MAX <=u x` each hold exactly at equality.
            if left == Some(0) || right == Some(u64::MAX) {
                Some(Outcome::Constant(true))
            } else if right == Some(0) || left == Some(u64::MAX) {
                Some(Outcome::Equal)
            } else {
                None
            }
        }
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            // `i64::MIN <=s x` and `x <=s i64::MAX` always hold;
            // `x <=s i64::MIN` and `i64::MAX <=s x` hold exactly at
            // equality.
            if left == Some(i64::MIN as u64) || right == Some(i64::MAX as u64) {
                Some(Outcome::Constant(true))
            } else if right == Some(i64::MIN as u64) || left == Some(i64::MAX as u64) {
                Some(Outcome::Equal)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The boolean result register of an emitted `[def result]` flag reader,
/// or none: the validator re-decides the emitted shape — one of the four
/// ordering kinds a pole can decide, one plain definition, no implicit
/// definitions or clobbers, and a nonempty flag-use roster, since a
/// boolean observing no condition state has no predicate to decide. The
/// equality kind is refused with the shape: no pole can decide
/// `left == right`.
fn flag_reader(instruction: &SelectedInstruction) -> Option<VirtualRegisterId> {
    if !decidable(instruction.kind)
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

/// The family's own mapping of a shared boundary-operand rejection onto
/// its error vocabulary. The producer's mapping lives inline in
/// `admission::admit`; the validator maps each site itself so a changed
/// producer mapping cannot relabel its refusal.
fn condition_error(error: ConditionStateError) -> BoundaryBooleanError {
    match error {
        ConditionStateError::Use => BoundaryBooleanError::UnsupportedUse,
        ConditionStateError::Literal | ConditionStateError::Producer => {
            BoundaryBooleanError::UnsupportedLiteral
        }
    }
}

/// Reconstruct the legality of folding `materialization` from first
/// principles: locate the instruction, verify the emitted flag-reader
/// shape and the result register, resolve every used unit's reaching
/// event on the module's shared condition-state walk and require the one
/// compare site whose published definitions cover them all, prove the
/// compare's operand audit yields a pole that decides the reader's
/// predicate, and check the target's materialize row against the contract
/// a constant fold assumes. Nothing in this audit reads the producer's
/// admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, BoundaryBooleanError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(BoundaryBooleanError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(BoundaryBooleanError::SourceMismatch)?;
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
        .ok_or(BoundaryBooleanError::SourceMismatch)?;
    let materialization_instruction =
        &function.blocks[block_index].instructions[materialization_index];
    let result = flag_reader(materialization_instruction)
        .ok_or(BoundaryBooleanError::UnsupportedInstruction)?;
    let result_register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == result)
        .ok_or(BoundaryBooleanError::UnsupportedUse)?;
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition
    // state bounds every path, and mark the cone of blocks that can reach
    // the materialization's block — only their entry sets can feed the
    // result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(BoundaryBooleanError::SourceMismatch)?;
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
        .map_err(|_| BoundaryBooleanError::UnsupportedUse)?;
        if compare_site.is_some_and(|site| site != event) {
            return Err(BoundaryBooleanError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    let compare_site = compare_site.ok_or(BoundaryBooleanError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if materialization_instruction
        .implicit_uses
        .iter()
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(BoundaryBooleanError::UnsupportedUse);
    }
    let (left, right) =
        boundary_operands(function, compare_instruction).map_err(condition_error)?;
    let outcome = pole_outcome(materialization_instruction.kind, left, right)
        .ok_or(BoundaryBooleanError::UnsupportedLiteral)?;
    // A decided predicate rewrites to the target's own materialize row: a
    // single `[def]` at the boolean result's class carrying no unit
    // traffic — the fold drops the flag uses, and any implicit definition
    // or clobber the row added would publish unit state the source
    // instruction never had. The equality collapse needs no row at all: the
    // reader keeps its own — every flag-reading boolean kind shares it.
    let row = match outcome {
        Outcome::Constant(_) => {
            let row = environment
                .constraint(environment.selected_keys().materialize_i64)
                .ok_or(BoundaryBooleanError::ConstraintMismatch)?;
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
                return Err(BoundaryBooleanError::ConstraintMismatch);
            }
            Some(row)
        }
        Outcome::Equal => None,
    };
    Ok(Reconstructed {
        function,
        block_index,
        materialization_index,
        materialization_id: materialization,
        provenance: materialization_instruction.provenance.clone(),
        result,
        outcome,
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
) -> Result<u64, BoundaryBooleanError> {
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
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
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(BoundaryBooleanError::IdentityOverflow)?,
        )
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(BoundaryBooleanError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    let reach_scan = implicit_uses
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
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
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| BoundaryBooleanError::IdentityOverflow)
}

/// Build the instruction the contract demands from the validator's own
/// record: a decided predicate becomes the target's own materialize row
/// carrying the outcome, while the boolean's result register, instruction
/// identity, and provenance stay with the result; an equality collapse
/// keeps the source instruction whole — the flag-reading kinds share one
/// row — and swaps only its kind field for `MaterializeBooleanEqual`. The
/// producer's `rewritten` constructor is not consulted; both sides derive
/// the same instruction from the source alone.
fn expected(reconstructed: &Reconstructed<'_>) -> SelectedInstruction {
    match reconstructed.outcome {
        Outcome::Constant(value) => {
            let row = reconstructed.row.expect("constant outcome carries its row");
            SelectedInstruction {
                id: reconstructed.materialization_id,
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(u128::from(u8::from(value))),
                },
                constraint: row.key,
                operands: row
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
                implicit_uses: row.implicit_uses.clone(),
                implicit_defs: row.implicit_defs.clone(),
                clobbers: row.clobbers.clone(),
                provenance: reconstructed.provenance.clone(),
            }
        }
        Outcome::Equal => {
            let source = &reconstructed.function.blocks[reconstructed.block_index].instructions
                [reconstructed.materialization_index];
            let mut rewritten = source.clone();
            rewritten.kind = SelectedInstructionKind::MaterializeBooleanEqual;
            rewritten
        }
    }
}

/// Reinsert the source boolean: undoing the validator's expected edit
/// must restore the complete source by content — every other
/// instruction, register, call, and settlement included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
    function_index: usize,
) -> Result<SelectedInstructionPlan, BoundaryBooleanError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(function_index)
        .ok_or(BoundaryBooleanError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(BoundaryBooleanError::ReplayMismatch)?;
    if block.instructions.len() <= reconstructed.materialization_index {
        return Err(BoundaryBooleanError::ReplayMismatch);
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
pub fn validate_boundary_boolean_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedBoundaryBoolean, BoundaryBooleanError> {
    let reconstructed = reconstruct(source, function_index, materialization, environment)?;
    if measured_steps(
        source.selected_plan(),
        reconstructed.function,
        &reconstructed.successors,
        reconstructed.implicit_uses,
    )? > budget.validation_steps()
    {
        return Err(BoundaryBooleanError::WorkBudgetExceeded);
    }
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .and_then(|block| block.instructions.get(reconstructed.materialization_index))
        != Some(&expected(&reconstructed))
    {
        return Err(BoundaryBooleanError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed, function_index)? != *source.selected_plan() {
        return Err(BoundaryBooleanError::ReplayMismatch);
    }
    Ok(ValidatedBoundaryBoolean {
        receipt: BoundaryBooleanReceipt {
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
    use register_environment::{
        ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
    };
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
        BoundaryBooleanError, BoundaryBooleanReceipt, ValidatedBoundaryBoolean,
        validate_boundary_boolean_fold,
    };

    const MATERIALIZE_A: SelectedInstructionId = SelectedInstructionId(2);
    const MATERIALIZE_B: SelectedInstructionId = SelectedInstructionId(3);
    const COMPARE: SelectedInstructionId = SelectedInstructionId(4);
    const BOOLEAN: SelectedInstructionId = SelectedInstructionId(5);
    const TERMINAL: SelectedInstructionId = SelectedInstructionId(6);

    const LITA: VirtualRegisterId = VirtualRegisterId(0);
    const LITB: VirtualRegisterId = VirtualRegisterId(1);
    const PARAM: VirtualRegisterId = VirtualRegisterId(2);
    const OUTPUT: VirtualRegisterId = VirtualRegisterId(3);

    fn budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap()
    }

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

    /// The proposal a defective producer would emit for the named reader:
    /// either the decided predicate's constant materialize, or the
    /// equality collapse that only re-kinds the flag reader.
    enum Forged {
        /// `rout = materialize value` on the target's materialize row —
        /// the constant fold a decided pole permits.
        Constant(u64),
        /// The reader kept whole with only its kind rewritten to
        /// `MaterializeBooleanEqual` — the near-pole collapse.
        Equal,
    }

    /// `ra = materialize 0; rb = materialize 0; compare PARAM, rb;
    /// `rout = boolean; return` — `PARAM` is an entry parameter with no
    /// producing instruction, so the compare's left side is unknown and
    /// the fold's legality rests on the right pole alone. With `rb` at
    /// zero the baseline is legal: `x <u 0` never holds. `edit` then
    /// breaks one precondition, and the returned proposal is what a
    /// defective producer would emit anyway: the reader rewritten though
    /// the pole no longer decides it.
    fn forged(
        target: NativeTarget,
        reader: SelectedInstructionKind,
        emitted: Forged,
        edit: impl FnOnce(&mut SelectedFunction, &ValidatedTargetRegisterEnvironment),
    ) -> (
        ValidatedBoundaryBoolean,
        ValidatedTargetRegisterEnvironment,
        SelectedInstructionPlan,
    ) {
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
                    id: PARAM,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::EntryParameter {
                        source_value: ValueId::new(4).unwrap(),
                        parameter_index: 0,
                    },
                    definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: OUTPUT,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: BOOLEAN,
                        source_value: ValueId::new(5).unwrap(),
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
                            value: IntegerValue::Unsigned(0),
                        },
                        materialize_row,
                        &[LITA],
                    ),
                    instruction(
                        MATERIALIZE_B,
                        SelectedInstructionKind::MaterializeI64 {
                            value: IntegerValue::Unsigned(0),
                        },
                        materialize_row,
                        &[LITB],
                    ),
                    instruction(
                        COMPARE,
                        SelectedInstructionKind::CompareI64,
                        compare_row,
                        &[PARAM, LITB],
                    ),
                    instruction(BOOLEAN, reader, boolean_row, &[OUTPUT]),
                ],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        TERMINAL,
                        SelectedInstructionKind::ReturnUnit,
                        return_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(1).unwrap(),
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
        let mut source = ValidatedBoundaryBoolean {
            receipt: BoundaryBooleanReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: plan.fuel_schedule,
            },
            transformed: Arc::new(plan),
        };
        edit(
            &mut Arc::make_mut(&mut source.transformed).functions[0],
            &environment,
        );
        let identity = selected_instruction_plan_identity(&source.transformed);
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        // The proposal a defective producer would emit: the reader
        // rewritten though its legality preconditions no longer hold. The
        // constant form carries the source instruction's provenance and
        // result register on the materialize row, exactly as the
        // producer's `rewritten` builds it.
        let mut proposed = source.transformed().clone();
        proposed.functions[0].blocks[0].instructions[3] = match emitted {
            Forged::Constant(value) => SelectedInstruction {
                provenance: source.transformed().functions[0].blocks[0].instructions[3]
                    .provenance
                    .clone(),
                ..instruction(
                    BOOLEAN,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(value.into()),
                    },
                    materialize_row,
                    &[OUTPUT],
                )
            },
            Forged::Equal => {
                let mut rewritten =
                    source.transformed().functions[0].blocks[0].instructions[3].clone();
                rewritten.kind = SelectedInstructionKind::MaterializeBooleanEqual;
                rewritten
            }
        };
        (source, environment, proposed)
    }

    /// The compare's right operand materializes 5, not a pole: `x <u 5`
    /// neither always holds nor never holds, so the predicate stays
    /// runtime-decided. A producer that folded anyway would publish a
    /// plan whose boolean is a constant the source never established; the
    /// validator's own pole audit must refuse with `UnsupportedLiteral`,
    /// not merely diff the proposal. Feeding that forged proposal is the
    /// observable proof that validation no longer relies on the
    /// producer's admission routine.
    #[test]
    fn validator_refuses_a_constant_fold_the_pole_does_not_decide() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) = forged(
            target,
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
            Forged::Constant(0),
            |function, _| {
                function.blocks[0].instructions[1].kind = SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(5),
                };
            },
        );
        assert_eq!(
            validate_boundary_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBooleanError::UnsupportedLiteral
        );
    }

    /// The same non-pole literal under an inclusive reader: `x <=u 5`
    /// is neither a tautology nor the equality coincidence `x <=u 0`
    /// carries, so the collapse is a legality error a defective producer
    /// could still emit.
    #[test]
    fn validator_refuses_an_equality_collapse_the_pole_does_not_decide() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) = forged(
            target,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
            Forged::Equal,
            |function, _| {
                function.blocks[0].instructions[1].kind = SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(5),
                };
            },
        );
        assert_eq!(
            validate_boundary_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBooleanError::UnsupportedLiteral
        );
    }

    /// A compare reading the same register twice publishes the identity
    /// `ra - ra` state — the constant family's case, carrying no operand
    /// pole at all. A producer that folded it anyway emits a constant the
    /// boundary audit never decided.
    #[test]
    fn validator_refuses_a_fold_over_the_identity_compare() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) = forged(
            target,
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
            Forged::Constant(0),
            |function, _| {
                let compare = &mut function.blocks[0].instructions[2];
                compare.operands[0].virtual_register = LITA;
                compare.operands[1].virtual_register = LITA;
            },
        );
        assert_eq!(
            validate_boundary_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBooleanError::UnsupportedLiteral
        );
    }

    /// A clobbered flag unit: the resolved event is the compare site but
    /// the unit is not among its published definitions, so the observed
    /// value is unknown and no pole can decide the reader. The
    /// validator's own publication audit must refuse the forged proposal.
    #[test]
    fn validator_refuses_a_fold_over_a_unit_the_compare_does_not_publish() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) = forged(
            target,
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
            Forged::Constant(0),
            |function, _| {
                let compare = &mut function.blocks[0].instructions[2];
                compare.clobbers = compare.implicit_defs.clone();
                compare.implicit_defs = Vec::new();
            },
        );
        assert_eq!(
            validate_boundary_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBooleanError::UnsupportedUse
        );
    }

    /// `left == right` varies with every unknown side, so no operand pole
    /// can decide it: an equality reader is not one of the four ordering
    /// kinds the fold admits. A producer that rewrote it anyway emits a
    /// constant the emitted shape never allowed.
    #[test]
    fn validator_refuses_a_fold_named_by_an_equality_reader() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) = forged(
            target,
            SelectedInstructionKind::MaterializeBooleanEqual,
            Forged::Constant(0),
            |_, _| {},
        );
        assert_eq!(
            validate_boundary_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBooleanError::UnsupportedInstruction
        );
    }
}
