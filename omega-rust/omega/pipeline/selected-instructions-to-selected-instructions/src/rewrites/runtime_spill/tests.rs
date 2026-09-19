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
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedFunction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedLocalStorageSlot, SelectedTerminator, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

mod control_flow;
mod dominance;
mod liveness_custody;
mod parameters;
mod redefinitions;
mod scalar_payloads;
mod structural_transports;

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
            // With the consumers' use operands in the same foreign class the
            // victim reaches the target-row check: the frame/load/store rows
            // cannot carry its class, which is a candidate-local limit
            // (`UnsupportedValue`), so recovery skips it and tries the next
            // candidate instead of aborting on a constraint mismatch.
            let mut foreign_class = source.clone();
            {
                let function = &mut Arc::make_mut(&mut foreign_class.transformed).functions[0];
                function.virtual_registers[1].class = float_class;
                for instruction in &mut function.blocks[0].instructions[1..] {
                    instruction.operands[0].class = float_class;
                }
            }
            assert_eq!(
                spill_selected_runtime_value(
                    &foreign_class,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget()
                )
                .unwrap_err(),
                RuntimeSpillError::UnsupportedValue
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
fn compiler_address_origins_and_exhausted_budget_do_not_gain_spill_authority() {
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
    // A register absent from the function has no value to admit.
    assert_eq!(
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(90), &environment, budget())
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

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then four per admitted
/// use, one per storage definition, and two per block of the rewritten
/// function — so the exact count admits the spill on both the proposal and
/// the independent replay path while one step below rejects both, at three
/// fixture sizes that each grow a different term of the charge.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for (source, exact_steps) in [
        // (1 block + 4 instructions) + (3 uses × 4) + 1 definition + 2
        // + (1 block + 4 instructions) = 25.
        (fixture(NativeTarget::linux_x64()), 25u64),
        // (3 blocks + 4 instructions) + (3 uses × 4) + 1 definition + 6
        // + (3 blocks + 4 instructions) = 33.
        (control_flow::cfg_fixture(NativeTarget::linux_x64()), 33u64),
        // (4 blocks + 5 instructions) + (2 uses × 4) + 2 definitions + 8
        // + (4 blocks + 5 instructions) = 36.
        (
            parameters::parameter_fixture(NativeTarget::linux_x64()),
            36u64,
        ),
        // (4 blocks + 6 instructions) + (2 uses × 4) + 2 definitions + 8
        // + (4 blocks + 6 instructions) + 1 slot × 10 = 48.
        (
            parameters::case_parameter_fixture(NativeTarget::linux_x64()),
            48u64,
        ),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, exact)
                .unwrap();
        validate_runtime_spill(
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
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, starved)
                .unwrap_err(),
            RuntimeSpillError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            RuntimeSpillError::WorkBudgetExceeded
        );
    }
}

