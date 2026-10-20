//! The `.text` section: every retained function, then every compiler-private
//! function, in that order,
//! each with exactly one symbol and its retained records rebased to object
//! text offsets.

use super::function_validation::FunctionValidation;
use crate::object_artifact::private_functions::ValidatedPrivateFunction;
use crate::object_artifact::{
    ObjectBoundarySettlement, ObjectCodeAttribution, ObjectCompilerPrivateFunction, ObjectError,
    ObjectForeignCall, ObjectFunction, ObjectPortEffect,
};
use machine_code::MachineCodePlan;
use object_file::{
    FunctionSymbolPlan, ObjectPlan, ObjectSymbolHandle, SectionKind, SymbolKind, SymbolPlan,
    SymbolSection, entry_symbol_name,
};
use semantic_vocabulary::MachineId;

/// The retained functions as emitted, with their symbols and every rebased
/// record the artifact carries beside them.
pub(super) struct EmittedFunctions {
    pub(super) functions: Vec<ObjectFunction>,
    pub(super) symbols_by_machine: std::collections::BTreeMap<MachineId, ObjectSymbolHandle>,
    pub(super) semantic_code_attribution: Vec<ObjectCodeAttribution>,
    pub(super) port_effects: Vec<ObjectPortEffect>,
    pub(super) boundary_settlements: Vec<ObjectBoundarySettlement>,
    pub(super) foreign_calls: Vec<ObjectForeignCall>,
}

