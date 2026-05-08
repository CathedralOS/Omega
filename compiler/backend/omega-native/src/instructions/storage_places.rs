use crate::control_flow::StateKey;
use crate::layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout};
use crate::object::{machine_storage_symbol_name, runtime_frame_storage_symbol_name};
use crate::plan::NativePlan;
use omega_core::arena::HandleSpan;
use omega_typed_program::expression::{Expression, IndexedExpression};
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeStoragePlace {
    pub(super) symbol: String,
    pub(super) byte_offset: usize,
    pub(super) byte_count: usize,
}

pub(super) fn resolve_runtime_storage_place(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    _source_state: &str,
    expression: &Expression,
) -> Option<RuntimeStoragePlace> {
    if let Some((byte_offset, byte_count)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        expression,
    ) {
        return Some(RuntimeStoragePlace {
            symbol: machine_storage_symbol_name(&native_plan.entry_machine),
            byte_offset,
            byte_count,
        });
    }

    let expression = match expression {
        Expression::Mutable(target) => target.as_ref(),
        _ => expression,
    };
    let normalized_expression;
    let expression = match expression {
        Expression::Indexed(indexed) => {
            normalized_expression = Expression::Name(indexed_expression_path(indexed)?);
            &normalized_expression
        }
        _ => expression,
    };
    let Expression::Name(path) = expression else {
        return None;
    };
    let [root_name, suffix @ ..] = path.as_slice() else {
        return None;
    };
    let slot = native_plan
        .runtime_storage
        .frame_slots
        .iter()
        .find(|(_, slot)| {
            slot.dispatch_index == dispatch_index
                && slot.source_key == source_key
                && slot.name == *root_name
        })
        .or_else(|| {
            native_plan
                .runtime_storage
                .frame_slots
                .iter()
                .find(|(_, slot)| slot.dispatch_index == dispatch_index && slot.name == *root_name)
        })
        .map(|(_, slot)| slot)?;
    let root_field = FieldLayout {
        name: slot.name.clone(),
        offset: slot.byte_offset,
        type_name: slot.type_name.clone(),
        layout: TypeLayout {
            size: slot.byte_size,
            alignment: slot.alignment,
        },
    };
    let (byte_offset, layout) =
        resolve_nested_field_layout(&native_plan.layouts, &root_field, suffix)?;

    Some(RuntimeStoragePlace {
        symbol: runtime_frame_storage_symbol_name(),
        byte_offset,
        byte_count: layout.size,
    })
}

pub(super) fn resolve_machine_owned_place(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    expression: &Expression,
) -> Option<(usize, usize)> {
    let expression = match expression {
        Expression::Mutable(target) => target.as_ref(),
        _ => expression,
    };
    let normalized_expression;
    let expression = match expression {
        Expression::Indexed(indexed) => {
            normalized_expression = Expression::Name(indexed_expression_path(indexed)?);
            &normalized_expression
        }
        _ => expression,
    };
    let Expression::Name(path) = expression else {
        return None;
    };
    let [root_name, suffix @ ..] = path.as_slice() else {
        return None;
    };
    let (machine_base_offset, root_field) =
        root_machine_field_layout(layouts, entry_machine, source_machine, root_name)?;
    let (field_offset, field_layout) = resolve_nested_field_layout(layouts, root_field, suffix)?;

    Some((machine_base_offset + field_offset, field_layout.size))
}

fn root_machine_field_layout<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    root_name: &str,
) -> Option<(usize, &'plan FieldLayout)> {
    root_machine_field_layout_for_machine(layouts, entry_machine, source_machine, root_name)
        .or_else(|| {
            layouts
                .machine_layouts
                .iter()
                .find_map(|(_, machine_layout)| {
                    root_machine_field_layout_for_machine(
                        layouts,
                        entry_machine,
                        &machine_layout.name,
                        root_name,
                    )
                })
        })
}

fn root_machine_field_layout_for_machine<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    root_name: &str,
) -> Option<(usize, &'plan FieldLayout)> {
    let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == source_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let root_field = field_layout(layouts, machine_layout.fields, root_name)?;
    Some((machine_base_offset, root_field))
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
    layouts
        .fields
        .span(entry_layout.fields)?
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

    for field_name in suffix {
        let field_segment = parse_field_segment(field_name)?;
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

pub(super) fn enum_variant_value(layouts: &LayoutPlan, expression: &Expression) -> Option<i64> {
    let Expression::Name(path) = expression else {
        return None;
    };
    let [type_name, variant_name] = path.as_slice() else {
        return None;
    };
    let data_layout = layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == *type_name)
        .map(|(_, data_layout)| data_layout)?;
    let DataShape::Enum { variants } = &data_layout.shape else {
        return None;
    };
    variants
        .iter()
        .position(|variant| variant == variant_name)
        .and_then(|index| i64::try_from(index).ok())
}

pub(super) fn static_integer_value(layouts: &LayoutPlan, expression: &Expression) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        _ => enum_variant_value(layouts, expression),
    }
}

pub(super) fn indexed_expression_path(indexed: &IndexedExpression) -> Option<Vec<ProgramName>> {
    let Expression::Integer(index) = &indexed.index else {
        return None;
    };
    let mut path = match &indexed.collection {
        Expression::Name(path) => path.clone(),
        Expression::Indexed(inner_indexed) => indexed_expression_path(inner_indexed)?,
        _ => return None,
    };
    let last_segment = path.last_mut()?;
    *last_segment = ProgramName::generated(format!("{last_segment}[{index}]"));
    Some(path)
}
