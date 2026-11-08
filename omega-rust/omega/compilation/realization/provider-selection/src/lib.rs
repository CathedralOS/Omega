/*
Whole-provider choice and admission over prepared declaration records.

Source adapters retain invocation-local correspondence, while portable adapters
retain exact closed declarations. Our policy compares those supplied identities
and complete applications without resolving names or opening a source program.
Opaque owner data travels with a candidate or choice and is not selection
authority, and no index escapes to be reinterpreted against another inventory.
*/

/*
Start at publication.rs: it selects canonical Build companions against the
emitted Terminal demand catalog by calling choice.rs. choice.rs chooses whole
providers and admits their correspondence, binding.rs completes a chosen plan's
closure, bodies.rs names the code each selection runs, and boundary.rs admits
boundary applications and their declaration policy.
*/

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

pub use choice::{
    CandidateDeclarations, ProviderCandidate, ProviderDeclaration, ProviderFamily,
    ProviderFamilyCoordinate, ProviderIdentity, ProviderSubject, SelectedProvider,
    SelectionProvenance, admit, choose, is_candidate_demanded,
};
pub use language_semantics::declaration_selection::CompositionMode;

pub use binding::{
    GenericProviderInstanceDeclarations, InstallationReachRow, ProviderInstanceDeclarations,
    ProviderInstanceRowDeclarations, ProviderPlanInstanceDeclarations, SelectedPlanDeclarations,
    SelectedProviderPlanBinding, bind_selected_provider_plan_facts, project_provider_instances,
    resolve_installation_reach, validate_qualification_grants,
};

pub use bodies::{
    CallbackProviderDeclarations, CheckedBodyCandidate, DirectTargetBodyAbsences,
    SelectedCallbackRow, reject_reached_inert_calls, select_callback_declarations,
    select_checked_body, selected_external_execution,
};

pub use boundary::{
    BoundaryProviderAdmission, CompilerIntrinsicPlanProvenance, select_boundary_applications,
};
