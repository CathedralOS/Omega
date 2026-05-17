use omega_calling_conventions::{HostAbiPlan, HostBindingMechanism};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_layout::{LayoutPlan, MachineLayout};
use omega_machine_bytes::EncodedMachinePlan;
use omega_object::{
    ObjectPlan, SectionKind, SectionPlan, SymbolKind, SymbolPlan, entry_symbol_name,
    machine_storage_symbol_name, runtime_frame_storage_symbol_name, section_name,
};
use omega_target::NativeTarget;
use omega_target_operations::TargetDataPlan;

pub struct ObjectPlanningInput<'plan> {
    pub target: NativeTarget,
    pub host_abi: &'plan HostAbiPlan,
    pub layouts: &'plan LayoutPlan,
    pub entry_machine_symbol: SymbolHandle,
    pub entry_machine_name: &'plan str,
    pub entry_state_key: StateKey,
    pub encoded_machine: &'plan EncodedMachinePlan,
    pub data: &'plan TargetDataPlan,
    pub runtime_frame_size: usize,
    pub runtime_frame_alignment: usize,
}

pub fn build_object_plan(input: ObjectPlanningInput<'_>) -> Result<ObjectPlan, Diagnostic> {
    let main_layout = input
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, layout)| layout.symbol == input.entry_machine_symbol)
        .map(|(_, layout)| layout)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "missing native layout for entry machine `{}`",
                input.entry_machine_name
            ))
        })?;
    let entry_symbol = entry_symbol_name(input.target);
    let entry_function = input
        .encoded_machine
        .functions
        .iter()
        .find(|(_, function)| function.source_key == input.entry_state_key)
        .map(|(_, function)| function)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "missing encoded entry function for state key {:?}",
                input.entry_state_key
            ))
        })?;
    let runtime_frame_offset = align_to(main_layout.layout.size, input.runtime_frame_alignment);
    let bss_size = runtime_frame_offset + input.runtime_frame_size;
    let bss_alignment = main_layout
        .layout
        .alignment
        .max(input.runtime_frame_alignment);

    let mut object_plan = ObjectPlan {
        target: input.target,
        sections: Arena::new(),
        symbols: Arena::new(),
        entry_symbol,
    };

    object_plan.sections.insert_many([
        SectionPlan {
            name: section_name(input.target, SectionKind::Text),
            kind: SectionKind::Text,
            size: input.encoded_machine.byte_count,
            alignment: 16,
        },
        SectionPlan {
            name: section_name(input.target, SectionKind::Data),
            kind: SectionKind::Data,
            size: input.data.bytes.len(),
            alignment: input.target.pointer_alignment,
        },
        SectionPlan {
            name: section_name(input.target, SectionKind::Bss),
            kind: SectionKind::Bss,
            size: bss_size,
            alignment: bss_alignment,
        },
    ]);

    object_plan.symbols.insert_many([
        SymbolPlan {
            name: object_plan.entry_symbol.clone(),
            section: Some(section_name(input.target, SectionKind::Text)),
            offset: entry_function.byte_offset,
            size: entry_function.byte_count,
            kind: SymbolKind::Function,
        },
        SymbolPlan {
            name: machine_storage_symbol(main_layout),
            section: Some(section_name(input.target, SectionKind::Bss)),
            offset: 0,
            size: main_layout.layout.size,
            kind: SymbolKind::Object,
        },
    ]);
    if input.runtime_frame_size > 0 {
        object_plan.symbols.insert(SymbolPlan {
            name: runtime_frame_storage_symbol_name(),
            section: Some(section_name(input.target, SectionKind::Bss)),
            offset: runtime_frame_offset,
            size: input.runtime_frame_size,
            kind: SymbolKind::Object,
        });
    }

    object_plan
        .symbols
        .insert_many(input.host_abi.bindings.iter().filter_map(|(_, binding)| {
            match &binding.mechanism {
                HostBindingMechanism::Import { symbol, .. } => Some(SymbolPlan {
                    name: symbol.to_string(),
                    section: None,
                    offset: 0,
                    size: 0,
                    kind: SymbolKind::Import,
                }),
                HostBindingMechanism::Syscall { .. } => None,
            }
        }));

    object_plan
        .symbols
        .insert_many(input.data.objects.iter().filter_map(|(_, data_object)| {
            let bytes = input.data.bytes.span(data_object.bytes)?;

            Some(SymbolPlan {
                name: data_object.symbol.to_string(),
                section: Some(section_name(input.target, SectionKind::Data)),
                offset: data_object.offset,
                size: bytes.len(),
                kind: SymbolKind::Object,
            })
        }));

    Ok(object_plan)
}

fn machine_storage_symbol(machine_layout: &MachineLayout) -> String {
    machine_storage_symbol_name(&machine_layout.name)
}

fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}
