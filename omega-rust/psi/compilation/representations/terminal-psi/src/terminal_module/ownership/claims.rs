use crate::StructuralPathSegment;
use semantic_vocabulary::{ClaimId, PlaceId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryClaim {
    pub claim: ClaimId,
    /// Structural parameter root that owns this claim.
    pub input: PlaceId,
    /// Statically typed structural projection below `input`. Empty names the
    /// complete parameter.
    pub path: Vec<StructuralPathSegment>,
}

/// Transfer one caller-local live claim through the structural argument at
/// `argument_index`. The callee reconstructs its own entry claim from that
/// parameter; callers cannot author callee-local claim identities. The path
/// names the caller's obligation below the actual argument and retains Case
/// guards, so an inactive payload is distinguishable from a lost live claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaimTransfer {
    pub claim: ClaimId,
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
}

/// Correlate successful completion of one exact bodyless boundary invocation
/// with one caller-local live claim and structural argument position.
///
/// The receipt becomes effective only after the boundary effect succeeds. A
/// rejected effect consumes no claim, so it cannot acknowledge completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompletionReceipt {
    pub claim: ClaimId,
    pub argument_index: u32,
}
