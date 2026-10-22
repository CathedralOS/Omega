//! Compiler-owned operations of the sealed activation-storage surface.
//!
//! These names describe operations, not allocation-provider requirements.
//! Declaration admission must authenticate the complete core signature and
//! opaque linear carrier before selecting any of them.

pub const ACTIVATION_CORE_SOURCE: &str = "activation.omg";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationStorageOperation {
    Establish,
    BorrowBytes,
    Release,
    /// Divide one established claim into two children that cover it.
    Split,
    /// Merge the two children of one split back into that claim.
    Merge,
}
