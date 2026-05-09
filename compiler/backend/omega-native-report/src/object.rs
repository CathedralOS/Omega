use omega_native::plan::NativePlan;
use omega_layout::{DataShape, FieldLayout};
use omega_object::{RelocationRecord, SectionPlan, SymbolPlan};

pub(super) fn write_layout_object_sections(output: &mut String, native_plan: &NativePlan) {
    output.push_str("\n## Layouts\n");
    output.push_str(&format!(
        "data layouts: {}\n",
        native_plan.layouts.data_layouts.len()
    ));
    output.push_str(&format!(
        "machine layouts: {}\n",
        native_plan.layouts.machine_layouts.len()
    ));
    output.push_str(&format!("fields: {}\n\n", native_plan.layouts.fields.len()));

    for (_, data_layout) in native_plan.layouts.data_layouts.iter() {
        output.push_str(&format!(
            "- data {}: size {}, align {}\n",
            data_layout.name, data_layout.layout.size, data_layout.layout.alignment
        ));

        match &data_layout.shape {
            DataShape::Enum { variants } => {
                let variants = variants
                    .iter()
                    .map(|variant| variant.name.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&format!("  variants: {}\n", variants));
            }
            DataShape::Record { fields } => {
                write_field_layouts(output, &native_plan.layouts.fields, *fields);
            }
        }
    }

    for (_, machine_layout) in native_plan.layouts.machine_layouts.iter() {
        output.push_str(&format!(
            "- machine {}: size {}, align {}\n",
            machine_layout.name, machine_layout.layout.size, machine_layout.layout.alignment
        ));
        write_field_layouts(output, &native_plan.layouts.fields, machine_layout.fields);
    }

    output.push_str("\n## Object\n");
    output.push_str(&format!(
        "sections: {}\n",
        native_plan.object.sections.len()
    ));
    for (_, section) in native_plan.object.sections.iter() {
        write_section_plan(output, section);
    }

    output.push_str(&format!("symbols: {}\n", native_plan.object.symbols.len()));
    for (_, symbol) in native_plan.object.symbols.iter() {
        write_symbol_plan(output, symbol);
    }
    output.push('\n');

    output.push_str("## Relocations\n");
    output.push_str(&format!(
        "records: {}\n",
        native_plan.relocations.records.len()
    ));
    if native_plan.relocations.records.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, relocation) in native_plan.relocations.records.iter() {
            write_relocation_record(output, relocation);
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

fn write_section_plan(output: &mut String, section: &SectionPlan) {
    output.push_str(&format!(
        "- section {} {:?}: size {}, align {}\n",
        section.name, section.kind, section.size, section.alignment
    ));
}

fn write_symbol_plan(output: &mut String, symbol: &SymbolPlan) {
    let section = symbol.section.as_deref().unwrap_or("none");
    output.push_str(&format!(
        "- symbol {} {:?}: section {}, offset {}, size {}\n",
        symbol.name, symbol.kind, section, symbol.offset, symbol.size
    ));
}

fn write_relocation_record(output: &mut String, relocation: &RelocationRecord) {
    output.push_str(&format!(
        "- {:?} {} text @{} width {} instruction #{} -> {}\n",
        relocation.kind,
        relocation.function_symbol,
        relocation.text_offset,
        relocation.byte_width,
        relocation.selected_instruction_index,
        relocation.symbol
    ));
}
