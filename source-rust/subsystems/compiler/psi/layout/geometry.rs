#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageGeometry {
    pub byte_size: u64,
    pub byte_alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumGeometry {
    pub tag_byte_offset: u64,
    pub tag: StorageGeometry,
    pub storage: StorageGeometry,
    pub common_field_offsets: Vec<u64>,
    pub payload_byte_offset: u64,
    pub case_field_offsets: Vec<Vec<u64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateGeometryError {
    EmptyCaseSet,
    ZeroAlignment,
    SizeOverflow,
}
