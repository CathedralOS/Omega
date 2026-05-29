mod abstract_ops;
mod assigned;
mod machine;
mod native_data;
mod operands;
mod runtime_values;
mod target_ops;

use crate::BackendReportInput;

pub(super) fn write_codegen_sections(output: &mut String, backend_plan: &BackendReportInput<'_>) {
    native_data::write_native_data_section(output, backend_plan);
    abstract_ops::write_abstract_operations_section(output, backend_plan);
    target_ops::write_target_operations_section(output, backend_plan);
    assigned::write_assigned_target_operations_section(output, backend_plan);
    machine::write_machine_instructions_section(output, backend_plan);
}
