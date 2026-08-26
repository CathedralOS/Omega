use super::super::data_addresses::collect_data_address_relocations;
use super::super::lookups::find_host_binding;
use super::super::offsets::{
    external_call_relocation_kind, external_call_relocation_offset_with_plan,
    external_call_relocation_width,
};
use super::context::InstructionRelocationContext;
use super::queries::selected_host_operation;
use omega_calling_conventions::{
    HostAbiPlan, HostBinding, HostBindingMechanism, HostImportLocator, HostOperationKey,
    PlatformCallData,
};
use omega_object_file::{
    RelocationRecord, object_symbol_handle_by_foreign_locator, object_symbol_handle_by_name,
};
use omega_target_operations::{InstructionOperand, SelectedInstructionKind};
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;

pub(super) fn collect_host_operation_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> Result<bool, Diagnostic> {
    let Some((operation_key, operands)) = selected_host_operation(instruction) else {
        return Ok(false);
    };

    validate_host_relocation_plan(context, operation_key, operands)?;

    collect_data_address_relocations(
        context.input,
        context.function_symbol_handle,
        context.selected_instruction_index,
        Some(operation_key),
        operands,
        context.selected_text_offset,
        context.relocation_plan,
    );
    collect_host_operation_call_relocation(context, operation_key, operands)?;
    Ok(true)
}

fn validate_host_relocation_plan(
    context: &InstructionRelocationContext<'_, '_>,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: HandleSpan<InstructionOperand>,
) -> Result<(), Diagnostic> {
    let Some(binding) =
        retained_host_binding_or_constant_result(context.input.host_abi, operation_key)?
    else {
        // The selected platform row materializes a deterministic constant and
        // transfers no control, so there is deliberately no host binding or
        // call relocation to validate. Result-place relocations are still
        // collected below by the constant-result layout.
        return Ok(());
    };
    let plan = binding.call_plan();
    if !matches!(binding.mechanism, HostBindingMechanism::Import { .. }) {
        return Ok(());
    }
    let operands = context
        .input
        .assigned_target_operations
        .instruction_operands(operands)
        .ok_or_else(|| Diagnostic::error("import relocation lost its selected operands"))?;
    if matches!(
        operation_key.capability,
        omega_calling_conventions::HostCapability::Custom(_)
            | omega_calling_conventions::HostCapability::Unknown
    ) {
        omega_instruction_selection::encode_authored_import_call_sequence(
            context.input.target,
            operands,
            plan,
        )?;
    } else {
        omega_instruction_selection::encode_host_call_sequence_with_plan(
            context.input.target,
            operation_key,
            operands,
            plan,
        )?;
    }
    Ok(())
}

fn retained_host_binding_or_constant_result(
    host_abi: &HostAbiPlan,
    operation_key: HostOperationKey,
) -> Result<Option<&HostBinding>, Diagnostic> {
    match retained_host_binding(host_abi, operation_key) {
        Ok(binding) => Ok(Some(binding)),
        Err(_) if selected_constant_result(host_abi, operation_key) => Ok(None),
        Err(diagnostic) => Err(diagnostic),
    }
}

fn selected_constant_result(host_abi: &HostAbiPlan, operation_key: HostOperationKey) -> bool {
    host_abi
        .platform_call_lowerings
        .iter()
        .any(|(_, lowering)| {
            matches!(lowering.data, PlatformCallData::ConstantResult { .. })
                && host_abi
                    .host_operations
                    .span(lowering.operations)
                    .is_some_and(|operations| {
                        operations
                            .iter()
                            .any(|operation| operation.key == operation_key)
                    })
        })
}

fn retained_host_binding(
    host_abi: &HostAbiPlan,
    operation_key: HostOperationKey,
) -> Result<&HostBinding, Diagnostic> {
    host_abi
        .bindings
        .iter()
        .find_map(|(_, binding)| (binding.operation_key == operation_key).then_some(binding))
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "cannot plan relocation for selected host operation `{}.{}` without its retained binding",
                operation_key.capability_name(),
                operation_key.operation_name()
            ))
        })
}

fn collect_host_operation_call_relocation(
    context: &mut InstructionRelocationContext<'_, '_>,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: HandleSpan<InstructionOperand>,
) -> Result<(), Diagnostic> {
    let Some(binding) = find_host_binding(context.input, operation_key) else {
        return Ok(());
    };
    let HostBindingMechanism::Import { locator } = &binding.mechanism else {
        return Ok(());
    };
    let symbol_handle = match locator {
        HostImportLocator::StringBackedBootstrap { symbol, .. } => {
            object_symbol_handle_by_name(&context.input.object, symbol.as_ref())
        }
        HostImportLocator::Normalized(locator) => {
            object_symbol_handle_by_foreign_locator(&context.input.object, locator)
        }
    };
    if !symbol_handle.is_valid() {
        return Err(Diagnostic::error(format!(
            "cannot plan relocation for selected host operation `{}.{}` without one exact import symbol",
            operation_key.capability_name(),
            operation_key.operation_name(),
        )));
    }
    let plan = binding.call_plan();
    let authored_import = matches!(
        operation_key.capability,
        omega_calling_conventions::HostCapability::Custom(_)
            | omega_calling_conventions::HostCapability::Unknown
    );

    context
        .relocation_plan
        .record_set
        .records
        .insert(RelocationRecord {
            origin: omega_object_file::RelocationOrigin::Instruction {
                function_symbol_handle: context.function_symbol_handle,
                selected_instruction_index: context.selected_instruction_index,
            },
            section: omega_object_file::SectionKind::Text,
            offset: external_call_relocation_offset_with_plan(
                context.input.target,
                operation_key,
                context.selected_text_offset,
                context
                    .input
                    .assigned_target_operations
                    .instruction_operands(operands)
                    .unwrap_or(&[]),
                authored_import,
                plan,
            ),
            byte_width: external_call_relocation_width(context.input.target.architecture),
            symbol_handle,
            addend: 0,
            kind: external_call_relocation_kind(context.input.target.architecture),
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{retained_host_binding, retained_host_binding_or_constant_result};
    use omega_calling_conventions::{
        HostCapability, HostOperation, HostOperationKey, build_host_abi_plan,
    };
    use omega_target::NativeTarget;

    #[test]
    fn selected_host_operation_without_retained_binding_rejects() {
        let host_abi = build_host_abi_plan(NativeTarget::linux_x64());
        let missing = HostOperationKey::new(HostCapability::Unknown, HostOperation::Unknown);
        let error = retained_host_binding(&host_abi, missing)
            .expect_err("a selected host operation cannot use the compatibility layout");
        assert!(error.message.contains("without its retained binding"));
    }

    #[test]
    fn selected_constant_result_needs_no_host_binding() {
        let host_abi = build_host_abi_plan(NativeTarget::linux_x64());
        let constant = HostOperationKey::new(
            HostCapability::Clock,
            HostOperation::WallClockUnitsPerSecond,
        );
        assert!(
            retained_host_binding_or_constant_result(&host_abi, constant)
                .expect("selected constant-result lowering")
                .is_none()
        );
    }
}
