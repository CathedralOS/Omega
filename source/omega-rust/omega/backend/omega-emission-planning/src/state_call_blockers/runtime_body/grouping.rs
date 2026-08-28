use super::model::RuntimeBodyStateCallBlocker;

pub(super) fn push_runtime_body_state_call_blocker(
    grouped_blockers: &mut Vec<RuntimeBodyStateCallBlocker>,
    blocker: RuntimeBodyStateCallBlocker,
) {
    if let Some(existing) = grouped_blockers.iter_mut().find(|existing| {
        existing.dispatch_index == blocker.dispatch_index
            && existing.source_key == blocker.source_key
            && existing.target_key == blocker.target_key
            && existing.argument_count == blocker.argument_count
            && existing.lowering == blocker.lowering
    }) {
        existing.count += 1;
        return;
    }

    grouped_blockers.push(blocker);
}

pub(super) fn repeated_count_suffix(count: usize) -> String {
    if count <= 1 {
        String::new()
    } else {
        format!(" ({count} sites)")
    }
}
