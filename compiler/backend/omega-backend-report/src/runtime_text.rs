use super::backend_state_name;

use crate::BackendReportInput;

pub(super) fn write_runtime_text_sections(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("## Runtime Text\n");
    output.push_str(&format!("uses: {}\n", backend_plan.runtime_text.uses.len()));
    output.push_str(&format!(
        "buffers: {}\n",
        backend_plan.runtime_text.buffers.len()
    ));
    output.push_str(&format!(
        "slots: {}\n",
        backend_plan.runtime_text.slots.len()
    ));
    output.push_str(&format!(
        "writes: {}\n",
        backend_plan.runtime_text.writes.len()
    ));
    output.push_str(&format!(
        "builders: {}\n",
        backend_plan.runtime_text.builders.len()
    ));
    output.push_str(&format!(
        "builder segments: {}\n",
        backend_plan.runtime_text.builder_segments.len()
    ));
    if backend_plan.runtime_text.uses.is_empty() {
        output.push_str("uses: none\n");
    } else {
        for (_, text_use) in backend_plan.runtime_text.uses.iter() {
            let source_name = backend_state_name(backend_plan, text_use.source_key);
            output.push_str(&format!(
                "- {} statement {} `{}` {:?} newline {}\n",
                source_name,
                text_use.statement_index,
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_use.expression),
                text_use.source,
                text_use.append_newline
            ));
        }
    }
    if backend_plan.runtime_text.buffers.is_empty() {
        output.push_str("buffers: none\n");
    } else {
        for (_, text_buffer) in backend_plan.runtime_text.buffers.iter() {
            let source_name = backend_state_name(backend_plan, text_buffer.source_key);
            output.push_str(&format!(
                "- buffer {} statement {} target `{}` text `{}` bytes {}\n",
                source_name,
                text_buffer.statement_index,
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_buffer.target),
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_buffer.text_place),
                text_buffer.byte_capacity
            ));
        }
    }
    if backend_plan.runtime_text.slots.is_empty() {
        output.push_str("slots: none\n");
    } else {
        for (_, text_slot) in backend_plan.runtime_text.slots.iter() {
            output.push_str(&format!(
                "- slot `{}` bytes {} input_buffer {}\n",
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_slot.place),
                text_slot.byte_capacity,
                text_slot.has_input_buffer
            ));
        }
    }
    if backend_plan.runtime_text.writes.is_empty() {
        output.push_str("writes: none\n");
    } else {
        for (_, text_write) in backend_plan.runtime_text.writes.iter() {
            let source_name = backend_state_name(backend_plan, text_write.source_key);
            output.push_str(&format!(
                "- write {} statement {} `{}` = `{}` {:?}\n",
                source_name,
                text_write.statement_index,
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_write.target),
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_write.value),
                text_write.kind
            ));
        }
    }
    if backend_plan.runtime_text.builders.is_empty() {
        output.push_str("builders: none\n");
    } else {
        for (_, text_builder) in backend_plan.runtime_text.builders.iter() {
            let source_name = backend_state_name(backend_plan, text_builder.source_key);
            output.push_str(&format!(
                "- builder {} statement {} `{}` segments {}\n",
                source_name,
                text_builder.statement_index,
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_builder.target),
                text_builder.segments.count()
            ));
            if let Some(segments) = backend_plan
                .runtime_text
                .builder_segments
                .span(text_builder.segments)
            {
                for segment in segments {
                    output.push_str(&format!(
                        "  - segment `{}` {:?}\n",
                        backend_plan
                            .runtime_text
                            .expressions
                            .display_name(segment.expression),
                        segment.kind
                    ));
                }
            }
        }
    }
    output.push('\n');
}
