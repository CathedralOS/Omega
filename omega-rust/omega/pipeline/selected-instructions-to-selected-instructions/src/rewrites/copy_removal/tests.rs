use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::{EffectLink, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedCallContract,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedLocalStorageSlot, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, SelectedValueBinding, SelectedValueTransport,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId,
    ObligationId, OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{
    CrashCause, CrashRouteBucket, CrashRouteGuard, SemanticFingerprint, TerminalPsiIdentity,
    VocabularyMarker,
};

use super::{
    CopyRemovalError, CopyRemovalReceipt, ValidatedCopyRemoval, remove_selected_copy,
    validate_copy_removal,
};
use crate::ValidatedSelectedAnalysis;

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

const LOAD: SelectedInstructionId = SelectedInstructionId(2);
const COPY: SelectedInstructionId = SelectedInstructionId(3);
const CONSUME: SelectedInstructionId = SelectedInstructionId(4);
const TERMINAL: SelectedInstructionId = SelectedInstructionId(5);
const CHAINED: SelectedInstructionId = SelectedInstructionId(6);
const BRANCH: SelectedInstructionId = SelectedInstructionId(7);
const LATE: SelectedInstructionId = SelectedInstructionId(8);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const SOURCE: VirtualRegisterId = VirtualRegisterId(1);
const COPIED: VirtualRegisterId = VirtualRegisterId(2);
const OTHER: VirtualRegisterId = VirtualRegisterId(3);
const SPARE: VirtualRegisterId = VirtualRegisterId(4);

fn register(
    id: VirtualRegisterId,
    class: register_model::RegisterClassId,
    origin: VirtualRegisterOrigin,
) -> VirtualRegister {
    VirtualRegister {
        id,
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        class,
        origin,
        definition_site: None,
        entry_fixed_view: None,
    }
}

fn copy_instruction(
    id: SelectedInstructionId,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    input: VirtualRegisterId,
    output: VirtualRegisterId,
) -> SelectedInstruction {
    let row = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap();
    instruction(id, SelectedInstructionKind::CopyI64, row, &[input, output])
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r1 = load8 r0; r2 = copy r1; compare r2, r1; return`.
fn fixture(target: NativeTarget) -> ValidatedCopyRemoval {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let load = environment.constraint(keys.load8.unwrap()).unwrap();
    let compare = environment.constraint(keys.compare_i64).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = copy.operands[0].class;
    let registers = vec![
        VirtualRegister {
            id: POINTER,
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
        register(
            SOURCE,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: LOAD,
                source_value: ValueId::new(2).unwrap(),
            },
        ),
        register(
            COPIED,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: COPY,
                source_value: ValueId::new(3).unwrap(),
            },
        ),
    ];
    let machine = MachineId::new(1).unwrap();
    let plan = SelectedInstructionPlan {
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
            virtual_registers: registers,
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![
                    instruction(
                        LOAD,
                        SelectedInstructionKind::Load8 { byte_offset: 0 },
                        load,
                        &[POINTER, SOURCE],
                    ),
                    instruction(
                        COPY,
                        SelectedInstructionKind::CopyI64,
                        copy,
                        &[SOURCE, COPIED],
                    ),
                    instruction(
                        CONSUME,
                        SelectedInstructionKind::CompareI64,
                        compare,
                        &[COPIED, SOURCE],
                    ),
                ],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        TERMINAL,
                        SelectedInstructionKind::ReturnUnit,
                        terminal_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        }]
        .into(),
    };
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedCopyRemoval {
        receipt: CopyRemovalReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedCopyRemoval {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(target);
    edit(
        &mut std::sync::Arc::make_mut(&mut source.transformed).functions[0],
        &environment,
    );
    let identity = selected_instruction_plan_identity(&source.transformed);
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

fn remove(
    source: &ValidatedCopyRemoval,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedCopyRemoval, CopyRemovalError> {
    remove_selected_copy(source, 0, COPY, environment, budget())
}

fn settlement(position: u32, operation: u64) -> SelectedBoundarySettlement {
    SelectedBoundarySettlement {
        block: SelectedBlockId(0),
        instruction_index: position,
        settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
            operation: OperationId::new(operation).unwrap(),
            boundary: BoundaryMachineId::new(1).unwrap(),
            source: ValueId::new(9).unwrap(),
        },
    }
}

fn successor(block: u32) -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::Semantic,
        psi_edge: EdgeId::new(2).unwrap(),
        block: SelectedBlockId(block),
        source_target: BlockId::new(u64::from(block + 1)).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        fuel: Vec::new(),
    }
}

/// An empty trailing block for edge-surface fixtures.
fn trailing_block(
    id: u32,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> SelectedBlock {
    let terminal_row = environment
        .constraint(environment.selected_keys().return_unit)
        .unwrap();
    SelectedBlock {
        id: SelectedBlockId(id),
        origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(id + 1)).unwrap()),
        instructions: Vec::new(),
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(20 + id),
                SelectedInstructionKind::ReturnUnit,
                terminal_row,
                &[],
            ),
            psi_return_edge: EdgeId::new(3).unwrap(),
        },
    }
}

/// Every same-block use of the destination rebinds to the source, the copy
/// and its roster row leave together, and the result replays on every target.
#[test]
fn same_block_use_rebinds_and_removes() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = remove(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        let block = &function.blocks[0];
        assert_eq!(block.instructions.len(), 2);
        assert_eq!(block.instructions[0].id, LOAD);
        assert_eq!(block.instructions[1].id, CONSUME);
        assert_eq!(block.instructions[1].operands[0].virtual_register, SOURCE);
        assert_eq!(block.instructions[1].operands[1].virtual_register, SOURCE);
        assert_eq!(function.virtual_registers.len(), 2);
        assert!(
            function
                .virtual_registers
                .iter()
                .all(|entry| entry.id != COPIED)
        );
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        validate_copy_removal(
            &source,
            0,
            COPY,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_copy_removal(&source, 0, COPY, &environment, budget(), detached).unwrap();
    }
}

/// A destination may be read more than once: a chained copy of it and a
/// terminator-carried instruction both rebind, and the chained copy — an
/// instruction the removal does not touch — stays.
#[test]
fn chained_and_terminator_carried_uses_rebind() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let Some(exit) = environment.selected_keys().hosted_exit_process_i32 else {
            continue;
        };
        let exit_row = environment.constraint(exit).unwrap();
        let source = mutated(target, |function, environment| {
            let class = function.virtual_registers[1].class;
            function.virtual_registers.push(register(
                OTHER,
                class,
                VirtualRegisterOrigin::InstructionResult {
                    instruction: CHAINED,
                    source_value: ValueId::new(4).unwrap(),
                },
            ));
            function.blocks[0]
                .instructions
                .insert(2, copy_instruction(CHAINED, environment, COPIED, OTHER));
            let terminator = std::mem::replace(
                &mut function.blocks[0].terminator,
                SelectedTerminator::Return {
                    instruction: instruction(
                        TERMINAL,
                        SelectedInstructionKind::ReturnUnit,
                        environment
                            .constraint(environment.selected_keys().return_unit)
                            .unwrap(),
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            );
            let SelectedTerminator::Return { .. } = terminator else {
                unreachable!()
            };
            function.blocks[0].terminator = SelectedTerminator::HostedExitProcess {
                instruction: instruction(
                    SelectedInstructionId(9),
                    SelectedInstructionKind::HostedExitProcessI32,
                    exit_row,
                    &[COPIED],
                ),
                nominal_return_edge: EdgeId::new(4).unwrap(),
            };
        });
        let result = remove(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        let block = &function.blocks[0];
        assert_eq!(block.instructions.len(), 3);
        // load8; chained copy now reading the source directly; the compare.
        assert_eq!(block.instructions[0].id, LOAD);
        assert_eq!(block.instructions[1].id, CHAINED);
        assert_eq!(block.instructions[1].operands[0].virtual_register, SOURCE);
        assert_eq!(block.instructions[2].id, CONSUME);
        assert_eq!(block.instructions[2].operands[0].virtual_register, SOURCE);
        let SelectedTerminator::HostedExitProcess { instruction, .. } = &block.terminator else {
            panic!("terminator kind changed")
        };
        assert_eq!(instruction.operands[0].virtual_register, SOURCE);
        // The chained copy's own result survives: only the destination row left.
        assert!(
            function
                .virtual_registers
                .iter()
                .any(|entry| entry.id == OTHER)
        );
        assert!(
            function
                .virtual_registers
                .iter()
                .all(|entry| entry.id != COPIED)
        );
        validate_copy_removal(
            &source,
            0,
            COPY,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// A source redefinition strictly between the copy and the last destination
/// use would leave later reads seeing the new value — it rejects — while the
/// same definition on the last use's own instruction, or after it, cannot
/// disturb the read and is admitted.
#[test]
fn source_redefinition_interval() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A redefinition strictly inside the interval rejects.
    let intervening = mutated(target, |function, environment| {
        function.blocks[0]
            .instructions
            .insert(2, copy_instruction(LATE, environment, POINTER, SOURCE));
    });
    assert_eq!(
        remove(&intervening, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
    // After the last use it cannot disturb the copied value.
    let late = mutated(target, |function, environment| {
        function.blocks[0]
            .instructions
            .push(copy_instruction(LATE, environment, POINTER, SOURCE));
    });
    let result = remove(&late, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0].instructions[1].operands[0].virtual_register,
        SOURCE
    );
    // On the last use's own instruction the read still precedes the write:
    // a consumer that defines the source while reading the destination.
    let self_defining = mutated(target, |function, environment| {
        function.blocks[0].instructions[2] = copy_instruction(CONSUME, environment, COPIED, SOURCE);
    });
    let result = remove(&self_defining, &environment).unwrap();
    let consumer = &result.transformed().functions[0].blocks[0].instructions[1];
    assert_eq!(consumer.operands[0].virtual_register, SOURCE);
    assert_eq!(consumer.operands[1].virtual_register, SOURCE);
}

/// Boundary settlements follow the removed ordinal: positions at or before
/// the copy stay put, every later position — including the after-body one —
/// shifts one earlier, and a settlement on another block is untouched.
#[test]
fn boundary_settlements_shift_over_the_removed_copy() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        function.boundary_settlements = vec![
            settlement(1, 7),
            settlement(2, 8),
            settlement(3, 9),
            SelectedBoundarySettlement {
                block: SelectedBlockId(1),
                instruction_index: 2,
                settlement: settlement(2, 10).settlement,
            },
        ];
        function.blocks.push(trailing_block(1, environment));
    });
    let result = remove(&source, &environment).unwrap();
    let settlements = &result.transformed().functions[0].boundary_settlements;
    assert_eq!(settlements.len(), 4);
    assert_eq!(settlements[0].instruction_index, 1);
    assert_eq!(settlements[1].instruction_index, 1);
    assert_eq!(settlements[2].instruction_index, 2);
    assert_eq!(settlements[3].block, SelectedBlockId(1));
    assert_eq!(settlements[3].instruction_index, 2);
    validate_copy_removal(
        &source,
        0,
        COPY,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// Every mention of the destination outside an admitted same-block `Use`
/// operand — an earlier position, another block, a write access, operand
/// constraints, or an edge transport — refuses.
#[test]
fn destination_mentions_outside_the_admitted_surface_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A read before the copy is a stale read, not a substitution candidate.
    let early_read = mutated(target, |function, _| {
        function.blocks[0].instructions[0].operands[0].virtual_register = COPIED;
    });
    assert_eq!(
        remove(&early_read, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
    // A read in another block is outside the same-block contract.
    let other_block = mutated(target, |function, environment| {
        let class = function.virtual_registers[1].class;
        function.virtual_registers.push(register(
            SPARE,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: LATE,
                source_value: ValueId::new(5).unwrap(),
            },
        ));
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                BRANCH,
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch_row,
                &[],
            ),
            when_nonzero: successor(1),
            when_zero: successor(1),
        };
        let mut block = trailing_block(1, environment);
        block
            .instructions
            .push(copy_instruction(LATE, environment, COPIED, SPARE));
        function.blocks.push(block);
    });
    assert_eq!(
        remove(&other_block, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
    // A `UseDef` on the destination is a second definition.
    let rewriting_use = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[0].access =
            register_model::RegisterOperandAccess::UseDef;
    });
    assert_eq!(
        remove(&rewriting_use, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
    // A plain second definition anywhere refuses.
    let second_definition = mutated(target, |function, environment| {
        function.blocks[0]
            .instructions
            .insert(2, copy_instruction(LATE, environment, POINTER, COPIED));
    });
    assert_eq!(
        remove(&second_definition, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
    // Allocation-shaped use operands stay outside the substitution surface.
    for (name, edit) in [
        (
            "fixed view",
            Box::new(|operand: &mut SelectedOperand| {
                operand.fixed_view = Some(register_model::RegisterViewId(0));
            }) as Box<dyn FnOnce(&mut SelectedOperand)>,
        ),
        (
            "tied operand",
            Box::new(|operand: &mut SelectedOperand| operand.tied_to = Some(0)),
        ),
        (
            "early clobber",
            Box::new(|operand: &mut SelectedOperand| operand.early_clobber = true),
        ),
        (
            "foreign class",
            Box::new(|operand: &mut SelectedOperand| {
                operand.class = register_model::RegisterClassId(u16::MAX);
            }),
        ),
    ] {
        let edited = mutated(target, |function, _| {
            edit(&mut function.blocks[0].instructions[2].operands[0]);
        });
        assert_eq!(
            remove(&edited, &environment).unwrap_err(),
            CopyRemovalError::UnsupportedUse,
            "{name}"
        );
    }
    // A copy whose destination nobody reads is dead code, not this family.
    let unread = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[0].virtual_register = SOURCE;
    });
    assert_eq!(
        remove(&unread, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
}

/// A destination named by an edge transport, a storage slot, a memory-access
/// row, or another register's origin loses its referent when the row leaves:
/// each refuses.
#[test]
fn edge_and_storage_mentions_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    // An edge binding reads the register on the edge.
    let edge_argument = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let mut edge = successor(1);
        edge.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(6).unwrap(),
                argument: ValueId::new(3).unwrap(),
                scalar_type,
            },
            transport: SelectedValueTransport::Registers {
                argument: COPIED,
                parameter: SPARE,
            },
        });
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(BRANCH, SelectedInstructionKind::Jump, jump_row, &[]),
            successor: edge,
        };
        function.blocks.push(trailing_block(1, environment));
    });
    assert_eq!(
        remove(&edge_argument, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
    // An edge binding's parameter slot defines the register on the edge.
    let edge_parameter = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let mut edge = successor(1);
        edge.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(3).unwrap(),
                argument: ValueId::new(6).unwrap(),
                scalar_type,
            },
            transport: SelectedValueTransport::Registers {
                argument: POINTER,
                parameter: COPIED,
            },
        });
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(BRANCH, SelectedInstructionKind::Jump, jump_row, &[]),
            successor: edge,
        };
        function.blocks.push(trailing_block(1, environment));
    });
    assert_eq!(
        remove(&edge_parameter, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
    // The destination's own spill slot names it directly.
    let spill_slot = mutated(target, |function, _| {
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: LocalStorageSlotId::Spill { register: COPIED },
            byte_size: 8,
            alignment: 8,
        });
    });
    assert_eq!(
        remove(&spill_slot, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
    // A memory-access row whose local slot names the destination.
    let access_slot = mutated(target, |function, _| {
        function.memory_accesses.push(SelectedMemoryAccess {
            instruction: CONSUME,
            origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(11).unwrap()),
            place: PlaceId::new(1).unwrap(),
            byte_offset: 0,
            byte_count: 8,
            role: SelectedMemoryAccessRole::WriteLocal {
                slot: LocalStorageSlotId::Spill { register: COPIED },
            },
        });
    });
    assert_eq!(
        remove(&access_slot, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
    // An instruction-carried frame slot naming the destination.
    let frame_slot = mutated(target, |function, _| {
        function.blocks[0].instructions[2].kind = SelectedInstructionKind::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Local(LocalStorageSlotId::Spill {
                register: COPIED,
            }),
            byte_offset: 0,
        };
    });
    assert_eq!(
        remove(&frame_slot, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
    // Another register's `SpillAddress` origin binds the destination's
    // storage; a second register claiming the copy as its producer is a
    // second product of the removed instruction.
    for (name, origin) in [
        (
            "spill address",
            VirtualRegisterOrigin::SpillAddress {
                instruction: CONSUME,
                register: COPIED,
            },
        ),
        (
            "claimed copy product",
            VirtualRegisterOrigin::InstructionResult {
                instruction: COPY,
                source_value: ValueId::new(6).unwrap(),
            },
        ),
    ] {
        let edited = mutated(target, |function, _| {
            let class = function.virtual_registers[1].class;
            function
                .virtual_registers
                .push(register(SPARE, class, origin));
        });
        assert_eq!(
            remove(&edited, &environment).unwrap_err(),
            CopyRemovalError::UnsupportedUse,
            "{name}"
        );
    }
}

/// The copy must be the target's plain `[use, def]` instruction: another
/// kind, a third operand, a writing input, operand constraints, or any
/// implicit unit surface refuses.
#[test]
fn copy_instruction_shape_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let not_copy = mutated(target, |function, _| {
        function.blocks[0].instructions[1].kind = SelectedInstructionKind::MaterializeI64 {
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
        };
    });
    assert_eq!(
        remove(&not_copy, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedInstruction
    );
    let extra_operand = mutated(target, |function, _| {
        let operand = function.blocks[0].instructions[1].operands[1];
        function.blocks[0].instructions[1].operands.push(operand);
    });
    assert_eq!(
        remove(&extra_operand, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedInstruction
    );
    let writing_input = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].access =
            register_model::RegisterOperandAccess::Def;
    });
    assert_eq!(
        remove(&writing_input, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedInstruction
    );
    for (name, edit) in [
        (
            "fixed view",
            Box::new(|operand: &mut SelectedOperand| {
                operand.fixed_view = Some(register_model::RegisterViewId(0));
            }) as Box<dyn FnOnce(&mut SelectedOperand)>,
        ),
        (
            "tied operand",
            Box::new(|operand: &mut SelectedOperand| operand.tied_to = Some(0)),
        ),
        (
            "early clobber",
            Box::new(|operand: &mut SelectedOperand| operand.early_clobber = true),
        ),
    ] {
        let edited = mutated(target, |function, _| {
            edit(&mut function.blocks[0].instructions[1].operands[1]);
        });
        assert_eq!(
            remove(&edited, &environment).unwrap_err(),
            CopyRemovalError::UnsupportedInstruction,
            "{name}"
        );
    }
    for (name, field) in [
        ("implicit use", 0usize),
        ("implicit def", 1usize),
        ("clobber", 2usize),
    ] {
        let edited = mutated(target, |function, _| {
            let instruction = &mut function.blocks[0].instructions[1];
            match field {
                0 => instruction
                    .implicit_uses
                    .push(register_model::RegisterUnitId(0)),
                1 => instruction
                    .implicit_defs
                    .push(register_model::RegisterUnitId(0)),
                _ => instruction.clobbers.push(register_model::RegisterUnitId(0)),
            }
        });
        assert_eq!(
            remove(&edited, &environment).unwrap_err(),
            CopyRemovalError::UnsupportedInstruction,
            "{name}"
        );
    }
}

/// The register contract itself: self-copy, missing roster rows, an origin
/// claiming another surface, a live-in fixed view, or a register shape the
/// copy cannot preserve.
#[test]
fn register_contract_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let self_copy = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[1].virtual_register = SOURCE;
    });
    assert_eq!(
        remove(&self_copy, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedRegister
    );
    let missing_output = mutated(target, |function, _| {
        function
            .virtual_registers
            .retain(|entry| entry.id != COPIED);
    });
    assert_eq!(
        remove(&missing_output, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedRegister
    );
    let missing_input = mutated(target, |function, _| {
        function
            .virtual_registers
            .retain(|entry| entry.id != SOURCE);
    });
    assert_eq!(
        remove(&missing_input, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedRegister
    );
    for (name, edit) in [
        (
            "entry parameter origin",
            Box::new(|register: &mut VirtualRegister| {
                register.origin = VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(3).unwrap(),
                    parameter_index: 0,
                };
            }) as Box<dyn FnOnce(&mut VirtualRegister)>,
        ),
        (
            "scratch of another operand",
            Box::new(|register: &mut VirtualRegister| {
                register.origin = VirtualRegisterOrigin::InstructionScratch {
                    instruction: COPY,
                    operand: 0,
                };
            }),
        ),
        (
            "scratch of another instruction",
            Box::new(|register: &mut VirtualRegister| {
                register.origin = VirtualRegisterOrigin::InstructionScratch {
                    instruction: CONSUME,
                    operand: 1,
                };
            }),
        ),
        (
            "live-in fixed view",
            Box::new(|register: &mut VirtualRegister| {
                register.entry_fixed_view = Some(register_model::RegisterViewId(0));
            }),
        ),
        (
            "foreign scalar type",
            Box::new(|register: &mut VirtualRegister| {
                register.scalar_type = ScalarType::Boolean;
            }),
        ),
        (
            "foreign class",
            Box::new(|register: &mut VirtualRegister| {
                register.class = register_model::RegisterClassId(u16::MAX);
            }),
        ),
    ] {
        let edited = mutated(target, |function, _| {
            let index = function
                .virtual_registers
                .iter()
                .position(|entry| entry.id == COPIED)
                .unwrap();
            edit(&mut function.virtual_registers[index]);
        });
        assert_eq!(
            remove(&edited, &environment).unwrap_err(),
            CopyRemovalError::UnsupportedRegister,
            "{name}"
        );
    }
}

/// The declared row must be the plain `[use, def]` the operands carry.
#[test]
fn constraint_row_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let wrong_shape = mutated(target, |function, environment| {
        function.blocks[0].instructions[1].constraint = environment.selected_keys().compare_i64;
    });
    assert_eq!(
        remove(&wrong_shape, &environment).unwrap_err(),
        CopyRemovalError::ConstraintMismatch
    );
    let unknown_row = mutated(target, |function, _| {
        function.blocks[0].instructions[1].constraint = register_model::RegisterConstraintKey {
            family: register_model::RegisterConstraintFamily::Instruction,
            variant: u32::MAX,
        };
    });
    assert_eq!(
        remove(&unknown_row, &environment).unwrap_err(),
        CopyRemovalError::ConstraintMismatch
    );
    // A roster class the row does not declare: both registers carry the same
    // foreign class, so only the row comparison can observe it.
    let wrong_class = mutated(target, |function, _| {
        function.virtual_registers[1].class = register_model::RegisterClassId(u16::MAX);
        function.virtual_registers[2].class = register_model::RegisterClassId(u16::MAX);
    });
    assert_eq!(
        remove(&wrong_class, &environment).unwrap_err(),
        CopyRemovalError::ConstraintMismatch
    );
}

/// Roster rows that would orphan the removed instruction — a call contract
/// or a memory-access row naming it — refuse.
#[test]
fn instruction_surface_rows_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let memory_row = mutated(target, |function, _| {
        function.memory_accesses.push(SelectedMemoryAccess {
            instruction: COPY,
            origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(11).unwrap()),
            place: PlaceId::new(1).unwrap(),
            byte_offset: 0,
            byte_count: 8,
            role: SelectedMemoryAccessRole::ReadPlace,
        });
    });
    assert_eq!(
        remove(&memory_row, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedInstruction
    );
    let call_row = mutated(target, |function, _| {
        function.calls.push(SelectedCallContract {
            instruction: COPY,
            operation: OperationId::new(41).unwrap(),
            call: legalized_operations::LegalizedScalarCall {
                source: legalized_operations::NativeCallOrigin::Authored,
                callee: MachineId::new(42).unwrap(),
                call_plan: calling_conventions::CallPlan {
                    policy: calling_conventions::CallingPolicy::MicrosoftX64,
                    parameters: Vec::new(),
                    result: None,
                    callback_materializations: Vec::new(),
                    ordinary_clobbers: calling_conventions::RegisterSet::new(std::iter::empty()),
                    stack_alignment: 16,
                    shadow_bytes: 0,
                    entry_control: calling_conventions::EntryControl::CallReturn,
                },
                arguments: Vec::new(),
                result_placement: None,
                structural_result: None,
                claim_transfers: Vec::new(),
                requirement_obligations: vec![ObligationId::new(43).unwrap()],
                crash_continuations: vec![CrashRouteBucket {
                    cause: CrashCause::Trap,
                    alternatives: vec![CrashRouteGuard::Truth],
                }],
            },
            effect: EffectLink {
                input: 0,
                output: 0,
            },
            ownership: Vec::new(),
        });
    });
    assert_eq!(
        remove(&call_row, &environment).unwrap_err(),
        CopyRemovalError::UnsupportedInstruction
    );
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        remove_selected_copy(&source, 1, COPY, &environment, budget()).unwrap_err(),
        CopyRemovalError::SourceMismatch
    );
    assert_eq!(
        remove_selected_copy(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        CopyRemovalError::SourceMismatch
    );
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        remove_selected_copy(&source, 0, COPY, &wrong_environment, budget()).unwrap_err(),
        CopyRemovalError::SourceMismatch
    );
}

#[test]
fn validation_budget_covers_the_mention_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let tiny = OptimizationWorkBudget::new(100, 100, 4, 100, 100).unwrap();
    assert_eq!(
        remove_selected_copy(&source, 0, COPY, &environment, tiny).unwrap_err(),
        CopyRemovalError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, a second scan of the
/// admitted function's blocks that also counts each successor transport,
/// and one step per roster, storage, access, and call row — eleven steps
/// for the single-block fixture, fifteen once the copy's block carries an
/// edge transport into a second block — so the exact count admits the
/// removal on both the proposal and the independent replay path while one
/// step below rejects both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second block behind a register-transport edge: one binding on the
    // copy's own block, one extra roster row, and one more block in both
    // scans lift the measured count to fifteen.
    let carried = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let mut edge = successor(1);
        edge.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(6).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            transport: SelectedValueTransport::Registers {
                argument: POINTER,
                parameter: SPARE,
            },
        });
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(BRANCH, SelectedInstructionKind::Jump, jump_row, &[]),
            successor: edge,
        };
        function.virtual_registers.push(register(
            SPARE,
            function.virtual_registers[1].class,
            VirtualRegisterOrigin::BlockParameter {
                source_value: ValueId::new(6).unwrap(),
                block: SelectedBlockId(1),
                parameter_index: 0,
            },
        ));
        function.blocks.push(trailing_block(1, environment));
    });
    for (source, exact_steps) in [(fixture(target), 11u64), (carried, 15u64)] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = remove_selected_copy(&source, 0, COPY, &environment, exact).unwrap();
        validate_copy_removal(
            &source,
            0,
            COPY,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            remove_selected_copy(&source, 0, COPY, &environment, starved).unwrap_err(),
            CopyRemovalError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_copy_removal(
                &source,
                0,
                COPY,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            CopyRemovalError::WorkBudgetExceeded
        );
    }
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input: re-running at
/// the same site is terminal because the copy no longer exists, and a
/// surviving chained copy whose destination nobody reads is dead code this
/// rule does not admit — removal substitutes uses, it does not erase
/// unread instructions.
#[test]
fn removal_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = remove(&fixture(target), &environment).unwrap();
    let second = remove(&fixture(target), &environment).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one.
    assert_eq!(
        remove_selected_copy(&first, 0, COPY, &environment, budget()).unwrap_err(),
        CopyRemovalError::SourceMismatch
    );
    // The chained copy still stands after the first removal, now reading the
    // source directly; with no remaining readers of its own destination the
    // second run finds no admitted use and stops.
    let chained_source = mutated(target, |function, environment| {
        let class = function.virtual_registers[1].class;
        function.virtual_registers.push(register(
            OTHER,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: CHAINED,
                source_value: ValueId::new(4).unwrap(),
            },
        ));
        function.blocks[0]
            .instructions
            .insert(2, copy_instruction(CHAINED, environment, COPIED, OTHER));
    });
    let first = remove(&chained_source, &environment).unwrap();
    assert_eq!(
        remove_selected_copy(&first, 0, CHAINED, &environment, budget()).unwrap_err(),
        CopyRemovalError::UnsupportedUse
    );
}

/// Replay corruption in a block the removal never touched still rejects:
/// the restore-by-content check compares the complete plan, not just the
/// block the copy left.
#[test]
fn replay_rejects_drift_outside_the_rewritten_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Stretch the fixture across one edge: the copy's block jumps to a
    // second block that returns.
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let tail = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(BRANCH, SelectedInstructionKind::Jump, jump_row, &[]),
                successor: successor(1),
            },
        );
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: tail,
        });
    });
    let result = remove(&source, &environment).unwrap();
    assert_eq!(result.transformed().functions[0].blocks.len(), 2);
    // An extra instruction in the untouched landing block rejects.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[1]
        .instructions
        .push(copy_instruction(
            SelectedInstructionId(30),
            &environment,
            POINTER,
            SOURCE,
        ));
    assert_eq!(
        validate_copy_removal(&source, 0, COPY, &environment, budget(), proposed).unwrap_err(),
        CopyRemovalError::ReplayMismatch
    );
    // Drift in the untouched block's terminator rejects.
    let mut proposed = result.transformed().clone();
    let SelectedTerminator::Return {
        instruction: return_instruction,
        ..
    } = &mut proposed.functions[0].blocks[1].terminator
    else {
        unreachable!()
    };
    return_instruction.id = SelectedInstructionId(31);
    assert_eq!(
        validate_copy_removal(&source, 0, COPY, &environment, budget(), proposed).unwrap_err(),
        CopyRemovalError::ReplayMismatch
    );
    // A phantom trailing block rejects.
    let mut proposed = result.transformed().clone();
    proposed.functions[0]
        .blocks
        .push(trailing_block(2, &environment));
    assert_eq!(
        validate_copy_removal(&source, 0, COPY, &environment, budget(), proposed).unwrap_err(),
        CopyRemovalError::ReplayMismatch
    );
}

#[test]
fn replay_rejects_anything_but_the_independent_function() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = remove(&source, &environment).unwrap();
    for mutation in 0..10 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The consumer's operand never rebound to the source.
            0 => {
                function.blocks[0].instructions[1].operands[0].virtual_register = COPIED;
            }
            // Rebound to a different register.
            1 => {
                function.blocks[0].instructions[1].operands[0].virtual_register = POINTER;
            }
            // The destination row stayed in the roster.
            2 => {
                let class = function.virtual_registers[1].class;
                function.virtual_registers.push(register(
                    COPIED,
                    class,
                    VirtualRegisterOrigin::InstructionResult {
                        instruction: COPY,
                        source_value: ValueId::new(3).unwrap(),
                    },
                ));
            }
            // The copy instruction stayed in the block.
            3 => {
                let row = environment
                    .constraint(environment.selected_keys().copy_i64)
                    .unwrap();
                function.blocks[0].instructions.insert(
                    1,
                    instruction(
                        COPY,
                        SelectedInstructionKind::CopyI64,
                        row,
                        &[SOURCE, COPIED],
                    ),
                );
            }
            // An unrelated instruction removed.
            4 => {
                function.blocks[0].instructions.remove(0);
            }
            // An unrelated register's scalar surface.
            5 => function.virtual_registers[1].scalar_type = ScalarType::Boolean,
            // A fabricated settlement has no source counterpart.
            6 => function.boundary_settlements.push(settlement(2, 11)),
            // The terminator's carried instruction is part of the function.
            7 => {
                let SelectedTerminator::Return { instruction, .. } =
                    &mut function.blocks[0].terminator
                else {
                    unreachable!()
                };
                instruction.kind = SelectedInstructionKind::ReturnScalar;
            }
            // The surviving instructions keep their provenance.
            8 => {
                function.blocks[0].instructions[0]
                    .provenance
                    .values
                    .push(ValueId::new(9).unwrap());
            }
            // A plan-level field is outside the function comparison but
            // inside the restored-source check.
            9 => proposed.entry = MachineId::new(2).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            validate_copy_removal(&source, 0, COPY, &environment, budget(), proposed).is_err(),
            "mutation {mutation}"
        );
    }
}
