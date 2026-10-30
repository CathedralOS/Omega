//! Relocation records: the foreign import symbols, then each function's
//! internal calls and foreign calls replayed against the emitted text.

use crate::object_artifact::call_sites::{validate_foreign_call_site, validate_internal_call_site};
use crate::object_artifact::{ObjectCompilerPrivateFunction, ObjectError, ObjectFunction};
use machine_code::MachineCodeFunction;
use machine_code::MachineCodePlan;
use object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
    RelocationRecord, SectionKind, normalized_foreign_import_symbol_name,
};
use semantic_vocabulary::MachineId;
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
            let symbol = object.layout.insert_import(
                normalized_foreign_import_symbol_name(&call.locator),
                &call.locator,
            );
            import_symbols.push((call.locator.clone(), symbol));
        }
    }
    Ok(import_symbols)
}

/// Everything a relocation record can bind: the emitted functions and their
/// symbols, the private functions callbacks target, and the import symbols.
#[derive(Clone, Copy)]
pub(super) struct RelocationInputs<'a> {
    pub(super) plan: &'a MachineCodePlan,
    pub(super) object: &'a ObjectPlan,
    pub(super) functions: &'a [ObjectFunction],
    pub(super) symbols_by_machine: &'a std::collections::BTreeMap<MachineId, ObjectSymbolHandle>,
    pub(super) private_functions: &'a [ObjectCompilerPrivateFunction],
    pub(super) import_symbols: &'a [(target::NormalizedForeignLocator, ObjectSymbolHandle)],
}

/// Each function's records in canonical function order.
pub(super) fn plan_relocations(
    inputs: RelocationInputs<'_>,
) -> Result<RelocationPlan, ObjectError> {
    let RelocationInputs {
        plan,
        object,
        functions,
        symbols_by_machine,
        private_functions: object_private_functions,
        import_symbols,
    } = inputs;
    let mut relocations =
        RelocationPlan::with_record_capacity(plan.target, relocation_record_capacity(plan));
    for (function, emitted) in plan.functions.iter().zip(functions) {
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
            CallSiteOwner::InvokeNoNormal(edge)
            | CallSiteOwner::ExternalCompletion(edge)
            | CallSiteOwner::CleanupAction { edge, .. } => RelocationOrigin::SemanticEdge {
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
            CallSiteOwner::InvokeNoNormal(_) => {
                return Err(ObjectError::ForeignCallOwnerNotInProvenance {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            CallSiteOwner::ExternalCompletion(edge) => RelocationOrigin::SemanticEdge {
                function_symbol_handle: emitted.symbol,
                edge_identity: edge.get(),
            },
            CallSiteOwner::CleanupAction { edge, .. } => RelocationOrigin::SemanticEdge {
                function_symbol_handle: emitted.symbol,
                edge_identity: edge.get(),
            },
        };
        for callback in &call.callback_addresses {
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

/// Exact relocation-record capacity across internal, foreign, and callback
/// relocations.
fn relocation_record_capacity(plan: &MachineCodePlan) -> usize {
    let ordinary_relocation_count = plan
        .functions
        .iter()
        .map(|function| function.internal_calls.len() + function.foreign_calls.len())
        .sum::<usize>();
    let callback_relocation_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.foreign_calls)
        .flat_map(|call| call.callback_addresses.iter())
        .map(|callback| match callback.encoding {
            machine_code::CallbackAddressEncoding::X86_64Relative32 { .. } => 1,
            machine_code::CallbackAddressEncoding::Aarch64PageAddress { .. } => 2,
        })
        .sum::<usize>();
    ordinary_relocation_count.saturating_add(callback_relocation_count)
}
