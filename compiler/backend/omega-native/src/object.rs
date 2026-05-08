use crate::abi::HostBindingMechanism;
use crate::layout::MachineLayout;
use crate::plan::NativePlan;
use crate::runtime_storage::{runtime_frame_storage_alignment, runtime_frame_storage_size};
use crate::target::{NativeTarget, ObjectFormat};
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
pub use omega_object::{ObjectPlan, SectionKind, SectionPlan, SymbolKind, SymbolPlan};

pub fn build_object_plan(native_plan: &NativePlan) -> Result<ObjectPlan, Diagnostic> {
    let main_layout = native_plan
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, layout)| layout.name == native_plan.entry_machine)
        .map(|(_, layout)| layout)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "missing native layout for entry machine `{}`",
                native_plan.entry_machine
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

fn entry_symbol_name(target: NativeTarget) -> String {
    match target.object_format {
        ObjectFormat::MachO => "_main".to_owned(),
        ObjectFormat::Elf | ObjectFormat::Coff => "main".to_owned(),
    }
}

fn machine_storage_symbol(machine_layout: &MachineLayout) -> String {
    machine_storage_symbol_name(&machine_layout.name)
}

pub fn machine_storage_symbol_name(machine_name: &str) -> String {
    format!("omega_machine_{machine_name}_storage")
}

pub fn runtime_frame_storage_symbol_name() -> String {
    "omega_runtime_frame_storage".to_owned()
}

fn section_name(target: NativeTarget, kind: SectionKind) -> String {
    match (target.object_format, kind) {
        (ObjectFormat::MachO, SectionKind::Text) => "__TEXT,__text".to_owned(),
        (ObjectFormat::MachO, SectionKind::Data) => "__DATA,__data".to_owned(),
        (ObjectFormat::MachO, SectionKind::Bss) => "__DATA,__bss".to_owned(),
        (_, SectionKind::Text) => ".text".to_owned(),
        (_, SectionKind::Data) => ".data".to_owned(),
        (_, SectionKind::Bss) => ".bss".to_owned(),
    }
}

fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}
