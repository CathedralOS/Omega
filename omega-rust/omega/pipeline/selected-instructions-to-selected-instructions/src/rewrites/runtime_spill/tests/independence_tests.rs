//! The validator's independence contract: it re-derives the spill's
//! legality from the source records alone, so a proposal carrying the
//! exact stores and reload pairs a defective producer would publish
//! still fails on the validator's own audit — and a proposal missing
//! the function the contract demands fails the replay comparison.
//! Nothing here consults the producer's `admission` state: the forged
//! proposals are built by hand from the target rows and the shared
//! contract vocabulary.
//!
//! Every existing test in this suite that validates a signed proposal is
//! already independence evidence — validation no longer calls
//! `admission::admit`, so each one replays the producer's output through
//! the validator's own reconstruction. The tests below isolate the two
//! directions that coverage cannot see by itself: a hand-built proposal
//! identical to the signed one validates standalone, and the same
//! mechanical edit on an illegal source dies in the audit rather than
//! laundering through a shared admission routine.

use super::control_flow::successor;
use super::{
    Arc, NativeTarget, RuntimeSpillError, ValidatedRuntimeSpill, budget, fixture,
    selected_instruction_plan_identity, validate_runtime_spill,
};
use crate::rewrites::runtime_spill::{control, control_mut, surviving_home_exists};
use crate::spill_selected_runtime_value;
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess, RegisterViewId};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedBoundarySettlementPayload, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedLocalStorageSlot, SelectedOperand, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{BlockId, IntegerSign, IntegerType, ScalarType, ValueId};

const VICTIM: VirtualRegisterId = VirtualRegisterId(1);

