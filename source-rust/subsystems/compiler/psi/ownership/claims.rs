use crate::StructuralPathSegment;
use semantic_vocabulary::{ClaimId, PlaceId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoanCompletion {
    pub operation: semantic_vocabulary::OperationId,
    pub place: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub multiplicity: crate::StructuralMultiplicity,
    pub claims: Vec<crate::StructuralResultClaimBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryClaim {
    pub claim: ClaimId,
    pub input: PlaceId,
    pub path: Vec<StructuralPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaimTransfer {
    pub claim: ClaimId,
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompletionReceipt {
    pub claim: ClaimId,
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
}
