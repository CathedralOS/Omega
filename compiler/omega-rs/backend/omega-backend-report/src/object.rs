use crate::BackendReportInput;
use omega_layout::{DataShape, FieldLayout};
use omega_object_file::{
    ObjectPlan, RelocationOrigin, RelocationRecord, SectionPlan, SymbolPlan, object_symbol_name,
    section_name, symbol_section_name,
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
            DataShape::Enum {
                common_fields,
                variants,
            } => {
                let variant_layouts = backend_plan.layouts.variants.span_or_empty(*variants);
                let names = variant_layouts
                    .iter()
                    .map(|variant| variant.name.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&format!("  variants (tag at offset 0): {}\n", names));
                if common_fields.count() > 0 {
                    output.push_str("  common fields (every case):\n");
                    for field in backend_plan.layouts.fields.span_or_empty(*common_fields) {
                        output.push_str(&format!(
                            "    - {}: offset {}, size {}, align {} ({})\n",
                            field.name,
                            field.offset,
                            field.layout.size,
                            field.layout.alignment,
                            field.type_name
                        ));
                    }
                }
                for (tag, variant) in variant_layouts.iter().enumerate() {
                    if variant.fields.count() == 0 {
                        continue;
                    }
                    output.push_str(&format!("  case {} (tag {}) payload:\n", variant.name, tag));
                    for field in backend_plan.layouts.fields.span_or_empty(variant.fields) {
                        output.push_str(&format!(
                            "    - {}: offset {}, size {}, align {} ({})\n",
                            field.name,
                            field.offset,
                            field.layout.size,
                            field.layout.alignment,
                            field.type_name
                        ));
                    }
                }
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
        backend_plan.object.layout.sections.len()
    ));
    for (_, section) in backend_plan.object.layout.sections.iter() {
        write_section_plan(output, backend_plan.object.target, section);
    }

    output.push_str(&format!(
        "symbols: {}\n",
        backend_plan.object.layout.symbols.len()
    ));
    for (_, symbol) in backend_plan.object.layout.symbols.iter() {
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
    fields: &psi_arena::Arena<FieldLayout>,
    field_span: psi_arena::HandleSpan<FieldLayout>,
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
    let origin = relocation_origin_label(backend_plan.object, relocation.origin);
    output.push_str(&format!(
        "- {:?} {} {} @{} width {} -> {} addend {}\n",
        relocation.kind,
        origin,
        section_name(backend_plan.target, relocation.section),
        relocation.offset,
        relocation.byte_width,
        object_symbol_name(backend_plan.object, relocation.symbol_handle),
        relocation.addend
    ));
}

fn relocation_origin_label(object: &ObjectPlan, origin: RelocationOrigin) -> String {
    match origin {
        RelocationOrigin::Instruction {
            function_symbol_handle,
            selected_instruction_index,
        } => format!(
            "instruction {} #{}",
            object_symbol_name(object, function_symbol_handle),
            selected_instruction_index
        ),
        RelocationOrigin::SemanticOperation {
            function_symbol_handle,
            operation_identity,
        } => format!(
            "semantic operation {} #{}",
            object_symbol_name(object, function_symbol_handle),
            operation_identity
        ),
        RelocationOrigin::SemanticEdge {
            function_symbol_handle,
            edge_identity,
        } => format!(
            "semantic edge {} #{}",
            object_symbol_name(object, function_symbol_handle),
            edge_identity
        ),
        RelocationOrigin::Materialization {
            object_symbol_handle,
        } => format!(
            "materialization {}",
            object_symbol_name(object, object_symbol_handle)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::relocation_origin_label;
    use omega_object_file::{ObjectPlan, RelocationOrigin, SymbolKind, SymbolPlan, SymbolSection};
    use omega_target::NativeTarget;

    #[test]
    fn report_names_semantic_edge_ownership_without_calling_it_an_operation() {
        let mut object = ObjectPlan::with_capacity(NativeTarget::linux_arm64(), 0, 1);
        let caller = object.layout.symbols.insert(SymbolPlan {
            name: "cleanup-caller".to_owned(),
            section: SymbolSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });

        assert_eq!(
            relocation_origin_label(
                &object,
                RelocationOrigin::SemanticEdge {
                    function_symbol_handle: caller,
                    edge_identity: 7,
                },
            ),
            "semantic edge cleanup-caller #7"
        );
    }
}
