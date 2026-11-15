//! Admission and exact correspondence for providers selected during target
//! settlement. The source coordinator chooses providers through
//! provider-planning; this crate works from prepared plans and Terminal
//! declarations to bind instances, bodies and boundary applications.

mod binding;
mod bodies;
mod boundary;
mod declarations;

pub use declarations::{
    ProviderDeclaration, ProviderFamily, ProviderFamilyCoordinate, ProviderIdentity,
    ProviderSubject, SelectionProvenance,
};
pub use language_semantics::declaration_selection::CompositionMode;

pub use binding::{
    GenericProviderInstanceDeclarations, InstallationReachRow, ProviderInstanceDeclarations,
    ProviderInstanceRowDeclarations, ProviderPlanInstanceDeclarations, SelectedPlanDeclarations,
    SelectedProviderPlanBinding, bind_selected_provider_plan_facts, project_provider_instances,
    resolve_installation_reach, validate_qualification_grants,
};

pub use bodies::{
    CallbackProviderDeclarations, CheckedBodyCandidate, SelectedCallbackRow,
    select_callback_declarations, select_checked_body, selected_external_execution,
};

pub use boundary::{
    BoundaryProviderAdmission, CompilerIntrinsicPlanProvenance, select_boundary_applications,
};
