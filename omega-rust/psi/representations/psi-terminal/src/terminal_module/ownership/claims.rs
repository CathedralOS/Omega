use crate::StructuralPathSegment;
use psi_core::{ClaimId, PlaceId};

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
/// parameter; callers cannot author callee-local claim identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaimTransfer {
    pub claim: ClaimId,
    pub argument_index: u32,
}

/// Transfer one claim returned by an in-module structural callee back into the
/// caller's claim namespace. Claim identities are machine-local, so neither
/// side may infer that equal numeric ids denote the same occurrence. The
/// returned claim's structural path is reconstructed from the callee's
/// verified result frontier and preserved beneath the operation result place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralResultClaimTransfer {
    pub callee_claim: ClaimId,
    pub caller_claim: ClaimId,
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
