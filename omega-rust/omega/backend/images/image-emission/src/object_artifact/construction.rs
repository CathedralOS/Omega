//! The public builder entries and the object construction route: validate
//! every retained machine-code function (`function_validation`), then lay
//! out and seal text, data, symbols and relocations from the validated
//! result.

use crate::object_artifact::call_sites::{validate_foreign_call_site, validate_internal_call_site};
use crate::object_artifact::private_functions::validate_private_functions;
use crate::object_artifact::replay::dynamic::forwarded_descriptor::{
    ValidatedForwardedDynamicApplication, validate_forwarded_dynamic_descriptors,
};
use crate::object_artifact::replay::dynamic::forwarded_parameter::validate_forwarded_dynamic_parameter_calls;
use crate::object_artifact::{
    ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution, ObjectCompilerPrivateFunction,
    ObjectDynamicConformanceSlot, ObjectDynamicConformanceTable, ObjectError, ObjectForeignCall,
    ObjectForwardedDynamicDescriptorAdapter, ObjectForwardedDynamicDescriptorSlot,
    ObjectForwardedDynamicDescriptorTable, ObjectFunction, ObjectPortEffect,
};
use machine_code::{
    CompilerPrivateMachineCodeFunction, MachineCodePlan, MachineCodePlanWithPrivateFunctions,
};
use object_file::{
    FunctionSymbolPlan, NormalizedImportPlan, ObjectPlan, ObjectSymbolHandle, RelocationKind,
    RelocationOrigin, RelocationPlan, RelocationRecord, SectionKind, SectionPlan, SymbolKind,
    SymbolPlan, SymbolSection, entry_symbol_name, normalized_foreign_import_symbol_name,
};
use target::Architecture;
use target_operations::CallSiteOwner;

mod function_validation;

/// Construct a self-contained object plan and exact text carrier.
///
/// Function order is semantic-artifact order and must already be canonical by
/// `MachineId`; this boundary rejects alternate ordering rather than silently
/// normalizing it. Each function gets exactly one symbol and one retained Psi
/// provenance row.
pub fn build_object_artifact(plan: &MachineCodePlan) -> Result<ObjectArtifact, ObjectError> {
    build_object_artifact_with_x86_feature_profile(plan, &[], None, None)
}

/// Construct an object that owns semantic program functions and a disjoint,
/// placement-identified compiler-private callback-function roster.
pub fn build_object_artifact_with_private_functions(
    plan: &MachineCodePlanWithPrivateFunctions,
) -> Result<ObjectArtifact, ObjectError> {
    build_object_artifact_with_x86_feature_profile(&plan.plan, &plan.private_functions, None, None)
}

/// Construct the bounded source-free object seam for feature-requiring scalar
/// x86 FMA. The profile is explicit because `NativeTarget` deliberately
/// collapses Windows and UEFI x86-64 physical layouts.
pub fn build_feature_required_x86_fma_object_artifact(
    plan: &MachineCodePlan,
    profile: target::TargetProfile,
) -> Result<ObjectArtifact, ObjectError> {
    if !plan
        .functions
        .iter()
        .any(|function| !function.x86_scalar_fma.is_empty())
    {
        return Err(ObjectError::MissingX86ScalarFmaFragment);
    }
    build_object_artifact_with_x86_feature_profile(plan, &[], Some(profile), None)
}

/// Consume exact deployment-feature and differential authority while building
/// an object whose generic F32/F64 FMA slots may enter executable emission.
/// Ordinary and feature-required-only builders retain their fail-closed
/// baseline behavior.
pub fn build_admitted_x86_fma_object_artifact(
    plan: &MachineCodePlan,
    provider: target::AdmittedX86ScalarFmaProvider,
) -> Result<ObjectArtifact, ObjectError> {
    if !provider.has_canonical_identity() {
        return Err(ObjectError::InvalidX86ScalarFmaProviderAdmission);
    }
    if provider.profile().native_target() != plan.target
        || plan
            .functions
            .iter()
            .flat_map(|function| &function.x86_scalar_fma)
            .any(|fragment| {
                let slot = match fragment.format {
                    machine_code::X86ScalarFmaFormat::Binary32 => {
                        target::X86ScalarFmaSlot::Binary32
                    }
                    machine_code::X86ScalarFmaFormat::Binary64 => {
                        target::X86ScalarFmaSlot::Binary64
                    }
                };
                !provider.admits(fragment.requirement, slot)
            })
    {
        return Err(ObjectError::InvalidX86ScalarFmaProviderAdmission);
    }
    if !plan
        .functions
        .iter()
        .any(|function| !function.x86_scalar_fma.is_empty())
    {
        return Err(ObjectError::MissingX86ScalarFmaFragment);
    }
    build_object_artifact_with_x86_feature_profile(
        plan,
        &[],
        Some(provider.profile()),
        Some(provider),
    )
}

