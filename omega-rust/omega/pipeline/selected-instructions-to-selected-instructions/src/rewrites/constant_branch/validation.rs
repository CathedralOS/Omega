//! Independent validation of constant condition branch folding.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! fold's legality from the source records — the terminator-carried
//! instruction's branch/kind pairing and zero-operand flag-reader shape,
//! the flag partition every implicit use must satisfy (a flag unit
//! resolving through the shared condition-state walk to the one compare
//! whose published definitions cover it, any other unit carried on the
//! jump row's own implicit surface), the jump row republishing exactly
//! the branch's implicit definitions and clobbers, and the compare's
//! compile-time constant operands — then decides the successor on its own
//! predicate table, rebuilds the `Jump` terminator the contract demands,
//! and requires the proposal to carry it in the branch's block.
//! Reinserting the source terminator must restore the complete source by
//! content: every other instruction, register, call, settlement, and
//! function included. A producer admission error therefore fails
//! validation even when the proposal is exactly what that producer
//! emitted.
use std::collections::BTreeSet;
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterUnitId};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionProvenance, SelectedSuccessor, SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{ConstantBranchError, ConstantBranchReceipt, ValidatedConstantBranch};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::terminator_instruction;
use crate::rewrites::condition_state::{
    ConditionStateError, adjacency, backward_cone, constant_operands, entry_index, instruction_at,
    reaching_event,
};

/// The validator's own reconstruction of the fold the contract permits:
/// the admitted branch's block, identity, and emitted provenance, the
/// decided successor the rebuilt `Jump` carries verbatim, the jump row
/// the terminator instruction is built from, the resolved-index
/// successor lists the shared walk ran over, and the surface sizes the
/// measured-step contract charges. It shares no state with the
/// producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    branch_id: SelectedInstructionId,
    provenance: SelectedInstructionProvenance,
    successor: SelectedSuccessor,
    row: &'source RegisterInstructionConstraint,
    successors: Vec<Vec<usize>>,
    implicit_uses: usize,
    flag_universe: usize,
    jump_surface: usize,
    compare_defs: usize,
}

/// The successor the branch selects when its compare observed
/// `(left, right)` — the predicates the selected catalog defines over
/// `left - right`, evaluated at compile time: `ConditionalBranchNonZero`
/// is the zero-condition reader selection emits for equality, so it takes
/// `when_nonzero` exactly when the difference is nonzero, and the
/// predicate-aware forms take `when_less` on their strict ordering. The
/// validator re-decides the table from the terminator and instruction
/// kind alone: a wrong predicate inside shared producer code would decide
/// both sides identically, so the fold's decision lives here, not in
/// `admission`.
fn decided(
    terminator: &SelectedTerminator,
    kind: SelectedInstructionKind,
    left: u64,
    right: u64,
) -> Option<&SelectedSuccessor> {
    match (terminator, kind) {
        (
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            },
            SelectedInstructionKind::ConditionalBranchNonZero,
        ) => Some(if left != right {
            when_nonzero
        } else {
            when_zero
        }),
        (
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            },
            SelectedInstructionKind::ConditionalBranchU64LessThan,
        ) => Some(if left < right {
            when_less
        } else {
            when_not_less
        }),
        (
            SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            },
            SelectedInstructionKind::ConditionalBranchI64LessThan,
        ) => Some(if (left as i64) < (right as i64) {
            when_less
        } else {
            when_not_less
        }),
        _ => None,
    }
}

/// The branch/kind pairings `decided` admits without consulting operands —
/// the emitted shape gate the validator applies before any flag
/// resolution runs.
fn branch_kind(terminator: &SelectedTerminator, instruction: &SelectedInstruction) -> bool {
    decided(terminator, instruction.kind, 0, 0).is_some()
}

/// The family's own mapping of a shared condition-state rejection onto
/// its error vocabulary. The producer's `From` implementation lives in
/// `admission`; the validator maps each site itself so a changed producer
/// mapping cannot relabel its refusal.
fn condition_error(error: ConditionStateError) -> ConstantBranchError {
    match error {
        ConditionStateError::Use => ConstantBranchError::UnsupportedUse,
        ConditionStateError::Producer => ConstantBranchError::UnsupportedProducer,
        ConditionStateError::Literal => ConstantBranchError::UnsupportedLiteral,
    }
}

