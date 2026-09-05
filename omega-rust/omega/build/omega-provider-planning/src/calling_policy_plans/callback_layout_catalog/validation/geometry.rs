use super::super::BoundaryCallbackLayoutEntry;
use omega_layout::TargetClosedPlanLaidDataLayoutIdentity;
use omega_target::NativeTarget;

pub(super) fn validate(
    entry: &BoundaryCallbackLayoutEntry,
    target: NativeTarget,
) -> Result<(), String> {
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
    let child = if let Some(field) = &entry.inline_field {
        let child = &field.child_layout;
        if !field.symbol.is_valid()
            || field.identity.is_empty()
            || !valid_layout(child)
            || field.extent != child.physical.size
            || field.alignment != child.physical.alignment
            || !contained(root, field.offset, field.extent, field.alignment)
            || field.offset.checked_add(terminal.offset) != Some(entry.composed_offset)
        {
            return Err(
                "callback layout catalog changed its named inline field geometry".to_owned(),
            );
        }
        child
    } else {
        if entry.composed_offset != terminal.offset {
            return Err("callback layout catalog changed its direct slot offset".to_owned());
        }
        root
    };
    if child.data_symbol != terminal.data_symbol
        || child.layout_subject_identity != terminal.layout_subject_identity
        || !contained(
            child,
            terminal.offset,
            terminal.byte_size,
            terminal.alignment,
        )
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

fn valid_layout(layout: &TargetClosedPlanLaidDataLayoutIdentity) -> bool {
    layout.data_symbol.is_valid()
        && !layout.data_identity.is_empty()
        && !layout.layout_subject_identity.is_empty()
        && layout.physical.alignment.is_power_of_two()
}

fn contained(
    layout: &TargetClosedPlanLaidDataLayoutIdentity,
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
