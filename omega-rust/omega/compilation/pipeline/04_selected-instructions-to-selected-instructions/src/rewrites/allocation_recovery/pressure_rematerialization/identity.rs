use crate::{
    PressureRematerializationIdentity, PressureRematerializationPlan,
    PressureRematerializationPolicy,
};

pub(crate) fn pressure_rematerialization_identity(
    plan: &PressureRematerializationPlan,
) -> PressureRematerializationIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-pressure-rematerialization.v2\0");
    bytes.extend_from_slice(&encode_terminal_pressure_rematerialization_content(plan));
    PressureRematerializationIdentity::from_canonical_bytes(&bytes)
}

pub(crate) fn encode_terminal_pressure_rematerialization_content(
    plan: &PressureRematerializationPlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source_selected.bytes());
    bytes.extend_from_slice(&plan.spill_choices.bytes());
    bytes.extend_from_slice(&plan.recovery_classifications.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 => 0,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1 => 1,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    bytes.extend_from_slice(
        &u64::try_from(plan.functions.len())
            .expect("function count fits u64")
            .to_le_bytes(),
    );
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        match &function.action {
            None => bytes.push(0),
            Some(action) => {
                bytes.push(1);
                bytes.extend_from_slice(&action.block.0.to_le_bytes());
                bytes.extend_from_slice(&action.pressure_point.0.to_le_bytes());
                bytes.extend_from_slice(&action.victim.0.to_le_bytes());
                bytes.extend_from_slice(&action.current_view.0.to_le_bytes());
                bytes.extend_from_slice(&action.reclaimed_view.0.to_le_bytes());
                bytes.extend_from_slice(&action.original_materialize.0.to_le_bytes());
                bytes.extend_from_slice(&action.source_value.get().to_le_bytes());
                match action.value {
                    semantic_vocabulary::IntegerValue::Unsigned(value) => {
                        bytes.extend_from_slice(&value.to_le_bytes());
                        bytes.push(0);
                    }
                    semantic_vocabulary::IntegerValue::Signed(value) => {
                        bytes.extend_from_slice(&value.to_le_bytes());
                        bytes.push(1);
                    }
                }
                bytes.extend_from_slice(
                    &u64::try_from(action.rewrites.len())
                        .expect("rewrite count fits u64")
                        .to_le_bytes(),
                );
                for rewrite in &action.rewrites {
                    bytes.extend_from_slice(&rewrite.point.0.to_le_bytes());
                    bytes.extend_from_slice(&rewrite.instruction.0.to_le_bytes());
                    bytes.extend_from_slice(&rewrite.operand.to_le_bytes());
                }
                bytes.extend_from_slice(&action.fresh_materialize.0.to_le_bytes());
                bytes.extend_from_slice(&action.result_virtual_register.0.to_le_bytes());
                bytes.push(match action.materialize_constraint.family {
                    register_model::RegisterConstraintFamily::Call => 0,
                    register_model::RegisterConstraintFamily::Return => 1,
                    register_model::RegisterConstraintFamily::SystemCall => 2,
                    register_model::RegisterConstraintFamily::InlineAssembly => 3,
                    register_model::RegisterConstraintFamily::Instruction => 4,
                });
                bytes.extend_from_slice(&action.materialize_constraint.variant.to_le_bytes());
            }
        }
    }
    bytes.extend_from_slice(&plan.transformed_selected.bytes());
    bytes
}
