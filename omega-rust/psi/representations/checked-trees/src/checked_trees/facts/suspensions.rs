//! EFX: normalized suspension contracts keyed by exact machine identity.
//!
//! Suspension stays independent from service reach and worker blocking. The
//! interface preserves public omission as an explicit negative guarantee;
//! checked inference remains implementation evidence.

use language_semantics::SuspensionPlan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SuspensionFacts {
    /// One exact-keyed entry per checked machine, in machine order.
    pub machines: Vec<MachineSuspensionFact>,
}

impl SuspensionFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<SuspensionPlan> {
        self.machines
            .iter()
            .find(|fact| fact.machine == machine)
            .map(|fact| fact.plan)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineSuspensionFact {
    pub machine: SymbolHandle,
    pub plan: SuspensionPlan,
}
