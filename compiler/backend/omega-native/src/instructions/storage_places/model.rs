#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::instructions) struct RuntimeStoragePlace {
    pub(in crate::instructions) symbol: String,
    pub(in crate::instructions) byte_offset: usize,
    pub(in crate::instructions) byte_count: usize,
}