/// Two runs over the identical source produce the identical validated
/// result, and the published artifact is a legal second input through the
/// sealed analysis boundary: the phase's real liveness and live-range
/// analyses accept it, re-admission of the same victim is terminal because
/// its private slot already exists, the produced `SpillAddress` register
/// stays outside admission, and the shared reload register — an ordinary
/// instruction result on the published plan — still admits a second,
/// independently validated spill whose receipt chains the first artifact.
#[test]
fn spill_is_deterministic_and_the_published_plan_re_admits() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    let second =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one. The real
    // prerequisite analyses the recovery route consumes accept it directly.
    let liveness = crate::analyze_liveness(&first).unwrap();
    crate::analyze_live_ranges(&first, &liveness).unwrap();
    // Re-admitting the same victim is terminal: the published plan already
    // carries its private slot, so the site refuses before any use scan.
    assert_eq!(
        spill_selected_runtime_value(&first, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap_err(),
        RuntimeSpillError::UnsupportedUse
    );
    // The spill-address register keeps its `SpillAddress` origin on the
    // second input, which stays outside admission exactly as on the first.
    let address_register = first.transformed().functions[0]
        .virtual_registers
        .iter()
        .find(|register| matches!(register.origin, VirtualRegisterOrigin::SpillAddress { .. }))
        .expect("the shared reload opened with a spill address")
        .id;
    assert_eq!(
        spill_selected_runtime_value(&first, 0, address_register, &environment, budget())
            .unwrap_err(),
        RuntimeSpillError::UnsupportedValue
    );
    // A fresh victim on the published plan still admits: the shared reload
    // register is an ordinary instruction result defined before its uses,
    // and its spill is a second independently validated rewrite whose
    // receipt binds the first artifact's identity as its source. Its storage
    // windows open only where the incumbent's have closed, so the last-writer
    // replay shares the declared slot and no second entry is appended.
    let shared_reload = first.transformed().functions[0]
        .virtual_registers
        .iter()
        .find(|register| {
            matches!(
                register.origin,
                VirtualRegisterOrigin::InstructionResult { instruction, .. }
                    if instruction.0 > 5
            )
        })
        .expect("the shared reload is the inserted load's result")
        .id;
    let respilled =
        spill_selected_runtime_value(&first, 0, shared_reload, &environment, budget()).unwrap();
    assert_eq!(
        respilled.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
    assert_eq!(
        respilled.transformed().functions[0]
            .local_storage_slots
            .len(),
        1
    );
    validate_runtime_spill(
        &first,
        0,
        shared_reload,
        &environment,
        budget(),
        respilled.transformed().clone(),
    )
    .unwrap();
}

/// An instruction that can destroy register content closes the shared
/// reload: a flexible use before and after a `CallUnit` — whose caller-saved
/// clobbers could write the unit hosting the open reload — each open their
/// own pair, so no produced interval ever demands a cross-call home recovery
/// may not have. Replay independently reconstructs that shape, so a dropped
/// pair, a use still naming the pre-call register, or a rebound use each
/// reject.
#[test]
fn a_call_closes_the_shared_reload_for_later_flexible_uses() {
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
                    .first()
                    .expect("every baseline target has a zero-argument unit call row"),
            )
            .unwrap();
        // A zero-argument unit call reads no operand register: it stands
        // strictly between the victim's two uses without consuming either.
        assert!(call.operands.is_empty());
        let mut source = fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            function.blocks[0].instructions[2] = admission::instruction(
                SelectedInstructionId(3),
                SelectedInstructionKind::CallUnit {
                    callee: MachineId::new(2).unwrap(),
                },
                call,
                &[],
            );
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let block = &result.transformed().functions[0].blocks[0];
        // The four originals plus one definition store and a reload pair on
        // each side of the call: [copy, store, address, load, copy, call,
        // address, load, copy].
        assert_eq!(block.instructions.len(), 9);
        let call_position = block
            .instructions
            .iter()
            .position(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::CallUnit { .. })
            })
            .unwrap();
        let loads: Vec<usize> = block
            .instructions
            .iter()
            .enumerate()
            .filter_map(|(position, instruction)| {
                matches!(instruction.kind, SelectedInstructionKind::Load64 { .. })
                    .then_some(position)
            })
            .collect();
        assert_eq!(loads.len(), 2);
        assert!(loads[0] < call_position);
        assert!(call_position < loads[1]);
        // Each flexible use names the pair opened inside its own span: the
        // pre-call and post-call reload registers are distinct.
        let pre_call = block
            .instructions
            .iter()
            .find(|instruction| instruction.id == SelectedInstructionId(2))
            .unwrap()
            .operands[0]
            .virtual_register;
        let post_call = block
            .instructions
            .iter()
            .find(|instruction| instruction.id == SelectedInstructionId(4))
            .unwrap()
            .operands[0]
            .virtual_register;
        assert_eq!(
            pre_call,
            block.instructions[loads[0]].operands[1].virtual_register
        );
        assert_eq!(
            post_call,
            block.instructions[loads[1]].operands[1].virtual_register
        );
        assert_ne!(pre_call, post_call);
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
                // Dropping the post-call load leaves that use's register
                // without its pair.
                0 => {
                    function.blocks[0].instructions.remove(loads[1]);
                }
                // A use still naming the pre-call reload across the call
                // boundary is exactly the shape replay refuses.
                1 => {
                    function.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|instruction| instruction.id == SelectedInstructionId(4))
                        .unwrap()
                        .operands[0]
                        .virtual_register = pre_call;
                }
                // Rebinding the post-call use back to the victim leaves the
                // second pair without its consumer.
                2 => {
                    function.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|instruction| instruction.id == SelectedInstructionId(4))
                        .unwrap()
                        .operands[0]
                        .virtual_register = VirtualRegisterId(1);
                }
                // A forged extra slot leaves a trailing slot the rewrite did
                // not publish.
                3 => function.local_storage_slots.push(SelectedLocalStorageSlot {
                    id: LocalStorageSlotId::Boundary {
                        operation: OperationId::new(1).unwrap(),
                    },
                    byte_size: 8,
                    alignment: 8,
                }),
                // Drift on an instruction the rewrite never touched — a use
                // operand appearing on the call — still rejects.
                4 => {
                    let operand = function.blocks[0].instructions[1].operands[0];
                    function.blocks[0].instructions[call_position]
                        .operands
                        .push(operand);
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

/// The crossing counterpart: while the victim's class still offers an
/// allocatable view avoiding every unit the intervening `CallUnit` writes —
/// the callee-saved candidates — the still-open reload survives the call, so
/// both flexible uses name one register whose produced interval reaches
/// across it. Crossing replay accepts exactly that shape: bounded replay of
/// the same plan rejects the missing post-call pair, the bounded emission
/// rejects under crossing replay, and a dropped pair or a use rebound to the
/// victim each reject. Where no view survives the call's writes at all, the
/// crossing policy degrades to the bounded shape.
#[test]
fn a_surviving_view_keeps_the_shared_reload_open_across_a_call() {
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
                    .first()
                    .expect("every baseline target has a zero-argument unit call row"),
            )
            .unwrap();
        assert!(call.operands.is_empty());
        let mut source = fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            function.blocks[0].instructions[2] = admission::instruction(
                SelectedInstructionId(3),
                SelectedInstructionKind::CallUnit {
                    callee: MachineId::new(2).unwrap(),
                },
                call,
                &[],
            );
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let result = crate::spill_selected_runtime_value_with_span_policy(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
            crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
        )
        .unwrap();
        let block = &result.transformed().functions[0].blocks[0];
        // The four originals plus one definition store and one shared pair
        // that crosses the call: [copy, store, address, load, copy, call,
        // copy].
        assert_eq!(block.instructions.len(), 7);
        let call_position = block
            .instructions
            .iter()
            .position(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::CallUnit { .. })
            })
            .unwrap();
        let loads: Vec<usize> = block
            .instructions
            .iter()
            .enumerate()
            .filter_map(|(position, instruction)| {
                matches!(instruction.kind, SelectedInstructionKind::Load64 { .. })
                    .then_some(position)
            })
            .collect();
        assert_eq!(loads.len(), 1);
        assert!(loads[0] < call_position);
        // Both flexible uses name the still-open reload, so its produced
        // interval reaches across the call.
        let reload = block.instructions[loads[0]].operands[1].virtual_register;
        let pre_call = block
            .instructions
            .iter()
            .find(|instruction| instruction.id == SelectedInstructionId(2))
            .unwrap()
            .operands[0]
            .virtual_register;
        let post_call = block
            .instructions
            .iter()
            .find(|instruction| instruction.id == SelectedInstructionId(4))
            .unwrap()
            .operands[0]
            .virtual_register;
        assert_eq!(pre_call, reload);
        assert_eq!(post_call, reload);
        assert!(
            crate::validate_runtime_spill_with_span_policy(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone(),
                crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
            )
            .is_ok()
        );
        // Bounded replay of the crossing plan expects a fresh pair after the
        // call; the bounded emission's post-call pair fails crossing replay.
        assert_eq!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone(),
            )
            .unwrap_err(),
            RuntimeSpillError::ReplayMismatch
        );
        let bounded =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        assert_eq!(
            crate::validate_runtime_spill_with_span_policy(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                bounded.transformed().clone(),
                crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
            )
            .unwrap_err(),
            RuntimeSpillError::ReplayMismatch
        );
        for mutation in 0..3 {
            let mut proposed = result.transformed().clone();
            let function = &mut proposed.functions[0];
            match mutation {
                // Dropping the shared load leaves both uses naming a register
                // no pair defines.
                0 => {
                    function.blocks[0].instructions.remove(loads[0]);
                }
                // A use rebound to the spilled victim restores nothing.
                1 => {
                    function.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|instruction| instruction.id == SelectedInstructionId(4))
                        .unwrap()
                        .operands[0]
                        .virtual_register = VirtualRegisterId(1);
                }
                // The bounded shape — a second private pair after the call —
                // is not the crossing canonical output either.
                _ => {
                    let (address, load) = {
                        let bounded_block = &bounded.transformed().functions[0].blocks[0];
                        let pair: Vec<_> = bounded_block
                            .instructions
                            .iter()
                            .skip(call_position + 1)
                            .take_while(|instruction| {
                                !matches!(instruction.kind, SelectedInstructionKind::CopyI64)
                            })
                            .cloned()
                            .collect();
                        (pair[0].clone(), pair[1].clone())
                    };
                    let block = &mut function.blocks[0];
                    let at = block
                        .instructions
                        .iter()
                        .position(|instruction| instruction.id == SelectedInstructionId(4))
                        .unwrap();
                    block.instructions.insert(at, load);
                    block.instructions.insert(at, address);
                }
            }
            assert_eq!(
                crate::validate_runtime_spill_with_span_policy(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    proposed,
                    crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
                )
                .unwrap_err(),
                RuntimeSpillError::ReplayMismatch,
                "{target:?} mutation {mutation}"
            );
        }
    }
}

