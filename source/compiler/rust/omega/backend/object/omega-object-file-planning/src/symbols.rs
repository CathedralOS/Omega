use crate::input::ObjectPlanningInput;
use omega_calling_conventions::{HostBindingMechanism, HostImportLocator};
use omega_layout::MachineLayout;
use omega_machine_bytes::EncodedMachineFunction;
use omega_object_file::{
    FunctionSymbolPlan, NormalizedImportPlan, ObjectPlan, SectionKind, SymbolKind, SymbolPlan,
    SymbolSection, machine_storage_symbol_name, normalized_foreign_import_symbol_name,
    private_function_symbol_name, runtime_frame_storage_symbol_name,
};
use omega_target::ForeignLocatorCandidate;
use psi_diagnostics::Diagnostic;

pub(super) fn object_symbol_capacity(input: &ObjectPlanningInput<'_>) -> usize {
    let host_import_count = input
        .host_abi
        .bindings
        .iter()
        .filter(|(_, binding)| matches!(binding.mechanism, HostBindingMechanism::Import { .. }))
        .count();
    let runtime_frame_symbol_count = usize::from(input.runtime_frame_size > 0);

    1usize
        .checked_add(input.encoded_machine.code.functions.len())
        .and_then(|count| count.checked_add(runtime_frame_symbol_count))
        .and_then(|count| count.checked_add(host_import_count))
        .and_then(|count| count.checked_add(input.data.objects.len()))
        .expect("object symbol capacity overflow")
}