/// Mutate the single-function fixture and refresh its recorded identity so
/// the edited plan — not the pristine one — is what the validator audits.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &ValidatedTargetRegisterEnvironment),
) -> ValidatedRuntimeSpill {
    let mut source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    edit(
        &mut Arc::make_mut(&mut source.transformed).functions[0],
        &environment,
    );
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// One instruction built from the target row's declared operand interface —
/// the same shape the producer's private constructor emits, assembled here
/// by hand so the forged proposals never touch `admission`.
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

/// The proposal a defective producer would publish for `source`: the private
/// spill slot declared, a `Store64` after every write that lands the victim's
/// value in storage, and a `FrameAddress`/`Load64` pair ahead of every read —
/// consecutive unpinned reads sharing the still-open pair until an
/// instruction that can destroy register content closes it — the operands
/// rebound and the boundary settlements remapped. This is the mechanical
/// edit with no legality audit behind it, built by hand so the tests below
/// never touch the producer's `admission` constructor or record. It covers
/// the direct-carrier shape the suite's fixtures use; a victim needing the
/// bit-preserving conversion pair would emit the conversion rows around the
/// same stores and loads.
fn forged(source: &ValidatedRuntimeSpill) -> SelectedInstructionPlan {
    let mut proposed = source.transformed().clone();
    let environment = baseline_target_register_environment(proposed.target).unwrap();
    let keys = environment.selected_keys();
    let address_row = environment
        .constraint(keys.frame_address.unwrap())
        .unwrap()
        .clone();
    let load_row = environment
        .constraint(keys.load64.unwrap())
        .unwrap()
        .clone();
    let store_row = environment
        .constraint(keys.store64.unwrap())
        .unwrap()
        .clone();
    let function = &mut proposed.functions[0];
    let victim = function
        .virtual_registers
        .iter()
        .find(|value| value.id == VICTIM)
        .unwrap()
        .clone();
    let slot = LocalStorageSlotId::Spill { register: VICTIM };
    function.local_storage_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: 8,
        alignment: 8,
    });
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
    let source_value = match victim.origin {
        VirtualRegisterOrigin::InstructionResult { source_value, .. } => source_value,
        _ => ValueId::new(1).unwrap(),
    };
    // The shared contract's surviving-home check — read on the same evidence
    // the audit reads, not on any producer record — decides whether one open
    // reload may serve consecutive unpinned uses.
    let mut implicit_use_units = std::collections::BTreeSet::new();
    let mut pinned_views = function
        .virtual_registers
        .iter()
        .filter_map(|register| register.entry_fixed_view)
        .collect::<Vec<_>>();
    for instruction in function.blocks.iter().flat_map(|block| {
        block
            .instructions
            .iter()
            .chain(std::iter::once(control(&block.terminator).0))
    }) {
        implicit_use_units.extend(instruction.implicit_uses.iter().copied());
        pinned_views.extend(
            instruction
                .operands
                .iter()
                .filter_map(|operand| operand.fixed_view),
        );
    }
    implicit_use_units.extend(address_row.implicit_uses.iter().copied());
    implicit_use_units.extend(load_row.implicit_uses.iter().copied());
    implicit_use_units.extend(store_row.implicit_uses.iter().copied());
    let pinned_units = pinned_views
        .iter()
        .flat_map(|view_id| {
            environment
                .physical()
                .model()
                .views
                .get(usize::from(view_id.0))
                .into_iter()
                .flat_map(|view| view.units.iter().chain(&view.write_units))
        })
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let shared = surviving_home_exists(
        &environment,
        victim.class,
        &implicit_use_units,
        &pinned_units,
        &std::collections::BTreeSet::new(),
    );
    let mut open_reload: Option<VirtualRegisterId> = None;
    for block_index in 0..function.blocks.len() {
        let mut instructions = Vec::new();
        let mut boundaries = Vec::new();
        let mut instruction_positions = Vec::new();
        for original in function.blocks[block_index].instructions.clone() {
            boundaries.push(instructions.len() as u32);
            let mut rewritten = original.clone();
            for operand in &mut rewritten.operands {
                if operand.virtual_register != VICTIM
                    || !matches!(
                        operand.access,
                        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                    )
                {
                    continue;
                }
                let share = shared && operand.fixed_view.is_none();
                let reloaded = match (share, open_reload) {
                    (true, Some(existing)) => existing,
                    _ => {
                        let address_instruction = SelectedInstructionId(next_instruction);
                        next_instruction += 1;
                        let load_instruction = SelectedInstructionId(next_instruction);
                        next_instruction += 1;
                        let address_register = VirtualRegisterId(next_register);
                        next_register += 1;
                        let reload_register = VirtualRegisterId(next_register);
                        next_register += 1;
                        function.virtual_registers.push(VirtualRegister {
                            id: address_register,
                            scalar_type: ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                            ),
                            class: address_row.operands[0].class,
                            origin: VirtualRegisterOrigin::SpillAddress {
                                instruction: address_instruction,
                                register: VICTIM,
                            },
                            definition_site: None,
                            entry_fixed_view: None,
                        });
                        function.virtual_registers.push(VirtualRegister {
                            id: reload_register,
                            scalar_type: victim.scalar_type,
                            class: victim.class,
                            origin: VirtualRegisterOrigin::InstructionResult {
                                instruction: load_instruction,
                                source_value,
                            },
                            definition_site: victim.definition_site,
                            entry_fixed_view: None,
                        });
                        instructions.push(instruction(
                            address_instruction,
                            SelectedInstructionKind::FrameAddress {
                                slot: FrameStorageSlotId::Local(slot),
                                byte_offset: 0,
                            },
                            &address_row,
                            &[address_register],
                        ));
                        instructions.push(instruction(
                            load_instruction,
                            SelectedInstructionKind::Load64 { byte_offset: 0 },
                            &load_row,
                            &[address_register, reload_register],
                        ));
                        if share {
                            open_reload = Some(reload_register);
                        }
                        reload_register
                    }
                };
                operand.virtual_register = reloaded;
            }
            instruction_positions.push(instructions.len() as u32);
            let emitted = rewritten.clone();
            instructions.push(rewritten);
            // A unit-writing instruction destroys the still-open register
            // content, and a victim write makes its value stale: the next
            // unpinned use opens a fresh pair — the bounded span the default
            // entrypoints produce and replay.
            if !original.clobbers.is_empty()
                || !original.implicit_defs.is_empty()
                || original.operands.iter().any(|operand| {
                    operand.virtual_register == VICTIM
                        && operand.access != RegisterOperandAccess::Use
                })
            {
                open_reload = None;
            }
            // Every write that lands the victim's value — a plain `Def`
            // storing the victim register, or a `UseDef` storing the
            // register its operand was redirected to — gains its own
            // following `Store64`.
            for operand in &original.operands {
                if operand.virtual_register != VICTIM {
                    continue;
                }
                let stored = match operand.access {
                    RegisterOperandAccess::Def => VICTIM,
                    RegisterOperandAccess::UseDef => {
                        emitted
                            .operands
                            .iter()
                            .find(|candidate| candidate.operand == operand.operand)
                            .unwrap()
                            .virtual_register
                    }
                    _ => continue,
                };
                let store_instruction = SelectedInstructionId(next_instruction);
                next_instruction += 1;
                instructions.push(instruction(
                    store_instruction,
                    SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset: 0,
                    },
                    &store_row,
                    &[stored],
                ));
            }
        }
        // Terminator operand reads reload at the block's end on their own
        // pairs, like the producer's end-of-block emission.
        let mut terminator = function.blocks[block_index].terminator.clone();
        for operand in &mut control_mut(&mut terminator).operands {
            if operand.virtual_register != VICTIM || operand.access != RegisterOperandAccess::Use {
                continue;
            }
            let address_instruction = SelectedInstructionId(next_instruction);
            next_instruction += 1;
            let load_instruction = SelectedInstructionId(next_instruction);
            next_instruction += 1;
            let address_register = VirtualRegisterId(next_register);
            next_register += 1;
            let reload_register = VirtualRegisterId(next_register);
            next_register += 1;
            function.virtual_registers.push(VirtualRegister {
                id: address_register,
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
                class: address_row.operands[0].class,
                origin: VirtualRegisterOrigin::SpillAddress {
                    instruction: address_instruction,
                    register: VICTIM,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            function.virtual_registers.push(VirtualRegister {
                id: reload_register,
                scalar_type: victim.scalar_type,
                class: victim.class,
                origin: VirtualRegisterOrigin::InstructionResult {
                    instruction: load_instruction,
                    source_value,
                },
                definition_site: victim.definition_site,
                entry_fixed_view: None,
            });
            instructions.push(instruction(
                address_instruction,
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                &address_row,
                &[address_register],
            ));
            instructions.push(instruction(
                load_instruction,
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                &load_row,
                &[address_register, reload_register],
            ));
            operand.virtual_register = reload_register;
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
/// signed result publishes — the single-block fixture on every baseline
/// target. The forged builder is the contract's edit, so the rejections
/// below come from the validator's own audit rather than a diff quirk,
/// and the signed proposal validates as a second input.
#[test]
fn forged_edit_matches_the_signed_proposal_and_validates() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let signed =
            spill_selected_runtime_value(&source, 0, VICTIM, &environment, budget()).unwrap();
        assert_eq!(&forged(&source), signed.transformed());
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), forged(&source))
            .unwrap();
        validate_runtime_spill(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            signed.transformed().clone(),
        )
        .unwrap();
    }
}

