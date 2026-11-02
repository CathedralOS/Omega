//! Whole-provider choice and admission over prepared declaration records.
//!
//! Source adapters retain invocation-local correspondence; portable adapters
//! retain exact closed declarations. The policy compares those supplied identities
//! and complete applications, never resolves names or opens a source program.
//! Opaque owner data travels with a candidate or choice and is not selection
//! authority. No index escapes to be reinterpreted against another inventory.
//!
//! Start at `publication.rs`: it selects canonical Build companions against the
//! emitted Terminal demand catalog by calling `choice`. `choice/` chooses whole
//! providers and admits their correspondence, `binding/` completes a chosen
//! plan's closure, `bodies/` names the code each selection runs, and
//! `boundary/` admits boundary applications and their declaration policy.

mod binding;
mod bodies;
mod boundary;
mod choice;
mod publication;

pub use publication::{
    CanonicalProviderDeclaration, PreparedProviderCandidate, PreparedProviderRealizations,
    PreparedTargetChoices, PreparedTargetProviderPublication, ProviderApplicationDemand,
    SelectedTargetProviderPublication, SelectedTargetProviders, TargetProviderChoice,
};

pub use choice::declarations::{
    CandidateDeclarations, ProviderCandidate, ProviderDeclaration, ProviderFamily,
    ProviderFamilyCoordinate, ProviderIdentity, ProviderSubject, SelectedProvider,
    SelectionProvenance,
};
pub use choice::provider_demands::is_candidate_demanded;
pub use choice::provider_selection::{admit, choose};
pub use language_semantics::declaration_selection::CompositionMode;

pub use binding::installation_reach::{InstallationReachRow, resolve_installation_reach};
pub use binding::provider_instances::{
    GenericProviderInstanceDeclarations, ProviderInstanceDeclarations,
    ProviderInstanceRowDeclarations, ProviderPlanInstanceDeclarations, project_provider_instances,
};
pub use binding::qualification_grants::validate_qualification_grants;
pub use binding::selected_binding::{
    SelectedPlanDeclarations, SelectedProviderPlanBinding, bind_selected_provider_plan_facts,
};

pub use bodies::callback_roots::{
    CallbackProviderDeclarations, SelectedCallbackRow, select_callback_declarations,
};
pub use bodies::checked_bodies::{CheckedBodyCandidate, select_checked_body};
pub use bodies::external_execution::selected_external_execution;
pub use bodies::target_bodies::{DirectTargetBodyAbsences, reject_reached_inert_calls};

pub use boundary::boundary_applications::{
    CompilerIntrinsicPlanProvenance, select_boundary_applications,
};
pub use boundary::boundary_review::BoundaryProviderAdmission;
