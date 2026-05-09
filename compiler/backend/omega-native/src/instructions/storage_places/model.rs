use omega_target_program::RuntimeStorageRegion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::instructions) struct RuntimeStoragePlace {
    pub(in crate::instructions) region: RuntimeStorageRegion,
    pub(in crate::instructions) byte_offset: usize,
    pub(in crate::instructions) byte_count: usize,
}
