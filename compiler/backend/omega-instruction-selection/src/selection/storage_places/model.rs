use omega_target_operations::RuntimeStorageRegion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct RuntimeStoragePlace {
    pub(in crate::selection) region: RuntimeStorageRegion,
    pub(in crate::selection) byte_offset: usize,
    pub(in crate::selection) byte_count: usize,
}
