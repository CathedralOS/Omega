use omega_target_program::RuntimeStorageRegion;

pub(crate) fn storage_region_symbol_name(
    region: RuntimeStorageRegion,
    entry_machine_name: &str,
) -> String {
    match region {
        RuntimeStorageRegion::Machine => {
            omega_object::machine_storage_symbol_name(entry_machine_name)
        }
        RuntimeStorageRegion::RuntimeFrame => omega_object::runtime_frame_storage_symbol_name(),
    }
}
