//! The scalar count, buffer and safe byte limit for a descriptor read.

use semantic_vocabulary::IntegerType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DescriptorInputGuard {
    pub count_scalar_argument: usize,
    pub structural_argument: usize,
    pub max_bytes: u64,
    pub count_type: IntegerType,
}
