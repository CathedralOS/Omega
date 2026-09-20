#![forbid(unsafe_code)]
#![allow(
    clippy::result_large_err,
    reason = "extent failures deliberately return the conserved authority needed for retry"
)]

//! Conservation model for authority over concrete address-space ranges.
//!
//! An `Extent` is not an allocator and an address is not authority. Root
//! extents enter through admitted providers; ordinary operations may only
//! split, attenuate, borrow, or rejoin authority already present.
//!
//! Start at `extent.rs`: the authority carrier, its conserved operations and
//! the diagnostic every operation reports. `identities` holds the normalized
//! identities and rights, `roots` the two ways a fresh root enters,
//! `loans` the borrow-carrying subranges, `external_loans` lending to
//! borrowers the checker cannot see, `mapping` translation activation and
//! release, `activation_claims` runtime-sized bounds an activation claims
//! inside its own provisioned storage.

mod activation_claims;
mod extent;
mod external_loans;
mod identities;
mod loans;
mod mapping;
mod roots;
#[cfg(test)]
mod tests;

pub use activation_claims::{
    ActivationClaim, ActivationClaimBranch, ActivationClaimId, ActivationClaimLedger,
    ActivationClaimProvenance, ActivationClaimRequest, ActivationClaimSiteId, ClaimBoundRow,
    ClaimDrainError, ClaimEstablishmentError, compose_claim_bounds,
    validate_committed_within_bound,
};
pub use extent::diagnostic::ExtentDiagnostic;
pub use extent::{
    AttenuationError, Extent, ExtentSharingMode, MergeError, OwnedExtentPartition,
    OwnedPartitionError, SplitError,
};
pub use external_loans::{
    CompletionObligations, ExternalBorrowerId, ExternalCompletionError, ExternalCompletionFactId,
    ExternalCompletionReceipt, ExternalLoan, ExternalLoanCompletion, ExternalLoanDirection,
    ExternalLoanGrant, ExternalLoanId, ExternalLoanStartError, ExternalReachMechanism,
    ExternalReachReceipt, ExternalReachReceiptId, begin_external_loan,
};
pub use identities::{
    AddressSpaceId, ExtentAliasClassId, ExtentBackingId, ExtentCapacityId,
    ExtentContentCustodyReceiptId, ExtentContentInterpretation, ExtentContentInterpretationId,
    ExtentContentValidityReceiptId, ExtentCustodyRootId, ExtentEstablishmentRouteId,
    ExtentIssuanceId, ExtentLineageId, ExtentLiveIssuancePremiseId, ExtentProvenanceId,
    ExtentProviderCorrespondenceId, ExtentProviderId, ExtentProviderInvocationId,
    ExtentProviderPlanId, ExtentQualificationId, ExtentRightId, ExtentRights,
    ExtentTrustProvenanceId, MappingEraId, ResidentClaimId,
};
pub use loans::{ExtentLoan, LoanPolarity};
pub use mapping::{
    BorrowedMappingError, MapActivationError, MappedExtent, MappedRangeReceiptContext,
    MappingGrant, MappingGrantId, MappingId, MappingReceiptContext, MappingSourceMode,
    OwnedMappingError, PeerWriteRevocationError, PeerWriteRevocationFactId,
    PeerWriteRevocationObligations, PeerWriteRevocationReceipt, PeerWriteRevocationStartError,
    PendingMap, PendingPeerWriteRevocation, PendingUnmap, TranslationActivationFactId,
    TranslationActivationReceipt, TranslationCompletionFactId, TranslationInstallObligations,
    TranslationReleaseObligations, TranslationReleaseReceipt, UnmapCompletionError,
    UnmappedExtents, map_borrowed, map_owned,
};
pub use roots::root_grants::{
    ExistingContentMintError, ExtentRootGrant, MintError, ProviderExistingContentGrant,
    ValidatedExtentGeometry,
};
pub use roots::root_origins::{
    ExtentProgramLocalOrigin, ExtentProviderInvocation, ExtentProviderIssuance, ExtentRootOrigin,
};
