use omega_abstract_operations::AbstractOperationPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_target::NativeTarget;
use omega_target_operations::{RuntimeTextReadSource, TargetOperationPlan};

pub fn build_target_operation_plan(
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    abstract_operations: &AbstractOperationPlan,
) -> TargetOperationPlan {
    let mut target_operations = TargetOperationPlan {
        target,
        functions: abstract_operations.functions.clone(),
        instructions: abstract_operations.instructions.clone(),
        operands: abstract_operations.operands.clone(),
        runtime_value_operands: abstract_operations.runtime_value_operands.clone(),
        host_bindings: omega_core::arena::Arena::new(),
    };

    for (_, instruction) in abstract_operations.instructions.iter() {
        let omega_abstract_operations::AbstractOperationKind::ReadRuntimeTextLine {
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        } = &instruction.kind
        else {
            continue;
        };

        if target_operations.host_binding(*operation_key).is_some() {
            continue;
        }

        if let Some((_, binding)) = host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == *operation_key)
        {
            target_operations.host_bindings.insert(binding.clone());
        }
    }

    target_operations
}