/// Append each retained function's bytes, give it its symbol (the entry
/// symbol for the entry machine), and rebase its validated stacks and
/// retained records to the object text offset.
pub(super) fn emit_functions(
    plan: &MachineCodePlan,
    validation: FunctionValidation,
    foreign_call_count: usize,
    object: &mut ObjectPlan,
    text_bytes: &mut Vec<u8>,
) -> Result<EmittedFunctions, ObjectError> {
    let FunctionValidation {
        text_size: _,
        mut validated_unit_stacks,
        mut validated_scalar_stacks,
        mut validated_foreign_call_stacks,
    } = validation;
    let mut functions = Vec::with_capacity(plan.functions.len());
    let mut semantic_code_attribution = Vec::new();
    let mut port_effects = Vec::new();
    let mut boundary_settlements = Vec::new();
    let mut foreign_calls = Vec::with_capacity(foreign_call_count);
    let mut symbols_by_machine = std::collections::BTreeMap::new();
    for function in &plan.functions {
        let text_offset = text_bytes.len();
        text_bytes.extend_from_slice(&function.bytes);
        let is_entry = function.machine == plan.entry;
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: if is_entry {
                entry_symbol_name(plan.target)
            } else {
                format!("omega_terminal_machine_{}", function.machine.get())
            },
            section: SymbolSection::Section(SectionKind::Text),
            offset: text_offset,
            size: function.bytes.len(),
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        if is_entry {
            object.layout.entry_symbol = symbol;
        }
        symbols_by_machine.insert(function.machine, symbol);
        for attribution in &function.semantic_code_attribution {
            semantic_code_attribution.push(ObjectCodeAttribution {
                machine: function.machine,
                attribution: *attribution,
                text_offset: text_offset
                    .checked_add(attribution.code_offset)
                    .ok_or(ObjectError::TextSizeOverflow)?,
            });
        }
        for effect in &function.port_effects {
            port_effects.push(ObjectPortEffect {
                machine: function.machine,
                effect: effect.clone(),
                text_offset: text_offset
                    .checked_add(effect.code_offset)
                    .ok_or(ObjectError::TextSizeOverflow)?,
            });
        }
        for settlement in &function.boundary_settlements {
            boundary_settlements.push(ObjectBoundarySettlement {
                machine: function.machine,
                settlement: settlement.clone(),
                text_offset: text_offset
                    .checked_add(settlement.code_offset)
                    .ok_or(ObjectError::TextSizeOverflow)?,
            });
        }
        for call in &function.foreign_calls {
            let caller_live_bytes = validated_foreign_call_stacks
                .remove(&(function.machine, call.owner))
                .expect("foreign stack validation precedes object projection");
            let scalar_result = call
                .scalar_result
                .clone()
                .map(|mut result| {
                    result.code_offset = text_offset
                        .checked_add(result.code_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    Ok(result)
                })
                .transpose()?;
            let scalar_arguments = call
                .scalar_arguments
                .iter()
                .cloned()
                .map(|mut argument| {
                    argument.code_offset = text_offset
                        .checked_add(argument.code_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    Ok(argument)
                })
                .collect::<Result<Vec<_>, ObjectError>>()?;
            let x86_floating_control = call
                .x86_floating_control
                .map(|mut control| {
                    control.save_offset = text_offset
                        .checked_add(control.save_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    control.restore_offset = text_offset
                        .checked_add(control.restore_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    Ok(control)
                })
                .transpose()?;
            let aarch64_floating_control = call
                .aarch64_floating_control
                .map(|mut control| {
                    control.save_offset = text_offset
                        .checked_add(control.save_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    control.restore_offset = text_offset
                        .checked_add(control.restore_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    Ok(control)
                })
                .transpose()?;
            let callback_addresses = call
                .callback_addresses
                .iter()
                .cloned()
                .map(|mut callback| {
                    callback.code_offset = text_offset
                        .checked_add(callback.code_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    match &mut callback.encoding {
                        machine_code::CallbackAddressEncoding::X86_64Relative32 {
                            relocation_offset,
                        } => {
                            *relocation_offset = text_offset
                                .checked_add(*relocation_offset)
                                .ok_or(ObjectError::TextSizeOverflow)?;
                        }
                        machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                            page_relocation_offset,
                            page_offset_relocation_offset,
                        } => {
                            *page_relocation_offset = text_offset
                                .checked_add(*page_relocation_offset)
                                .ok_or(ObjectError::TextSizeOverflow)?;
                            *page_offset_relocation_offset = text_offset
                                .checked_add(*page_offset_relocation_offset)
                                .ok_or(ObjectError::TextSizeOverflow)?;
                        }
                    }
                    Ok(callback)
                })
                .collect::<Result<Vec<_>, ObjectError>>()?;
            foreign_calls.push(ObjectForeignCall {
                machine: function.machine,
                owner: call.owner,
                operation_ordinal: call.operation_ordinal,
                binding: target_operations::NormalizedForeignCallBinding {
                    boundary_entry_plan: call.boundary_entry_plan.clone(),
                    mechanism: target_operations::NormalizedForeignCallMechanism::Import {
                        locator: call.locator.clone(),
                        same_stack_contribution: call.same_stack_contribution.clone(),
                    },
                },
                provider_execution: call.provider_execution,
                caller_live_bytes,
                scalar_arguments,
                callback_addresses,
                scalar_result,
                x86_floating_control,
                aarch64_floating_control,
                text_offset: text_offset
                    .checked_add(call.offset)
                    .ok_or(ObjectError::TextSizeOverflow)?,
            });
        }
        let (unit_stack, mut unit_call_stacks) = validated_unit_stacks
            .remove(&function.machine)
            .map_or((None, Vec::new()), |(stack, calls)| (Some(stack), calls));
        for call in &mut unit_call_stacks {
            call.text_offset = text_offset
                .checked_add(call.text_offset)
                .ok_or(ObjectError::TextSizeOverflow)?;
        }
        let (scalar_stack, mut scalar_call_stacks) = validated_scalar_stacks
            .remove(&function.machine)
            .map_or((None, Vec::new()), |(stack, calls)| (Some(stack), calls));
        for call in &mut scalar_call_stacks {
            call.text_offset = text_offset
                .checked_add(call.text_offset)
                .ok_or(ObjectError::TextSizeOverflow)?;
        }
        functions.push(ObjectFunction {
            machine: function.machine,
            attachment: function.attachment,
            nominal_cleanup_subject: function.nominal_cleanup_subject,
            scalar_abi: function.scalar_abi.clone(),
            mixed_structural_scalar_abi: function.mixed_structural_scalar_abi.clone(),
            structural_call_scalar_return: function.structural_call_scalar_return,
            parameter_abi: function.parameter_abi.clone(),
            provenance: function.provenance.clone(),
            symbol,
            text_offset,
            byte_count: function.bytes.len(),
            x86_floating_control: function.x86_floating_control,
            unit_stack,
            scalar_stack,
            unit_call_stacks,
            scalar_call_stacks,
            internal_unit_calls: function.internal_unit_calls.clone(),
            internal_unit_scalar_calls: function.internal_unit_scalar_calls.clone(),
            installed_provider_unit_scalar_calls: function
                .installed_provider_unit_scalar_calls
                .clone(),
            unit_scalar_homes: function.unit_scalar_homes.clone(),
            unit_integer_constants: function.unit_integer_constants.clone(),
            unit_affine_scalar_records: function.unit_affine_scalar_records.clone(),
            unit_structural_scalar_field_stores: function
                .unit_structural_scalar_field_stores
                .clone(),
            unit_write_only_primitive_stores: function.unit_write_only_primitive_stores.clone(),
            scalar_structural_scalar_field_stores: function
                .scalar_structural_scalar_field_stores
                .clone(),
            unit_parameters: function.unit_parameters.clone(),
            unit_parameter_homes: function.unit_parameter_homes.clone(),
            unit_continuations: function.unit_continuations.clone(),
            unit_affine_cleanup: function.unit_affine_cleanup.clone(),
            scalar_affine_cleanup: function.scalar_affine_cleanup.clone(),
            scalar_control_affine_cleanups: function.scalar_control_affine_cleanups.clone(),
            scalar_structural_parameters: function.scalar_structural_parameters.clone(),
            scalar_structural_parameter_homes: function.scalar_structural_parameter_homes.clone(),
            structural_return: function.structural_return.clone(),
        });
    }
    Ok(EmittedFunctions {
        functions,
        symbols_by_machine,
        semantic_code_attribution,
        port_effects,
        boundary_settlements,
        foreign_calls,
    })
}

/// Append each validated compiler-private function's bytes under its private
/// symbol, rejecting a symbol that collides with one already in the object.
pub(crate) fn emit_private_functions(
    validated_private_functions: Vec<ValidatedPrivateFunction<'_>>,
    object: &mut ObjectPlan,
    text_bytes: &mut Vec<u8>,
) -> Result<Vec<ObjectCompilerPrivateFunction>, ObjectError> {
    let mut object_private_functions = Vec::with_capacity(validated_private_functions.len());
    for private in validated_private_functions {
        let text_offset = text_bytes.len();
        text_bytes.extend_from_slice(&private.machine.function.bytes);
        if object
            .layout
            .symbols
            .iter()
            .any(|(_, symbol)| symbol.name == private.machine.private_symbol.as_ref())
        {
            return Err(ObjectError::PrivateFunctionSymbolCollision);
        }
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: private.machine.private_symbol.to_string(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: text_offset,
            size: private.machine.function.bytes.len(),
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object.layout.function_symbols.insert(FunctionSymbolPlan {
            identity: private.machine.identity,
            symbol,
        });
        let function = crate::ObjectPrivateFunctionEntry::from_function(
            &private.function,
            symbol,
            text_offset,
        );
        let mut symbol_bindings = arena::Arena::with_capacity(1);
        symbol_bindings.insert(crate::PrivateObjectSymbolBinding {
            local: private.body.object.layout.entry_symbol,
            linked: symbol,
        });
        object_private_functions.push(ObjectCompilerPrivateFunction {
            entry_bridge: None,
            identity: private.machine.identity,
            source_psi: private.machine.source_psi,
            function,
            body: private.body,
            text_offset,
            data_offset: 0,
            symbol_bindings,
        });
    }
    Ok(object_private_functions)
}
