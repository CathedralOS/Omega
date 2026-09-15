//! Decoded attempt shapes and their operands.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShapeResult {
    Scalar(i64),
    Handle(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShapeScalar {
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeLogicalInput {
    pub(crate) ordinal: u8,
    pub(crate) kind: u8,
    pub(crate) resolution: ShapeLogicalInputResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShapeLogicalInputResolution {
    Resolved(u64),
    Null,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeLogicalOutput {
    pub(crate) kind: u8,
    pub(crate) identity: u64,
    pub(crate) source: u8,
    pub(crate) source_identity: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeRootedPath<'a> {
    pub(crate) ordinal: u8,
    pub(crate) root: u8,
    pub(crate) bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeReturnedPath<'a> {
    pub(crate) ordinal: u8,
    pub(crate) kind: u8,
    pub(crate) completeness: u8,
    pub(crate) bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeObservedRegion {
    pub(crate) ordinal: u8,
    pub(crate) kind: u8,
    pub(crate) offset: u64,
    pub(crate) length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeMetadata {
    pub(crate) ordinal: u8,
    pub(crate) kind: u8,
    pub(crate) device: u64,
    pub(crate) mode: u32,
    pub(crate) link_count: u64,
    pub(crate) inode: u64,
    pub(crate) user: u32,
    pub(crate) group: u32,
    pub(crate) referenced_device: u64,
    pub(crate) access_time: i64,
    pub(crate) modification_time: i64,
    pub(crate) change_time: i64,
    pub(crate) birth_time: i64,
    pub(crate) size: i64,
    pub(crate) blocks_512: u64,
    pub(crate) preferred_block_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeMutableBytes<'a> {
    pub(crate) ordinal: u8,
    pub(crate) pre: &'a [u8],
    pub(crate) post: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeMutableI64 {
    pub(crate) ordinal: u8,
    pub(crate) pre: i64,
    pub(crate) post: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeAuthorizedPath<'a> {
    pub(crate) ordinal: u8,
    pub(crate) access: u8,
    pub(crate) root: u8,
    pub(crate) bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeRefusal {
    pub(crate) ordinal: u8,
    pub(crate) access: u8,
    pub(crate) reason: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttemptShape<'a> {
    pub(crate) operation: u16,
    pub(crate) provider: u8,
    pub(crate) result: ShapeResult,
    pub(crate) post_error: i32,
    pub(crate) scalars: Vec<(u8, ShapeScalar)>,
    pub(crate) byte_operands: Vec<(u8, &'a [u8])>,
    pub(crate) path_like_operands: Vec<(u8, &'a [u8])>,
    pub(crate) rooted_paths: Vec<ShapeRootedPath<'a>>,
    pub(crate) returned_paths: Vec<ShapeReturnedPath<'a>>,
    pub(crate) returned_path_count: usize,
    pub(crate) observed_regions: Vec<ShapeObservedRegion>,
    pub(crate) metadata: Vec<ShapeMetadata>,
    pub(crate) mutable_byte_resolutions: Vec<(u8, &'a [u8])>,
    pub(crate) mutable_i64_resolutions: Vec<(u8, i64)>,
    pub(crate) mutable_bytes: Vec<ShapeMutableBytes<'a>>,
    pub(crate) mutable_i64s: Vec<ShapeMutableI64>,
    pub(crate) authorized_paths: Vec<ShapeAuthorizedPath<'a>>,
    pub(crate) inputs: Vec<ShapeLogicalInput>,
    pub(crate) output: Option<ShapeLogicalOutput>,
    pub(crate) retired: Vec<u64>,
    pub(crate) refusal_count: usize,
    pub(crate) refusals: Vec<ShapeRefusal>,
}
