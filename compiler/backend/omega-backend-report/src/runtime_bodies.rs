use crate::{BackendReportInput, backend_state_name};

pub(super) fn write_runtime_bodies_section(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("\n## Runtime Bodies\n");
    output.push_str(&format!(
        "bodies: {}\n",
        backend_plan.runtime_bodies.bodies.len()
    ));
    output.push_str(&format!(
        "operations: {}\n",
        backend_plan.runtime_bodies.operations.len()
    ));
    if backend_plan.runtime_bodies.bodies.is_empty() {
        output.push_str("none\n");
        return;
    }

    for (_, body) in backend_plan.runtime_bodies.bodies.iter() {
        let source_name = backend_state_name(backend_plan, body.key);
        output.push_str(&format!("- #{} {}\n", body.dispatch_index, source_name));

        match backend_plan
            .runtime_bodies
            .operations
            .paged_span(body.operations)
        {
            Some(operations) if operations.is_empty() => {
                output.push_str("  operations: none\n");
            }
            Some(operations) => {
                output.push_str("  operations:\n");
                for operation in operations.iter() {
                    let source_name = backend_state_name(backend_plan, operation.source_key);
                    output.push_str(&format!(
                        "    - {} statement {} {:?}\n",
                        source_name, operation.statement_index, operation.kind
                    ));
                }
            }
            None => output.push_str("  operations: invalid span\n"),
        }
    }
}
