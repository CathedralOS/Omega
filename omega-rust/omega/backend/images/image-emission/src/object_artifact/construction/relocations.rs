//! Relocation records: the foreign import symbols, the table and adapter
//! materializations, then each function's dynamic-table addresses, internal
//! calls and foreign calls replayed against the emitted text.

use super::same_dynamic_table_application;
use crate::object_artifact::call_sites::{validate_foreign_call_site, validate_internal_call_site};
use crate::object_artifact::{
    ObjectCompilerPrivateFunction, ObjectDynamicConformanceTable, ObjectError,
    ObjectForwardedDynamicDescriptorAdapter, ObjectForwardedDynamicDescriptorTable, ObjectFunction,
};
use machine_code::MachineCodeFunction;
use machine_code::MachineCodePlan;
use object_file::{
    NormalizedImportPlan, ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin,
    RelocationPlan, RelocationRecord, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
    normalized_foreign_import_symbol_name,
};
use semantic_vocabulary::MachineId;
use target::Architecture;
use target_operations::CallSiteOwner;

/// One import symbol per distinct normalized foreign locator, rejecting two
/// locators whose compatibility fingerprints collide.
pub(super) fn declare_foreign_imports(
    plan: &MachineCodePlan,
    object: &mut ObjectPlan,
) -> Result<Vec<(target::NormalizedForeignLocator, ObjectSymbolHandle)>, ObjectError> {
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
    Ok(import_symbols)
}

/// Everything a relocation record can bind: the emitted functions and their
/// symbols, the private functions callbacks target, the data tables, the
/// forwarded adapters, and the import symbols.
#[derive(Clone, Copy)]
pub(super) struct RelocationInputs<'a> {
    pub(super) plan: &'a MachineCodePlan,
    pub(super) object: &'a ObjectPlan,
    pub(super) functions: &'a [ObjectFunction],
    pub(super) symbols_by_machine: &'a std::collections::BTreeMap<MachineId, ObjectSymbolHandle>,
    pub(super) private_functions: &'a [ObjectCompilerPrivateFunction],
    pub(super) dynamic_conformance_tables: &'a [ObjectDynamicConformanceTable],
    pub(super) forwarded_dynamic_descriptor_tables: &'a [ObjectForwardedDynamicDescriptorTable],
    pub(super) forwarded_dynamic_descriptor_adapters:
        &'a [ObjectForwardedDynamicDescriptorAdapter],
    pub(super) import_symbols: &'a [(target::NormalizedForeignLocator, ObjectSymbolHandle)],
}

/// Materialization records for the tables and adapters first, then each
/// function's records in canonical function order.
pub(super) fn plan_relocations(
    inputs: RelocationInputs<'_>,
) -> Result<RelocationPlan, ObjectError> {
    let RelocationInputs {
        plan,
        object,
        functions,
        symbols_by_machine,
        private_functions: object_private_functions,
        dynamic_conformance_tables,
        forwarded_dynamic_descriptor_tables,
        forwarded_dynamic_descriptor_adapters,
        import_symbols,
    } = inputs;
    let mut relocations = RelocationPlan::with_record_capacity(
        plan.target,
        relocation_record_capacity(
            plan,
            dynamic_conformance_tables,
            forwarded_dynamic_descriptor_tables,
            forwarded_dynamic_descriptor_adapters,
        ),
    );
    materialization_relocations(
        &mut relocations,
        plan,
        dynamic_conformance_tables,
        forwarded_dynamic_descriptor_tables,
        forwarded_dynamic_descriptor_adapters,
    )?;
    for (function, emitted) in plan.functions.iter().zip(functions) {
        dynamic_table_address_relocations(
            &mut relocations,
            function,
            emitted,
            dynamic_conformance_tables,
            forwarded_dynamic_descriptor_tables,
        )?;
        internal_call_relocations(
            &mut relocations,
            plan,
            function,
            emitted,
            symbols_by_machine,
        )?;
        foreign_call_relocations(
            &mut relocations,
            plan,
            object,
            function,
            emitted,
            object_private_functions,
            import_symbols,
        )?;
    }
    Ok(relocations)
}

