use crate::identity::NativeStringStorage;
use omega_backend_plan::NativePlan;

pub(in crate::identity) fn count_layout_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, data_layout) in native_plan.layouts.data_layouts.iter() {
        storage.count_program_name_identity(&data_layout.name);
        if let omega_layout::DataShape::Enum { variants } = &data_layout.shape {
            for variant in variants {
                storage.count_program_name_identity(&variant.name);
            }
        }
    }
    for (_, field) in native_plan.layouts.fields.iter() {
        storage.count_program_name_identity(&field.name);
        storage.count_identity(&field.type_name);
    }
    for (_, machine_layout) in native_plan.layouts.machine_layouts.iter() {
        storage.count_program_name_identity(&machine_layout.name);
    }
}
