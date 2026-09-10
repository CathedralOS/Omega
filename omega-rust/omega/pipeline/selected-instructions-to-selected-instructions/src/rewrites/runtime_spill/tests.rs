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

use super::*;

mod control_flow;
mod dominance;
mod parameters;
mod scalar_payloads;

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
        projected_structural_call_returns: Vec::new(),
        functions: vec![SelectedFunction {
            machine,
            attachment: None,
            provenance: Default::default(),
            ranked: None,
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
        }],
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
fn every_future_use_gets_a_distinct_short_lived_reload() {
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
        assert_eq!(function.virtual_registers.len(), 11);
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
            3
        );
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
fn address_values_fixed_uses_and_exhausted_budget_do_not_gain_spill_authority() {
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
    let mut fixed = source.clone();
    Arc::make_mut(&mut fixed.transformed).functions[0].blocks[0].instructions[1].operands[0]
        .fixed_view = Some(register_model::RegisterViewId(0));
    assert_eq!(
        spill_selected_runtime_value(&fixed, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap_err(),
        RuntimeSpillError::UnsupportedUse
    );
}
