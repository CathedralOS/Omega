//! The validator's independence contract: it re-derives the regeneration's
//! legality from the source records alone, so a proposal carrying the
//! exact insertions a defective producer would publish still fails on the
//! validator's own audit — and a proposal missing the function the
//! contract demands fails the replay comparison. Nothing here consults
//! the producer's `admission` state: the forged proposals are built by
//! hand.

use super::{budget, fixture, instruction, mutated, rematerialize, spread, successor};
use crate::rewrites::runtime_spill::{control, control_mut};
use crate::{
    RuntimeRematerializationError, ValidatedRuntimeRematerialization,
    validate_runtime_rematerialization,
};
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBoundarySettlementPayload, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionProvenance, SelectedOperand, SelectedValueBinding,
    SelectedValueTransport, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType, ValueId};
use target::NativeTarget;

const VICTIM: VirtualRegisterId = VirtualRegisterId(1);

/// The proposal a defective producer would publish for `source`: one fresh
/// `MaterializeI64` and fresh register inserted before every use of
/// `VICTIM` — instruction-operand uses in place, terminator uses at the
/// block's end — the operands rebound and the boundary settlements
/// remapped. This is the mechanical edit with no legality audit behind
/// it, built by hand so the tests below never touch the producer's
/// `admission` constructor or record. When the source's definition is
/// not an immediate materialization the emitted value is arbitrary: the
/// audit must have already refused the source before any stream
/// comparison runs.
fn forged(source: &ValidatedRuntimeRematerialization) -> SelectedInstructionPlan {
    let mut proposed = source.transformed().clone();
    let environment = baseline_target_register_environment(proposed.target).unwrap();
    let materialize = environment
        .constraint(environment.selected_keys().materialize_i64)
        .unwrap()
        .clone();
    let function = &mut proposed.functions[0];
    let victim_row = function
        .virtual_registers
        .iter()
        .find(|value| value.id == VICTIM)
        .unwrap()
        .clone();
    let (definition, source_value) = match victim_row.origin {
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } => (instruction, source_value),
        _ => (SelectedInstructionId(0), ValueId::new(1).unwrap()),
    };
    let value = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .find(|candidate| candidate.id == definition)
        .and_then(|instruction| match instruction.kind {
            SelectedInstructionKind::MaterializeI64 { value } => Some(value),
            _ => None,
        })
        .unwrap_or(IntegerValue::Unsigned(0));
    let mut next_instruction = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain([control(&block.terminator).0.id.0])
        })
        .max()
        .unwrap_or(0)
        + 1;
    let mut next_register = function
        .virtual_registers
        .iter()
        .map(|value| value.id.0)
        .max()
        .unwrap_or(0)
        + 1;
    let mut materialization = |function: &mut selected_instructions::SelectedFunction| {
        let instruction = SelectedInstructionId(next_instruction);
        next_instruction += 1;
        let register = VirtualRegisterId(next_register);
        next_register += 1;
        let operand = &materialize.operands[0];
        function.virtual_registers.push(VirtualRegister {
            id: register,
            scalar_type: victim_row.scalar_type,
            class: victim_row.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value,
            },
            definition_site: victim_row.definition_site,
            entry_fixed_view: None,
        });
        (
            register,
            selected_instructions::SelectedInstruction {
                id: instruction,
                kind: SelectedInstructionKind::MaterializeI64 { value },
                constraint: materialize.key,
                operands: vec![SelectedOperand {
                    operand: operand.operand,
                    virtual_register: register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                }],
                implicit_uses: materialize.implicit_uses.clone(),
                implicit_defs: materialize.implicit_defs.clone(),
                clobbers: materialize.clobbers.clone(),
                provenance: SelectedInstructionProvenance {
                    values: vec![source_value],
                    ..Default::default()
                },
            },
        )
    };
    for block_index in 0..function.blocks.len() {
        let mut instructions = Vec::new();
        let mut boundaries = Vec::new();
        let mut instruction_positions = Vec::new();
        for original in &function.blocks[block_index].instructions.clone() {
            boundaries.push(instructions.len() as u32);
            let mut rewritten = original.clone();
            let mut inserted = Vec::new();
            for operand in &mut rewritten.operands {
                if operand.virtual_register != VICTIM
                    || operand.access != RegisterOperandAccess::Use
                {
                    continue;
                }
                let (register, materialized) = materialization(function);
                inserted.push(materialized);
                operand.virtual_register = register;
            }
            instructions.extend(inserted);
            instruction_positions.push(instructions.len() as u32);
            instructions.push(rewritten);
        }
        let mut terminator = function.blocks[block_index].terminator.clone();
        for operand in &mut control_mut(&mut terminator).operands {
            if operand.virtual_register != VICTIM || operand.access != RegisterOperandAccess::Use {
                continue;
            }
            let (register, materialized) = materialization(function);
            instructions.push(materialized);
            operand.virtual_register = register;
        }
        boundaries.push(instructions.len() as u32);
        instruction_positions.push(instructions.len() as u32);
        let block = function.blocks[block_index].id;
        for settlement in &mut function.boundary_settlements {
            if settlement.block != block {
                continue;
            }
            let positions = if matches!(
                settlement.settlement,
                SelectedBoundarySettlementPayload::ClaimCompletion(_)
            ) {
                &boundaries
            } else {
                &instruction_positions
            };
            settlement.instruction_index = positions[settlement.instruction_index as usize];
        }
        function.blocks[block_index].instructions = instructions;
        function.blocks[block_index].terminator = terminator;
    }
    proposed
}

