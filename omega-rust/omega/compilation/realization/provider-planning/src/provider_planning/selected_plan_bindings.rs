//! Selected execution facts and grants, separate from immutable checked Psi.

use effects::provider_grants::ResolvedAuthoredSelectedProviderGrant;

/// Exact selected plans after provider grants, qualification associations,
/// operator uses and installation reach have been admitted. This result owns
/// realization data only; the checked program is borrowed during admission.
#[derive(Debug)]
pub struct SelectedProviderPlanBinding {
    pub(crate) selected: effects::SelectedProviderPlanFacts,
    pub(crate) grants: Vec<ResolvedAuthoredSelectedProviderGrant>,
}

impl SelectedProviderPlanBinding {
    pub fn into_parts(
        self,
    ) -> (
        effects::SelectedProviderPlanFacts,
        Vec<ResolvedAuthoredSelectedProviderGrant>,
    ) {
        (self.selected, self.grants)
    }
}