pub(crate) fn same_dynamic_table_application(
    left: &terminal_psi::ClosedConformanceApplication,
    right: &terminal_psi::ClosedConformanceApplication,
) -> bool {
    left.commitment == right.commitment
        && left.declaration_identity == right.declaration_identity
        && left.telescope == right.telescope
        && left.subject_identity == right.subject_identity
        && left.trait_identity == right.trait_identity
        && left.trait_lifetime_arguments == right.trait_lifetime_arguments
        && left.trait_arguments == right.trait_arguments
        && left.realization_callables == right.realization_callables
        && left.rows == right.rows
        && left.report_fingerprint == right.report_fingerprint
}

pub(crate) fn build_object_artifact_with_x86_feature_profile(
    plan: &MachineCodePlan,
    private_functions: &[CompilerPrivateMachineCodeFunction],
    x86_feature_profile: Option<target::TargetProfile>,
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
) -> Result<ObjectArtifact, ObjectError> {
    if plan.functions.is_empty() {
        return Err(ObjectError::EmptyPlan);
    }
    let forwarded_dynamic_applications =
        validate_forwarded_dynamic_descriptors(plan.target, &plan.functions)?;
    validate_forwarded_dynamic_parameter_calls(plan.target, &plan.functions)?;
    let validated_private_functions = validate_private_functions(plan.target, private_functions)?;
    let function_validation::FunctionValidation {
        mut text_size,
        mut validated_unit_stacks,
        mut validated_scalar_stacks,
        mut validated_foreign_call_stacks,
    } = function_validation::validate_functions(
        plan,
        x86_feature_profile,
        x86_scalar_fma_provider,
    )?;
    for private in &validated_private_functions {
        text_size = text_size
            .checked_add(private.machine.function.bytes.len())
            .ok_or(ObjectError::TextSizeOverflow)?;
    }
    for application in &forwarded_dynamic_applications {
        for adapter in &application.adapters {
            text_size = text_size
                .checked_add(adapter.bytes.len())
                .ok_or(ObjectError::TextSizeOverflow)?;
        }
    }

    let dynamic_applications = collect_dynamic_applications(plan)?;

    let foreign_call_count = plan
        .functions
        .iter()
        .map(|function| function.foreign_calls.len())
        .sum::<usize>();
    let symbol_capacity = plan
        .functions
        .len()
        .saturating_add(private_functions.len())
        .saturating_add(foreign_call_count)
        .saturating_add(dynamic_applications.len())
        .saturating_add(forwarded_dynamic_applications.len())
        .saturating_add(
            forwarded_dynamic_applications
                .iter()
                .map(|application| application.adapters.len())
                .sum::<usize>(),
        );
    let mut object = if private_functions.is_empty() {
        ObjectPlan::with_capacity(plan.target, 1, symbol_capacity)
    } else {
        ObjectPlan::with_capacities(plan.target, 1, symbol_capacity, private_functions.len())
    };
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: text_size,
        alignment: 16,
    });
    let dynamic_data_size =
        dynamic_data_section_size(&dynamic_applications, &forwarded_dynamic_applications)?;
    if dynamic_data_size != 0 {
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Data,
            size: dynamic_data_size,
            alignment: 8,
        });
    }

    let mut text_bytes = Vec::with_capacity(text_size);
    let mut functions = Vec::with_capacity(plan.functions.len());
    let mut object_private_functions = Vec::with_capacity(private_functions.len());
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
            let callback_address = call
                .callback_address
                .clone()
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
                .transpose()?;
            foreign_calls.push(ObjectForeignCall {
                machine: function.machine,
                owner: call.owner,
                operation_ordinal: call.operation_ordinal,
                locator: call.locator.clone(),
                provider_execution: call.provider_execution,
                boundary_entry_plan: call.boundary_entry_plan.clone(),
                caller_live_bytes,
                same_stack_contribution: call.same_stack_contribution.clone(),
                scalar_arguments,
                callback_address,
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
            scalar_abi: function.scalar_abi.clone(),
            mixed_structural_scalar_abi: function.mixed_structural_scalar_abi.clone(),
            structural_call_scalar_return: function.structural_call_scalar_return,
            parameter_abi: function.parameter_abi.clone(),
            provenance: function.provenance.clone(),
            symbol,
            text_offset,
            byte_count: function.bytes.len(),
            x86_scalar_fma: function.x86_scalar_fma.clone(),
            x86_scalar_fma_occurrences: function.x86_scalar_fma_occurrences.clone(),
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
            dynamic_calls: function.dynamic_calls.clone(),
            stored_dynamic_calls: function.stored_dynamic_calls.clone(),
            dynamic_parameter_calls: function.dynamic_parameter_calls.clone(),
            forwarded_dynamic_parameter_calls: function.forwarded_dynamic_parameter_calls.clone(),
            forwarded_dynamic_descriptor_calls: function.forwarded_dynamic_descriptor_calls.clone(),
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

    let mut forwarded_dynamic_descriptor_adapters = Vec::new();
    let mut forwarded_adapter_symbols = std::collections::BTreeMap::new();
    for application in &forwarded_dynamic_applications {
        for adapter in &application.adapters {
            let target_symbol = symbols_by_machine
                .get(&adapter.identity.realization)
                .copied()
                .ok_or(ObjectError::UnknownDynamicConformanceTarget(
                    adapter.identity.realization,
                ))?;
            let text_offset = text_bytes.len();
            text_bytes.extend_from_slice(&adapter.bytes);
            let commitment = adapter
                .identity
                .application
                .as_bytes()
                .into_iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let symbol = object.layout.symbols.insert(SymbolPlan {
                name: format!(
                    "omega_forwarded_descriptor_adapter_{commitment}_{}_{}",
                    adapter.identity.row_index,
                    adapter.identity.realization.get()
                ),
                section: SymbolSection::Section(SectionKind::Text),
                offset: text_offset,
                size: adapter.bytes.len(),
                kind: SymbolKind::Function,
                import_library: String::new(),
            });
            if forwarded_adapter_symbols
                .insert(adapter.identity.clone(), symbol)
                .is_some()
            {
                return Err(ObjectError::DuplicateForwardedDynamicDescriptorAdapter);
            }
            forwarded_dynamic_descriptor_adapters.push(ObjectForwardedDynamicDescriptorAdapter {
                record: adapter.clone(),
                symbol,
                target_symbol,
                text_offset,
                byte_count: adapter.bytes.len(),
            });
        }
    }

    let mut data_bytes = Vec::with_capacity(dynamic_data_size);
    let mut dynamic_conformance_tables = Vec::with_capacity(dynamic_applications.len());
    for (table_index, application) in dynamic_applications.into_iter().enumerate() {
        if application.rows.is_empty() {
            return Err(ObjectError::InvalidDynamicConformanceTable);
        }
        let data_offset = data_bytes.len();
        let byte_count = application
            .rows
            .len()
            .checked_mul(8)
            .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?;
        data_bytes.resize(
            data_bytes
                .len()
                .checked_add(byte_count)
                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
            0,
        );
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: format!(
                "omega_dynamic_conformance_table_{table_index}_{}",
                application.report_fingerprint
            ),
            section: SymbolSection::Section(SectionKind::Data),
            offset: data_offset,
            size: byte_count,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let slots = application
            .rows
            .iter()
            .enumerate()
            .map(|(row_index, row)| {
                let (target, target_symbol) = match &row.realization_callable_identity {
                    Some(callable_identity) => {
                        let matching = application
                            .realization_callables
                            .iter()
                            .filter(|callable| {
                                callable.source_callable_identity == *callable_identity
                            })
                            .collect::<Vec<_>>();
                        let [callable] = matching.as_slice() else {
                            return Err(ObjectError::InvalidDynamicConformanceTable);
                        };
                        let target_symbol =
                            symbols_by_machine.get(&callable.machine).copied().ok_or(
                                ObjectError::UnknownDynamicConformanceTarget(callable.machine),
                            )?;
                        (Some(callable.machine), Some(target_symbol))
                    }
                    None => (None, None),
                };
                Ok(ObjectDynamicConformanceSlot {
                    row_index: u32::try_from(row_index)
                        .map_err(|_| ObjectError::DynamicConformanceDataSizeOverflow)?,
                    realization_callable_identity: row.realization_callable_identity.clone(),
                    target,
                    target_symbol,
                    data_offset: data_offset
                        .checked_add(
                            row_index
                                .checked_mul(8)
                                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                        )
                        .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                })
            })
            .collect::<Result<Vec<_>, ObjectError>>()?;
        dynamic_conformance_tables.push(ObjectDynamicConformanceTable {
            application,
            symbol,
            data_offset,
            byte_count,
            slots,
        });
    }

    let mut forwarded_dynamic_descriptor_tables =
        Vec::with_capacity(forwarded_dynamic_applications.len());
    for application in &forwarded_dynamic_applications {
        if application.adapters.is_empty() {
            return Err(ObjectError::InvalidForwardedDynamicDescriptorTable);
        }
        let data_offset = data_bytes.len();
        let byte_count = application
            .adapters
            .len()
            .checked_mul(8)
            .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?;
        data_bytes.resize(
            data_bytes
                .len()
                .checked_add(byte_count)
                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
            0,
        );
        let commitment = application
            .application
            .commitment
            .as_bytes()
            .into_iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: format!("omega_forwarded_descriptor_table_{commitment}"),
            section: SymbolSection::Section(SectionKind::Data),
            offset: data_offset,
            size: byte_count,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let slots = application
            .adapters
            .iter()
            .enumerate()
            .map(|(row_index, adapter)| {
                let adapter_symbol = forwarded_adapter_symbols
                    .get(&adapter.identity)
                    .copied()
                    .ok_or(ObjectError::InvalidForwardedDynamicDescriptorTable)?;
                Ok(ObjectForwardedDynamicDescriptorSlot {
                    row_index: u32::try_from(row_index)
                        .map_err(|_| ObjectError::DynamicConformanceDataSizeOverflow)?,
                    adapter: adapter.identity.clone(),
                    adapter_symbol,
                    data_offset: data_offset
                        .checked_add(
                            row_index
                                .checked_mul(8)
                                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                        )
                        .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                })
            })
            .collect::<Result<Vec<_>, ObjectError>>()?;
        forwarded_dynamic_descriptor_tables.push(ObjectForwardedDynamicDescriptorTable {
            application: application.application.clone(),
            symbol,
            data_offset,
            byte_count,
            slots,
        });
    }

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
        let mut function = private.function;
        function.symbol = symbol;
        function.text_offset = text_offset;
        object_private_functions.push(ObjectCompilerPrivateFunction {
            identity: private.machine.identity,
            source_psi: private.machine.source_psi,
            function,
        });
    }

    let mut import_symbols = Vec::<(target::NormalizedForeignLocator, ObjectSymbolHandle)>::new();
    for function in &plan.functions {
        for call in &function.foreign_calls {
            if import_symbols
                .iter()
                .any(|(locator, _)| locator == &call.locator)
            {
                continue;
            }
            if import_symbols.iter().any(|(locator, _)| {
                locator.non_authoritative_compatibility_fingerprint()
                    == call.locator.non_authoritative_compatibility_fingerprint()
            }) {
                return Err(ObjectError::ForeignLocatorIdentityCollision {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            let symbol = object.layout.symbols.insert(SymbolPlan {
                name: normalized_foreign_import_symbol_name(&call.locator),
                section: SymbolSection::None,
                offset: 0,
                size: 0,
                kind: SymbolKind::Import,
                import_library: String::new(),
            });
            object.layout.normalized_imports.push(NormalizedImportPlan {
                symbol,
                locator: call.locator.clone(),
            });
            import_symbols.push((call.locator.clone(), symbol));
        }
    }

    let mut relocations = RelocationPlan::with_record_capacity(
        plan.target,
        relocation_record_capacity(
            plan,
            &dynamic_conformance_tables,
            &forwarded_dynamic_descriptor_tables,
            &forwarded_dynamic_descriptor_adapters,
        ),
    );
    for table in &dynamic_conformance_tables {
        for slot in &table.slots {
            let Some(target_symbol) = slot.target_symbol else {
                continue;
            };
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: table.symbol,
                },
                section: SectionKind::Data,
                offset: slot.data_offset,
                byte_width: 8,
                symbol_handle: target_symbol,
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }
    }
    for table in &forwarded_dynamic_descriptor_tables {
        for slot in &table.slots {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: table.symbol,
                },
                section: SectionKind::Data,
                offset: slot.data_offset,
                byte_width: 8,
                symbol_handle: slot.adapter_symbol,
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }
    }
    for adapter in &forwarded_dynamic_descriptor_adapters {
        let (offset, kind) = match plan.target.architecture {
            Architecture::X86_64 => (
                adapter
                    .text_offset
                    .checked_add(adapter.record.direct_call_offset)
                    .and_then(|offset| offset.checked_add(1))
                    .ok_or(ObjectError::TextSizeOverflow)?,
                RelocationKind::X86_64Relative32,
            ),
            Architecture::Aarch64 => (
                adapter
                    .text_offset
                    .checked_add(adapter.record.direct_call_offset)
                    .ok_or(ObjectError::TextSizeOverflow)?,
                RelocationKind::Aarch64Branch26,
            ),
        };
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: adapter.symbol,
            },
            section: SectionKind::Text,
            offset,
            byte_width: 4,
            symbol_handle: adapter.target_symbol,
            addend: 0,
            kind,
        });
    }
    for (function, emitted) in plan.functions.iter().zip(&functions) {
        for call in &function.dynamic_calls {
            let table = dynamic_conformance_tables
                .iter()
                .find(|table| {
                    table.application.commitment == call.dynamic_dispatch.application.commitment
                        && same_dynamic_table_application(
                            &table.application,
                            &call.dynamic_dispatch.application,
                        )
                })
                .ok_or(ObjectError::InvalidDynamicConformanceTable)?;
            let origin = RelocationOrigin::SemanticOperation {
                function_symbol_handle: emitted.symbol,
                operation_identity: call.psi_operation.get(),
            };
            let mut push_address =
                |local_offset: usize, kind: RelocationKind| -> Result<(), ObjectError> {
                    relocations.push_record(RelocationRecord {
                        origin,
                        section: SectionKind::Text,
                        offset: emitted
                            .text_offset
                            .checked_add(local_offset)
                            .ok_or(ObjectError::TextSizeOverflow)?,
                        byte_width: 4,
                        symbol_handle: table.symbol,
                        addend: 0,
                        kind,
                    });
                    Ok(())
                };
            match call.table_address.encoding {
                machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                    relocation_offset,
                } => push_address(relocation_offset, RelocationKind::X86_64Relative32)?,
                machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                } => {
                    push_address(page_relocation_offset, RelocationKind::Aarch64Page21)?;
                    push_address(
                        page_offset_relocation_offset,
                        RelocationKind::Aarch64PageOffset12,
                    )?;
                }
            }
        }
        for call in &function.stored_dynamic_calls {
            let establishment = &call.establishment;
            let table = dynamic_conformance_tables
                .iter()
                .find(|table| {
                    table.application.commitment == establishment.stored.application.commitment
                        && same_dynamic_table_application(
                            &table.application,
                            &establishment.stored.application,
                        )
                })
                .ok_or(ObjectError::InvalidDynamicConformanceTable)?;
            let origin = RelocationOrigin::SemanticOperation {
                function_symbol_handle: emitted.symbol,
                operation_identity: establishment.psi_operation.get(),
            };
            let mut push_address =
                |local_offset: usize, kind: RelocationKind| -> Result<(), ObjectError> {
                    relocations.push_record(RelocationRecord {
                        origin,
                        section: SectionKind::Text,
                        offset: emitted
                            .text_offset
                            .checked_add(local_offset)
                            .ok_or(ObjectError::TextSizeOverflow)?,
                        byte_width: 4,
                        symbol_handle: table.symbol,
                        addend: 0,
                        kind,
                    });
                    Ok(())
                };
            match establishment.table_address.encoding {
                machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                    relocation_offset,
                } => push_address(relocation_offset, RelocationKind::X86_64Relative32)?,
                machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                } => {
                    push_address(page_relocation_offset, RelocationKind::Aarch64Page21)?;
                    push_address(
                        page_offset_relocation_offset,
                        RelocationKind::Aarch64PageOffset12,
                    )?;
                }
            }
        }
        for call in &function.forwarded_dynamic_descriptor_calls {
            for argument in &call.dynamic_arguments {
                let application = match &argument.custody.source {
                    abstract_operations::AbstractDynamicDescriptorSource::Selection {
                        application,
                        ..
                    }
                    | abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                        application,
                        ..
                    } => application,
                    abstract_operations::AbstractDynamicDescriptorSource::Parameter(_) => {
                        return Err(ObjectError::InvalidForwardedDynamicDescriptorEvidence {
                            caller: function.machine,
                            operation: call.psi_operation,
                        });
                    }
                };
                let table = forwarded_dynamic_descriptor_tables
                    .iter()
                    .find(|table| {
                        table.application.commitment == application.commitment
                            && same_dynamic_table_application(&table.application, application)
                    })
                    .ok_or(ObjectError::InvalidForwardedDynamicDescriptorTable)?;
                let origin = RelocationOrigin::SemanticOperation {
                    function_symbol_handle: emitted.symbol,
                    operation_identity: call.psi_operation.get(),
                };
                let mut push_address =
                    |local_offset: usize, kind: RelocationKind| -> Result<(), ObjectError> {
                        relocations.push_record(RelocationRecord {
                            origin,
                            section: SectionKind::Text,
                            offset: emitted
                                .text_offset
                                .checked_add(local_offset)
                                .ok_or(ObjectError::TextSizeOverflow)?,
                            byte_width: 4,
                            symbol_handle: table.symbol,
                            addend: 0,
                            kind,
                        });
                        Ok(())
                    };
                match argument.table_address.encoding {
                    machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                        relocation_offset,
                    } => push_address(relocation_offset, RelocationKind::X86_64Relative32)?,
                    machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                        page_relocation_offset,
                        page_offset_relocation_offset,
                    } => {
                        push_address(page_relocation_offset, RelocationKind::Aarch64Page21)?;
                        push_address(
                            page_offset_relocation_offset,
                            RelocationKind::Aarch64PageOffset12,
                        )?;
                    }
                }
            }
        }
        for call in &function.internal_calls {
            let target_symbol = symbols_by_machine.get(&call.target).copied().ok_or(
                ObjectError::UnknownInternalCallTarget {
                    caller: function.machine,
                    target: call.target,
                },
            )?;
            let (kind, byte_width) = validate_internal_call_site(
                plan.target.architecture,
                function.machine,
                &function.bytes,
                *call,
            )?;
            let offset = emitted
                .text_offset
                .checked_add(call.offset)
                .ok_or(ObjectError::TextSizeOverflow)?;
            let origin = match call.owner {
                CallSiteOwner::Operation(operation) => RelocationOrigin::SemanticOperation {
                    function_symbol_handle: emitted.symbol,
                    operation_identity: operation.get(),
                },
                CallSiteOwner::CleanupAction { edge, .. } => RelocationOrigin::SemanticEdge {
                    function_symbol_handle: emitted.symbol,
                    edge_identity: edge.get(),
                },
            };
            relocations.push_record(RelocationRecord {
                origin,
                section: SectionKind::Text,
                offset,
                byte_width,
                symbol_handle: target_symbol,
                addend: 0,
                kind,
            });
        }
        for call in &function.foreign_calls {
            let origin = match call.owner {
                CallSiteOwner::Operation(operation) => RelocationOrigin::SemanticOperation {
                    function_symbol_handle: emitted.symbol,
                    operation_identity: operation.get(),
                },
                CallSiteOwner::CleanupAction { edge, .. } => RelocationOrigin::SemanticEdge {
                    function_symbol_handle: emitted.symbol,
                    edge_identity: edge.get(),
                },
            };
            if let Some(callback) = &call.callback_address {
                let Some((callback_symbol, callback_symbol_plan)) =
                    object_file::object_function_symbol(&object, callback.target.callback_function)
                else {
                    return Err(ObjectError::MissingCallbackPrivateFunction {
                        caller: function.machine,
                        owner: call.owner,
                    });
                };
                let matching_private = object_private_functions
                    .iter()
                    .filter(|private| private.identity == callback.target.callback_function)
                    .collect::<Vec<_>>();
                let [private] = matching_private.as_slice() else {
                    return Err(ObjectError::MissingCallbackPrivateFunction {
                        caller: function.machine,
                        owner: call.owner,
                    });
                };
                if private.function.symbol != callback_symbol
                    || private.function.text_offset != callback_symbol_plan.offset
                    || private.function.byte_count != callback_symbol_plan.size
                {
                    return Err(ObjectError::MissingCallbackPrivateFunction {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                let mut push_callback_relocation =
                    |local_offset: usize, kind: RelocationKind| -> Result<(), ObjectError> {
                        let offset = emitted
                            .text_offset
                            .checked_add(local_offset)
                            .ok_or(ObjectError::TextSizeOverflow)?;
                        relocations.push_record(RelocationRecord {
                            origin,
                            section: SectionKind::Text,
                            offset,
                            byte_width: 4,
                            symbol_handle: callback_symbol,
                            addend: 0,
                            kind,
                        });
                        Ok(())
                    };
                match callback.encoding {
                    machine_code::CallbackAddressEncoding::X86_64Relative32 {
                        relocation_offset,
                    } => push_callback_relocation(
                        relocation_offset,
                        RelocationKind::X86_64Relative32,
                    )?,
                    machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                        page_relocation_offset,
                        page_offset_relocation_offset,
                    } => {
                        push_callback_relocation(
                            page_relocation_offset,
                            RelocationKind::Aarch64Page21,
                        )?;
                        push_callback_relocation(
                            page_offset_relocation_offset,
                            RelocationKind::Aarch64PageOffset12,
                        )?;
                    }
                }
            }
            let target_symbol = import_symbols
                .iter()
                .find_map(|(locator, symbol)| (locator == &call.locator).then_some(*symbol))
                .ok_or(ObjectError::MissingForeignImportSymbol {
                    caller: function.machine,
                    owner: call.owner,
                })?;
            let (kind, byte_width) = validate_foreign_call_site(
                plan.target.architecture,
                function.machine,
                &function.bytes,
                call,
            )?;
            let offset = emitted
                .text_offset
                .checked_add(call.offset)
                .ok_or(ObjectError::TextSizeOverflow)?;
            relocations.push_record(RelocationRecord {
                origin,
                section: SectionKind::Text,
                offset,
                byte_width,
                symbol_handle: target_symbol,
                addend: 0,
                kind,
            });
        }
    }

    Ok(ObjectArtifact {
        hosted_receiver: None,
        requires_graph_storage_replay: false,
        fragment_replay: None,
        psi: plan.psi,
        target: plan.target,
        x86_feature_profile,
        x86_scalar_fma_provider,
        entry: plan.entry,
        object,
        relocations,
        text_bytes,
        data_bytes,
        dynamic_conformance_tables,
        forwarded_dynamic_descriptor_adapters,
        forwarded_dynamic_descriptor_tables,
        functions,
        private_functions: object_private_functions,
        semantic_code_attribution,
        port_effects,
        boundary_settlements,
        foreign_calls,
    })
}

