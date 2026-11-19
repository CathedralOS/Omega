//! Rejoin the provider declarations selected by Build to their exact source and closed operands.

#[path = "provider_declarations/requests.rs"]
mod requests;
#[path = "provider_declarations/retained.rs"]
mod retained;
#[cfg(test)]
#[path = "provider_declarations/tests.rs"]
mod tests;

pub use super::retained_product_application::RetainedProductApplication;
use crate::declaration_selection::BuildSelectionOrigin;
use crate::provider_choice_inputs::application_id;
pub use crate::provider_choice_inputs::{
    ProviderApplicationId, ProviderOperatorFamilyCoordinate, ProviderOperatorFamilySelection,
    ProviderSelection, ProviderSelectionIdentity, ProviderSelectionSubject, TargetProviderDefaults,
    source_application,
};
use crate::source_provider_choices::authored_defaults::{form, identity};
pub use crate::source_provider_choices::retain_provider_defaults;
pub use crate::source_state::provider_declarations::ProviderPlanningContext;
pub use requests::collect_product_provider_selections;
pub use semantic_vocabulary::source_selections::CompositionMode;
use semantic_vocabulary::source_selections::ProductTarget;
use source_admission::RetainedDeclaration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedProviderSelection {
    pub subject: RetainedDeclaration,
    pub subject_identity: String,
    pub subject_application: Option<RetainedProductApplication>,
    pub provider: RetainedDeclaration,
    pub provider_identity: String,
    pub provider_application: Option<RetainedProductApplication>,
    pub target: ProductTarget,
    pub composition_mode: CompositionMode,
    pub origin: BuildSelectionOrigin,
}
