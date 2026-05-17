use omega_checked_trees::name::ProgramName;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout};

pub(in crate::selection) fn resolve_nested_field_layout_with_symbols(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: &[ProgramName],
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
    suffix: impl IntoIterator<Item = (&'suffix ProgramName, SymbolHandle, Option<usize>)>,
) -> Option<(usize, TypeLayout)> {
    resolve_nested_field_layout(layouts, root_field, suffix)
}

#[derive(Clone, Copy)]
pub(in crate::selection) struct NestedFieldLayoutCursor<'layout> {
    byte_offset: usize,
    type_symbol: SymbolHandle,
    type_name: &'layout str,
    layout: TypeLayout,
}

impl<'layout> NestedFieldLayoutCursor<'layout> {
    pub(in crate::selection) fn from_root(root_field: &'layout FieldLayout) -> Self {
        Self {
            byte_offset: root_field.offset,
            type_symbol: root_field.type_symbol,
            type_name: root_field.type_name.as_ref(),
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
    field_name: &ProgramName,
    field_symbol: SymbolHandle,
    field_index: Option<usize>,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    let field_segment = parse_field_segment(field_name, field_index)?;
    let data_layout = data_layout(layouts, cursor.type_symbol, cursor.type_name)?;
    let DataShape::Record { fields } = &data_layout.shape else {
        return None;
    };
    let field = field_layout_by_symbol(layouts, *fields, field_symbol)?;
    let mut next = NestedFieldLayoutCursor {
        byte_offset: cursor.byte_offset + field.offset,
        type_symbol: field.type_symbol,
        type_name: &field.type_name,
        layout: field.layout,
    };

    if let Some(index) = field_segment.index {
        let array = parse_array_type_name(next.type_name)?;
        if index >= array.length {
            return None;
        }
        let element_layout = TypeLayout {
            size: next.layout.size / array.length,
            alignment: next.layout.alignment,
        };
        next.byte_offset += element_layout.size * index;
        next.type_symbol = field.type_symbol;
        next.type_name = array.element_type_name;
        next.layout = element_layout;
    }

    Some(next)
}

fn resolve_nested_field_layout<'suffix>(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: impl IntoIterator<Item = (&'suffix ProgramName, SymbolHandle, Option<usize>)>,
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
    _type_name: &str,
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

fn field_layout_by_symbol<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
    field_symbol: SymbolHandle,
) -> Option<&'plan FieldLayout> {
    field_symbol
        .is_valid()
        .then(|| {
            layouts
                .fields
                .span(fields)?
                .iter()
                .find(|field| field.symbol == field_symbol)
        })
        .flatten()
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
