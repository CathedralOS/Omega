use super::Arc;
use crate::RuntimeSpillError;
use crate::RuntimeSpillReceipt;
use crate::ValidatedRuntimeSpill;
use crate::rewrites::runtime_spill::admission;
use crate::spill_selected_runtime_value;
use crate::validate_runtime_spill;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

mod control_flow;
mod dominance;
mod liveness_custody;
mod parameters;
mod scalar_payloads;

// Analysis reuse exercises the same selected-stage fixture as spill recovery.
#[path = "../../analyses/liveness/reuse_tests.rs"]
mod analysis_reuse;

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap()
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim.
fn fixture(target: NativeTarget) -> ValidatedRuntimeSpill {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let source_value = ValueId::new(1).unwrap();
    let mut registers = vec![VirtualRegister {
        id: VirtualRegisterId(0),
        scalar_type,
        class: copy.operands[0].class,
        origin: VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index: 0,
        },
        definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
        entry_fixed_view: None,
    }];
    let mut instructions = Vec::new();
    for ordinal in 1..=4 {
        let instruction = SelectedInstructionId(ordinal);
        let register = VirtualRegisterId(ordinal);
        registers.push(VirtualRegister {
            id: register,
            scalar_type,
            class: copy.operands[0].class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        });
        instructions.push(admission::instruction(
            instruction,
            SelectedInstructionKind::CopyI64,
            copy,
            &[
                VirtualRegisterId(if ordinal == 1 { 0 } else { 1 }),
                register,
            ],
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
                    instruction: admission::instruction(
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
    ValidatedRuntimeSpill {
        receipt: RuntimeSpillReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: Arc::new(plan),
    }
}

#[test]
fn spill_history_shares_unchanged_functions_and_replays_by_content() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(target);
    let plan = Arc::make_mut(&mut source.transformed);
    for ordinal in 2..=64 {
        let mut function = plan.functions[0].clone();
        function.machine = MachineId::new(ordinal).unwrap();
        plan.functions.push(function);
    }
    let identity = selected_instruction_plan_identity(plan);
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    let first =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    let second =
        spill_selected_runtime_value(&first, 1, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    for ordinal in 0..64 {
        assert_eq!(
            std::ptr::eq(
                &source.transformed().functions[ordinal],
                &first.transformed().functions[ordinal]
            ),
            ordinal != 0,
        );
        assert_eq!(
            std::ptr::eq(
                &first.transformed().functions[ordinal],
                &second.transformed().functions[ordinal]
            ),
            ordinal != 1,
        );
    }
    assert!(
        source.transformed().functions[0]
            .local_storage_slots
            .is_empty()
    );
    assert!(
        first.transformed().functions[1]
            .local_storage_slots
            .is_empty()
    );

    // Separately allocated, byte-identical input remains valid: sharing saves
    // storage but does not replace independent semantic replay.
    let mut detached = first.transformed().clone();
    detached.functions = detached.functions.iter().cloned().collect();
    assert_eq!(
        selected_instruction_plan_identity(&detached),
        first.receipt().transformed_selected()
    );
    assert!(
        validate_runtime_spill(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
            detached.clone()
        )
        .is_ok()
    );
    detached.functions[63].entry_block = SelectedBlockId(99);
    assert!(
        validate_runtime_spill(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
            detached
        )
        .is_err()
    );
    assert_ne!(
        first.transformed().functions[63].entry_block,
        SelectedBlockId(99)
    );
}

#[test]
fn every_future_flexible_use_names_the_block_shared_reload() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = fixture(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(function.local_storage_slots.len(), 1);
        // One address/reload register pair serves all three flexible uses: the
        // block has no clobbering instruction, so a surviving home always
        // exists and the first use's pair stays open for the later two.
        assert_eq!(function.virtual_registers.len(), 7);
        let instructions = &function.blocks[0].instructions;
        assert!(matches!(
            instructions[1].kind,
            SelectedInstructionKind::Store64 { .. }
        ));
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| matches!(
                    instruction.kind,
                    SelectedInstructionKind::Load64 { .. }
                ))
                .count(),
            1
        );
        let reload = instructions[3].operands[1].virtual_register;
        assert!(matches!(
            instructions[3].kind,
            SelectedInstructionKind::Load64 { .. }
        ));
        // The three rewritten consumers are the surviving original body
        // instructions, each naming the shared reload register.
        for original_id in [2u32, 3, 4] {
            let rewritten = instructions
                .iter()
                .find(|instruction| instruction.id == SelectedInstructionId(original_id))
                .unwrap();
            assert_eq!(rewritten.operands[0].virtual_register, reload);
        }
        for inserted in instructions
            .iter()
            .filter(|instruction| instruction.id.0 > 5)
        {
            assert_eq!(inserted.provenance, Default::default());
        }
        assert!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone()
            )
            .is_ok()
        );
        // A use rebound to a different register breaks the shared shape replay
        // reconstructs.
        let mut rebound = result.transformed().clone();
        rebound.functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| instruction.id == SelectedInstructionId(4))
            .unwrap()
            .operands[0]
            .virtual_register = VirtualRegisterId(1);
        assert_eq!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                rebound
            )
            .unwrap_err(),
            RuntimeSpillError::ReplayMismatch
        );
    }
}