/// A victim whose definition-site custody is not an ordinary parameter or
/// node — here erased outright — is not a spill family member: the
/// mechanical stores and reloads still publish, and the validator's own
/// custody audit refuses with `UnsupportedValue`.
#[test]
fn forged_proposal_does_not_launder_an_uncustodied_victim() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.virtual_registers[1].definition_site = None;
    });
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), forged(&source))
            .unwrap_err(),
        RuntimeSpillError::UnsupportedValue
    );
}

/// A fixed view on a non-boundary victim is a pin the spill cannot honour —
/// the rewritten register would have to arrive pinned at every load and
/// store. The mechanical edit still publishes; the validator's own custody
/// audit refuses it.
#[test]
fn forged_proposal_does_not_launder_an_entry_pinned_victim() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.virtual_registers[1].entry_fixed_view = Some(RegisterViewId(0));
    });
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), forged(&source))
            .unwrap_err(),
        RuntimeSpillError::UnsupportedValue
    );
}

/// A defined-but-unused victim stores a value nothing ever reloads: the
/// contract admits only spills that serve a use. The mechanical edit — a
/// lone store and slot — still publishes; the validator's own use audit
/// refuses the source.
#[test]
fn forged_proposal_does_not_launder_an_unused_victim() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        for instruction in &mut function.blocks[0].instructions[1..] {
            instruction.operands[0].virtual_register = VirtualRegisterId(0);
        }
    });
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), forged(&source))
            .unwrap_err(),
        RuntimeSpillError::UnsupportedValue
    );
}

/// A tied read position would bind the victim to a second register's home —
/// inadmissible custody the mechanical edit ignores. The validator's own
/// use scan refuses with `UnsupportedUse`.
#[test]
fn forged_proposal_does_not_launder_a_tied_use() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].tied_to = Some(1);
    });
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), forged(&source))
            .unwrap_err(),
        RuntimeSpillError::UnsupportedUse
    );
}