pub(super) fn insert_object_symbols(
    input: &ObjectPlanningInput<'_>,
    main_layout: &MachineLayout,
    entry_function: &EncodedMachineFunction,
    runtime_frame_offset: usize,
    object_plan: &mut ObjectPlan,
) -> Result<(), Diagnostic> {
    validate_encoded_function_linkage(input)?;
    for (_, binding) in input.host_abi.bindings.iter() {
        let HostBindingMechanism::Import {
            locator: HostImportLocator::Normalized(locator),
        } = &binding.mechanism
        else {
            continue;
        };
        if locator.target().native_target() != input.target {
            return Err(Diagnostic::error(format!(
                "normalized foreign locator 0x{:016x} targets `{}` but object planning targets {:?}",
                locator.normalized_identity(),
                locator.target().target_name(),
                input.target,
            )));
        }
        if matches!(
            locator.locator(),
            ForeignLocatorCandidate::ElfVersioned { .. }
        ) {
            return Err(Diagnostic::error(format!(
                "versioned ELF foreign locator 0x{:016x} reached object planning before ELF symbol-version emission semantics are implemented",
                locator.normalized_identity(),
            )));
        }
    }
    object_plan.layout.entry_symbol = insert_function_symbol(
        entry_function,
        entry_function.symbol.to_string(),
        object_plan,
    );
    for (_, function) in input.encoded_machine.code.functions.iter() {
        if function.identity != entry_function.identity {
            let private_symbol = if function.identity.callback_thunk_placement_index().is_some() {
                // The callback symbol is already derived from the exact
                // placement row. Identity alone deliberately cannot recreate
                // its site/fingerprint-bound spelling.
                function.symbol.to_string()
            } else {
                private_function_symbol_name(function.identity).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "encoded function `{}` has no canonical private linkage name",
                        function.symbol
                    ))
                })?
            };
            insert_function_symbol(function, private_symbol, object_plan);
        }
    }
    object_plan.layout.symbols.insert(SymbolPlan {
        name: machine_storage_symbol_name(&main_layout.name),
        section: SymbolSection::Section(SectionKind::Bss),
        offset: 0,
        size: main_layout.layout.size,
        kind: SymbolKind::Object,
        import_library: String::new(),
    });
    if input.runtime_frame_size > 0 {
        object_plan.layout.symbols.insert(SymbolPlan {
            name: runtime_frame_storage_symbol_name(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: runtime_frame_offset,
            size: input.runtime_frame_size,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
    }

    object_plan
        .layout
        .symbols
        .insert_many(input.host_abi.bindings.iter().filter_map(|(_, binding)| {
            match &binding.mechanism {
                HostBindingMechanism::Import {
                    locator: HostImportLocator::StringBackedBootstrap { library, symbol },
                } => Some(SymbolPlan {
                    name: symbol.to_string(),
                    section: SymbolSection::None,
                    offset: 0,
                    size: 0,
                    kind: SymbolKind::Import,
                    import_library: library.to_string(),
                }),
                HostBindingMechanism::Import {
                    locator: HostImportLocator::Normalized(_),
                } => None,
                // Syscalls and field-model calls have no import symbol: the
                // callee address is a number (syscall) or read from the
                // receiver/table at call time (vtable/table-function).
                HostBindingMechanism::Syscall { .. }
                | HostBindingMechanism::VtableSlot { .. }
                | HostBindingMechanism::VtableField { .. }
                | HostBindingMechanism::TableFunction { .. } => None,
            }
        }));

    for (_, binding) in input.host_abi.bindings.iter() {
        let HostBindingMechanism::Import {
            locator: HostImportLocator::Normalized(locator),
        } = &binding.mechanism
        else {
            continue;
        };
        if object_plan
            .layout
            .normalized_imports
            .iter()
            .any(|import| &import.locator == locator)
        {
            continue;
        }
        let symbol = object_plan.layout.symbols.insert(SymbolPlan {
            name: normalized_foreign_import_symbol_name(locator),
            section: SymbolSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
            import_library: String::new(),
        });
        object_plan
            .layout
            .normalized_imports
            .push(NormalizedImportPlan {
                symbol,
                locator: locator.clone(),
            });
    }

    object_plan
        .layout
        .symbols
        .insert_many(input.data.objects.iter().filter_map(|(_, data_object)| {
            let bytes = input.data.bytes.span(data_object.bytes)?;

            Some(SymbolPlan {
                name: data_object.symbol.to_string(),
                section: SymbolSection::Section(SectionKind::Data),
                offset: data_object.offset,
                size: bytes.len(),
                kind: SymbolKind::Object,
                import_library: String::new(),
            })
        }));

    validate_function_object_symbol_names(object_plan)
}

fn insert_function_symbol(
    function: &EncodedMachineFunction,
    symbol_name: String,
    object_plan: &mut ObjectPlan,
) -> omega_object_file::ObjectSymbolHandle {
    let symbol = object_plan.layout.symbols.insert(SymbolPlan {
        name: symbol_name,
        section: SymbolSection::Section(SectionKind::Text),
        offset: function.byte_offset,
        size: function.byte_count,
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    object_plan
        .layout
        .function_symbols
        .insert(FunctionSymbolPlan {
            identity: function.identity,
            symbol,
        });
    symbol
}

fn validate_encoded_function_linkage(input: &ObjectPlanningInput<'_>) -> Result<(), Diagnostic> {
    let functions = input.encoded_machine.code.functions.storage_slice();
    for (index, function) in functions.iter().enumerate() {
        if !function.identity.is_valid() {
            return Err(Diagnostic::error(format!(
                "encoded function `{}` has invalid compiler-private identity {:?}",
                function.symbol, function.identity
            )));
        }
        if function.symbol.is_empty() || function.byte_count == 0 {
            return Err(Diagnostic::error(format!(
                "encoded function identity {:?} has no nonempty text symbol",
                function.identity
            )));
        }
        let end = function
            .byte_offset
            .checked_add(function.byte_count)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "encoded function `{}` text interval overflows",
                    function.symbol
                ))
            })?;
        if end > input.encoded_machine.code.byte_count {
            return Err(Diagnostic::error(format!(
                "encoded function `{}` text interval exceeds the encoded program",
                function.symbol
            )));
        }
        for earlier in &functions[..index] {
            if earlier.identity == function.identity {
                return Err(Diagnostic::error(format!(
                    "encoded function identity {:?} names more than one text function",
                    function.identity
                )));
            }
            let earlier_end = earlier
                .byte_offset
                .checked_add(earlier.byte_count)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "encoded function `{}` text interval overflows",
                        earlier.symbol
                    ))
                })?;
            if function.byte_offset < earlier_end && earlier.byte_offset < end {
                return Err(Diagnostic::error(format!(
                    "encoded functions `{}` and `{}` have overlapping text intervals",
                    earlier.symbol, function.symbol
                )));
            }
        }
    }
    Ok(())
}

fn validate_function_object_symbol_names(object_plan: &ObjectPlan) -> Result<(), Diagnostic> {
    for (_, binding) in object_plan.layout.function_symbols.iter() {
        let symbol = object_plan.layout.symbols.get(binding.symbol);
        if object_plan
            .layout
            .symbols
            .iter()
            .filter(|(_, candidate)| candidate.name == symbol.name)
            .count()
            != 1
        {
            return Err(Diagnostic::error(format!(
                "function object symbol `{}` is declared more than once",
                symbol.name
            )));
        }
    }
    Ok(())
}
