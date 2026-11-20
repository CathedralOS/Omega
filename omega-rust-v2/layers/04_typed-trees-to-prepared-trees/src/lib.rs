//! Preparation freezes constants and evaluates the static values that close type identities.
//!
//! Start with preparation/entry_points.rs for source packages and preparation/mod.rs
//! for the typing and evaluation rounds. The evaluation modules compute owned values.
//! Invocation evaluates selected calls under their checked contracts. Target semantics
//! describes the target inputs those computations can observe. The frozen handoff to
//! lowering is defined in omega-prepared.
#![forbid(unsafe_code)]

mod evaluation;
mod invocation;
mod preparation;
#[cfg(test)]
#[path = "target_semantics/properties/tests.rs"]
mod target_semantics_tests;

pub use evaluation::owned_values::{CheckedCanonicalIdentity, CheckedDataEstablishment};
pub use invocation::admission::{CheckedInvocationFact, CheckedProvider, SelectedRealization};
pub use invocation::static_callbacks::{CheckedCallableRefinement, CheckedCallableUse};
pub use invocation::termination::CheckedRecursiveComponent;
pub use omega_prepared::{
    AddressSpaceIdentity, CertifiedResources, ConstantMaterialization, ConstantMaterializationError, Endianness, EntryProfileIdentity, EntryStackGuarantee,
    LayoutExtent, LayoutGeometry, MaterializationComponent, MaterializationError, PreparationDependencies, PreparationGrant, PreparationResourceReceipt,
    PreparedFloat, PreparedTrees, PreparedValue, PreparedValueOrigin, SelectedLayout, SelectedTargetProperty, TargetIdentity, TargetObservation,
    TargetProperty, TargetPropertyQuery, TargetSemantics, TargetSemanticsError, ValueInputs,
};
pub use omega_target_contract::{FloatFormat, FloatOperation, FloatRealization, FloatSemantics};
pub use preparation::entry_points::{
    prepare_source_package, prepare_source_package_for_target, prepare_source_package_with_boundary_slots, prepare_source_package_with_open_target,
};
pub use preparation::sponsorship::{CheckedPreparationBound, SponsoredPreparationError, prepare_source_package_under_grant};
pub use preparation::typing_pass::{CheckedPreparationEvidence, OpenTargetDemand, PreparationAnswers, PreparedPass, prepare_typing_pass};
pub use preparation::{EvaluatedPreparationRound, OpenTargetPreparation, PreparationAdvanceError, PreparationProgress, PreparationRound, PreparationStep};