/// On a legal source the mechanical edit is exactly the proposal the
/// signed result publishes — the single-block fixture and the dominated
/// successor spread alike. The forged builder is the contract's edit, so
/// the rejections below come from the validator's own audit rather than a
/// diff quirk, and the signed proposal validates as a second input.
#[test]
fn forged_edit_matches_the_signed_proposal_and_validates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for source in [fixture(target), spread(target)] {
        let signed = rematerialize(&source, &environment).unwrap();
        assert_eq!(&forged(&source), signed.transformed());
        validate_runtime_rematerialization(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap();
    }
}

/// The victim's definition computes rather than materializes: the value
/// is a copy result, not a cheap regenerable immediate. A producer that
/// admitted it anyway would publish the mechanical insertions — the
/// validator's own custody audit must refuse with `UnsupportedValue`.
#[test]
fn forged_proposal_does_not_launder_a_non_immediate_definition() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[0] = instruction(
            SelectedInstructionId(1),
            SelectedInstructionKind::CopyI64,
            copy,
            &[VirtualRegisterId(0), VICTIM],
        );
    });
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedValue
    );
}

/// The definition carries hidden effects the target row does not
/// declare: the recorded instruction is not the validated materialize
/// row, and the mechanical insertions still publish. The validator's own
/// definition audit refuses it.
#[test]
fn forged_proposal_does_not_launder_an_effectful_definition() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let clobbered = environment
            .constraint(environment.selected_keys().call_unit[0])
            .unwrap();
        let mut definition = function.blocks[0].instructions[0].clone();
        definition.clobbers = clobbered.clobbers.clone();
        assert!(!definition.clobbers.is_empty());
        function.blocks[0].instructions[0] = definition;
    });
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedValue
    );
}

/// A fixed view on an instruction-operand use pins the operand to a
/// physical unit the fresh register cannot satisfy — the use is
/// inadmissible, yet the mechanical insertions still publish. The
/// validator's own use scan refuses with `UnsupportedUse`.
#[test]
fn forged_proposal_does_not_launder_a_fixed_use() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].fixed_view =
            Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedUse
    );
}

/// An early-clobber use would let the fresh register die inside the
/// consumer's own effects: the use is inadmissible, yet the mechanical
/// insertions still publish. The validator's own use scan refuses it.
#[test]
fn forged_proposal_does_not_launder_an_early_clobber_use() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].early_clobber = true;
    });
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedUse
    );
}

/// An output tied to the use position would extend the fresh register's
/// value identity into an allocation-shaped pair: the use is
/// inadmissible, yet the mechanical insertions still publish. The
/// validator's own use scan refuses it.
#[test]
fn forged_proposal_does_not_launder_a_tied_output() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[1].tied_to = Some(0);
    });
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedUse
    );
}

/// A `Registers` value binding carrying the victim out of a block is a
/// transport the rule does not admit: regenerating on one side of the
/// edge cannot serve it. A producer that skipped the transport audit
/// still publishes the mechanical insertions; the validator's own edge
/// audit refuses it.
#[test]
fn forged_proposal_does_not_launder_an_outgoing_transport() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let mut edge = successor(1);
        edge.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(9).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            transport: SelectedValueTransport::Registers {
                argument: VICTIM,
                parameter: VirtualRegisterId(0),
            },
        });
        let tail_terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            selected_instructions::SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(20),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: edge,
            },
        );
        function.blocks.push(selected_instructions::SelectedBlock {
            id: selected_instructions::SelectedBlockId(1),
            origin: selected_instructions::SelectedBlockOrigin::Source(
                semantic_vocabulary::BlockId::new(2).unwrap(),
            ),
            instructions: Vec::new(),
            terminator: tail_terminator,
        });
    });
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedUse
    );
}