/// Collect each distinct dynamic-conformance application addressed by a
/// direct or stored dynamic call, rejecting commitment collisions.
fn collect_dynamic_applications(
    plan: &MachineCodePlan,
) -> Result<Vec<terminal_psi::ClosedConformanceApplication>, ObjectError> {
    let mut dynamic_applications = Vec::<terminal_psi::ClosedConformanceApplication>::new();
    for application in plan
        .functions
        .iter()
        .flat_map(|function| &function.dynamic_calls)
        .map(|call| &call.dynamic_dispatch.application)
        .chain(
            plan.functions
                .iter()
                .flat_map(|function| &function.stored_dynamic_calls)
                .map(|call| &call.establishment.stored.application),
        )
    {
        if let Some(existing) = dynamic_applications
            .iter()
            .find(|existing| existing.commitment == application.commitment)
        {
            if !same_dynamic_table_application(existing, application) {
                return Err(ObjectError::DynamicConformanceCommitmentCollision);
            }
        } else {
            dynamic_applications.push(application.clone());
        }
    }
    Ok(dynamic_applications)
}

/// Size of the `.data` section holding the dynamic-conformance and forwarded
/// descriptor tables, eight bytes per slot.
fn dynamic_data_section_size(
    dynamic_applications: &[terminal_psi::ClosedConformanceApplication],
    forwarded_dynamic_applications: &[ValidatedForwardedDynamicApplication],
) -> Result<usize, ObjectError> {
    dynamic_applications
        .iter()
        .try_fold(0usize, |size, application| {
            application
                .rows
                .len()
                .checked_mul(8)
                .and_then(|bytes| size.checked_add(bytes))
                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)
        })?
        .checked_add(forwarded_dynamic_applications.iter().try_fold(
            0usize,
            |size, application| {
                application
                    .adapters
                    .len()
                    .checked_mul(8)
                    .and_then(|bytes| size.checked_add(bytes))
                    .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)
            },
        )?)
        .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)
}

