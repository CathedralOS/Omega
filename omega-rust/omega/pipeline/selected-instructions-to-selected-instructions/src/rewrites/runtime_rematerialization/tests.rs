use super::Arc;
use crate::RuntimeRematerializationError;
use crate::RuntimeRematerializationReceipt;
use crate::ValidatedRuntimeRematerialization;
use crate::rematerialize_selected_runtime_value;
use crate::validate_runtime_rematerialization;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedOperand, SelectedSuccessor, SelectedSuccessorRole,
    SelectedTerminator, SelectedValueBinding, SelectedValueTransport, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, OperationId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap()
}

/// Build an instruction from a validated target row, as selection and the
/// sibling rewrites do: the row supplies the complete operand interface.
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

/// A raw selected-stage unit fixture, not a source/Terminal admission claim.
/// Register 1 is defined by one `MaterializeI64` and consumed by three copies.
fn fixture(target: NativeTarget) -> ValidatedRuntimeRematerialization {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let source_value = ValueId::new(1).unwrap();
    let mut registers = vec![
        VirtualRegister {
            id: VirtualRegisterId(0),
            scalar_type,
            class: copy.operands[0].class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value,
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
        VirtualRegister {
            id: VirtualRegisterId(1),
            scalar_type,
            class: materialize.operands[0].class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(1),
                source_value,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
    ];
    let mut instructions = vec![instruction(
        SelectedInstructionId(1),
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(7),
        },
        materialize,
        &[VirtualRegisterId(1)],
    )];
    for ordinal in 2..=4u32 {
        let instruction_id = SelectedInstructionId(ordinal);
        let register = VirtualRegisterId(ordinal);
        registers.push(VirtualRegister {
            id: register,
            scalar_type,
            class: copy.operands[1].class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: instruction_id,
                source_value,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        });
        instructions.push(instruction(
            instruction_id,
            SelectedInstructionKind::CopyI64,
            copy,
            &[VirtualRegisterId(1), register],
        ));
    }
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
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
                origin: selected_instructions::SelectedBlockOrigin::Source(
                    BlockId::new(1).unwrap(),
                ),
                instructions,
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        SelectedInstructionId(5),
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
    ValidatedRuntimeRematerialization {
        receipt: RuntimeRematerializationReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: Arc::new(plan),
    }
}

#[test]
fn every_flexible_use_regenerates_the_same_immediate() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = fixture(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let result = rematerialize_selected_runtime_value(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
        )
        .unwrap();
        let original = &source.transformed().functions[0];
        let function = &result.transformed().functions[0];
        // No private storage is created; the victim keeps its original dead
        // definition and gains one fresh register per use.
        assert!(function.local_storage_slots.is_empty());
        assert_eq!(function.virtual_registers.len(), 8);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 7);
        assert_eq!(instructions[0], original.blocks[0].instructions[0]);
        for (position, expected) in [(1usize, 1usize), (3, 2), (5, 3)] {
            let inserted = &instructions[position];
            assert!(matches!(
                inserted.kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(7)
                }
            ));
            let consumer = &instructions[position + 1];
            assert_eq!(consumer.id, original.blocks[0].instructions[expected].id);
            let fresh = consumer.operands[0].virtual_register;
            assert_eq!(inserted.operands[0].virtual_register, fresh);
            assert_ne!(fresh, VirtualRegisterId(1));
            assert_eq!(inserted.provenance.values, vec![ValueId::new(1).unwrap()]);
            assert!(inserted.provenance.operations.is_empty());
            assert!(inserted.provenance.fuel.is_empty());
        }
        for fresh in &function.virtual_registers[5..] {
            assert_eq!(fresh.scalar_type, original.virtual_registers[1].scalar_type);
            assert_eq!(fresh.class, original.virtual_registers[1].class);
            assert_eq!(
                fresh.definition_site,
                original.virtual_registers[1].definition_site
            );
            assert!(fresh.entry_fixed_view.is_none());
            assert!(matches!(
                fresh.origin,
                VirtualRegisterOrigin::InstructionResult { source_value, .. }
                    if source_value == ValueId::new(1).unwrap()
            ));
        }
        assert!(
            validate_runtime_rematerialization(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone()
            )
            .is_ok()
        );
    }
}

