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
            .map(|(index, field_name)| (field_name, suffix_symbol(index), None, None)),
    )
}

pub(in crate::selection) fn resolve_nested_field_layout_with_pairs<'suffix>(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: impl IntoIterator<
        Item = (
            &'suffix Identifier,
            SymbolHandle,
            Option<usize>,
            Option<&'suffix Identifier>,
        ),
    >,
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

    pub(in crate::selection) fn type_descriptor(self) -> &'layout TypeLayoutDescriptor {
        self.type_descriptor
    }

    pub(in crate::selection) fn from_indexed_fixed_array_element(
        cursor: Self,
        index: usize,
        element_type: &'layout TypeLayoutDescriptor,
        element_layout: TypeLayout,
    ) -> Self {
        Self {
            byte_offset: cursor.byte_offset + element_layout.size * index,
            type_symbol: element_type.storage_symbol(),
            type_name: "",
            type_descriptor: element_type,
            layout: element_layout,
        }
    }
}

pub(in crate::selection) fn resolve_nested_field_layout_step<'layout>(
    layouts: &'layout LayoutPlan,
    cursor: NestedFieldLayoutCursor<'layout>,
    field_name: &Identifier,
    field_symbol: SymbolHandle,
    field_index: Option<usize>,
    case_variant: Option<&Identifier>,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    let field_segment = parse_field_segment(field_name, field_index)?;
    let data_layout = data_layout(layouts, cursor.type_descriptor.storage_symbol())?;
    let field = match &data_layout.shape {
        DataShape::Record { fields } => {
            field_layout_by_symbol_or_name(layouts, *fields, field_symbol, field_name)?
        }
        // A member of case-bearing data: a COMMON field (mixed shapes --
        // `event.consumed`, readable without case knowledge) or a CASE PAYLOAD
        // field (`cmd.steps`), searched across every variant's payload span.
        // All offsets are ABSOLUTE within the value (tag at 0, common fields
        // after the tag, payloads packed from the shared base), so the offset
        // arithmetic below is identical to a record field.
        DataShape::Enum {
            common_fields,
            variants,
        } => {
            // A destructure binding records the CASE VARIANT its payload field
            // came from (`Tx::Transfer { amount }` -> variant `Transfer`).
            // Resolve within THAT variant so a same-named payload field in an
            // earlier variant (`Tx::Deposit { amount }`, at a different offset)
            // is not picked instead. Common fields stay name-resolvable.
            let by_case_variant = case_variant.and_then(|variant_name| {
                field_layout_by_name(layouts, *common_fields, field_name).or_else(|| {
                    layouts
                        .variants
                        .span_or_empty(*variants)
                        .iter()
                        .find(|variant| variant.name.as_str() == variant_name.as_str())
                        .and_then(|variant| {
                            field_layout_by_name(layouts, variant.fields, field_name)
                        })
                })
            });
            // When the member resolved to a specific field SYMBOL (e.g. a
            // destructured case-payload field bound to its variant), match that
            // symbol EXACTLY across the common fields + every variant. This must
            // not name-fall-back, or a same-named field in an EARLIER variant (at
            // a different offset) would be picked instead of the intended one.
            let by_symbol = field_symbol.is_valid().then(|| {
                field_layout_by_symbol(layouts, *common_fields, field_symbol).or_else(|| {
                    layouts
                        .variants
                        .span_or_empty(*variants)
                        .iter()
                        .find_map(|variant| {
                            field_layout_by_symbol(layouts, variant.fields, field_symbol)
                        })
                })
            });
            // Variant-disambiguated match wins; then a resolved symbol; then,
            // with neither (or no match), fall back to the first field with the
            // name, common-fields-first then variant order.
            by_case_variant
                .or_else(|| by_symbol.flatten())
                .or_else(|| {
                    field_layout_by_name(layouts, *common_fields, field_name).or_else(|| {
                        layouts
                            .variants
                            .span_or_empty(*variants)
                            .iter()
                            .find_map(|variant| {
                                field_layout_by_name(layouts, variant.fields, field_name)
                            })
                    })
                })?
        }
    };
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
    suffix: impl IntoIterator<
        Item = (
            &'suffix Identifier,
            SymbolHandle,
            Option<usize>,
            Option<&'suffix Identifier>,
        ),
    >,
) -> Option<(usize, TypeLayout)> {
    let mut cursor = NestedFieldLayoutCursor::from_root(root_field);

    for (field_name, field_symbol, field_index, case_variant) in suffix {
        cursor = resolve_nested_field_layout_step(
            layouts,
            cursor,
            field_name,
            field_symbol,
            field_index,
            case_variant,
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
    field_layout_by_symbol(layouts, fields, field_symbol)
        .or_else(|| field_layout_by_name(layouts, fields, field_name))
}

/// Match a field by SYMBOL only (no name fallback). `None` for an invalid symbol.
fn field_layout_by_symbol<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
    field_symbol: SymbolHandle,
) -> Option<&'plan FieldLayout> {
    if !field_symbol.is_valid() {
        return None;
    }
    layouts
        .fields
        .span(fields)?
        .iter()
        .find(|field| field.symbol == field_symbol)
}

/// Match a field by NAME (ignoring any `[index]` suffix).
fn field_layout_by_name<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
    field_name: &Identifier,
) -> Option<&'plan FieldLayout> {
    let name = field_name_without_index(field_name);
    layouts
        .fields
        .span(fields)?
        .iter()
        .find(|field| field.name.as_str() == name)
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
