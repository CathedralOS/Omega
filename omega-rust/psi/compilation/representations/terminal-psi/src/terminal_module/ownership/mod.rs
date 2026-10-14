//! Access, borrow, suspension, claim-transfer, and cleanup vocabulary.

mod access;
mod borrows;
mod claims;
mod cleanup;
mod placement;
mod suspension;

pub use access::{StructuralAccess, StructuralMultiplicity};
pub use borrows::{
    RetainedBorrowContentProjection, RetainedBorrowCustody, RetainedBorrowPlace,
    RetainedBorrowPlaceRoot, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
    TerminalBorrowPlace, TerminalBorrowPlaceSegment, TerminalReborrowRestorationClass,
    TerminalReborrowRestoredCallUse, TerminalReborrowSharedCohortMember,
};
pub use claims::{ClaimTransfer, CompletionReceipt, EntryClaim, LoanCompletion};
pub use cleanup::{NominalAffineCleanup, StructuralAffineDiscard, TerminalAffineCleanupAction};
pub use placement::{
    PlacedRamCorrespondence, PlacedRamFieldCorrespondence, PlacedScalarBoundaryDemand,
    TerminalPlacedAccessor, TerminalPlacedAccessorKind, TerminalPlacedFieldPlan,
    TerminalPlacedViewInput, TerminalPlacedViewPlan, canonical_placed_view_identity,
};
pub use suspension::{
    TerminalSuspensionCallPlan, TerminalSuspensionCallSite, TerminalSuspensionCallTarget,
    TerminalSuspensionLiveValue, TerminalSuspensionLoan, TerminalSuspensionLoanBacking,
    TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource, TerminalSuspensionLoans,
    TerminalSuspensionPlace, TerminalSuspensionStorage, TerminalSuspensionValueLoans,
    TerminalSuspensionValueType, suspension_frontier_commitment,
};
