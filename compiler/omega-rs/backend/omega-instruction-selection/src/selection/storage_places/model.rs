use omega_abstract_operations::RuntimeStorageRegion;

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
pub(in crate::selection) struct RuntimeFrameBaseIndexedTarget {
    pub(in crate::selection) base_byte_offset: usize,
    pub(in crate::selection) index_offset: usize,
    pub(in crate::selection) element_byte_size: usize,
    pub(in crate::selection) field_byte_offset: usize,
    pub(in crate::selection) byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct RuntimeFrameFixedIndexedTarget {
    pub(in crate::selection) descriptor_offset: usize,
    pub(in crate::selection) element_index: usize,
    pub(in crate::selection) element_byte_size: usize,
    pub(in crate::selection) field_byte_offset: usize,
    pub(in crate::selection) byte_count: usize,
}

impl RuntimeFrameFixedIndexedTarget {
    pub(in crate::selection) fn pointee_field_byte_offset(&self) -> Option<usize> {
        self.element_index
            .checked_mul(self.element_byte_size)?
            .checked_add(self.field_byte_offset)
    }
}