/// Absolute slot records for every bound table slot, and the direct-call
/// record inside each forwarded adapter.
fn materialization_relocations(
    relocations: &mut RelocationPlan,
    plan: &MachineCodePlan,
    dynamic_conformance_tables: &[ObjectDynamicConformanceTable],
    forwarded_dynamic_descriptor_tables: &[ObjectForwardedDynamicDescriptorTable],
    forwarded_dynamic_descriptor_adapters: &[ObjectForwardedDynamicDescriptorAdapter],
) -> Result<(), ObjectError> {
    for table in dynamic_conformance_tables {
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
    for table in forwarded_dynamic_descriptor_tables {
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
    for adapter in forwarded_dynamic_descriptor_adapters {
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
    Ok(())
}

/// Table-address records for the function's direct, stored and forwarded
/// dynamic calls, each bound to the table its application resolved to.
fn dynamic_table_address_relocations(
    relocations: &mut RelocationPlan,
    function: &MachineCodeFunction,
    emitted: &ObjectFunction,
    dynamic_conformance_tables: &[ObjectDynamicConformanceTable],
    forwarded_dynamic_descriptor_tables: &[ObjectForwardedDynamicDescriptorTable],
) -> Result<(), ObjectError> {
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
        push_table_address_relocations(
            relocations,
            origin,
            emitted,
            table.symbol,
            &call.table_address.encoding,
        )?;
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
        push_table_address_relocations(
            relocations,
            origin,
            emitted,
            table.symbol,
            &establishment.table_address.encoding,
        )?;
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
            push_table_address_relocations(
                relocations,
                origin,
                emitted,
                table.symbol,
                &argument.table_address.encoding,
            )?;
        }
    }
    Ok(())
}

/// The text records that bind one table-address materialization to its
/// table symbol: a single relative32 on x86-64, a page and page-offset pair
/// on AArch64.
fn push_table_address_relocations(
    relocations: &mut RelocationPlan,
    origin: RelocationOrigin,
    emitted: &ObjectFunction,
    table_symbol: ObjectSymbolHandle,
    encoding: &machine_code::DynamicTableAddressEncoding,
) -> Result<(), ObjectError> {
    let mut push = |local_offset: usize, kind: RelocationKind| -> Result<(), ObjectError> {
        relocations.push_record(RelocationRecord {
            origin,
            section: SectionKind::Text,
            offset: emitted
                .text_offset
                .checked_add(local_offset)
                .ok_or(ObjectError::TextSizeOverflow)?,
            byte_width: 4,
            symbol_handle: table_symbol,
            addend: 0,
            kind,
        });
        Ok(())
    };
    match *encoding {
        machine_code::DynamicTableAddressEncoding::X86_64Relative32 { relocation_offset } => {
            push(relocation_offset, RelocationKind::X86_64Relative32)
        }
        machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
            page_relocation_offset,
            page_offset_relocation_offset,
        } => {
            push(page_relocation_offset, RelocationKind::Aarch64Page21)?;
            push(
                page_offset_relocation_offset,
                RelocationKind::Aarch64PageOffset12,
            )
        }
    }
}

/// One typed record per internal call site, bound to the callee's symbol.
fn internal_call_relocations(
    relocations: &mut RelocationPlan,
    plan: &MachineCodePlan,
    function: &MachineCodeFunction,
    emitted: &ObjectFunction,
    symbols_by_machine: &std::collections::BTreeMap<MachineId, ObjectSymbolHandle>,
) -> Result<(), ObjectError> {
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
    Ok(())
}

/// One typed record per foreign call site bound to its import symbol, plus
/// the callback-address records bound to the private callback function.
fn foreign_call_relocations(
    relocations: &mut RelocationPlan,
    plan: &MachineCodePlan,
    object: &ObjectPlan,
    function: &MachineCodeFunction,
    emitted: &ObjectFunction,
    object_private_functions: &[ObjectCompilerPrivateFunction],
    import_symbols: &[(target::NormalizedForeignLocator, ObjectSymbolHandle)],
) -> Result<(), ObjectError> {
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
                object_file::object_function_symbol(object, callback.target.callback_function)
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
                machine_code::CallbackAddressEncoding::X86_64Relative32 { relocation_offset } => {
                    push_callback_relocation(relocation_offset, RelocationKind::X86_64Relative32)?
                }
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
    Ok(())
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