/// The crossing policy's degradation: when the intervening call's implicit
/// uses cover every unit the victim's class offers, no view can host even a
/// call-free shared interval, so `UnitWriteCrossing` produces the identical
/// private-per-use plan the bounded policy does and validates identically.
#[test]
fn no_surviving_view_degrades_crossing_to_the_bounded_shape() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let call_row = environment
            .constraint(
                *environment
                    .selected_keys()
                    .call_unit
                    .first()
                    .expect("every baseline target has a zero-argument unit call row"),
            )
            .unwrap();
        let model = environment.physical().model();
        let mut source = fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            let victim_class = function.virtual_registers[1].class;
            let mut call = admission::instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::CallUnit {
                    callee: MachineId::new(2).unwrap(),
                },
                call_row,
                &[],
            );
            for class in &model.classes {
                if class.id != victim_class {
                    continue;
                }
                for view_id in &class.views {
                    if let Some(view) = model.views.get(usize::from(view_id.0)) {
                        call.implicit_uses
                            .extend(view.units.iter().chain(&view.write_units).copied());
                    }
                }
            }
            call.implicit_uses.sort_unstable();
            call.implicit_uses.dedup();
            function.blocks[0].instructions.insert(2, call);
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let bounded =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let crossing = crate::spill_selected_runtime_value_with_span_policy(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
            crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
        )
        .unwrap();
        // With no surviving view the crossing policy emits the bounded plan
        // exactly — one private pair per use — and both replays accept it.
        assert_eq!(
            crossing.transformed().functions[0].blocks[0].instructions,
            bounded.transformed().functions[0].blocks[0].instructions
        );
        assert!(
            crate::validate_runtime_spill_with_span_policy(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                crossing.transformed().clone(),
                crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
            )
            .is_ok()
        );
        assert!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                crossing.transformed().clone(),
            )
            .is_ok()
        );
    }
}

