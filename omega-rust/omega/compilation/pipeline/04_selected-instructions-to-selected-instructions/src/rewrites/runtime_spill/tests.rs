use super::Arc;
use super::RuntimeSpillReceipt;
use crate::RuntimeSpillError;
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
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

mod control_flow;
mod dominance;
mod independence_tests;
mod liveness_custody;
mod parameters;
mod read_modify_write;
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
fn ieee_raw_bit_spills_retain_type_and_transport_fp_register_residence() {
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

            let float_class = environment
                .constraint(match format {
                    semantic_vocabulary::IeeeFloatFormat::Binary32 => {
                        environment.selected_keys().bits_to_float32.unwrap()
                    }
                    semantic_vocabulary::IeeeFloatFormat::Binary64 => {
                        environment.selected_keys().bits_to_float64.unwrap()
                    }
                })
                .unwrap()
                .operands[1]
                .class;
            let carrier_class = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap()
                .operands[0]
                .class;
            // Only the victim's own class moved: its consumers still read the
            // carrier class, so the uses themselves stay rejected before the
            // transport question is ever reached.
            let mut floating_home = source.clone();
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
            // victim rides the target's declared bit-preserving pair: each
            // definition store is `Float*ToBits` into the carrier class then
            // `Store64`, and each reload is `FrameAddress` + `Load64` with a
            // `BitsToFloat*` restoring the victim's own class — the same
            // eight-byte private slot a direct spill uses.
            let mut foreign_class = source.clone();
            {
                let function = &mut Arc::make_mut(&mut foreign_class.transformed).functions[0];
                function.virtual_registers[1].class = float_class;
                for instruction in &mut function.blocks[0].instructions[1..] {
                    instruction.operands[0].class = float_class;
                }
            }
            let result = spill_selected_runtime_value(
                &foreign_class,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
            )
            .unwrap();
            let function = &result.transformed().functions[0];
            let (to_kind, from_kind) = match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => (
                    SelectedInstructionKind::Float32ToBits,
                    SelectedInstructionKind::BitsToFloat32,
                ),
                semantic_vocabulary::IeeeFloatFormat::Binary64 => (
                    SelectedInstructionKind::Float64ToBits,
                    SelectedInstructionKind::BitsToFloat64,
                ),
            };
            let instructions = &function.blocks[0].instructions;
            assert_eq!(function.local_storage_slots.len(), 1);
            assert_eq!(function.local_storage_slots[0].byte_size, 8);
            assert_eq!(
                instructions
                    .iter()
                    .filter(|instruction| instruction.kind == to_kind)
                    .count(),
                1
            );
            let stores = instructions
                .iter()
                .filter(|instruction| {
                    matches!(instruction.kind, SelectedInstructionKind::Store64 { .. })
                })
                .count();
            assert_eq!(stores, 1);
            let loads = instructions
                .iter()
                .filter(|instruction| {
                    matches!(instruction.kind, SelectedInstructionKind::Load64 { .. })
                })
                .count();
            let restores = instructions
                .iter()
                .filter(|instruction| instruction.kind == from_kind)
                .count();
            // One store sequence for the origin definition; every reload is
            // address, load, restore in that order.
            assert!(loads >= 1 && loads == restores);
            // The `Store64` reads the conversion's carrier result, not the
            // victim register, and every produced carrier register holds the
            // raw bits in the rows' shared class.
            let store = instructions
                .iter()
                .find(|instruction| {
                    matches!(instruction.kind, SelectedInstructionKind::Store64 { .. })
                })
                .unwrap();
            let stored = store.operands[0].virtual_register;
            assert_ne!(stored, VirtualRegisterId(1));
            let carrier = function
                .virtual_registers
                .iter()
                .find(|register| register.id == stored)
                .unwrap();
            assert_eq!(carrier.class, carrier_class);
            assert_eq!(carrier.scalar_type, address_type);
            // Every rewritten consumer names a reload register in the
            // victim's own class and exact scalar type.
            for original_id in [2u32, 3, 4] {
                let rewritten = instructions
                    .iter()
                    .find(|instruction| instruction.id == SelectedInstructionId(original_id))
                    .unwrap();
                let reload = rewritten.operands[0].virtual_register;
                let produced = function
                    .virtual_registers
                    .iter()
                    .find(|register| register.id == reload)
                    .unwrap();
                assert_eq!(produced.class, float_class);
                assert_eq!(produced.scalar_type, scalar_type);
            }
            assert!(
                validate_runtime_spill(
                    &foreign_class,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    result.transformed().clone()
                )
                .is_ok()
            );
            // Replay demands the conversion stream verbatim: dropping the
            // restore, rebinding its input, or storing the victim directly
            // each fails independently.
            let mut dropped = result.transformed().clone();
            let position = dropped.functions[0].blocks[0]
                .instructions
                .iter()
                .position(|instruction| instruction.kind == from_kind)
                .unwrap();
            dropped.functions[0].blocks[0].instructions.remove(position);
            assert_eq!(
                validate_runtime_spill(
                    &foreign_class,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    dropped
                )
                .unwrap_err(),
                RuntimeSpillError::ReplayMismatch
            );
            let mut direct_store = result.transformed().clone();
            let position = direct_store.functions[0].blocks[0]
                .instructions
                .iter()
                .position(|instruction| {
                    matches!(instruction.kind, SelectedInstructionKind::Store64 { .. })
                })
                .unwrap();
            direct_store.functions[0].blocks[0].instructions[position].operands[0]
                .virtual_register = VirtualRegisterId(1);
            assert_eq!(
                validate_runtime_spill(
                    &foreign_class,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    direct_store
                )
                .unwrap_err(),
                RuntimeSpillError::ReplayMismatch
            );
            // A foreign-class victim without the float payload has no
            // declared pair to ride — still the candidate-local rejection
            // recovery expects, so it tries the next candidate instead of
            // aborting on a constraint mismatch.
            let mut foreign_nonfloat = source.clone();
            {
                let function = &mut Arc::make_mut(&mut foreign_nonfloat.transformed).functions[0];
                function.virtual_registers[1].class = float_class;
                function.virtual_registers[1].scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
                for instruction in &mut function.blocks[0].instructions[1..] {
                    instruction.operands[0].class = float_class;
                }
            }
            assert_eq!(
                spill_selected_runtime_value(
                    &foreign_nonfloat,
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
