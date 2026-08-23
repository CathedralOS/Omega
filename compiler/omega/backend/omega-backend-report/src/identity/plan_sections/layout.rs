use crate::BackendReportInput;
use crate::identity::BackendStringStorage;

pub(in crate::identity) fn count_layout_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, data_layout) in backend_plan.layouts.data_layouts.iter() {
        storage.count_program_name_identity(&data_layout.name);
        if let omega_layout::DataShape::Enum { variants, .. } = &data_layout.shape {
            for variant in backend_plan.layouts.variants.span_or_empty(*variants) {
                storage.count_program_name_identity(&variant.name);
            }
        }
    }
    for (_, field) in backend_plan.layouts.fields.iter() {
        storage.count_program_name_identity(&field.name);
        storage.count_identity(&field.type_name);
    }
    for (_, machine_layout) in backend_plan.layouts.machine_layouts.iter() {
        storage.count_program_name_identity(&machine_layout.name);
        if let Some(attached_data) = &machine_layout.attached_data {
            storage.count_program_name_identity(attached_data);
        }
    }
}
