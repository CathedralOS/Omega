use crate::plan::NativePlan;
use crate::runtime_storage::{runtime_frame_storage_alignment, runtime_frame_storage_size};
use omega_calling_conventions::HostBindingMechanism;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_layout::MachineLayout;
use omega_object::{
    ObjectPlan, SectionKind, SectionPlan, SymbolKind, SymbolPlan, entry_symbol_name,
    machine_storage_symbol_name, runtime_frame_storage_symbol_name, section_name,
};

pub fn build_object_plan(native_plan: &NativePlan) -> Result<ObjectPlan, Diagnostic> {
    let main_layout = native_plan
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, layout)| layout.symbol == native_plan.entry_key.machine)
        .map(|(_, layout)| layout)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "missing native layout for entry machine `{}`",
                native_plan.entry_machine_name()
            ))
        })?;
    let entry_symbol = entry_symbol_name(native_plan.target);
    let runtime_frame_size = runtime_frame_storage_size(&native_plan.runtime_storage);
    let runtime_frame_alignment = runtime_frame_storage_alignment(&native_plan.runtime_storage);
    let runtime_frame_offset = align_to(main_layout.layout.size, runtime_frame_alignment);
    let bss_size = runtime_frame_offset + runtime_frame_size;
    let bss_alignment = main_layout.layout.alignment.max(runtime_frame_alignment);

    let mut object_plan = ObjectPlan {
        target: native_plan.target,
        sections: Arena::new(),
        symbols: Arena::new(),
        entry_symbol,
    };

    object_plan.sections.insert_many([
        SectionPlan {
            name: section_name(native_plan.target, SectionKind::Text),
            kind: SectionKind::Text,
            size: native_plan.machine_code.byte_count,
            alignment: 16,
        },
        SectionPlan {
            name: section_name(native_plan.target, SectionKind::Data),
            kind: SectionKind::Data,
            size: native_plan.data.bytes.len(),
            alignment: native_plan.target.pointer_alignment,
        },
        SectionPlan {
            name: section_name(native_plan.target, SectionKind::Bss),
            kind: SectionKind::Bss,
            size: bss_size,
            alignment: bss_alignment,
        },
    ]);

    object_plan.symbols.insert_many([
        SymbolPlan {
            name: object_plan.entry_symbol.clone(),
            section: Some(section_name(native_plan.target, SectionKind::Text)),
            offset: 0,
            size: native_plan.machine_code.byte_count,
            kind: SymbolKind::Function,
        },
        SymbolPlan {
            name: machine_storage_symbol(main_layout),
            section: Some(section_name(native_plan.target, SectionKind::Bss)),
            offset: 0,
            size: main_layout.layout.size,
            kind: SymbolKind::Object,
        },
    ]);
    if runtime_frame_size > 0 {
        object_plan.symbols.insert(SymbolPlan {
            name: runtime_frame_storage_symbol_name(),
            section: Some(section_name(native_plan.target, SectionKind::Bss)),
            offset: runtime_frame_offset,
            size: runtime_frame_size,
            kind: SymbolKind::Object,
        });
    }

    object_plan
        .symbols
        .insert_many(
            native_plan
                .host_abi
                .bindings
                .iter()
                .filter_map(|(_, binding)| match &binding.mechanism {
                    HostBindingMechanism::Import { symbol, .. } => Some(SymbolPlan {
                        name: symbol.clone(),
                        section: None,
                        offset: 0,
                        size: 0,
                        kind: SymbolKind::Import,
                    }),
                    HostBindingMechanism::Syscall { .. } => None,
                }),
        );

    object_plan
        .symbols
        .insert_many(
            native_plan
                .data
                .objects
                .iter()
                .filter_map(|(_, data_object)| {
                    let bytes = native_plan.data.bytes.span(data_object.bytes)?;

                    Some(SymbolPlan {
                        name: data_object.symbol.clone(),
                        section: Some(section_name(native_plan.target, SectionKind::Data)),
                        offset: data_object.offset,
                        size: bytes.len(),
                        kind: SymbolKind::Object,
                    })
                }),
        );

    Ok(object_plan)
}

fn machine_storage_symbol(machine_layout: &MachineLayout) -> String {
    machine_storage_symbol_name(&machine_layout.name)
}

fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}