/// Reconstruct the legality of folding `branch` from first principles:
/// locate the terminator-carried instruction, verify the emitted
/// branch/kind pairing and zero-operand flag-reader shape, build the flag
/// universe from the target's compare rows and the jump surface from its
/// jump row — requiring the jump row to republish exactly the branch's
/// implicit definitions and clobbers — partition every implicit use, run
/// each flag unit's reaching event on the module's shared condition-state
/// walk, and require the one compare site whose published definitions
/// cover them all with compile-time-constant operands. Nothing in this
/// audit reads the producer's admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, ConstantBranchError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConstantBranchError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ConstantBranchError::SourceMismatch)?;
    // The branch names its terminator-carried instruction: the block whose
    // terminator instruction carries the id holds the read position at the
    // end of its body stream.
    let block_index = function
        .blocks
        .iter()
        .position(|block| terminator_instruction(&block.terminator).id == branch)
        .ok_or(ConstantBranchError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let branch_instruction = terminator_instruction(&block.terminator);
    // The terminator must be the emitted zero-operand flag reader: the
    // variant/kind pairing must match — a `ConditionalBranch` terminator
    // carrying a less-than kind is not the shape selection emits — explicit
    // operands are never present, and a use roster holding no flag unit has
    // no condition to decide.
    if !branch_kind(&block.terminator, branch_instruction)
        || !branch_instruction.operands.is_empty()
        || branch_instruction.implicit_uses.is_empty()
    {
        return Err(ConstantBranchError::UnsupportedInstruction);
    }
    let keys = environment.selected_keys();
    // The flag universe partitions the branch's implicit uses: the units
    // the target's three compare rows publish are the condition state the
    // fold decides, and every other observed unit must survive on the jump
    // row's own implicit surface.
    let mut flag_universe = BTreeSet::new();
    for key in [
        keys.compare_i64,
        keys.compare_i64_immediate,
        keys.compare_i64_zero,
    ] {
        flag_universe.extend(
            environment
                .constraint(key)
                .ok_or(ConstantBranchError::ConstraintMismatch)?
                .implicit_defs
                .iter()
                .copied(),
        );
    }
    let jump_row = environment
        .constraint(keys.jump)
        .ok_or(ConstantBranchError::ConstraintMismatch)?;
    let jump_surface: BTreeSet<RegisterUnitId> = jump_row
        .implicit_uses
        .iter()
        .chain(jump_row.implicit_defs.iter())
        .chain(jump_row.clobbers.iter())
        .copied()
        .collect();
    // The rebuilt terminator publishes the jump row's surface where the
    // branch's stood, so the two must agree on definitions and clobbers:
    // a definition or clobber the jump lacks would hand downstream readers
    // an older event at this site, and one the branch lacked would publish
    // new state the source never had.
    if !jump_row.operands.is_empty()
        || BTreeSet::from_iter(jump_row.implicit_defs.iter().copied())
            != BTreeSet::from_iter(branch_instruction.implicit_defs.iter().copied())
        || BTreeSet::from_iter(jump_row.clobbers.iter().copied())
            != BTreeSet::from_iter(branch_instruction.clobbers.iter().copied())
    {
        return Err(ConstantBranchError::ConstraintMismatch);
    }
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition
    // state bounds every path, and mark the cone of blocks that can reach
    // the branch's block — only their entry sets can feed the result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(ConstantBranchError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every flag unit the branch reads must reach from the same compare:
    // on every execution path to the terminator, the last event touching
    // each used flag unit is that one instruction, and the unit is among
    // its published definitions — a clobber there would leave the observed
    // value unknown. Every other used unit must keep its observation on
    // the jump row's surface. The compare stays published for readers the
    // rewrite leaves behind.
    let read_position = block.instructions.len();
    let mut flag_uses = 0usize;
    let mut compare_site = None;
    for unit in &branch_instruction.implicit_uses {
        if !flag_universe.contains(unit) {
            if !jump_surface.contains(unit) {
                return Err(ConstantBranchError::UnsupportedUse);
            }
            continue;
        }
        flag_uses += 1;
        let event = reaching_event(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            read_position,
            *unit,
        )
        .map_err(condition_error)?;
        if compare_site.is_some_and(|site| site != event) {
            return Err(ConstantBranchError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    if flag_uses == 0 {
        return Err(ConstantBranchError::UnsupportedInstruction);
    }
    let compare_site = compare_site.ok_or(ConstantBranchError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if branch_instruction
        .implicit_uses
        .iter()
        .filter(|unit| flag_universe.contains(unit))
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(ConstantBranchError::UnsupportedUse);
    }
    let (left, right) =
        constant_operands(function, compare_instruction).map_err(condition_error)?;
    let successor = decided(&block.terminator, branch_instruction.kind, left, right)
        .ok_or(ConstantBranchError::UnsupportedInstruction)?
        .clone();
    Ok(Reconstructed {
        function,
        block_index,
        branch_id: branch,
        provenance: branch_instruction.provenance.clone(),
        successor,
        row: jump_row,
        successors,
        implicit_uses: branch_instruction.implicit_uses.len(),
        flag_universe: flag_universe.len(),
        jump_surface: jump_surface.len(),
        compare_defs: compare_instruction.implicit_defs.len(),
    })
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: one step per block plus one per instruction
/// across the plan, two scans of the reconstructed function's blocks for
/// the producer audit, and the shared walk's setup and fixpoint bound at
/// one per used unit — flag or not — so the bound stays independent of
/// the partition split.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    reconstructed: &Reconstructed<'_>,
) -> Result<u64, ConstantBranchError> {
    let function = reconstructed.function;
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let edge_count = reconstructed
        .successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    // Producer scans walk the whole function once per compared register —
    // two at most. The shared walk's setup resolves every edge's target
    // index, fills the predecessor lists, and marks the backward cone.
    // Each used unit then pays its partition checks — flag-universe and
    // jump-surface membership and the compare-definitions lookup — plus,
    // for a flag unit, the block-prefix scan and — on an in-block miss —
    // every block's stream for its last event, after which entry-set
    // propagation requeues a block only while its set grows: a set holds
    // at most one element per event site plus the unknown marker, so pops
    // stay under `blocks × (elements + 1)` and each pop visits its
    // out-edges — no more than the widest terminator's — at a bounded
    // union cost.
    let elements = function_scan
        .checked_add(1)
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(ConstantBranchError::IdentityOverflow)?,
        )
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let widest_out = reconstructed
        .successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .and_then(|total| {
            total.checked_add(
                reconstructed
                    .flag_universe
                    .checked_add(reconstructed.jump_surface)?
                    .checked_add(reconstructed.compare_defs)?,
            )
        })
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(ConstantBranchError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .and_then(|total| total.checked_add(reconstructed.flag_universe))
        .and_then(|total| total.checked_add(reconstructed.jump_surface))
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let reach_scan = reconstructed
        .implicit_uses
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(ConstantBranchError::IdentityOverflow)?;
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
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| ConstantBranchError::IdentityOverflow)
}

/// Build the terminator the contract demands from the validator's own
/// record: the target's own jump row — zero-operand by the audit — supplies
/// the implicit surface while the branch's instruction identity and
/// provenance stay with the terminator, and the decided successor carries
/// its record verbatim. The producer's `rewritten` constructor is not
/// consulted; both sides derive the same terminator from the source
/// alone.
fn expected(reconstructed: &Reconstructed<'_>) -> SelectedTerminator {
    SelectedTerminator::Jump {
        instruction: SelectedInstruction {
            id: reconstructed.branch_id,
            kind: SelectedInstructionKind::Jump,
            constraint: reconstructed.row.key,
            operands: Vec::new(),
            implicit_uses: reconstructed.row.implicit_uses.clone(),
            implicit_defs: reconstructed.row.implicit_defs.clone(),
            clobbers: reconstructed.row.clobbers.clone(),
            provenance: reconstructed.provenance.clone(),
        },
        successor: reconstructed.successor.clone(),
    }
}

/// Reinsert the source terminator: undoing the validator's expected edit
/// must restore the complete source by content — every other instruction,
/// register, roster row, call, and settlement included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
    function_index: usize,
) -> Result<SelectedInstructionPlan, ConstantBranchError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(function_index)
        .ok_or(ConstantBranchError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(ConstantBranchError::ReplayMismatch)?;
    block.terminator = reconstructed.function.blocks[reconstructed.block_index]
        .terminator
        .clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs
/// the fold's preconditions from the source, requires the proposed
/// terminator in the branch's block to equal the `Jump` its own record
/// produces, and restores the complete source by content — every other
/// instruction, register, roster row, call, and settlement included. The
/// producer's `admission::admit` is never consulted, so a wrong legality
/// decision fails here even when the proposal matches the edit the
/// producer emitted.
pub fn validate_constant_branch_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConstantBranch, ConstantBranchError> {
    let reconstructed = reconstruct(source, function_index, branch, environment)?;
    if measured_steps(source.selected_plan(), &reconstructed)? > budget.validation_steps() {
        return Err(ConstantBranchError::WorkBudgetExceeded);
    }
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .map(|block| &block.terminator)
        != Some(&expected(&reconstructed))
    {
        return Err(ConstantBranchError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed, function_index)? != *source.selected_plan() {
        return Err(ConstantBranchError::ReplayMismatch);
    }
    Ok(ValidatedConstantBranch {
        receipt: ConstantBranchReceipt {
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
    use register_environment::{
        ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
    };
    use register_model::{RegisterInstructionConstraint, RegisterUnitId};
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
        SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, VirtualRegister,
        VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
        ScalarType, ValueId,
    };
    use target::NativeTarget;
    use target_operations_to_selected_instructions::selected_instruction_plan_identity;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        ConstantBranchError, ConstantBranchReceipt, ValidatedConstantBranch,
        validate_constant_branch_fold,
    };

    const MATERIALIZE_A: SelectedInstructionId = SelectedInstructionId(2);
    const MATERIALIZE_AGAIN: SelectedInstructionId = SelectedInstructionId(3);
    const MATERIALIZE_B: SelectedInstructionId = SelectedInstructionId(4);
    const COMPARE: SelectedInstructionId = SelectedInstructionId(5);
    const BRANCH: SelectedInstructionId = SelectedInstructionId(6);

    const LITA: VirtualRegisterId = VirtualRegisterId(0);
    const LITB: VirtualRegisterId = VirtualRegisterId(1);

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

    fn arm(block: SelectedBlockId, target: u64, edge: u64) -> SelectedSuccessor {
        SelectedSuccessor {
            role: SelectedSuccessorRole::Semantic,
            psi_edge: EdgeId::new(edge).unwrap(),
            block,
            source_target: BlockId::new(target).unwrap(),
            bindings: Vec::new(),
            structural_bindings: Vec::new(),
            structural_case: None,
            fuel: Vec::new(),
        }
    }

    fn returning(
        block: u32,
        instruction_id: u32,
        edge: u64,
        row: &RegisterInstructionConstraint,
    ) -> SelectedBlock {
        SelectedBlock {
            id: SelectedBlockId(block),
            origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(block) + 1).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(instruction_id),
                    SelectedInstructionKind::ReturnUnit,
                    row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(edge).unwrap(),
            },
        }
    }

    /// `ra = materialize 3; rb = materialize 5; compare ra, rb;` then a
    /// `ConditionalBranchNonZero` terminator on the published flags to two
    /// returning blocks — a legal fold where the branch decides
    /// `when_nonzero`. Each test mutates one legality premise, so the
    /// source names a fold the contract must refuse while a defective
    /// producer would still emit it.
    fn plan(
        environment: &ValidatedTargetRegisterEnvironment,
        target: NativeTarget,
    ) -> SelectedInstructionPlan {
        let keys = environment.selected_keys();
        let materialize = environment.constraint(keys.materialize_i64).unwrap();
        let compare = environment.constraint(keys.compare_i64).unwrap();
        let branch_row = environment.constraint(keys.conditional_branch).unwrap();
        let terminal_row = environment.constraint(keys.return_unit).unwrap();
        let class = compare.operands[0].class;
        let scalar_type = u64_scalar();
        let register = |id: VirtualRegisterId, origin: VirtualRegisterOrigin| VirtualRegister {
            id,
            scalar_type,
            class,
            origin,
            definition_site: None,
            entry_fixed_view: None,
        };
        let machine = MachineId::new(1).unwrap();
        SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target,
            entry: machine,
            functions: vec![SelectedFunction {
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
                    register(
                        LITA,
                        VirtualRegisterOrigin::InstructionResult {
                            instruction: MATERIALIZE_A,
                            source_value: ValueId::new(2).unwrap(),
                        },
                    ),
                    register(
                        LITB,
                        VirtualRegisterOrigin::InstructionResult {
                            instruction: MATERIALIZE_B,
                            source_value: ValueId::new(3).unwrap(),
                        },
                    ),
                ],
                blocks: vec![
                    SelectedBlock {
                        id: SelectedBlockId(0),
                        origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                        instructions: vec![
                            instruction(
                                MATERIALIZE_A,
                                SelectedInstructionKind::MaterializeI64 {
                                    value: IntegerValue::Unsigned(3),
                                },
                                materialize,
                                &[LITA],
                            ),
                            instruction(
                                MATERIALIZE_B,
                                SelectedInstructionKind::MaterializeI64 {
                                    value: IntegerValue::Unsigned(5),
                                },
                                materialize,
                                &[LITB],
                            ),
                            instruction(
                                COMPARE,
                                SelectedInstructionKind::CompareI64,
                                compare,
                                &[LITA, LITB],
                            ),
                        ],
                        terminator: SelectedTerminator::ConditionalBranch {
                            instruction: instruction(
                                BRANCH,
                                SelectedInstructionKind::ConditionalBranchNonZero,
                                branch_row,
                                &[],
                            ),
                            when_nonzero: arm(SelectedBlockId(1), 2, 2),
                            when_zero: arm(SelectedBlockId(2), 3, 3),
                        },
                    },
                    returning(1, 8, 4, terminal_row),
                    returning(2, 9, 5, terminal_row),
                ],
            }]
            .into(),
        }
    }

    fn source(plan: SelectedInstructionPlan) -> ValidatedConstantBranch {
        let identity = selected_instruction_plan_identity(&plan);
        ValidatedConstantBranch {
            receipt: ConstantBranchReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: plan.fuel_schedule,
            },
            transformed: Arc::new(plan),
        }
    }

    /// The proposal a defective producer emits for this source: the branch
    /// folded to a `Jump` on the `when_nonzero` arm under the target's own
    /// jump row — the edit the contract demands only when the fold's
    /// legality actually holds.
    fn forged_jump(
        source: &ValidatedConstantBranch,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> SelectedInstructionPlan {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let mut proposed = source.transformed().clone();
        let terminator = &proposed.functions[0].blocks[0].terminator;
        let SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            ..
        } = terminator
        else {
            unreachable!("the fixture terminator is a ConditionalBranch")
        };
        let jumped = SelectedTerminator::Jump {
            instruction: SelectedInstruction {
                id: instruction.id,
                kind: SelectedInstructionKind::Jump,
                constraint: jump_row.key,
                operands: Vec::new(),
                implicit_uses: jump_row.implicit_uses.clone(),
                implicit_defs: jump_row.implicit_defs.clone(),
                clobbers: jump_row.clobbers.clone(),
                provenance: instruction.provenance.clone(),
            },
            successor: when_nonzero.clone(),
        };
        proposed.functions[0].blocks[0].terminator = jumped;
        proposed
    }

    fn budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(100, 100, 100_000, 100, 100).unwrap()
    }

    /// `ra = materialize 3; ra = materialize 9; rb = materialize 5;
    /// compare ra, rb; branch` — the compare's left operand has two
    /// producers, so its value is not pinned function-wide and the fold's
    /// constant was never proven. A producer that admitted it anyway
    /// would publish a plan whose branch is rewritten to a `Jump` the
    /// source never established; the validator's own operand audit must
    /// refuse with `UnsupportedProducer`, not merely diff the proposal.
    /// Feeding that forged proposal is the observable proof that
    /// validation no longer relies on the producer's admission routine.
    #[test]
    fn validator_refuses_a_proposal_its_own_audit_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap();
        let mut plan = plan(&environment, target);
        plan.functions[0].blocks[0].instructions.insert(
            1,
            instruction(
                MATERIALIZE_AGAIN,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(9),
                },
                materialize,
                &[LITA],
            ),
        );
        let source = source(plan);
        let proposed = forged_jump(&source, &environment);
        assert_eq!(
            validate_constant_branch_fold(&source, 0, BRANCH, &environment, budget(), proposed)
                .unwrap_err(),
            ConstantBranchError::UnsupportedProducer
        );
    }

    /// The branch's use roster gains a unit that is neither a flag the
    /// fold decides nor a unit on the jump row's implicit surface — the
    /// rebuilt terminator would silently drop the observation. A producer
    /// that skipped the use partition still emits the `Jump`; the
    /// validator's own partition refuses with `UnsupportedUse`.
    #[test]
    fn validator_refuses_an_observation_the_jump_cannot_carry() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let mut plan = plan(&environment, target);
        let terminator = &mut plan.functions[0].blocks[0].terminator;
        let SelectedTerminator::ConditionalBranch { instruction, .. } = terminator else {
            unreachable!("the fixture terminator is a ConditionalBranch")
        };
        instruction.implicit_uses.push(RegisterUnitId(u16::MAX));
        let source = source(plan);
        let proposed = forged_jump(&source, &environment);
        assert_eq!(
            validate_constant_branch_fold(&source, 0, BRANCH, &environment, budget(), proposed)
                .unwrap_err(),
            ConstantBranchError::UnsupportedUse
        );
    }

    /// The compare is gone: every flag unit the branch reads reaches only
    /// the entry's unknown state. A producer that mistook the read for a
    /// resolved one still emits the `Jump`; the validator's own reaching
    /// walk refuses with `UnsupportedUse`.
    #[test]
    fn validator_refuses_flags_that_never_reached_a_compare() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let mut plan = plan(&environment, target);
        plan.functions[0].blocks[0]
            .instructions
            .retain(|instruction| instruction.id != COMPARE);
        let source = source(plan);
        let proposed = forged_jump(&source, &environment);
        assert_eq!(
            validate_constant_branch_fold(&source, 0, BRANCH, &environment, budget(), proposed)
                .unwrap_err(),
            ConstantBranchError::UnsupportedUse
        );
    }
}
