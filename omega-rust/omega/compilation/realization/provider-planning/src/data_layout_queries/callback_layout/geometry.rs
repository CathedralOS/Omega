use super::PrivateCallbackLayout;
use crate::data_layout_queries::TargetClosedDataLayoutIdentity;
use crate::data_layout_queries::sizing::pointer_layout;
use target::NativeTarget;

pub(super) fn validate(entry: &PrivateCallbackLayout, target: NativeTarget) -> Result<(), String> {
    let root = &entry.root_layout;
    let terminal = &entry.terminal_slot;
    let slot = pointer_layout(target);
    if !valid_layout(root)
        || terminal.byte_size == 0
        || terminal.byte_size != slot.size
        || terminal.alignment != slot.alignment
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
    {
        return Err("callback layout catalog changed its terminal owner".to_owned());
    }
    layout_plans::validate_private_callback_placements(
        terminal.placements.as_slice(),
        terminal.byte_size as u64,
        terminal.alignment as u64,
        parent.physical.size as u64,
        parent.physical.alignment as u64,
    )?;
    let placements = layout_plans::compose_private_callback_placements(
        terminal.placements.as_slice(),
        offset as u64,
    )?;
    if placements.as_slice() != entry.composed_placements.as_slice() {
        return Err("callback layout catalog changed its composed placements".to_owned());
    }
    layout_plans::validate_private_callback_placements(
        &placements,
        terminal.byte_size as u64,
        terminal.alignment as u64,
        root.physical.size as u64,
        root.physical.alignment as u64,
    )?;
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
