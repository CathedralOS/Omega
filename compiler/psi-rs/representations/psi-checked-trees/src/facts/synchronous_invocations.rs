//! EFX: normalized direct synchronous boundary-invocation facts.
//!
//! These plans remain independent from transitive service reach and from the
//! suspension/blocking axes. Canonical binding identities are retained exactly
//! as produced by checked normalization; consumers must not reconstruct them
//! from flow calls or service rows.

use psi_language_semantics::SynchronousInvocationPlan;
use psi_symbols::SymbolHandle;

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
    pub plan: SynchronousInvocationPlan,
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_language_semantics::SynchronousInvocationInterface;

    #[test]
    fn exact_machine_owner_preserves_public_empty_private_empty_and_binding_identity() {
        let published = SymbolHandle::from_arena_index(1);
        let internal = SymbolHandle::from_arena_index(2);
        let unknown = SymbolHandle::from_arena_index(3);
        let facts = SynchronousInvocationFacts {
            machines: vec![
                MachineSynchronousInvocationFact {
                    machine: published,
                    plan: SynchronousInvocationPlan {
                        interface: SynchronousInvocationInterface::PublishedCeiling,
                        published: Vec::new(),
                        checked_inferred: vec!["parameter:0".to_owned()],
                    },
                },
                MachineSynchronousInvocationFact {
                    machine: internal,
                    plan: SynchronousInvocationPlan {
                        interface: SynchronousInvocationInterface::InternalInferred,
                        published: Vec::new(),
                        checked_inferred: vec!["service:Clock".to_owned()],
                    },
                },
            ],
        };

        let published_plan = facts.for_machine(published).expect("published plan");
        assert_eq!(
            published_plan.interface,
            SynchronousInvocationInterface::PublishedCeiling
        );
        assert!(published_plan.published.is_empty());
        assert_eq!(published_plan.checked_inferred, ["parameter:0"]);

        let internal_plan = facts.for_machine(internal).expect("internal plan");
        assert_eq!(
            internal_plan.interface,
            SynchronousInvocationInterface::InternalInferred
        );
        assert!(internal_plan.published.is_empty());
        assert_eq!(internal_plan.checked_inferred, ["service:Clock"]);
        assert_eq!(facts.for_machine(unknown), None);
    }
}