/// An early-clobber read could see the reload die inside the consumer's own
/// effects before the operand is consumed — inadmissible, yet the mechanical
/// edit still publishes. The validator's own use scan refuses it.
#[test]
fn forged_proposal_does_not_launder_an_early_clobber_use() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].early_clobber = true;
    });
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), forged(&source))
            .unwrap_err(),
        RuntimeSpillError::UnsupportedUse
    );
}

/// A use operand recorded in a different class than the victim's cannot read
/// the produced reload register — an inconsistent plan the mechanical edit
/// publishes anyway. The validator's own use scan refuses it.
#[test]
fn forged_proposal_does_not_launder_a_foreign_class_use() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let float_class = environment
            .constraint(environment.selected_keys().bits_to_float64.unwrap())
            .unwrap()
            .operands[1]
            .class;
        function.blocks[0].instructions[1].operands[0].class = float_class;
    });
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), forged(&source))
            .unwrap_err(),
        RuntimeSpillError::UnsupportedUse
    );
}

/// A private slot already declared for the victim means storage is claimed
/// twice — the mechanical edit appends a second row and still publishes.
/// The validator's own slot audit refuses it.
#[test]
fn forged_proposal_does_not_launder_a_redeclared_slot() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: LocalStorageSlotId::Spill { register: VICTIM },
            byte_size: 8,
            alignment: 8,
        });
    });
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), forged(&source))
            .unwrap_err(),
        RuntimeSpillError::UnsupportedUse
    );
}

/// A use block the definition cannot dominate would reload storage no store
/// ever initialized: the source is inadmissible, yet the mechanical edit
/// still publishes pairs into the join block. The validator's own dominance
/// audit refuses it.
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
            SelectedTerminator::ConditionalBranch {
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
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![definition],
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(21),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(3),
            },
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(22),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(3),
            },
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: uses,
            terminator: tail_terminator,
        });
    });
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), forged(&source))
            .unwrap_err(),
        RuntimeSpillError::UnsupportedUse
    );
}

/// On a legal source the contract demands exactly one transformed function:
/// a proposal that spills nothing, drops a demanded store or reload pair,
/// renames a fresh register, or carries an extra register row is not that
/// function — however it was produced.
#[test]
fn validator_rejects_proposals_that_miss_the_demanded_function() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The unchanged source declares no slot, stores nothing, and reloads
    // nothing — not the demanded function.
    assert_eq!(
        validate_runtime_spill(
            &source,
            0,
            VICTIM,
            &environment,
            budget(),
            source.transformed().clone(),
        )
        .unwrap_err(),
        RuntimeSpillError::ReplayMismatch
    );
    // The demanded definition store is missing.
    let mut dropped_store = forged(&source);
    dropped_store.functions[0].blocks[0].instructions.remove(1);
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), dropped_store)
            .unwrap_err(),
        RuntimeSpillError::ReplayMismatch
    );
    // The demanded reload pair is missing — the first use names a register
    // nothing loads.
    let mut dropped_pair = forged(&source);
    dropped_pair.functions[0].blocks[0]
        .instructions
        .retain(|instruction| instruction.id != SelectedInstructionId(7));
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), dropped_pair)
            .unwrap_err(),
        RuntimeSpillError::ReplayMismatch
    );
    // A fresh register's identity does not match the insertion order.
    let mut renamed = forged(&source);
    renamed.functions[0].virtual_registers[6].id = VirtualRegisterId(90);
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), renamed).unwrap_err(),
        RuntimeSpillError::ReplayMismatch
    );
    // A spare register row the spill never produced.
    let mut spare = forged(&source);
    let mut extra = spare.functions[0].virtual_registers[6].clone();
    extra.id = VirtualRegisterId(91);
    extra.origin = VirtualRegisterOrigin::InstructionResult {
        instruction: SelectedInstructionId(91),
        source_value: ValueId::new(1).unwrap(),
    };
    spare.functions[0].virtual_registers.push(extra);
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), spare).unwrap_err(),
        RuntimeSpillError::ReplayMismatch
    );
    // The slot declaration is missing even though the stream is exact.
    let mut undeclared = forged(&source);
    undeclared.functions[0].local_storage_slots.pop();
    assert_eq!(
        validate_runtime_spill(&source, 0, VICTIM, &environment, budget(), undeclared).unwrap_err(),
        RuntimeSpillError::ReplayMismatch
    );
}
