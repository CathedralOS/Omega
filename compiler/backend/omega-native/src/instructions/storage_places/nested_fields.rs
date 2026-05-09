use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout};
use omega_typed_program::name::ProgramName;

pub(in crate::instructions) fn resolve_nested_field_layout(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: &[ProgramName],
) -> Option<(usize, TypeLayout)> {
    let mut byte_offset = root_field.offset;
    let mut type_symbol = root_field.type_symbol;
    let mut type_name = root_field.type_name.as_str();
    let mut layout = root_field.layout;

    for field_name in suffix {
        let field_segment = parse_field_segment(field_name)?;
        let data_layout = data_layout(layouts, type_symbol, type_name)?;
        let DataShape::Record { fields } = &data_layout.shape else {
            return None;
        };
        let field = field_layout(layouts, *fields, field_segment.name)?;
        byte_offset += field.offset;
        type_symbol = field.type_symbol;
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
            type_symbol = field.type_symbol;
            type_name = array.element_type_name;
            layout = element_layout;
        }
    }

    Some((byte_offset, layout))
}

fn data_layout<'plan>(
    layouts: &'plan LayoutPlan,
    type_symbol: SymbolHandle,
    type_name: &str,
) -> Option<&'plan omega_layout::DataLayout> {
    if type_symbol.is_valid() {
        return layouts
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.symbol == type_symbol)
            .map(|(_, data_layout)| data_layout);
    }

    layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == type_name)
        .map(|(_, data_layout)| data_layout)
}

pub(in crate::instructions) fn field_layout<'plan>(
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
