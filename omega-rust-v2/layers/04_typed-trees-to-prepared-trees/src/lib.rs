//! L4 computes the static values that close types and freezes package constants for lowering.
//!
//! Start at [`prepare_source_package_diagnosed`] in `preparation/entry_points.rs`. The CLI
//! passes a resolved `SourcePackage` and receives `Result<PreparedTrees, PreparationFailure>`.
//! Inside that call, `PreparationRound::begin` in `preparation/rounds.rs` asks L3 to type the package.
//! When L3 requests a value, [`PreparationRound`] evaluates the typed request and returns
//! the answer to L3. Once typing completes, L4 publishes the typed trees and frozen values
//! together in [`PreparedTrees`]. L5 reads that product in `lower_package`.
//!
//! Follow arithmetic into `evaluation/` and selected calls into `invocation/`.
//! The signed-arithmetic pair in `tests/fixtures/signed_arithmetic/` exercises both.
#![forbid(unsafe_code)]

#[cfg(test)]
extern crate self as omega_preparation;

#[cfg(test)]
#[path = "../tests/support/source_package.rs"]
mod test_source_package;

mod evaluation;
mod invocation;
mod preparation;
mod reasons;
#[cfg(test)]
#[path = "target_semantics/properties/tests.rs"]
mod target_semantics_tests;

// Package entry points and the diagnostics returned to their callers.
pub use preparation::entry_points::{
    prepare_source_package, prepare_source_package_diagnosed, prepare_source_package_diagnosed_with_inputs, prepare_source_package_for_target,
    prepare_source_package_with_boundary_slots, prepare_source_package_with_open_target,
};
pub use preparation::rounds::PreparationFailure;
pub use reasons::{PreparationReason, PreparationRefusal, RefusalCause};

// Frozen values and the target inputs recorded during evaluation.
pub use omega_prepared::{
    AddressSpaceIdentity, CertifiedResources, ConstantMaterialization, ConstantMaterializationError, Endianness, EntryProfileIdentity, EntryStackGuarantee,
    LayoutExtent, LayoutGeometry, MaterializationComponent, MaterializationError, PreparationDependencies, PreparationGrant, PreparationResourceReceipt,
    PolicyConstructionOccurrence, PolicySchemaInput, PreparedFloat, PreparedTrees, PreparedValue, PreparedValueOrigin, SelectedLayout, SelectedTargetProperty, TargetIdentity, TargetObservation,
    TargetProperty, TargetPropertyQuery, TargetSemantics, TargetSemanticsError, ValueInputs,
};
pub use omega_target_contract::{FloatFormat, FloatOperation, FloatRealization, FloatSemantics};

// Callers that step through typing rounds or supply checked evidence.
pub use preparation::typing_pass::{CheckedPreparationEvidence, OpenTargetDemand, PreparationAnswers, PreparedPass, prepare_typing_pass, prepare_typing_pass_with_cause};
pub use preparation::rounds::{
    EvaluatedPreparationRound, OpenTargetPreparation, PreparationAdvanceError, PreparationProgress,
    PreparationRound, PreparationStep,
};
pub use evaluation::owned_values::{CheckedCanonicalIdentity, CheckedDataEstablishment};
pub use invocation::admission::{CheckedInvocationFact, CheckedProvider, SelectedRealization};
pub use invocation::static_callbacks::{CheckedCallableRefinement, CheckedCallableUse};
pub use invocation::termination::CheckedRecursiveComponent;

// Preparation under a caller's resource grant.
pub use preparation::sponsorship::{CheckedPreparationBound, SponsoredPreparationError, prepare_source_package_under_grant};