#[test]
fn ieee_raw_bit_spills_retain_type_and_reject_fp_register_residence() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for format in [
            semantic_vocabulary::IeeeFloatFormat::Binary32,
            semantic_vocabulary::IeeeFloatFormat::Binary64,
        ] {
            let mut source = fixture(target);
            let scalar_type = ScalarType::IeeeFloat(format);
            for register in
                &mut Arc::make_mut(&mut source.transformed).functions[0].virtual_registers
            {
                register.scalar_type = scalar_type;
            }
            let identity = selected_instruction_plan_identity(source.transformed());
            source.receipt.source_selected = identity;
            source.receipt.transformed_selected = identity;
            let result = spill_selected_runtime_value(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
            )
            .unwrap();
            let address_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
            assert!(
                result.transformed().functions[0]
                    .virtual_registers
                    .iter()
                    .all(|register| register.scalar_type
                        == if matches!(register.origin, VirtualRegisterOrigin::SpillAddress { .. })
                        {
                            address_type
                        } else {
                            scalar_type
                        })
            );
            let mut forged = result.transformed().clone();
            forged.functions[0].virtual_registers[6].scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
            assert!(
                validate_runtime_spill(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    forged
                )
                .is_err()
            );
            let mut forged_address = result.transformed().clone();
            forged_address.functions[0].virtual_registers[5].scalar_type = scalar_type;
            assert!(
                validate_runtime_spill(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    forged_address
                )
                .is_err()
            );

            let mut floating_home = source.clone();
            let float_class = environment
                .constraint(environment.selected_keys().bits_to_float64.unwrap())
                .unwrap()
                .operands[1]
                .class;
            Arc::make_mut(&mut floating_home.transformed).functions[0].virtual_registers[1].class =
                float_class;
            assert!(
                spill_selected_runtime_value(
                    &floating_home,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn independent_replay_rejects_storage_use_source_and_fuel_corruption() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    for mutation in 0..11 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            0 => function.local_storage_slots[0].byte_size = 4,
            1 => function.local_storage_slots[0].alignment = 4,
            2 => {
                function.blocks[0].instructions[1].operands[0].virtual_register =
                    VirtualRegisterId(0)
            }
            3 => {
                function.blocks[0].instructions[3].kind =
                    SelectedInstructionKind::Load64 { byte_offset: 8 }
            }
            4 => {
                function.blocks[0].instructions[4].operands[0].virtual_register =
                    VirtualRegisterId(1)
            }
            5 => {
                function.virtual_registers[6].definition_site =
                    Some(ValueDefinitionSite::FunctionParameter(1))
            }
            6 => {
                function.virtual_registers[5].origin = VirtualRegisterOrigin::SpillAddress {
                    instruction: SelectedInstructionId(7),
                    register: VirtualRegisterId(0),
                }
            }
            7 => function.blocks[0].instructions.swap(0, 1),
            8 => function.blocks[0].instructions[3]
                .provenance
                .values
                .push(ValueId::new(1).unwrap()),
            9 => {
                function.blocks[0].instructions.remove(6);
            }
            10 => function.blocks[0].instructions[3].provenance.fuel.push(
                optimization_unit::FuelSettlement {
                    site: optimization_unit::PsiProvenance::Operation(
                        semantic_vocabulary::OperationId::new(1).unwrap(),
                    ),
                    units: 1,
                },
            ),
            _ => unreachable!(),
        }
        assert!(
            validate_runtime_spill(
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
fn fixed_view_instruction_uses_pin_their_reload_at_the_call_operand() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let call = environment
            .constraint(
                *environment
                    .selected_keys()
                    .call_unit
                    .get(1)
                    .expect("every baseline target has a one-argument unit call row"),
            )
            .unwrap();
        let [call_operand] = call.operands.as_slice() else {
            panic!("a one-argument unit call row has exactly one pinned use operand");
        };
        assert_eq!(
            call_operand.access,
            register_model::RegisterOperandAccess::Use
        );
        assert!(call_operand.fixed_view.is_some());
        let mut source = fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            // The second body instruction becomes a real one-argument unit
            // call, so its use of the victim is an honest ABI-pinned site.
            function.blocks[0].instructions[1] = admission::instruction(
                SelectedInstructionId(2),
                SelectedInstructionKind::CallUnit {
                    callee: MachineId::new(2).unwrap(),
                },
                call,
                &[VirtualRegisterId(1)],
            );
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let original = &source.transformed().functions[0];
        let transformed = &result.transformed().functions[0];
        // The pinned call operand gains its own address/load pair immediately
        // before the call; the operand keeps its access and fixed view while
        // moving to the fresh reload register.
        let block = &transformed.blocks[0];
        let position = block
            .instructions
            .iter()
            .position(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::CallUnit { .. })
            })
            .unwrap();
        // The pinned call operand gains a private reload pair; both flexible
        // uses share one pair opened after the call, and the definition gains
        // a store.
        assert_eq!(
            block.instructions.len(),
            original.blocks[0].instructions.len() + 5
        );
        assert!(matches!(
            block.instructions[position - 2].kind,
            SelectedInstructionKind::FrameAddress { .. }
        ));
        assert!(matches!(
            block.instructions[position - 1].kind,
            SelectedInstructionKind::Load64 { .. }
        ));
        let original_operand = original.blocks[0].instructions[1].operands[0];
        let rewritten_operand = block.instructions[position].operands[0];
        let reload_register = block.instructions[position - 1].operands[1].virtual_register;
        assert_eq!(original_operand.virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten_operand.virtual_register, reload_register);
        assert_eq!(rewritten_operand.access, original_operand.access);
        assert_eq!(rewritten_operand.fixed_view, original_operand.fixed_view);
        // The two flexible uses after the call name the same shared reload —
        // a distinct register from the pinned pair's.
        let shared = block.instructions[position + 2].operands[1].virtual_register;
        assert!(matches!(
            block.instructions[position + 1].kind,
            SelectedInstructionKind::FrameAddress { .. }
        ));
        assert!(matches!(
            block.instructions[position + 2].kind,
            SelectedInstructionKind::Load64 { .. }
        ));
        assert_ne!(shared, reload_register);
        for original_id in [3u32, 4] {
            let rewritten = block
                .instructions
                .iter()
                .find(|instruction| instruction.id == SelectedInstructionId(original_id))
                .unwrap();
            assert_eq!(rewritten.operands[0].virtual_register, shared);
        }
        assert!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone()
            )
            .is_ok()
        );
        for mutation in 0..5 {
            let mut proposed = result.transformed().clone();
            let function = &mut proposed.functions[0];
            match mutation {
                0 => {
                    function.blocks[0].instructions.remove(position - 1);
                }
                1 => {
                    function.blocks[0]
                        .instructions
                        .swap(position - 2, position - 1);
                }
                2 => {
                    function.blocks[0].instructions[position].operands[0].virtual_register =
                        VirtualRegisterId(1);
                }
                3 => {
                    function.blocks[0].instructions[position].operands[0].fixed_view = None;
                }
                4 => {
                    function.virtual_registers.pop();
                }
                _ => unreachable!(),
            }
            assert_eq!(
                validate_runtime_spill(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    proposed
                )
                .unwrap_err(),
                RuntimeSpillError::ReplayMismatch,
                "{target:?} mutation {mutation}"
            );
        }
    }
}

#[test]
fn address_values_and_exhausted_budget_do_not_gain_spill_authority() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let source = fixture(NativeTarget::linux_x64());
    let foreign_environment =
        baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        spill_selected_runtime_value(
            &source,
            0,
            VirtualRegisterId(1),
            &foreign_environment,
            budget()
        )
        .unwrap_err(),
        RuntimeSpillError::SourceMismatch
    );
    assert_eq!(
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(0), &environment, budget())
            .unwrap_err(),
        RuntimeSpillError::UnsupportedValue
    );
    let tiny = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, tiny)
            .unwrap_err(),
        RuntimeSpillError::WorkBudgetExceeded
    );
    let mut address = source.clone();
    Arc::make_mut(&mut address.transformed).functions[0].virtual_registers[1].origin =
        VirtualRegisterOrigin::SpillAddress {
            instruction: SelectedInstructionId(1),
            register: VirtualRegisterId(0),
        };
    assert_eq!(
        spill_selected_runtime_value(&address, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap_err(),
        RuntimeSpillError::UnsupportedValue
    );
}
