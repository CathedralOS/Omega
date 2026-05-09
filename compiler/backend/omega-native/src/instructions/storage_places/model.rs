#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::instructions) struct RuntimeStoragePlace {
    pub(in crate::instructions) region: RuntimeStorageRegion,
    pub(in crate::instructions) byte_offset: usize,
    pub(in crate::instructions) byte_count: usize,
}

impl RuntimeStoragePlace {
    pub(in crate::instructions) fn symbol_name(&self, entry_machine_name: &str) -> String {
        self.region.symbol_name(entry_machine_name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::instructions) enum RuntimeStorageRegion {
    Machine,
    RuntimeFrame,
}

impl RuntimeStorageRegion {
    pub(in crate::instructions) fn symbol_name(self, entry_machine_name: &str) -> String {
        match self {
            RuntimeStorageRegion::Machine => {
                omega_object::machine_storage_symbol_name(entry_machine_name)
            }
            RuntimeStorageRegion::RuntimeFrame => omega_object::runtime_frame_storage_symbol_name(),
        }
    }
}
