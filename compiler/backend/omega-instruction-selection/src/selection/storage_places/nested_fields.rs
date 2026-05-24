use omega_checked_trees::name::Identifier;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout, TypeLayoutDescriptor};

pub(in crate::selection) fn resolve_nested_field_layout_with_symbols(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: &[Identifier],
    mut suffix_symbol: impl FnMut(usize) -> SymbolHandle,
) -> Option<(usize, TypeLayout)> {
    resolve_nested_field_layout(
        layouts,
        root_field,
        suffix
            .iter()
            .enumerate()
            .map(|(index, field_name)| (field_name, suffix_symbol(index), None)),
    )
}

pub(in crate::selection) fn resolve_nested_field_layout_with_pairs<'suffix>(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: impl IntoIterator<Item = (&'suffix Identifier, SymbolHandle, Option<usize>)>,
) -> Option<(usize, TypeLayout)> {
    resolve_nested_field_layout(layouts, root_field, suffix)
}

#[derive(Clone, Copy)]
pub(in crate::selection) struct NestedFieldLayoutCursor<'layout> {
    byte_offset: usize,
    type_symbol: SymbolHandle,
    type_name: &'layout str,
    type_descriptor: &'layout TypeLayoutDescriptor,
    layout: TypeLayout,
}

impl<'layout> NestedFieldLayoutCursor<'layout> {
    pub(in crate::selection) fn from_root(root_field: &'layout FieldLayout) -> Self {
        Self {
            byte_offset: root_field.offset,
            type_symbol: root_field.type_symbol,
            type_name: root_field.type_name.as_ref(),
            type_descriptor: &root_field.type_descriptor,
            layout: root_field.layout,
        }
    }

    pub(in crate::selection) fn byte_offset(self) -> usize {
        self.byte_offset
    }

    pub(in crate::selection) fn layout(self) -> TypeLayout {
        self.layout
    }
}

pub(in crate::selection) fn resolve_nested_field_layout_step<'layout>(
    layouts: &'layout LayoutPlan,
    cursor: NestedFieldLayoutCursor<'layout>,
    field_name: &Identifier,
    field_symbol: SymbolHandle,
    field_index: Option<usize>,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    let field_segment = parse_field_segment(field_name, field_index)?;
    let data_layout = data_layout(layouts, cursor.type_descriptor.storage_symbol())?;
    let DataShape::Record { fields } = &data_layout.shape else {
        return None;
    };
    let field = field_layout_by_symbol_or_name(layouts, *fields, field_symbol, field_name)?;
    let mut next = NestedFieldLayoutCursor {
        byte_offset: cursor.byte_offset + field.offset,
        type_symbol: field.type_symbol,
        type_name: &field.type_name,
        type_descriptor: &field.type_descriptor,
        layout: field.layout,
    };

    if let Some(index) = field_segment.index {
        let (element_type, length) = next.type_descriptor.fixed_array()?;
        if index >= length {
            return None;
        }
        let element_layout = TypeLayout {
            size: next.layout.size / length,
            alignment: next.layout.alignment,
        };
        next.byte_offset += element_layout.size * index;
        next.type_symbol = element_type.storage_symbol();
        next.type_name = "";
        next.type_descriptor = element_type;
        next.layout = element_layout;
    }

    Some(next)
}

fn resolve_nested_field_layout<'suffix>(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: impl IntoIterator<Item = (&'suffix Identifier, SymbolHandle, Option<usize>)>,
) -> Option<(usize, TypeLayout)> {
    let mut cursor = NestedFieldLayoutCursor::from_root(root_field);

    for (field_name, field_symbol, field_index) in suffix {
        cursor = resolve_nested_field_layout_step(
            layouts,
            cursor,
            field_name,
            field_symbol,
            field_index,
        )?;
    }

    Some((cursor.byte_offset(), cursor.layout()))
}

fn data_layout<'plan>(
    layouts: &'plan LayoutPlan,
    type_symbol: SymbolHandle,
) -> Option<&'plan omega_layout::DataLayout> {
    if !type_symbol.is_valid() {
        return None;
    }

    layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.symbol == type_symbol)
        .map(|(_, data_layout)| data_layout)
}

fn field_layout_by_symbol_or_name<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
    field_symbol: SymbolHandle,
    field_name: &Identifier,
) -> Option<&'plan FieldLayout> {
    let fields = layouts.fields.span(fields)?;
    fields
        .iter()
        .find(|field| field_symbol.is_valid() && field.symbol == field_symbol)
        .or_else(|| {
            let name = field_name_without_index(field_name);
            fields.iter().find(|field| field.name.as_str() == name)
        })
}

fn field_name_without_index(field_name: &str) -> &str {
    field_name
        .split_once('[')
        .map(|(name, _)| name)
        .unwrap_or(field_name)
}

struct FieldSegment {
    index: Option<usize>,
}

fn parse_field_segment(segment: &str, explicit_index: Option<usize>) -> Option<FieldSegment> {
    if explicit_index.is_some() {
        return Some(FieldSegment {
            index: explicit_index,
        });
    }

    let Some((_field_name, index_suffix)) = segment.split_once('[') else {
        return Some(FieldSegment { index: None });
    };
    let index = index_suffix.strip_suffix(']')?.parse::<usize>().ok()?;
    Some(FieldSegment { index: Some(index) })
}
