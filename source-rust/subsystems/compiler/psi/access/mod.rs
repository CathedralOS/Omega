/*
Structural ownership, multiplicity (`StructuralMultiplicity`), access capabilities
(`StructuralAccess`, `ReferenceAccessCapability`), placed loan regions, affine cleanup actions
(`TerminalAffineCleanupAction`), and claim transfer contracts in `TerminalModule`.
*/

mod capabilities;
mod declarations;
pub use declarations::AccessDeclarations;
pub(crate) mod borrows;
pub(crate) mod claims;
pub(crate) mod cleanup;
pub(crate) mod placement;

pub use borrows::{
    RetainedBorrowContentProjection, RetainedBorrowCustody, RetainedBorrowPlace,
    RetainedBorrowPlaceRoot, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
    TerminalBorrowPlace, TerminalBorrowPlaceSegment, TerminalReborrowRestorationClass,
    TerminalReborrowRestoredCallUse, TerminalReborrowSharedCohortMember,
};
pub use capabilities::{StructuralAccess, StructuralMultiplicity, structural_access_covers};
pub use claims::{ClaimTransfer, CompletionReceipt, EntryClaim, LoanCompletion};
pub use cleanup::{
    NominalCleanupSelection, OwnedCleanupInvocation, StructuralAffineDiscard,
    TerminalAffineCleanupAction,
};
pub use placement::{
    AccessRamCorrespondence, AccessRamFieldCorrespondence, AccessViewInputError,
    AccessViewPlanError, AccessorScalarBoundaryDemand, ResidentReferenceSource,
    TerminalAccessFieldPlan, TerminalAccessLifecycleExecution, TerminalAccessLifecycleOperation,
    TerminalAccessLifecyclePlan, TerminalAccessOwner, TerminalAccessProfileOperand,
    TerminalAccessProfileRequirement, TerminalAccessStorage, TerminalAccessViewInput,
    TerminalAccessViewPlan, TerminalAccessor, TerminalAccessorKind, canonical_access_view_identity,
};
