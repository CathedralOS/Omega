use super::{Arc, RuntimeSplitReceipt};
use crate::rewrites::test_support::{budget, instruction, measured_step_budget};
use crate::{
    RuntimeSplitError, ValidatedRuntimeSplit, split_selected_runtime_value, validate_runtime_split,
};
use optimization_core::OptimizationUnitIdentity;
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
    SelectedFunction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId,
    OperationId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

const TARGETS: [fn() -> NativeTarget; 4] = [
    NativeTarget::linux_x64,
    NativeTarget::linux_arm64,
    NativeTarget::windows_x64,
    NativeTarget::macos_arm64,
];

fn scalar() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
}

fn register(id: u32, instruction: u32) -> VirtualRegister {
    VirtualRegister {
        id: VirtualRegisterId(id),
        scalar_type: scalar(),
        class: register_model::RegisterClassId(0),
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(instruction),
            source_value: ValueId::new(1).unwrap(),
        },
        definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
        entry_fixed_view: None,
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim.
/// Register 0 is a zero-argument scalar call's result, pinned to the
/// target's return view; two copies read it and the scalar return reads it
/// again through its own pinned view. One hosted settlement sits before the
/// second copy so the insertion's position shift is observable.
fn fixture(target: NativeTarget) -> ValidatedRuntimeSplit {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let call = environment.constraint(keys.call_scalar[0]).unwrap();
    assert_eq!(call.operands.len(), 1);
    assert_eq!(call.operands[0].access, RegisterOperandAccess::Def);
    assert!(call.operands[0].fixed_view.is_some());
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let machine = MachineId::new(1).unwrap();
    let instructions = vec![
        instruction(
            SelectedInstructionId(1),
            SelectedInstructionKind::CallScalar { callee: machine },
            call,
            &[VirtualRegisterId(0)],
        ),
        instruction(
            SelectedInstructionId(2),
            SelectedInstructionKind::CopyI64,
            copy,
            &[VirtualRegisterId(0), VirtualRegisterId(1)],
        ),
        instruction(
            SelectedInstructionId(3),
            SelectedInstructionKind::CopyI64,
            copy,
            &[VirtualRegisterId(0), VirtualRegisterId(2)],
        ),
    ];
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
            boundary_settlements: vec![settlement(0), settlement(2)],
            entry_block: SelectedBlockId(0),
            virtual_registers: vec![register(0, 1), register(1, 2), register(2, 3)],
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: selected_instructions::SelectedBlockOrigin::Source(
                    BlockId::new(1).unwrap(),
                ),
                instructions,
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        SelectedInstructionId(4),
                        SelectedInstructionKind::ReturnScalar,
                        environment.constraint(keys.return_i64).unwrap(),
                        &[VirtualRegisterId(0)],
                    ),
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        }]
        .into(),
    };
    source_of(plan)
}

fn settlement(instruction_index: u32) -> SelectedBoundarySettlement {
    SelectedBoundarySettlement {
        block: SelectedBlockId(0),
        instruction_index,
        settlement: SelectedBoundarySettlementPayload::HostedExitProcessI32 {
            operation: OperationId::new(1).unwrap(),
            boundary: BoundaryMachineId::new(1).unwrap(),
            source: ValueId::new(1).unwrap(),
        },
    }
}

fn source_of(plan: SelectedInstructionPlan) -> ValidatedRuntimeSplit {
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedRuntimeSplit {
        receipt: RuntimeSplitReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: Arc::new(plan),
    }
}

