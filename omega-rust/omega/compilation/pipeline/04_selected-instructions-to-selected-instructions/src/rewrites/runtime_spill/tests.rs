//! Measured-step controls for the runtime spill. The charge follows the
//! function the spill rewrites plus one step per plan function: function
//! storage is shared copy-on-write, so an unrelated body of any size is
//! cloned, compared and hashed as one shared entry. A required spill of a
//! small function therefore stays inside the pass budget however large the
//! program around it grows, and producer and replay agree on the boundary.

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use selected_instructions::{
    SelectedInstructionId, SelectedInstructionKind, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin, selected_instruction_plan_identity,
};
use semantic_vocabulary::MachineId;
use target::NativeTarget;

use super::test_support::runtime_spill_source;
use super::{
    RuntimeSpillError, RuntimeSpillReceipt, RuntimeSpillSpanPolicy, ValidatedRuntimeSpill,
    spill_selected_runtime_value_with_span_policy, validate_runtime_spill_with_span_policy,
};
use crate::rewrites::test_support::measured_step_budget;

const VICTIM: VirtualRegisterId = VirtualRegisterId(1);
const POLICY: RuntimeSpillSpanPolicy = RuntimeSpillSpanPolicy::UnitWriteBounded;

/// The one-function spill fixture followed by `extra` unrelated straight-line
/// functions, each copying its entry parameter into `body` fresh registers.
fn with_unrelated_functions(
    target: NativeTarget,
    environment: &ValidatedTargetRegisterEnvironment,
    extra: u64,
    body: u32,
) -> ValidatedRuntimeSpill {
    let source = runtime_spill_source(target, OptimizationUnitIdentity::from_bytes([2; 32]));
    let copy = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap();
    let mut plan = source.transformed().clone();
    let template = plan.functions[0].clone();
    for ordinal in 0..extra {
        let mut function = template.clone();
        function.machine = MachineId::new(2 + ordinal).unwrap();
        function.virtual_registers.truncate(1);
        let mut instructions = Vec::new();
        for position in 1..=body {
            let instruction = SelectedInstructionId(position);
            let register = VirtualRegisterId(position);
            let VirtualRegisterOrigin::InstructionResult { source_value, .. } =
                template.virtual_registers[1].origin
            else {
                unreachable!("the fixture's copies define instruction results")
            };
            function.virtual_registers.push(VirtualRegister {
                id: register,
                origin: VirtualRegisterOrigin::InstructionResult {
                    instruction,
                    source_value,
                },
                ..template.virtual_registers[1].clone()
            });
            instructions.push(crate::rewrites::instruction(
                instruction,
                SelectedInstructionKind::CopyI64,
                copy,
                &[VirtualRegisterId(0), register],
            ));
        }
        let block = &mut function.blocks[0];
        block.instructions = instructions;
        if let SelectedTerminator::Return { instruction, .. } = &mut block.terminator {
            instruction.id = SelectedInstructionId(body + 1);
        }
        plan.functions.push(function);
    }
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedRuntimeSpill {
        receipt: RuntimeSpillReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: source.receipt.optimization_unit,
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: Arc::new(plan),
    }
}

fn spill_within(
    source: &ValidatedRuntimeSpill,
    environment: &ValidatedTargetRegisterEnvironment,
    steps: u64,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    spill_selected_runtime_value_with_span_policy(
        source,
        0,
        VICTIM,
        environment,
        measured_step_budget(steps),
        POLICY,
    )
}

/// The smallest validation-step budget admitting the spill of `VICTIM` in
/// function zero.
fn required_steps(
    source: &ValidatedRuntimeSpill,
    environment: &ValidatedTargetRegisterEnvironment,
) -> u64 {
    let mut admitting = 1u64;
    while spill_within(source, environment, admitting).is_err() {
        admitting = admitting.checked_mul(2).unwrap();
        assert!(admitting < 1 << 24, "the fixture spill never admitted");
    }
    let mut starving = 0u64;
    while admitting - starving > 1 {
        let middle = starving + (admitting - starving) / 2;
        if spill_within(source, environment, middle).is_ok() {
            admitting = middle;
        } else {
            starving = middle;
        }
    }
    admitting
}

fn plan_instructions(source: &ValidatedRuntimeSpill) -> u64 {
    source
        .transformed()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .map(|block| block.instructions.len() as u64 + 1)
        .sum()
}

#[test]
fn unrelated_function_bodies_charge_one_step_each() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let alone = with_unrelated_functions(target, &environment, 0, 0);
        let widened = with_unrelated_functions(target, &environment, 16, 512);
        let single = required_steps(&alone, &environment);
        let required = required_steps(&widened, &environment);
        // Sixteen bodies of 513 instruction and block steps each add sixteen
        // steps, not the 8208 a whole-plan scan would charge.
        assert_eq!(required, single + 16);
        assert!(plan_instructions(&widened) > required * 100);
        assert_eq!(
            spill_within(&widened, &environment, required - 1),
            Err(RuntimeSpillError::WorkBudgetExceeded)
        );
        let spilled = spill_within(&widened, &environment, required).unwrap();
        // The charge is honest only because unrelated storage stays shared.
        for index in 1..widened.transformed().functions.len() {
            assert!(
                spilled
                    .transformed()
                    .functions
                    .shares_function_storage(&widened.transformed().functions, index)
            );
        }
        // Independent replay reconstructs the same charge at the same
        // boundary and admits exactly the produced plan.
        let replay = |steps| {
            validate_runtime_spill_with_span_policy(
                &widened,
                0,
                VICTIM,
                &environment,
                measured_step_budget(steps),
                spilled.transformed().clone(),
                POLICY,
            )
        };
        assert_eq!(replay(required).unwrap(), spilled);
        assert_eq!(
            replay(required - 1),
            Err(RuntimeSpillError::WorkBudgetExceeded)
        );
    }
}
