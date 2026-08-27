//! Exact relocations for compiler-private callback address stores.

use super::context::InstructionRelocationContext;
use omega_object_file::object_function_symbol;
use omega_target::Architecture;
use omega_target_operations::SelectedInstructionKind;
use psi_diagnostics::Diagnostic;

pub(super) fn collect_callback_private_store_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> Result<bool, Diagnostic> {
    let SelectedInstructionKind::WriteFunctionAddressToRuntimeStorage {
        function,
        target_region,
        target_offset,
    } = instruction
    else {
        return Ok(false);
    };
    let (function_symbol, _) =
        object_function_symbol(context.input.object, *function).ok_or_else(|| {
            Diagnostic::error(format!(
                "callback address store target {function:?} has no exact object function symbol"
            ))
        })?;
    context.insert_data_address_at_instruction_start(function_symbol);
    match context.input.target.architecture {
        Architecture::X86_64 => {
            let (_, sites) = omega_instruction_selection::
                x86_64_encode_runtime_storage_function_address_write_with_sites(
                    *target_region,
                    *target_offset,
                )?;
            for (byte_offset, side) in sites.iter() {
                if side != omega_instruction_selection::PlaceCopySide::Target {
                    return Err(Diagnostic::error(
                        "callback address store encoded a non-target storage site",
                    ));
                }
                context.insert_data_address_at_relative_offset(
                    byte_offset,
                    context.storage_region_symbol_handle(*target_region),
                );
            }
        }
        Architecture::Aarch64 => {
            // Source ADRP/ADD occupy bytes 0..8; destination ADRP/ADD begin at 8.
            context.insert_data_address_at_relative_offset(
                8,
                context.storage_region_symbol_handle(*target_region),
            );
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RelocationPlanningInput;
    use omega_assigned_target_operations::AssignedTargetOperationPlan;
    use omega_calling_conventions::build_host_abi_plan;
    use omega_control_flow::{MachineFunctionIdentity, StateKey};
    use omega_machine_bytes::EncodedMachinePlan;
    use omega_object_file::{
        FunctionSymbolPlan, ObjectPlan, RelocationKind, RelocationPlan, SectionKind, SymbolKind,
        SymbolPlan, SymbolSection, storage_region_symbol_name,
    };
    use omega_target::NativeTarget;
    use omega_target_operations::{RuntimeStorageRegion, TargetDataPlan, TargetOperationPlan};
    use psi_symbols::SymbolHandle;

    fn key() -> StateKey {
        StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        }
    }

    #[test]
    fn emits_exact_x86_and_aarch64_function_and_storage_relocations() {
        for target in [NativeTarget::windows_x64(), NativeTarget::linux_arm64()] {
            let source_identity = MachineFunctionIdentity::source(key());
            let callback_identity = MachineFunctionIdentity::callback_thunk(key(), 7).unwrap();
            let mut object = ObjectPlan::with_capacities(target, 0, 3, 2);
            let source_symbol = object.layout.symbols.insert(SymbolPlan {
                name: "registrar".into(),
                section: SymbolSection::Section(SectionKind::Text),
                offset: 0,
                size: 32,
                kind: SymbolKind::Function,
                import_library: String::new(),
            });
            object.layout.function_symbols.insert(FunctionSymbolPlan {
                identity: source_identity,
                symbol: source_symbol,
            });
            let callback_symbol = object.layout.symbols.insert(SymbolPlan {
                name: "callback_private".into(),
                section: SymbolSection::Section(SectionKind::Text),
                offset: 32,
                size: 8,
                kind: SymbolKind::Function,
                import_library: String::new(),
            });
            object.layout.function_symbols.insert(FunctionSymbolPlan {
                identity: callback_identity,
                symbol: callback_symbol,
            });
            let storage_symbol = object.layout.symbols.insert(SymbolPlan {
                name: storage_region_symbol_name(RuntimeStorageRegion::Machine, "Main"),
                section: SymbolSection::Section(SectionKind::Bss),
                offset: 0,
                size: 64,
                kind: SymbolKind::Object,
                import_library: String::new(),
            });

            let instructions = TargetOperationPlan::default();
            let assigned = AssignedTargetOperationPlan::default();
            let encoded = EncodedMachinePlan::with_capacity(target, 0, 0, 0);
            let data = TargetDataPlan::default();
            let host_abi = build_host_abi_plan(target);
            let input = RelocationPlanningInput {
                target,
                instructions: &instructions,
                assigned_target_operations: &assigned,
                encoded_machine: &encoded,
                data: &data,
                object: &object,
                host_abi: &host_abi,
                entry_machine_name: "Main",
            };
            let mut relocations = RelocationPlan::with_target(target);
            let mut context = InstructionRelocationContext {
                input,
                function_symbol_handle: source_symbol,
                selected_instruction_index: 11,
                selected_text_offset: 5,
                selected_text_width: match target.architecture {
                    Architecture::X86_64 => 27,
                    Architecture::Aarch64 => 20,
                },
                relocation_plan: &mut relocations,
            };
            assert!(
                collect_callback_private_store_relocations(
                    &mut context,
                    &SelectedInstructionKind::WriteFunctionAddressToRuntimeStorage {
                        function: callback_identity,
                        target_region: RuntimeStorageRegion::Machine,
                        target_offset: 24,
                    },
                )
                .unwrap()
            );

            let records = relocations
                .records()
                .map(|(_, record)| record)
                .collect::<Vec<_>>();
            match target.architecture {
                Architecture::X86_64 => {
                    assert_eq!(records.len(), 2);
                    assert_eq!(
                        (records[0].offset, records[0].byte_width, records[0].kind),
                        (7, 8, RelocationKind::Absolute64)
                    );
                    assert_eq!(
                        (records[1].offset, records[1].byte_width, records[1].kind),
                        (17, 8, RelocationKind::Absolute64)
                    );
                    assert_eq!(records[0].symbol_handle, callback_symbol);
                    assert_eq!(records[1].symbol_handle, storage_symbol);
                }
                Architecture::Aarch64 => {
                    assert_eq!(records.len(), 4);
                    assert_eq!(
                        (records[0].offset, records[0].kind),
                        (5, RelocationKind::Aarch64Page21)
                    );
                    assert_eq!(
                        (records[1].offset, records[1].kind),
                        (9, RelocationKind::Aarch64PageOffset12)
                    );
                    assert_eq!(
                        (records[2].offset, records[2].kind),
                        (13, RelocationKind::Aarch64Page21)
                    );
                    assert_eq!(
                        (records[3].offset, records[3].kind),
                        (17, RelocationKind::Aarch64PageOffset12)
                    );
                    assert!(
                        records[..2]
                            .iter()
                            .all(|record| record.symbol_handle == callback_symbol)
                    );
                    assert!(
                        records[2..]
                            .iter()
                            .all(|record| record.symbol_handle == storage_symbol)
                    );
                }
            }
            assert!(records.iter().all(|record| {
                record.origin
                    == omega_object_file::RelocationOrigin::Instruction {
                        function_symbol_handle: source_symbol,
                        selected_instruction_index: 11,
                    }
            }));
        }
    }
}