/// The shared/private boundary: when every view of the victim's class meets
/// a unit that can never host an interval — here an intervening call whose
/// implicit uses cover every unit, so each one may be live through every
/// interior point — no home survives and each flexible use keeps its own
/// private reload pair, exactly the per-use shape the rewrite produced
/// before block-local sharing existed. Replay requires that shape too: two
/// uses may not share a register.
#[test]
fn no_surviving_view_keeps_every_flexible_use_on_a_private_pair() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let call_row = environment
            .constraint(
                *environment
                    .selected_keys()
                    .call_unit
                    .first()
                    .expect("every baseline target has a zero-argument unit call row"),
            )
            .unwrap();
        let model = environment.physical().model();
        let mut source = fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            let victim_class = function.virtual_registers[1].class;
            let mut call = admission::instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::CallUnit {
                    callee: MachineId::new(2).unwrap(),
                },
                call_row,
                &[],
            );
            // Extend the call's effects so every unit the victim's class
            // offers is implicitly used somewhere in the function: each one
            // may be live through every interior point, so no view can host
            // even a call-free shared interval.
            for class in &model.classes {
                if class.id != victim_class {
                    continue;
                }
                for view_id in &class.views {
                    if let Some(view) = model.views.get(usize::from(view_id.0)) {
                        call.implicit_uses
                            .extend(view.units.iter().chain(&view.write_units).copied());
                    }
                }
            }
            call.implicit_uses.sort_unstable();
            call.implicit_uses.dedup();
            function.blocks[0].instructions.insert(2, call);
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let block = &result.transformed().functions[0].blocks[0];
        // Five originals plus the definition store and one private pair per
        // use: [copy, store, address, load, copy, call, address, load, copy,
        // address, load, copy].
        assert_eq!(block.instructions.len(), 12);
        assert_eq!(
            block
                .instructions
                .iter()
                .filter(|instruction| {
                    matches!(instruction.kind, SelectedInstructionKind::Load64 { .. })
                })
                .count(),
            3
        );
        // Each consumer names its own reload register — no two share.
        let reloads: Vec<_> = [2u32, 3, 4]
            .iter()
            .map(|original_id| {
                block
                    .instructions
                    .iter()
                    .find(|instruction| instruction.id == SelectedInstructionId(*original_id))
                    .unwrap()
                    .operands[0]
                    .virtual_register
            })
            .collect();
        for (index, reload) in reloads.iter().enumerate() {
            assert!(!reloads[..index].contains(reload));
            assert_ne!(*reload, VirtualRegisterId(1));
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
        // A proposed pair short of the per-use shape — the last two uses
        // sharing one register — cannot replay: the second use's expected
        // pair is not in the stream.
        let mut shared_anyway = result.transformed().clone();
        shared_anyway.functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| instruction.id == SelectedInstructionId(4))
            .unwrap()
            .operands[0]
            .virtual_register = reloads[1];
        assert_eq!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                shared_anyway
            )
            .unwrap_err(),
            RuntimeSpillError::ReplayMismatch
        );
    }
}

