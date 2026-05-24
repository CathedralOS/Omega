use omega_abstract_operations::AbstractOperationPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_core::arena::{Handle, HandleSpan};
use omega_target::NativeTarget;
use omega_target_operations::{TargetOperation, TargetOperationFunction, TargetOperationPlan};

pub fn build_target_operation_plan(
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    abstract_operations: &AbstractOperationPlan,
) -> TargetOperationPlan {
    let mut target_operations = TargetOperationPlan::with_capacity(
        target,
        abstract_operations.functions.len(),
        abstract_operations.instructions.len(),
        abstract_operations.operands.len(),
        abstract_operations.runtime_value_operands.len(),
    );

    target_operations.operands = abstract_operations.operands.clone();
    target_operations.runtime_value_operands = abstract_operations.runtime_value_operands.clone();

    for (_, instruction) in abstract_operations.instructions.iter() {
        target_operations
            .instructions
            .insert(TargetOperation::from(instruction));
    }

    for (_, function) in abstract_operations.functions.iter() {
        target_operations
            .functions
            .insert(TargetOperationFunction {
                symbol: std::sync::Arc::clone(&function.symbol),
                source_key: function.source_key,
                instructions: remap_instruction_span(function.instructions),
            });
    }

    for (instruction_key, instruction) in abstract_operations.instructions.iter() {
        let omega_abstract_operations::AbstractOperationKind::ReadRuntimeTextLine {
            ..
        } = &instruction.kind
        else {
            continue;
        };

        let omega_target_operations::TargetOperationKind::ReadRuntimeTextLine {
            source: omega_target_operations::RuntimeTextReadSource::HostOperation { operation_key },
            ..
        } = &target_operations
            .instructions
            .get(remap_instruction_handle(instruction_key))
            .kind
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

fn remap_instruction_handle(
    handle: omega_core::arena::Handle<omega_abstract_operations::AbstractOperation>,
) -> omega_core::arena::Handle<TargetOperation> {
    omega_core::arena::Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_instruction_span(
    span: omega_core::arena::HandleSpan<omega_abstract_operations::AbstractOperation>,
) -> HandleSpan<TargetOperation> {
    if span.is_empty() {
        return HandleSpan::empty();
    }

    HandleSpan::from_parts(
        Handle::from_parts(span.start().arena_index(), span.start().generation()),
        span.count(),
    )
}
