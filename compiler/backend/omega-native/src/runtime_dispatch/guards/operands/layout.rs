use crate::runtime_dispatch::guards::StateGuardOperandStorage;
use omega_core::arena::HandleSpan;
use omega_layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResolvedOperandLayout {
    pub storage: StateGuardOperandStorage,
    pub byte_offset: usize,
    pub layout: TypeLayout,
}

pub(super) fn resolve_guard_operand_layout(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    expression: &Expression,
) -> Option<ResolvedOperandLayout> {
    let Expression::Name(path) = expression else {
        return None;
    };
    let [root_name, suffix @ ..] = path.as_slice() else {
        return None;
    };
    let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == source_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let root_field = field_layout(layouts, machine_layout.fields, root_name)?;

    resolve_nested_field_layout(layouts, root_field, suffix).map(|(byte_offset, layout)| {
        ResolvedOperandLayout {
            storage: StateGuardOperandStorage::MachineOwned,
            byte_offset: machine_base_offset + byte_offset,
            layout,
        }
    })
}

fn machine_storage_offset(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
) -> Option<usize> {
    if entry_machine == source_machine {
        return Some(0);
    }

    let entry_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == entry_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let fields = layouts.fields.span(entry_layout.fields)?;

    fields
        .iter()
        .find(|field| field.type_name == source_machine)
        .map(|field| field.offset)
}

fn resolve_nested_field_layout(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: &[ProgramName],
) -> Option<(usize, TypeLayout)> {
    let mut byte_offset = root_field.offset;
    let mut type_name = root_field.type_name.as_str();
    let mut layout = root_field.layout;

    for segment in suffix {
        let field_segment = parse_field_segment(segment)?;
        let data_layout = layouts
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.name == type_name)
            .map(|(_, data_layout)| data_layout)?;
        let DataShape::Record { fields } = &data_layout.shape else {
            return None;
        };
        let field = field_layout(layouts, *fields, field_segment.name)?;
        byte_offset += field.offset;
        type_name = &field.type_name;
        layout = field.layout;

        if let Some(index) = field_segment.index {
            let array = parse_array_type_name(type_name)?;
            if index >= array.length {
                return None;
            }
            let element_layout = TypeLayout {
                size: layout.size / array.length,
                alignment: layout.alignment,
            };
            byte_offset += element_layout.size * index;
            type_name = array.element_type_name;
            layout = element_layout;
        }
    }

    Some((byte_offset, layout))
}

struct FieldSegment<'name> {
    name: &'name str,
    index: Option<usize>,
}

fn parse_field_segment(segment: &str) -> Option<FieldSegment<'_>> {
    let Some((field_name, index_suffix)) = segment.split_once('[') else {
        return Some(FieldSegment {
            name: segment,
            index: None,
        });
    };
    let index = index_suffix.strip_suffix(']')?.parse::<usize>().ok()?;
    Some(FieldSegment {
        name: field_name,
        index: Some(index),
    })
}

struct ArrayTypeName<'name> {
    element_type_name: &'name str,
    length: usize,
}

fn parse_array_type_name(type_name: &str) -> Option<ArrayTypeName<'_>> {
    let inner = type_name.strip_prefix('[')?.strip_suffix(']')?;
    let (element_type_name, length) = inner.split_once(';')?;
    Some(ArrayTypeName {
        element_type_name: element_type_name.trim(),
        length: length.trim().parse::<usize>().ok()?,
    })
}

fn field_layout<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
    field_name: &str,
) -> Option<&'plan FieldLayout> {
    layouts
        .fields
        .span(fields)?
        .iter()
        .find(|field| field.name == field_name)
}
