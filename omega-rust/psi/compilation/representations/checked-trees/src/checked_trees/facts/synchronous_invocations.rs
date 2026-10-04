//! EFX: normalized direct synchronous boundary-invocation facts.
//!
//! These plans remain independent from transitive service reach and from the
//! suspension/blocking axes. Canonical binding identities are retained exactly
//! as produced by checked normalization; consumers must not reconstruct them
//! from flow calls or service rows.

use language_semantics::SynchronousInvocationPlan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SynchronousInvocationFacts {
    /// One exact-keyed entry per checked machine, in machine order.
    pub machines: Vec<MachineSynchronousInvocationFact>,
}

impl SynchronousInvocationFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&SynchronousInvocationPlan> {
        self.machines
            .iter()
            .find(|fact| fact.machine == machine)
            .map(|fact| &fact.plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineSynchronousInvocationFact {
    pub machine: SymbolHandle,
    /// Exact symbolic targets retained before provider settlement can rewrite
    /// the typed call structure. The string plan below is a review-facing
    /// rendering, not an identity source.
    pub published_targets: Vec<flow_effects::InvocationTarget>,
    pub checked_inferred_targets: Vec<flow_effects::InvocationTarget>,
    pub plan: SynchronousInvocationPlan,
}
