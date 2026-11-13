// Selected provider source custody and instance projection for explicit
// inspection. Equal-looking source from another owner is not interchangeable
// (`Arc::ptr_eq`). Source declarations do not replace route-owned provider
// application evidence; projection refuses while that evidence is unavailable.

use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees::{FrozenTypedTrees, provider_declarations::PreparedProviderInventory};

pub struct SelectedProviderSource {
    inventory: Arc<PreparedProviderInventory>,
    plans: Arc<effects::SelectedProviderPlanFacts>,
}

impl std::fmt::Debug for SelectedProviderSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SelectedProviderSource")
            .field("plans", &self.plans)
            .finish_non_exhaustive()
    }
}

impl PartialEq for SelectedProviderSource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(self.program(), other.program()) && self.plans == other.plans
    }
}
impl Eq for SelectedProviderSource {}

impl SelectedProviderSource {
    pub fn new(
        inventory: Arc<PreparedProviderInventory>,
        plans: Arc<effects::SelectedProviderPlanFacts>,
    ) -> Self {
        Self { inventory, plans }
    }

    pub fn program(&self) -> &Arc<FrozenTypedTrees> {
        self.inventory.program()
    }
    pub fn inventory(&self) -> &Arc<PreparedProviderInventory> {
        &self.inventory
    }
    pub fn plans(&self) -> &effects::SelectedProviderPlanFacts {
        &self.plans
    }

    pub fn project_instances(&self) -> Result<effects::SelectedProviderPlanFacts, Vec<Diagnostic>> {
        if self.plans.plans().is_empty() {
            return provider_selection::project_provider_instances(&self.plans, &[]);
        }
        Err(vec![Diagnostic::error(
            "no product route yet: selected provider instances require route-owned application custody",
        )])
    }
}
