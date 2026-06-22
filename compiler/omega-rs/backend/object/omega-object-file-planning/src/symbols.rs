use crate::input::ObjectPlanningInput;
use omega_calling_conventions::HostBindingMechanism;
use omega_layout::MachineLayout;
use omega_machine_bytes::EncodedMachineFunction;
use omega_object_file::{
    ObjectPlan, SectionKind, SymbolKind, SymbolPlan, SymbolSection, entry_symbol_name,
    machine_storage_symbol_name, runtime_frame_storage_symbol_name,
};

pub(super) fn object_symbol_capacity(input: &ObjectPlanningInput<'_>) -> usize {
    let host_import_count = input
        .host_abi
        .bindings
        .iter()
        .filter(|(_, binding)| matches!(binding.mechanism, HostBindingMechanism::Import { .. }))
        .count();
    let runtime_frame_symbol_count = usize::from(input.runtime_frame_size > 0);

    2usize
        .checked_add(runtime_frame_symbol_count)
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
) {
    object_plan.layout.entry_symbol = object_plan.layout.symbols.insert(SymbolPlan {
        name: entry_symbol_name(input.target),
        section: SymbolSection::Section(SectionKind::Text),
        offset: entry_function.byte_offset,
        size: entry_function.byte_count,
        kind: SymbolKind::Function,
    });
    object_plan.layout.symbols.insert(SymbolPlan {
        name: machine_storage_symbol_name(&main_layout.name),
        section: SymbolSection::Section(SectionKind::Bss),
        offset: 0,
        size: main_layout.layout.size,
        kind: SymbolKind::Object,
    });
    if input.runtime_frame_size > 0 {
        object_plan.layout.symbols.insert(SymbolPlan {
            name: runtime_frame_storage_symbol_name(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: runtime_frame_offset,
            size: input.runtime_frame_size,
            kind: SymbolKind::Object,
        });
    }

    object_plan
        .layout
        .symbols
        .insert_many(input.host_abi.bindings.iter().filter_map(|(_, binding)| {
            match &binding.mechanism {
                HostBindingMechanism::Import { symbol, .. } => Some(SymbolPlan {
                    name: symbol.to_string(),
                    section: SymbolSection::None,
                    offset: 0,
                    size: 0,
                    kind: SymbolKind::Import,
                }),
                HostBindingMechanism::Syscall { .. } => None,
            }
        }));

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
            })
        }));
}