/// Exact relocation-record capacity across internal, foreign, callback,
/// dynamic-table, and forwarded-descriptor relocations.
fn relocation_record_capacity(
    plan: &MachineCodePlan,
    dynamic_conformance_tables: &[ObjectDynamicConformanceTable],
    forwarded_dynamic_descriptor_tables: &[ObjectForwardedDynamicDescriptorTable],
    forwarded_dynamic_descriptor_adapters: &[ObjectForwardedDynamicDescriptorAdapter],
) -> usize {
    let ordinary_relocation_count = plan
        .functions
        .iter()
        .map(|function| function.internal_calls.len() + function.foreign_calls.len())
        .sum::<usize>();
    let callback_relocation_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.foreign_calls)
        .filter_map(|call| call.callback_address.as_ref())
        .map(|callback| match callback.encoding {
            machine_code::CallbackAddressEncoding::X86_64Relative32 { .. } => 1,
            machine_code::CallbackAddressEncoding::Aarch64PageAddress { .. } => 2,
        })
        .sum::<usize>();
    let dynamic_table_relocation_count = dynamic_conformance_tables
        .iter()
        .flat_map(|table| &table.slots)
        .filter(|slot| slot.target_symbol.is_some())
        .count();
    let forwarded_table_relocation_count = forwarded_dynamic_descriptor_tables
        .iter()
        .map(|table| table.slots.len())
        .sum::<usize>();
    let forwarded_adapter_relocation_count = forwarded_dynamic_descriptor_adapters.len();
    let dynamic_address_relocation_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.dynamic_calls)
        .map(|call| match call.table_address.encoding {
            machine_code::DynamicTableAddressEncoding::X86_64Relative32 { .. } => 1,
            machine_code::DynamicTableAddressEncoding::Aarch64PageAddress { .. } => 2,
        })
        .sum::<usize>();
    let stored_dynamic_address_relocation_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.stored_dynamic_calls)
        .map(|call| match call.establishment.table_address.encoding {
            machine_code::DynamicTableAddressEncoding::X86_64Relative32 { .. } => 1,
            machine_code::DynamicTableAddressEncoding::Aarch64PageAddress { .. } => 2,
        })
        .sum::<usize>();
    let forwarded_address_relocation_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.forwarded_dynamic_descriptor_calls)
        .flat_map(|call| &call.dynamic_arguments)
        .map(|argument| match argument.table_address.encoding {
            machine_code::DynamicTableAddressEncoding::X86_64Relative32 { .. } => 1,
            machine_code::DynamicTableAddressEncoding::Aarch64PageAddress { .. } => 2,
        })
        .sum::<usize>();
    ordinary_relocation_count
        .saturating_add(callback_relocation_count)
        .saturating_add(dynamic_table_relocation_count)
        .saturating_add(dynamic_address_relocation_count)
        .saturating_add(stored_dynamic_address_relocation_count)
        .saturating_add(forwarded_table_relocation_count)
        .saturating_add(forwarded_adapter_relocation_count)
        .saturating_add(forwarded_address_relocation_count)
}
