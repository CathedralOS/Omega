use crate::BackendReportInput;

pub(super) fn write_abstract_operations_section(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("## Abstract Operations\n");
    output.push_str(&format!(
        "functions: {}\n",
        backend_plan.abstract_operations.code.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        backend_plan.abstract_operations.code.instructions.len()
    ));
    output.push_str(&format!(
        "operands: {}\n",
        backend_plan.abstract_operations.code.operands.len()
    ));
    output.push('\n');
}
