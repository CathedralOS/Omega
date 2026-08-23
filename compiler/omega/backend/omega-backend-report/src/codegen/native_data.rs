use super::super::backend_state_name;
use crate::BackendReportInput;
use omega_target_operations::TargetDataObject;

pub(super) fn write_native_data_section(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("## Native Data\n");
    output.push_str(&format!("objects: {}\n", backend_plan.data.objects.len()));
    output.push_str(&format!("bytes: {}\n", backend_plan.data.bytes.len()));
    if backend_plan.data.objects.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, data_object) in backend_plan.data.objects.iter() {
            write_target_data_object(output, backend_plan, data_object);
        }
    }
    output.push('\n');
}

fn write_target_data_object(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    data_object: &TargetDataObject,
) {
    let byte_count = backend_plan
        .data
        .bytes
        .span(data_object.bytes)
        .map_or(0, |bytes| bytes.len());
    let source_name = backend_state_name(backend_plan, data_object.source_key);

    output.push_str(&format!(
        "- {} @{} bytes {} align {} from {} statement {}\n",
        data_object.symbol,
        data_object.offset,
        byte_count,
        data_object.alignment,
        source_name,
        data_object.source_statement
    ));
}
