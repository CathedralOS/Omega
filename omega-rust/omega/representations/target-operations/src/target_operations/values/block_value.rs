//! A scalar arrival belongs to its destination block, not an ABI or operation.

use semantic_vocabulary::{BlockId, ScalarType, ValueId};

/// Exact reference to one typed block parameter. Successor bindings supply its
/// value simultaneously on the selected edge; physical residence is downstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetScalarBlockValue {
    pub block: BlockId,
    pub value: ValueId,
    pub scalar_type: ScalarType,
}
