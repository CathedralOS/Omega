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
//! `extent.rs` is the root: the authority carrier and its conserved
//! operations. `identities.rs` holds the normalized identities and rights,
//! `root_origins.rs` and `root_grants.rs` the two ways a fresh root enters,
//! `loans.rs` the borrow-carrying subranges, `external_loans.rs` lending to
//! borrowers the checker cannot see, `mapping.rs` translation activation and
//! release, and `diagnostic.rs` the shared failure type.

mod diagnostic;
mod extent;
mod external_loans;
mod identities;
mod loans;
mod mapping;
mod root_grants;
mod root_origins;
#[cfg(test)]
mod tests;

pub use diagnostic::ExtentDiagnostic;
pub use extent::{
    AttenuationError, Extent, MergeError, OwnedExtentPartition, OwnedPartitionError, SplitError,
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
    OwnedMappingError, PendingMap, PendingUnmap, TranslationActivationFactId,
    TranslationActivationReceipt, TranslationCompletionFactId, TranslationInstallObligations,
    TranslationReleaseObligations, TranslationReleaseReceipt, UnmapCompletionError,
    UnmappedExtents, map_borrowed, map_owned,
};
pub use root_grants::{
    ExistingContentMintError, ExtentRootGrant, MintError, ProviderExistingContentGrant,
    ValidatedExtentGeometry,
};
pub use root_origins::{
    ExtentProgramLocalOrigin, ExtentProviderInvocation, ExtentProviderIssuance, ExtentRootOrigin,
};
