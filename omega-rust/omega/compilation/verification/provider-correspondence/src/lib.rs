//! Match selected plans, applications and candidate bodies to their exact declarations.
//! Admission and native planning both use these checks over accepted Terminal records.

mod binding;
mod bodies;
mod boundary;
mod declarations;

pub use declaration_contracts::selection::CompositionMode;
pub use declarations::{
    ProviderDeclaration, ProviderFamily, ProviderFamilyCoordinate, ProviderIdentity,
    ProviderSubject, SelectionProvenance,
};

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

pub mod trust;
