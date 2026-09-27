//! Structural-place roots declared by a machine and their construction coordinates.
//!
//! Parameter, block, result, literal, attachment and affine-local roots share
//! this vocabulary. Content projections consume the root kinds they support;
//! roots also serve ordinary control, storage and cleanup without content.

use crate::{BoundaryMachineId, StructuralFieldId, StructuralTypeId};

/// Role of a module-declared structural-place root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralPlaceKind {
    Parameter {
        position: u32,
        is_self: bool,
    },
    /// One block-local structural binding. The position is dense and zero-based
    /// in the owning block's structural parameter vector, never an authored
    /// source argument position or a machine `self` parameter.
    BlockParameter {
        block: crate::BlockId,
        position: u32,
    },
    Result,
    /// One structural operation result, established only after the exact
    /// producer succeeds. The producer identity prevents a result place from
    /// being treated as live before or independently of that operation.
    OperationResult {
        producer: crate::OperationId,
        structural_type: StructuralTypeId,
    },
    /// One immutable borrowed byte-sequence literal established by an exact
    /// terminal operation. The declaration ordinal makes source-order
    /// identity explicit without retaining a source-tree handle.
    ByteSequenceLiteral {
        declaration_ordinal: u32,
        structural_type: StructuralTypeId,
    },
    /// One authored provider-backed attachment field specialized out of the
    /// runtime record into an exact bodyless boundary requirement. Installation
    /// must bind the named boundary; this root is semantic evidence and has no
    /// target layout of its own.
    ProviderAttachment {
        attachment: StructuralTypeId,
        field: StructuralFieldId,
        boundary: BoundaryMachineId,
    },
    /// One whole, claim-free affine local established by an explicit terminal
    /// operation. The exact concrete type and source declaration coordinate
    /// make trivial disposal independently checkable without retaining a
    /// source-tree handle.
    TrivialAffineLocal {
        declaration_ordinal: u32,
        structural_type: StructuralTypeId,
        /// Present only for one statically ordered element of an abandoned
        /// fixed-array construction. The root type and literal index make the
        /// prefix schedule independently replayable without turning the
        /// uninitialized aggregate into an ABI input.
        construction: Option<AffineConstructionElement>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AffineConstructionElement {
    pub root_structural_type: StructuralTypeId,
    pub index: u64,
}