fn edited(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedRuntimeSplit {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut plan = fixture(target).transformed().clone();
    edit(&mut plan.functions[0], &environment);
    source_of(plan)
}

#[test]
fn split_copies_after_the_pinned_definition_and_rebinds_every_later_reference() {
    for target in TARGETS.map(|target| target()) {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let split =
            split_selected_runtime_value(&source, 0, VirtualRegisterId(0), &environment, budget())
                .unwrap();
        let function = &split.transformed().functions[0];
        let block = &function.blocks[0];
        assert_eq!(block.instructions.len(), 4);
        let copy = &block.instructions[1];
        assert_eq!(copy.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(copy.id, SelectedInstructionId(5));
        assert_eq!(copy.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(copy.operands[0].access, RegisterOperandAccess::Use);
        let fresh = copy.operands[1].virtual_register;
        assert_eq!(fresh, VirtualRegisterId(3));
        assert!(
            copy.operands
                .iter()
                .all(|operand| operand.fixed_view.is_none())
        );
        // The pinned write keeps the victim; nothing after the copy reads it.
        assert_eq!(
            block.instructions[0].operands[0].virtual_register,
            VirtualRegisterId(0)
        );
        assert!(block.instructions[0].operands[0].fixed_view.is_some());
        for later in &block.instructions[2..] {
            assert_eq!(later.operands[0].virtual_register, fresh);
        }
        let terminal = super::super::runtime_spill::control(&block.terminator)
            .0
            .operands[0];
        assert_eq!(terminal.virtual_register, fresh);
        assert!(terminal.fixed_view.is_some());
        let added = function.virtual_registers.last().unwrap();
        assert_eq!(added.id, fresh);
        assert_eq!(added.scalar_type, scalar());
        assert_eq!(
            added.origin,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(5),
                source_value: ValueId::new(1).unwrap(),
            }
        );
        assert_eq!(added.entry_fixed_view, None);
        // Settlements at or before the definition stay; later ones shift.
        let positions: Vec<u32> = function
            .boundary_settlements
            .iter()
            .map(|settlement| settlement.instruction_index)
            .collect();
        assert_eq!(positions, [0, 3]);
        assert_eq!(
            split.receipt().source_selected(),
            source.receipt().transformed_selected()
        );
        assert_eq!(
            split.receipt().transformed_selected(),
            selected_instruction_plan_identity(split.transformed())
        );
    }
}

#[test]
fn independent_replay_rejects_moved_omitted_and_corrupted_splits() {
    let target = NativeTarget::windows_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let split =
        split_selected_runtime_value(&source, 0, VirtualRegisterId(0), &environment, budget())
            .unwrap();
    assert!(
        validate_runtime_split(
            &source,
            0,
            VirtualRegisterId(0),
            &environment,
            budget(),
            split.transformed().clone(),
        )
        .is_ok()
    );
    for mutation in 0..9 {
        let mut proposed = split.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // An omitted split: the copy is gone but the rebinding remains.
            0 => {
                function.blocks[0].instructions.remove(1);
            }
            // A moved split: the copy runs after the first rebound reader.
            1 => function.blocks[0].instructions.swap(1, 2),
            // One reader left on the pinned register.
            2 => {
                function.blocks[0].instructions[3].operands[0].virtual_register =
                    VirtualRegisterId(0)
            }
            // The copy pinned to a physical view.
            3 => {
                function.blocks[0].instructions[1].operands[1].fixed_view =
                    Some(register_model::RegisterViewId(0))
            }
            4 => {
                function.virtual_registers[3].origin = VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(1),
                    source_value: ValueId::new(1).unwrap(),
                }
            }
            5 => {
                function.virtual_registers.pop();
            }
            6 => function.boundary_settlements[1].instruction_index = 2,
            7 => {
                function.blocks[0].instructions[1].provenance.values.clear();
            }
            // The whole split recorded against a different fresh identity.
            8 => {
                function.blocks[0].instructions[1].operands[1].virtual_register =
                    VirtualRegisterId(9)
            }
            _ => unreachable!(),
        }
        assert_eq!(
            validate_runtime_split(
                &source,
                0,
                VirtualRegisterId(0),
                &environment,
                budget(),
                proposed,
            ),
            Err(RuntimeSplitError::ReplayMismatch),
            "mutation {mutation}"
        );
    }
    // Replay against a different source — a stale analysis — rejects.
    let stale = edited(target, |function, _| {
        function.blocks[0].instructions.pop();
    });
    assert!(
        validate_runtime_split(
            &stale,
            0,
            VirtualRegisterId(0),
            &environment,
            budget(),
            split.transformed().clone(),
        )
        .is_err()
    );
}

#[test]
fn admission_rejects_unpinned_redefined_and_early_references() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let attempt = |source: &ValidatedRuntimeSplit, register: u32| {
        split_selected_runtime_value(
            source,
            0,
            VirtualRegisterId(register),
            &environment,
            budget(),
        )
        .err()
    };
    let source = fixture(target);
    // A copy-defined register has no pinned definition to split away from.
    assert_eq!(
        attempt(&source, 1),
        Some(RuntimeSplitError::UnsupportedValue)
    );
    // A read ahead of the definition in its own block.
    let early = edited(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions.insert(
            0,
            instruction(
                SelectedInstructionId(9),
                SelectedInstructionKind::CopyI64,
                copy,
                &[VirtualRegisterId(0), VirtualRegisterId(1)],
            ),
        );
    });
    assert_eq!(attempt(&early, 0), Some(RuntimeSplitError::UnsupportedUse));
    // A second write to the victim.
    let redefined = edited(target, |function, _| {
        function.blocks[0].instructions[2].operands[1].virtual_register = VirtualRegisterId(0);
    });
    assert_eq!(
        attempt(&redefined, 0),
        Some(RuntimeSplitError::UnsupportedUse)
    );
    // A tied read cannot be relocated.
    let tied = edited(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].tied_to = Some(1);
    });
    assert_eq!(attempt(&tied, 0), Some(RuntimeSplitError::UnsupportedUse));
    // An entry-pinned victim is outside the family.
    let entry = edited(target, |function, _| {
        function.virtual_registers[0].entry_fixed_view = Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        attempt(&entry, 0),
        Some(RuntimeSplitError::UnsupportedValue)
    );
    // A value nothing reads needs no split.
    let unread = edited(target, |function, _| {
        function.blocks[0].instructions.truncate(1);
        let terminal = super::super::runtime_spill::control_mut(&mut function.blocks[0].terminator);
        terminal.operands.clear();
    });
    assert_eq!(
        attempt(&unread, 0),
        Some(RuntimeSplitError::UnsupportedValue)
    );
    for error in [
        RuntimeSplitError::UnsupportedValue,
        RuntimeSplitError::UnsupportedUse,
    ] {
        assert!(error.is_inadmissible());
    }
    assert!(!RuntimeSplitError::WorkBudgetExceeded.is_inadmissible());
}

#[test]
fn foreign_environment_and_exhausted_budget_reject() {
    let source = fixture(NativeTarget::linux_x64());
    let foreign = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        split_selected_runtime_value(&source, 0, VirtualRegisterId(0), &foreign, budget()).err(),
        Some(RuntimeSplitError::SourceMismatch)
    );
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    assert_eq!(
        split_selected_runtime_value(
            &source,
            0,
            VirtualRegisterId(0),
            &environment,
            measured_step_budget(1),
        )
        .err(),
        Some(RuntimeSplitError::WorkBudgetExceeded)
    );
}
