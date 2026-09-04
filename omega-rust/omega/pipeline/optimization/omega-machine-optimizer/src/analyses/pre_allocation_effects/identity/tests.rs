use omega_optimization_core::OptimizationUnitIdentity;
use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    RegisterUnitId, TargetRegisterEnvironmentIdentity,
};
use omega_selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalogIdentity, MachineEncodedEffects, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSizeKnowledge, MachineTrapBehavior, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance,
};
use omega_target::NativeTarget;
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
    PreAllocationMachineEffectIdentity, PreAllocationMachineEffectPlan,
};

use super::pre_allocation_machine_effect_identity;

fn plan() -> PreAllocationMachineEffectPlan {
    let constraint = RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: 4,
    };
    let mut plan = PreAllocationMachineEffectPlan {
        identity: PreAllocationMachineEffectIdentity::from_bytes([0; 32]),
        selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
        optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target: NativeTarget::linux_x64(),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
        register_constraints: RegisterConstraintCatalogIdentity::from_bytes([4; 32]),
        machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([5; 32]),
        functions: vec![FunctionMachineEffects {
            machine: MachineId::new(1).unwrap(),
            blocks: vec![BlockMachineEffects {
                block: SelectedBlockId(0),
                instructions: vec![InstructionMachineEffects {
                    instruction: SelectedInstructionId(0),
                    kind: SelectedInstructionKind::CompareI64Zero,
                    constraint,
                    unit_uses: vec![RegisterUnitId(0)],
                    unit_defs: vec![RegisterUnitId(1)],
                    unit_clobbers: Vec::new(),
                    memory: MachineMemoryEffect::NoneV1,
                    trap: MachineTrapBehavior::NeverV1,
                    barrier: MachineBarrier::None,
                    call: MachineCallEffect::NoneV1,
                    cleanup: MachineCleanupEffect::NoneV1,
                    provenance: SelectedInstructionProvenance::default(),
                    alternatives: vec![MachineAlternative {
                        key: MachineAlternativeKey {
                            family: MachineAlternativeFamily::CompareI64Zero,
                            variant: 0,
                        },
                        applicability: MachineAlternativeApplicability::Always,
                        size: MachineSizeKnowledge::ExactBytes(3),
                        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                        encoded: MachineEncodedEffects::fallthrough_v1(vec![0], vec![]),
                    }],
                }],
            }],
        }],
        structural_unit_functions: Vec::new(),
    };
    plan.identity = pre_allocation_machine_effect_identity(&plan);
    plan
}

#[test]
fn identity_binds_roots_effect_rows_provenance_and_alternatives() {
    let source = plan();
    let baseline = source.identity;
    assert_eq!(baseline, pre_allocation_machine_effect_identity(&source));

    let mut changed = source.clone();
    changed.selected = SelectedInstructionPlanIdentity::from_bytes([9; 32]);
    assert_ne!(baseline, pre_allocation_machine_effect_identity(&changed));
    let mut changed = source.clone();
    changed.functions[0].blocks[0].instructions[0]
        .unit_clobbers
        .push(RegisterUnitId(2));
    assert_ne!(baseline, pre_allocation_machine_effect_identity(&changed));
    let mut changed = source.clone();
    changed.functions[0].blocks[0].instructions[0].barrier = MachineBarrier::ControlFlow;
    assert_ne!(baseline, pre_allocation_machine_effect_identity(&changed));
    let mut changed = source.clone();
    changed.functions[0].blocks[0].instructions[0].alternatives[0].size =
        MachineSizeKnowledge::ExactBytes(4);
    assert_ne!(baseline, pre_allocation_machine_effect_identity(&changed));
}