/// A use block the definition cannot dominate regenerates where no
/// definition ever ran: the source is inadmissible, yet the mechanical
/// insertions still publish into the join block. The validator's own
/// dominance audit refuses it.
#[test]
fn forged_proposal_does_not_launder_an_undominated_use() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let definition = function.blocks[0].instructions.remove(0);
        let uses = std::mem::take(&mut function.blocks[0].instructions);
        let tail_terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            selected_instructions::SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    SelectedInstructionId(20),
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    branch,
                    &[],
                ),
                when_nonzero: successor(1),
                when_zero: successor(2),
            },
        );
        function.blocks.push(selected_instructions::SelectedBlock {
            id: selected_instructions::SelectedBlockId(1),
            origin: selected_instructions::SelectedBlockOrigin::Source(
                semantic_vocabulary::BlockId::new(2).unwrap(),
            ),
            instructions: vec![definition],
            terminator: selected_instructions::SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(21),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(3),
            },
        });
        function.blocks.push(selected_instructions::SelectedBlock {
            id: selected_instructions::SelectedBlockId(2),
            origin: selected_instructions::SelectedBlockOrigin::Source(
                semantic_vocabulary::BlockId::new(3).unwrap(),
            ),
            instructions: Vec::new(),
            terminator: selected_instructions::SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(22),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(3),
            },
        });
        function.blocks.push(selected_instructions::SelectedBlock {
            id: selected_instructions::SelectedBlockId(3),
            origin: selected_instructions::SelectedBlockOrigin::Source(
                semantic_vocabulary::BlockId::new(4).unwrap(),
            ),
            instructions: uses,
            terminator: tail_terminator,
        });
    });
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedUse
    );
}

/// An entry-pinned victim is custody the regeneration family does not
/// admit: the mechanical insertions still publish, and the validator's
/// own custody audit refuses with `UnsupportedValue`.
#[test]
fn forged_proposal_does_not_launder_an_entry_pinned_victim() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.virtual_registers[1].entry_fixed_view = Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedValue
    );
}

/// A victim whose definition-site custody is not an ordinary parameter
/// or node — here erased outright — is not a regenerable family member:
/// the mechanical insertions still publish, and the validator's own
/// custody audit refuses it.
#[test]
fn forged_proposal_does_not_launder_an_uncustodied_victim() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.virtual_registers[1].definition_site = None;
    });
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedValue
    );
}

/// On a legal source the contract demands exactly one transformed
/// function: a proposal that regenerates nothing, drops a demanded
/// materialization, renames a fresh register, or carries an extra
/// register row is not that function — however it was produced.
#[test]
fn validator_rejects_proposals_that_miss_the_demanded_function() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The unchanged source regenerates nothing.
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            source.transformed().clone(),
        )
        .unwrap_err(),
        RuntimeRematerializationError::ReplayMismatch
    );
    // One demanded materialization is missing.
    let mut dropped = forged(&source);
    dropped.functions[0].blocks[0].instructions.remove(1);
    dropped.functions[0].virtual_registers.pop();
    assert_eq!(
        validate_runtime_rematerialization(&source, 0, VICTIM, &environment, budget(), dropped)
            .unwrap_err(),
        RuntimeRematerializationError::ReplayMismatch
    );
    // A fresh register's identity does not match the insertion order.
    let mut renamed = forged(&source);
    renamed.functions[0].virtual_registers[5].id = VirtualRegisterId(90);
    assert_eq!(
        validate_runtime_rematerialization(&source, 0, VICTIM, &environment, budget(), renamed)
            .unwrap_err(),
        RuntimeRematerializationError::ReplayMismatch
    );
    // A spare register row the regeneration never produced.
    let mut spare = forged(&source);
    let mut extra = spare.functions[0].virtual_registers[5].clone();
    extra.id = VirtualRegisterId(91);
    extra.origin = VirtualRegisterOrigin::InstructionResult {
        instruction: SelectedInstructionId(91),
        source_value: ValueId::new(1).unwrap(),
    };
    spare.functions[0].virtual_registers.push(extra);
    assert_eq!(
        validate_runtime_rematerialization(&source, 0, VICTIM, &environment, budget(), spare)
            .unwrap_err(),
        RuntimeRematerializationError::ReplayMismatch
    );
}