#[test]
fn terminator_operands_regenerate_at_block_end_and_keep_fixed_views() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let mut source = fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            let instruction = instruction(
                SelectedInstructionId(2000),
                SelectedInstructionKind::ReturnScalar,
                environment.constraint(keys.return_i64).unwrap(),
                &[VirtualRegisterId(1)],
            );
            assert!(instruction.operands[0].fixed_view.is_some());
            function.blocks[0].terminator = SelectedTerminator::Return {
                instruction,
                psi_return_edge: EdgeId::new(1).unwrap(),
            };
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let result = rematerialize_selected_runtime_value(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
        )
        .unwrap();
        let function = &result.transformed().functions[0];
        let block = &function.blocks[0];
        // Three copy uses plus the terminator operand: four regenerations, the
        // last appended after the final body instruction.
        assert_eq!(block.instructions.len(), 8);
        let tail = &block.instructions[7];
        assert!(matches!(
            tail.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(7)
            }
        ));
        let terminal = super::super::runtime_spill::control(&block.terminator)
            .0
            .operands[0];
        let fresh = tail.operands[0].virtual_register;
        assert_eq!(terminal.virtual_register, fresh);
        assert!(terminal.fixed_view.is_some());
        assert!(
            validate_runtime_rematerialization(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone()
            )
            .is_ok()
        );
    }
}

#[test]
fn independent_replay_rejects_regeneration_and_lineage_corruption() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let result = rematerialize_selected_runtime_value(
        &source,
        0,
        VirtualRegisterId(1),
        &environment,
        budget(),
    )
    .unwrap();
    for mutation in 0..11 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            0 => {
                function.blocks[0].instructions[1].kind = SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(8),
                }
            }
            1 => {
                function.blocks[0].instructions[1].operands[0].virtual_register =
                    VirtualRegisterId(0)
            }
            2 => {
                function.blocks[0].instructions[2].operands[0].virtual_register =
                    VirtualRegisterId(1)
            }
            3 => function.blocks[0].instructions.swap(0, 1),
            4 => {
                function.virtual_registers[5].scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
            }
            5 => {
                function.virtual_registers[5].origin = VirtualRegisterOrigin::SpillAddress {
                    instruction: SelectedInstructionId(6),
                    register: VirtualRegisterId(0),
                }
            }
            6 => {
                function.virtual_registers[5].definition_site =
                    Some(ValueDefinitionSite::FunctionParameter(1))
            }
            7 => {
                function.virtual_registers[5].entry_fixed_view =
                    Some(register_model::RegisterViewId(0))
            }
            8 => {
                function.blocks[0].instructions.remove(1);
            }
            9 => {
                function.blocks[0].instructions[1]
                    .provenance
                    .operations
                    .push(semantic_vocabulary::OperationId::new(1).unwrap());
            }
            10 => {
                function.blocks[0].instructions[1].provenance.fuel.push(
                    optimization_unit::FuelSettlement {
                        site: optimization_unit::PsiProvenance::Operation(
                            semantic_vocabulary::OperationId::new(1).unwrap(),
                        ),
                        units: 1,
                    },
                );
            }
            _ => unreachable!(),
        }
        assert!(
            validate_runtime_rematerialization(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                proposed
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn non_materialize_definitions_and_fixed_uses_gain_no_regeneration() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let source = fixture(NativeTarget::linux_x64());
    let foreign_environment =
        baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        rematerialize_selected_runtime_value(
            &source,
            0,
            VirtualRegisterId(1),
            &foreign_environment,
            budget()
        )
        .unwrap_err(),
        RuntimeRematerializationError::SourceMismatch
    );
    // An entry parameter has no instruction definition to repeat.
    assert_eq!(
        rematerialize_selected_runtime_value(
            &source,
            0,
            VirtualRegisterId(0),
            &environment,
            budget()
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedValue
    );
    // A copy result is a real runtime value but not a cheap regenerable one.
    assert_eq!(
        rematerialize_selected_runtime_value(
            &source,
            0,
            VirtualRegisterId(2),
            &environment,
            budget()
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedValue
    );
    let tiny = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        rematerialize_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, tiny)
            .unwrap_err(),
        RuntimeRematerializationError::WorkBudgetExceeded
    );
    let mut fixed = source.clone();
    Arc::make_mut(&mut fixed.transformed).functions[0].blocks[0].instructions[1].operands[0]
        .fixed_view = Some(register_model::RegisterViewId(0));
    assert_eq!(
        rematerialize_selected_runtime_value(
            &fixed,
            0,
            VirtualRegisterId(1),
            &environment,
            budget()
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedUse
    );
    let mut entry_pinned = source.clone();
    Arc::make_mut(&mut entry_pinned.transformed).functions[0].virtual_registers[1]
        .entry_fixed_view = Some(register_model::RegisterViewId(0));
    assert_eq!(
        rematerialize_selected_runtime_value(
            &entry_pinned,
            0,
            VirtualRegisterId(1),
            &environment,
            budget()
        )
        .unwrap_err(),
        RuntimeRematerializationError::UnsupportedValue
    );
}

fn successor(block: u32) -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::Semantic,
        psi_edge: EdgeId::new(2).unwrap(),
        block: SelectedBlockId(block),
        source_target: BlockId::new(u64::from(block) + 1).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        fuel: Vec::new(),
    }
}