/// Physical slot reuse: the shared reload register re-admits as a victim, and
/// its storage windows open only after the incumbent's have closed — it is
/// defined by the incumbent's own load — so the last-writer replay shares the
/// declared `Spill` slot. No second `local_storage_slots` entry appears, the
/// frame keeps one eight-byte charge, and every emitted frame access names the
/// incumbent's slot. Replay recomputes the identical decision, so a forged
/// extra slot or an access retargeted to a private slot rejects.
#[test]
fn disjoint_windows_share_the_declared_spill_slot() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    let slot = first.transformed().functions[0].local_storage_slots[0].id;
    let reload = first.transformed().functions[0]
        .virtual_registers
        .iter()
        .find(|register| {
            matches!(
                register.origin,
                VirtualRegisterOrigin::InstructionResult { instruction, .. }
                    if instruction.0 > 5
            )
        })
        .expect("the shared reload is the inserted load's result")
        .id;
    let second = spill_selected_runtime_value(&first, 0, reload, &environment, budget()).unwrap();
    let function = &second.transformed().functions[0];
    // The slot stayed singular: no second storage entry was appended, so the
    // frame's declared demand is unchanged by the second victim.
    assert_eq!(function.local_storage_slots.len(), 1);
    assert_eq!(function.local_storage_slots[0].id, slot);
    // Every frame access in the block — the incumbent's and the second
    // victim's alike — names that one slot.
    let stores = function.blocks[0]
        .instructions
        .iter()
        .filter(|instruction| {
            matches!(
                instruction.kind,
                SelectedInstructionKind::Store64 { slot: named, .. }
                    | SelectedInstructionKind::FrameAddress { slot: named, .. }
                    if named == FrameStorageSlotId::Local(slot))
        })
        .count();
    assert_eq!(
        stores,
        function.blocks[0]
            .instructions
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::Store64 { .. }
                        | SelectedInstructionKind::FrameAddress { .. }
                )
            })
            .count()
    );
    assert!(stores >= 4);
    validate_runtime_spill(
        &first,
        0,
        reload,
        &environment,
        budget(),
        second.transformed().clone(),
    )
    .unwrap();
    // Replay rejects a storage list the rewrite did not produce: the shared
    // slot appends nothing, so a forged second entry cannot validate.
    let mut forged = second.transformed().clone();
    forged.functions[0]
        .local_storage_slots
        .push(SelectedLocalStorageSlot {
            id: LocalStorageSlotId::Spill { register: reload },
            byte_size: 8,
            alignment: 8,
        });
    assert_eq!(
        validate_runtime_spill(&first, 0, reload, &environment, budget(), forged).unwrap_err(),
        RuntimeSpillError::ReplayMismatch
    );
    // A generated access retargeted to a private slot replay never chose —
    // here the second victim's own store — rejects the same way.
    let mut retargeted = second.transformed().clone();
    let position = retargeted.functions[0].blocks[0]
        .instructions
        .iter()
        .rposition(|instruction| {
            matches!(instruction.kind, SelectedInstructionKind::Store64 { .. })
        })
        .expect("the second victim's store is present");
    retargeted.functions[0].blocks[0].instructions[position].kind =
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(LocalStorageSlotId::Spill { register: reload }),
            byte_offset: 0,
        };
    assert_eq!(
        validate_runtime_spill(&first, 0, reload, &environment, budget(), retargeted).unwrap_err(),
        RuntimeSpillError::ReplayMismatch
    );
}

