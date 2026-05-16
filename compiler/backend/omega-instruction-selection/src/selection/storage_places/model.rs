use omega_checked_trees::expression::Expression;
use omega_checked_trees::name::ProgramName;
use omega_target_operations::RuntimeStorageRegion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct RuntimeStoragePlace {
    pub(in crate::selection) region: RuntimeStorageRegion,
    pub(in crate::selection) byte_offset: usize,
    pub(in crate::selection) byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct RuntimeFrameIndexedTarget {
    pub(in crate::selection) descriptor_offset: usize,
    pub(in crate::selection) index_offset: usize,
    pub(in crate::selection) element_byte_size: usize,
    pub(in crate::selection) field_byte_offset: usize,
    pub(in crate::selection) byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct IndexedTargetPath {
    pub(in crate::selection) collection: Expression,
    pub(in crate::selection) index: Expression,
    pub(in crate::selection) suffix: Vec<ProgramName>,
}