/// Edit the single fixture function, then refresh the receipt identities so
/// the mutated plan is a well-formed analysis source.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedRuntimeRematerialization {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(target);
    edit(
        &mut Arc::make_mut(&mut source.transformed).functions[0],
        &environment,
    );
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

fn rematerialize(
    source: &ValidatedRuntimeRematerialization,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedRuntimeRematerialization, RuntimeRematerializationError> {
    rematerialize_selected_runtime_value(source, 0, VirtualRegisterId(1), environment, budget())
}

/// The fixture stretched across one edge: block 0 keeps the definition and
/// the first copy; the remaining two copies move to its dominated successor.
fn spread(target: NativeTarget) -> ValidatedRuntimeRematerialization {
    mutated(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let tail_instructions = function.blocks[0].instructions.split_off(2);
        let tail_terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(20),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(1),
            },
        );
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: selected_instructions::SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: tail_instructions,
            terminator: tail_terminator,
        });
    })
}

/// A use in a block the definition block dominates regenerates locally: each
/// use block receives its own fresh materializations immediately before the
/// consuming instructions.
#[test]
fn dominated_successor_uses_regenerate_in_their_own_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = spread(target);
    let result = rematerialize(&source, &environment).unwrap();
    let function = &result.transformed().functions[0];
    // Block 0 regenerates for its own use only.
    let head = &function.blocks[0].instructions;
    assert_eq!(head.len(), 3);
    assert!(matches!(
        head[1].kind,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(7)
        }
    ));
    assert_eq!(head[2].id, SelectedInstructionId(2));
    assert_eq!(
        head[2].operands[0].virtual_register,
        head[1].operands[0].virtual_register
    );
    // Block 1's two dominated uses each regenerate in that block.
    let tail = &function.blocks[1].instructions;
    assert_eq!(tail.len(), 4);
    for pair in [0usize, 2] {
        assert!(matches!(
            tail[pair].kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(7)
            }
        ));
        assert_eq!(
            tail[pair + 1].operands[0].virtual_register,
            tail[pair].operands[0].virtual_register
        );
    }
    assert_eq!(tail[1].id, SelectedInstructionId(3));
    assert_eq!(tail[3].id, SelectedInstructionId(4));
    validate_runtime_rematerialization(
        &source,
        0,
        VirtualRegisterId(1),
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A use block the definition cannot dominate — or a use riding an outgoing
/// edge transport — gains no regeneration: regenerating there would read the
/// victim where no definition ever ran, or feed an edge the rewrite does not
/// admit.
#[test]
fn undominated_uses_and_outgoing_transports_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The definition moves to one leg of a fork; the copies stay in the join
    // block, reachable through the leg that never ran the definition.
    let forked = mutated(target, |function, environment| {
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
            origin: selected_instructions::SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
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
            origin: selected_instructions::SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
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
            origin: selected_instructions::SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: uses,
            terminator: tail_terminator,
        });
    });
    assert_eq!(
        rematerialize(&forked, &environment).unwrap_err(),
        RuntimeRematerializationError::UnsupportedUse
    );
    // A `Registers` value binding carrying the victim out of a block is a
    // transport the rule does not admit.
    let carried = mutated(target, |function, environment| {
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
                argument: VirtualRegisterId(1),
                parameter: VirtualRegisterId(0),
            },
        });
        let tail_terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(20),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: edge,
            },
        );
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: selected_instructions::SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: tail_terminator,
        });
    });
    assert_eq!(
        rematerialize(&carried, &environment).unwrap_err(),
        RuntimeRematerializationError::UnsupportedUse
    );
    // An output tied to the use position would extend the fresh register's
    // value identity into an allocation-shaped pair.
    let tied = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[1].tied_to = Some(0);
    });
    assert_eq!(
        rematerialize(&tied, &environment).unwrap_err(),
        RuntimeRematerializationError::UnsupportedUse
    );
}

