use crate::BackendReportInput;
use omega_layout::{DataShape, FieldLayout};
use omega_object_file::{
    RelocationRecord, SectionPlan, SymbolPlan, object_symbol_name, section_name,
    symbol_section_name,
};
use omega_target::NativeTarget;

pub(super) fn write_layout_object_sections(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("\n## Layouts\n");
    output.push_str(&format!(
        "data layouts: {}\n",
        backend_plan.layouts.data_layouts.len()
    ));
    output.push_str(&format!(
        "machine layouts: {}\n",
        backend_plan.layouts.machine_layouts.len()
    ));
    output.push_str(&format!(
        "fields: {}\n\n",
        backend_plan.layouts.fields.len()
    ));
    output.push_str(&format!(
        "variants: {}\n\n",
        backend_plan.layouts.variants.len()
    ));

    for (_, data_layout) in backend_plan.layouts.data_layouts.iter() {
        output.push_str(&format!(
            "- data {}: size {}, align {}\n",
            data_layout.name, data_layout.layout.size, data_layout.layout.alignment
        ));

        match &data_layout.shape {
            DataShape::Enum { variants } => {
                let variants = backend_plan
                    .layouts
                    .variants
                    .span_or_empty(*variants)
                    .iter()
                    .map(|variant| variant.name.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&format!("  variants: {}\n", variants));
            }
            DataShape::Record { fields } => {
                write_field_layouts(output, &backend_plan.layouts.fields, *fields);
            }
        }
    }

    for (_, machine_layout) in backend_plan.layouts.machine_layouts.iter() {
        let attached_data = machine_layout
            .attached_data
            .as_ref()
            .map(|name| name.as_str())
            .unwrap_or("<none>");
        output.push_str(&format!(
            "- machine {}: attached {}, size {}, align {}\n",
            machine_layout.name,
            attached_data,
            machine_layout.layout.size,
            machine_layout.layout.alignment
        ));
        write_field_layouts(output, &backend_plan.layouts.fields, machine_layout.fields);
    }

    output.push_str("\n## Object\n");
    output.push_str(&format!(
        "sections: {}\n",
        backend_plan.object.sections.len()
    ));
    for (_, section) in backend_plan.object.sections.iter() {
        write_section_plan(output, backend_plan.object.target, section);
    }

    output.push_str(&format!("symbols: {}\n", backend_plan.object.symbols.len()));
    for (_, symbol) in backend_plan.object.symbols.iter() {
        write_symbol_plan(output, backend_plan.object.target, symbol);
    }
    output.push('\n');

    output.push_str("## Relocations\n");
    output.push_str(&format!(
        "records: {}\n",
        backend_plan.relocations.record_set.records.len()
    ));
    if backend_plan.relocations.record_set.records.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, relocation) in backend_plan.relocations.record_set.records.iter() {
            write_relocation_record(output, backend_plan, relocation);
        }
    }
}

fn write_field_layouts(
    output: &mut String,
    fields: &omega_core::arena::Arena<FieldLayout>,
    field_span: omega_core::arena::HandleSpan<FieldLayout>,
) {
    let Some(fields) = fields.span(field_span) else {
        output.push_str("  fields: invalid span\n");
        return;
    };

    if fields.is_empty() {
        output.push_str("  fields: none\n");
        return;
    }

    output.push_str("  fields:\n");
    for field in fields {
        output.push_str(&format!(
            "    - {} @{}: {} size {}, align {}\n",
            field.name, field.offset, field.type_name, field.layout.size, field.layout.alignment
        ));
    }
}

fn write_section_plan(output: &mut String, target: NativeTarget, section: &SectionPlan) {
    output.push_str(&format!(
        "- section {} {:?}: size {}, align {}\n",
        section_name(target, section.kind),
        section.kind,
        section.size,
        section.alignment
    ));
}

fn write_symbol_plan(output: &mut String, target: NativeTarget, symbol: &SymbolPlan) {
    let section = symbol_section_name(target, symbol.section);
    let section = if section.is_empty() {
        "none"
    } else {
        section.as_str()
    };
    output.push_str(&format!(
        "- symbol {} {:?}: section {}, offset {}, size {}\n",
        symbol.name, symbol.kind, section, symbol.offset, symbol.size
    ));
}

fn write_relocation_record(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    relocation: &RelocationRecord,
) {
    output.push_str(&format!(
        "- {:?} {} text @{} width {} instruction #{} -> {}\n",
        relocation.kind,
        object_symbol_name(backend_plan.object, relocation.function_symbol_handle),
        relocation.text_offset,
        relocation.byte_width,
        relocation.selected_instruction_index,
        object_symbol_name(backend_plan.object, relocation.symbol_handle)
    ));
}
