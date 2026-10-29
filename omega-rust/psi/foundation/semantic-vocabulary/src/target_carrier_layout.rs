//! Target-supplied carrier geometry for portable layout observations.

/// Source-visible pointer and view carriers, not an execution engine's private
/// reference descriptors. The containing application binds these values to its
/// target; geometry alone grants no storage, access or lifetime authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TargetCarrierLayout {
    pub pointer_byte_size: u64,
    pub pointer_byte_alignment: u64,
    pub descriptor_byte_size: u64,
    pub descriptor_byte_alignment: u64,
}
