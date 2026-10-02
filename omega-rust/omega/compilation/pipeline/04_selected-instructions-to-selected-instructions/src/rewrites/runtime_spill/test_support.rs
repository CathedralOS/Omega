//! Raw selected-stage fixture shared by spill rewriting and allocation tests.
//! This does not establish source or Terminal admission.

use super::{RuntimeSpillReceipt, ValidatedRuntimeSpill, admission};
use optimization_core::OptimizationUnitIdentity;
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
use std::sync::Arc;
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

/// A raw selected-stage unit fixture, not a source/Terminal admission claim.
pub fn runtime_spill_source(
    target: NativeTarget,
    optimization_unit: OptimizationUnitIdentity,
) -> ValidatedRuntimeSpill {
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
            optimization_unit,
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: Arc::new(plan),
    }
}
