#![forbid(unsafe_code)]

/*
Leaf vocabulary the native program forms share.

Abstract operations, the optimization unit, target operations, the prepared
execution plan and selected instructions all carry the same small set of
value-level types: where a row came from and what it is charged, how a
structural store or copy names its destination, what backs an access
reference, and which storage a structural path selects. None of these is a
program: they hold identities, structural declarations and paths, never a
function body or a fact table.

They used to be defined inside the abstract-operations and optimization-unit
crates, so any form that carried one depended on those earlier program
containers. Defined here, below every form, a later stage can name them
without being able to read an earlier program.
*/

pub mod access_lifecycles;
pub mod access_reference_validation;
pub mod access_references;
pub mod address_joins;
pub mod owner_loans;
pub mod provenance;
pub mod storage_projection;
pub mod structural_stores;
pub mod structural_type_catalog;
pub mod values;

pub mod calls;
pub mod identity;
pub mod ownership;
pub mod ownership_frontiers;

pub use access_lifecycles::AccessLifecycleOperands;
pub use access_references::AccessReferenceBacking;
pub use calls::StructuralCallFormal;
pub use identity::{access_view_catalog_identity_bytes, scalar_refinement_identity_bytes};
pub use ownership::CompletionClaimSource;
pub use ownership_frontiers::{
    OwnershipFrontierFact, OwnershipFrontierSite, OwnershipFrontierSnapshot,
};
pub use provenance::{
    EffectLink, OwnershipEvent, PsiProvenance, ValueDefinitionSite, WorkSettlement,
    encode_value_definition_site_identity,
};
pub use structural_stores::{
    StructuralCopyPath, StructuralScalarStoreDestination, StructuralWindowRoot,
};
pub use structural_type_catalog::StructuralTypeCatalog;
pub use values::{AbstractFunctionResult, AbstractResult, AbstractStructuralBinding, ValueBinding};
