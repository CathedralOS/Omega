use super::backend_state_name;

use crate::BackendReportInput;
use omega_runtime_storage::RuntimeFrameSlotKind;

pub(super) fn write_storage_sections(output: &mut String, backend_plan: &BackendReportInput<'_>) {
    output.push_str("## State Storage\n");
    output.push_str(&format!(
        "locals: {}\n",
        backend_plan.state_storage.locals.len()
    ));
    for (_, local) in backend_plan.state_storage.locals.iter() {
        let source_name = backend_state_name(backend_plan, local.source_key);
        let type_name = backend_plan
            .state_storage
            .type_references
            .display_name(local.type_reference);
        output.push_str(&format!(
            "- {} statement {} local `{}`: {} required {}\n",
            source_name, local.statement_index, local.name, type_name, local.required
        ));
    }
    output.push_str(&format!(
        "mutations: {}\n",
        backend_plan.state_storage.mutations.len()
    ));
    for (_, mutation) in backend_plan.state_storage.mutations.iter() {
        let source_name = backend_state_name(backend_plan, mutation.source_key);
        output.push_str(&format!(
            "- {} statement {} {:?}/{:?}: `{}` = `{}` required {}\n",
            source_name,
            mutation.statement_index,
            mutation.mutation_kind,
            mutation.lowering,
            backend_plan
                .state_storage
                .expressions
                .display_name(mutation.target),
            backend_plan
                .state_storage
                .expressions
                .display_name(mutation.value),
            mutation.required
        ));
    }
    output.push('\n');

    output.push_str("## Runtime Storage\n");
    output.push_str(&format!(
        "frame slots: {}\n",
        backend_plan.runtime_storage.frame_slots.len()
    ));
    for (_, slot) in backend_plan.runtime_storage.frame_slots.iter() {
        let source_name = backend_state_name(backend_plan, slot.source_key);
        let kind = runtime_frame_slot_kind_name(&slot.kind);
        output.push_str(&format!(
            "- #{} {} statement {} {} `{}`: {} offset {} bytes {} align {}\n",
            slot.dispatch_index,
            source_name,
            slot.statement_index,
            kind,
            slot.name,
            slot.type_name,
            slot.byte_offset,
            slot.byte_size,
            slot.alignment
        ));
    }
    output.push_str(&format!(
        "writes: {}\n",
        backend_plan.runtime_storage.writes.len()
    ));
    for (_, write) in backend_plan.runtime_storage.writes.iter() {
        let source_name = backend_state_name(backend_plan, write.source_key);
        output.push_str(&format!(
            "- #{} {} statement {} {:?}/{:?}: `{}` = `{}`\n",
            write.dispatch_index,
            source_name,
            write.statement_index,
            write.mutation_kind,
            write.lowering,
            backend_plan
                .runtime_storage
                .expressions
                .display_name(write.target),
            backend_plan
                .runtime_storage
                .expressions
                .display_name(write.value)
        ));
    }
    output.push('\n');

    output.push_str("## State Values\n");
    output.push_str(&format!(
        "values: {}\n",
        backend_plan.state_values.values.len()
    ));
    for (_, value) in backend_plan.state_values.values.iter() {
        let source_name = backend_state_name(backend_plan, value.source_key);
        output.push_str(&format!(
            "- {} statement {} {:?}/{:?}: `{}` required {}\n",
            source_name,
            value.statement_index,
            value.role,
            value.kind,
            backend_plan
                .state_values
                .expressions
                .display_name(value.expression),
            value.required
        ));
    }
    output.push('\n');
}

fn runtime_frame_slot_kind_name(kind: &RuntimeFrameSlotKind) -> String {
    match kind {
        RuntimeFrameSlotKind::Parameter => "parameter".into(),
        RuntimeFrameSlotKind::DynamicReceiver { .. } => "dynamic-receiver".into(),
        RuntimeFrameSlotKind::DynamicResultScratch { .. } => "dynamic-result-scratch".into(),
        RuntimeFrameSlotKind::LocalStorage => "local".into(),
        RuntimeFrameSlotKind::StateCallResult {
            role, call_ordinal, ..
        } => {
            format!("state-call-result({role:?}#{call_ordinal})")
        }
    }
}