/// The other half of a destructive interleave: an incumbent reload sitting
/// after the victim's store would read the victim's bytes where it expects the
/// incumbent's, so the candidate stays private even though every proposed
/// reload sees only the new writer.
#[test]
fn an_incumbent_load_after_the_new_store_keeps_the_victim_private() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let address = environment.constraint(keys.frame_address.unwrap()).unwrap();
    let load = environment.constraint(keys.load64.unwrap()).unwrap();
    let incumbent = LocalStorageSlotId::Spill {
        register: VirtualRegisterId(50),
    };
    let frame_slot = FrameStorageSlotId::Local(incumbent);
    let mut source = fixture(target);
    {
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let scalar_type = function.virtual_registers[0].scalar_type;
        let class = function.virtual_registers[0].class;
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: incumbent,
            byte_size: 8,
            alignment: 8,
        });
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(51),
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::SpillAddress {
                instruction: SelectedInstructionId(90),
                register: VirtualRegisterId(50),
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(52),
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(91),
                source_value: ValueId::new(9).unwrap(),
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        });
        // The incumbent's reload pair sits between the victim's definition and
        // its first use — inside the window where the reused slot would hold
        // the victim's bytes, not the incumbent's.
        let instructions = &mut function.blocks[0].instructions;
        instructions.insert(
            1,
            admission::instruction(
                SelectedInstructionId(90),
                SelectedInstructionKind::FrameAddress {
                    slot: frame_slot,
                    byte_offset: 0,
                },
                address,
                &[VirtualRegisterId(51)],
            ),
        );
        instructions.insert(
            2,
            admission::instruction(
                SelectedInstructionId(91),
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                load,
                &[VirtualRegisterId(51), VirtualRegisterId(52)],
            ),
        );
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.local_storage_slots.as_slice(),
        [
            SelectedLocalStorageSlot {
                id: incumbent,
                byte_size: 8,
                alignment: 8,
            },
            SelectedLocalStorageSlot {
                id: LocalStorageSlotId::Spill {
                    register: VirtualRegisterId(1),
                },
                byte_size: 8,
                alignment: 8,
            },
        ]
    );
    validate_runtime_spill(
        &source,
        0,
        VirtualRegisterId(1),
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A slot stays private to its first victim while any part of its access
/// stream falls outside the idiom this rewrite itself emits: a nonzero-offset
/// store or address, a byte-oriented hosted access, or an address register
/// read by anything but a zero-offset `Load64`. The last-writer replay cannot
/// classify those accesses, so the candidate is skipped and the victim appends
/// its own slot.
#[test]
fn unclassifiable_slot_accesses_keep_the_victim_private() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let store = environment.constraint(keys.store64.unwrap()).unwrap();
    let address = environment.constraint(keys.frame_address.unwrap()).unwrap();
    let load = environment.constraint(keys.load64.unwrap()).unwrap();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let host_write = environment
        .constraint(keys.hosted_write_byte_i32.unwrap())
        .unwrap();
    let incumbent = LocalStorageSlotId::Spill {
        register: VirtualRegisterId(50),
    };
    let frame_slot = FrameStorageSlotId::Local(incumbent);
    for mutation in 0..5 {
        let mut source = fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            let scalar_type = function.virtual_registers[0].scalar_type;
            let class = function.virtual_registers[0].class;
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: incumbent,
                byte_size: 8,
                alignment: 8,
            });
            function.virtual_registers.push(VirtualRegister {
                id: VirtualRegisterId(51),
                scalar_type,
                class,
                origin: VirtualRegisterOrigin::SpillAddress {
                    instruction: SelectedInstructionId(90),
                    register: VirtualRegisterId(50),
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            function.virtual_registers.push(VirtualRegister {
                id: VirtualRegisterId(52),
                scalar_type,
                class,
                origin: VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(91),
                    source_value: ValueId::new(9).unwrap(),
                },
                definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
                entry_fixed_view: None,
            });
            let instructions = &mut function.blocks[0].instructions;
            match mutation {
                // A nonzero-offset store writes bytes the zero-offset idiom
                // cannot account for.
                0 => instructions.push(admission::instruction(
                    SelectedInstructionId(90),
                    SelectedInstructionKind::Store64 {
                        slot: frame_slot,
                        byte_offset: 4,
                    },
                    store,
                    &[VirtualRegisterId(0)],
                )),
                // A nonzero-offset address escapes the same idiom.
                1 => instructions.push(admission::instruction(
                    SelectedInstructionId(90),
                    SelectedInstructionKind::FrameAddress {
                        slot: frame_slot,
                        byte_offset: 4,
                    },
                    address,
                    &[VirtualRegisterId(51)],
                )),
                // A byte-oriented hosted access names the slot directly.
                2 => instructions.push(admission::instruction(
                    SelectedInstructionId(90),
                    SelectedInstructionKind::HostedWriteByteI32 { slot: incumbent },
                    host_write,
                    &[VirtualRegisterId(0)],
                )),
                // The slot's address register feeding anything but a
                // zero-offset load leaves reads the scan cannot classify.
                3 => {
                    instructions.push(admission::instruction(
                        SelectedInstructionId(90),
                        SelectedInstructionKind::FrameAddress {
                            slot: frame_slot,
                            byte_offset: 0,
                        },
                        address,
                        &[VirtualRegisterId(51)],
                    ));
                    instructions.push(admission::instruction(
                        SelectedInstructionId(91),
                        SelectedInstructionKind::CopyI64,
                        copy,
                        &[VirtualRegisterId(51), VirtualRegisterId(52)],
                    ));
                }
                // A partial load through the slot's own address still reads
                // bytes outside the classified stream.
                _ => {
                    instructions.push(admission::instruction(
                        SelectedInstructionId(90),
                        SelectedInstructionKind::FrameAddress {
                            slot: frame_slot,
                            byte_offset: 0,
                        },
                        address,
                        &[VirtualRegisterId(51)],
                    ));
                    instructions.push(admission::instruction(
                        SelectedInstructionId(91),
                        SelectedInstructionKind::Load64 { byte_offset: 8 },
                        load,
                        &[VirtualRegisterId(51), VirtualRegisterId(52)],
                    ));
                }
            }
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.local_storage_slots.as_slice(),
            [
                SelectedLocalStorageSlot {
                    id: incumbent,
                    byte_size: 8,
                    alignment: 8,
                },
                SelectedLocalStorageSlot {
                    id: LocalStorageSlotId::Spill {
                        register: VirtualRegisterId(1),
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            ],
            "mutation {mutation}"
        );
        validate_runtime_spill(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The destructive interleave the last-writer replay refuses: an incumbent
/// store between the new victim's own store and a reload could leave the
/// incumbent's bytes where the reload expects the victim's, so admission keeps
/// the victim on a fresh private slot even though the function already
/// declares a candidate.
#[test]
fn an_interleaved_incumbent_store_keeps_the_victim_private() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let store = environment
        .constraint(environment.selected_keys().store64.unwrap())
        .unwrap();
    let mut source = fixture(target);
    {
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let incumbent = LocalStorageSlotId::Spill {
            register: VirtualRegisterId(9),
        };
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: incumbent,
            byte_size: 8,
            alignment: 8,
        });
        // The incumbent writes its slot strictly between the victim's
        // definition and first use — inside the window a reused slot would
        // have to keep stable for the reload.
        function.blocks[0].instructions.insert(
            1,
            admission::instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(incumbent),
                    byte_offset: 0,
                },
                store,
                &[VirtualRegisterId(0)],
            ),
        );
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    let function = &result.transformed().functions[0];
    // The candidate stayed private to its first victim; this victim appended
    // its own slot, so frame demand charges both bytes.
    assert_eq!(
        function.local_storage_slots.as_slice(),
        [
            SelectedLocalStorageSlot {
                id: LocalStorageSlotId::Spill {
                    register: VirtualRegisterId(9),
                },
                byte_size: 8,
                alignment: 8,
            },
            SelectedLocalStorageSlot {
                id: LocalStorageSlotId::Spill {
                    register: VirtualRegisterId(1),
                },
                byte_size: 8,
                alignment: 8,
            },
        ]
    );
    // The incumbent's original store kept its slot while every generated
    // access names the fresh private slot.
    let mut incumbent_stores = 0usize;
    for instruction in &function.blocks[0].instructions {
        match instruction.kind {
            SelectedInstructionKind::Store64 { slot, .. }
            | SelectedInstructionKind::FrameAddress { slot, .. } => {
                if slot
                    == FrameStorageSlotId::Local(LocalStorageSlotId::Spill {
                        register: VirtualRegisterId(9),
                    })
                {
                    incumbent_stores += 1;
                    assert_eq!(instruction.id, SelectedInstructionId(6));
                } else {
                    assert_eq!(
                        slot,
                        FrameStorageSlotId::Local(LocalStorageSlotId::Spill {
                            register: VirtualRegisterId(1),
                        })
                    );
                }
            }
            _ => {}
        }
    }
    assert_eq!(incumbent_stores, 1);
    validate_runtime_spill(
        &source,
        0,
        VirtualRegisterId(1),
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // And the dropped second declaration is a replay mismatch: admission
    // chose a fresh slot, so it must be the appended tail entry.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].local_storage_slots.pop();
    assert_eq!(
        validate_runtime_spill(
            &source,
            0,
            VirtualRegisterId(1),
            &environment,
            budget(),
            dropped
        )
        .unwrap_err(),
        RuntimeSpillError::ReplayMismatch
    );
}