/// Boundary settlements in a use block shift over the inserted
/// materializations: a hosted settlement names the surviving instruction's
/// new position while a claim completion retains its gap before them.
#[test]
fn boundary_settlements_shift_over_inserted_materializations() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.boundary_settlements = vec![
            SelectedBoundarySettlement {
                block: SelectedBlockId(0),
                instruction_index: 2,
                settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                    operation: OperationId::new(7).unwrap(),
                    boundary: BoundaryMachineId::new(1).unwrap(),
                    source: ValueId::new(9).unwrap(),
                },
            },
            SelectedBoundarySettlement {
                block: SelectedBlockId(0),
                instruction_index: 2,
                settlement: SelectedBoundarySettlementPayload::ClaimCompletion(
                    legalized_operations::LegalizedBoundarySettlement {
                        operation: OperationId::new(8).unwrap(),
                        boundary: BoundaryMachineId::new(1).unwrap(),
                        provider_execution:
                            target_operations::ProviderExecutionBinding::from_execution_record(
                                target_operations::ProviderPlanReportIdentity::new(1).unwrap(),
                                1,
                                1,
                                1,
                                1,
                            )
                            .unwrap(),
                        realization: target_operations::ClaimCompletionOnlyRealization,
                        arguments: Vec::new(),
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                        fuel: Vec::new(),
                        effect: optimization_unit::EffectLink {
                            input: 0,
                            output: 1,
                        },
                        ownership: Vec::new(),
                    },
                ),
            },
        ];
    });
    let result = rematerialize(&source, &environment).unwrap();
    let settlements = &result.transformed().functions[0].boundary_settlements;
    // Position 2 named copy id 3. The hosted write follows the surviving
    // instruction past the two materializations now in front of it; the
    // completion claim stays in the gap before those insertions.
    assert_eq!(settlements[0].instruction_index, 4);
    assert_eq!(settlements[1].instruction_index, 3);
    validate_runtime_rematerialization(
        &source,
        0,
        VirtualRegisterId(1),
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// Two runs over the identical source produce the identical validated result.
/// The published plan is a legal second input — the sealed transformed
/// program still admits regeneration — but the consumed victim is terminal:
/// every use is rebound, so a second pass over the same register finds
/// nothing to regenerate.
#[test]
fn rematerialization_is_deterministic_and_per_victim_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = rematerialize(&fixture(target), &environment).unwrap();
    let second = rematerialize(&fixture(target), &environment).unwrap();
    assert_eq!(first, second);
    // The consumed victim is at the rule's fixed point: no remaining uses.
    assert_eq!(
        rematerialize(&first, &environment).unwrap_err(),
        RuntimeRematerializationError::UnsupportedValue
    );
    // The transformed artifact is still a fully legal input rather than a
    // frozen one: a regenerated register admits and replays again, so the
    // recovery coordinator — not a plan-level fixed-point loop — decides how
    // many victims to regenerate.
    let regenerated =
        first.transformed().functions[0].blocks[0].instructions[1].operands[0].virtual_register;
    let chained =
        rematerialize_selected_runtime_value(&first, 0, regenerated, &environment, budget())
            .unwrap();
    validate_runtime_rematerialization(
        &first,
        0,
        regenerated,
        &environment,
        budget(),
        chained.transformed().clone(),
    )
    .unwrap();
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, two steps per admitted
/// use, and two per block of the admitted function — thirteen steps for the
/// single-block fixture, sixteen once its uses spread across a dominated
/// successor — so the exact count admits regeneration on both the proposal
/// and the independent replay path while one step below rejects both.
#[test]
fn validation_budget_covers_the_admission_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (source, exact_steps) in [(fixture(target), 13u64), (spread(target), 16u64)] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = rematerialize_selected_runtime_value(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            exact,
        )
        .unwrap();
        validate_runtime_rematerialization(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            rematerialize_selected_runtime_value(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                starved
            )
            .unwrap_err(),
            RuntimeRematerializationError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_runtime_rematerialization(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            RuntimeRematerializationError::WorkBudgetExceeded
        );
    }
}

/// Replay corruption in a block the regeneration never touched still
/// rejects: the restore-by-content check compares the complete plan, not
/// just the use blocks carrying fresh materializations.
#[test]
fn replay_rejects_drift_in_an_untouched_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Every use stays in the definition block; the trailing block is
    // dominated but carries no use, so the rewrite never enters it.
    let source = mutated(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let tail = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(20),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(1),
            },
        );
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: selected_instructions::SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: tail,
        });
    });
    let result = rematerialize(&source, &environment).unwrap();
    assert_eq!(result.transformed().functions[0].blocks.len(), 2);
    // An extra instruction in the untouched landing block rejects.
    let mut proposed = result.transformed().clone();
    let copy = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap()
        .clone();
    proposed.functions[0].blocks[1]
        .instructions
        .push(instruction(
            SelectedInstructionId(30),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[VirtualRegisterId(0), VirtualRegisterId(2)],
        ));
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        RuntimeRematerializationError::ReplayMismatch
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
        validate_runtime_rematerialization(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        RuntimeRematerializationError::ReplayMismatch
    );
    // A phantom trailing block rejects.
    let mut proposed = result.transformed().clone();
    let terminal_row = environment
        .constraint(environment.selected_keys().return_unit)
        .unwrap()
        .clone();
    proposed.functions[0].blocks.push(SelectedBlock {
        id: SelectedBlockId(2),
        origin: selected_instructions::SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
        instructions: Vec::new(),
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(32),
                SelectedInstructionKind::ReturnUnit,
                &terminal_row,
                &[],
            ),
            psi_return_edge: EdgeId::new(3).unwrap(),
        },
    });
    assert_eq!(
        validate_runtime_rematerialization(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        RuntimeRematerializationError::ReplayMismatch
    );
}
