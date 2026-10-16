use super::PrivateCallbackLayout;
use crate::TargetClosedDataLayoutIdentity;
use target::NativeTarget;

pub(super) fn validate(entry: &PrivateCallbackLayout, target: NativeTarget) -> Result<(), String> {
    let root = &entry.root_layout;
    let terminal = &entry.terminal_slot;
    if !valid_layout(root)
        || terminal.byte_size == 0
        || terminal.byte_size != target.pointer_size
        || terminal.alignment != target.pointer_alignment
    {
        return Err(
            "callback layout catalog changed its root or target pointer geometry".to_owned(),
        );
    }
    let mut parent = root;
    let mut offset = 0usize;
    let mut seen = vec![root.data_symbol];
    for field in entry.inline_fields()? {
        let child = &field.child_layout;
        if seen.contains(&child.data_symbol)
            || !field.symbol.is_valid()
            || field.identity.is_empty()
            || !valid_layout(child)
            || field.field_layout.symbol != field.symbol
            || field.field_layout.type_descriptor.inline_data_symbol() != Some(child.data_symbol)
            || field.field_layout.offset != field.offset
            || field.field_layout.layout != child.physical
            || field.extent != child.physical.size
            || field.alignment != child.physical.alignment
            || !contained(parent, field.offset, field.extent, field.alignment)
        {
            return Err(
                "callback layout catalog changed its named inline field geometry".to_owned(),
            );
        }
        offset = offset
            .checked_add(field.offset)
            .ok_or_else(|| "callback inline path offset overflowed".to_owned())?;
        seen.push(child.data_symbol);
        parent = child;
    }
    if parent.data_symbol != terminal.data_symbol
        || parent.layout_subject_identity.as_deref()
            != Some(terminal.layout_subject_identity.as_ref())
        || !contained(
            parent,
            terminal.offset,
            terminal.byte_size,
            terminal.alignment,
        )
        || offset.checked_add(terminal.offset) != Some(entry.composed_offset)
        || !contained(
            root,
            entry.composed_offset,
            terminal.byte_size,
            terminal.alignment,
        )
    {
        return Err(
            "callback layout catalog changed its terminal owner or composed extent".to_owned(),
        );
    }
    Ok(())
}
fn valid_layout(layout: &TargetClosedDataLayoutIdentity) -> bool {
    layout.data_symbol.is_valid()
        && !layout.data_identity.is_empty()
        && layout
            .layout_subject_identity
            .as_ref()
            .is_none_or(|identity| !identity.is_empty())
        && layout.physical.alignment.is_power_of_two()
}
fn contained(
    layout: &TargetClosedDataLayoutIdentity,
    offset: usize,
    extent: usize,
    alignment: usize,
) -> bool {
    alignment.is_power_of_two()
        && layout.physical.alignment >= alignment
        && offset.is_multiple_of(alignment)
        && offset
            .checked_add(extent)
            .is_some_and(|end| end <= layout.physical.size)
}
