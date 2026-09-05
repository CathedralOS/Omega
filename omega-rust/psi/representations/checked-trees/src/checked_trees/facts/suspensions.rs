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

#[cfg(test)]
mod tests {
    use super::*;
    use language_semantics::SuspensionInterface;

    #[test]
    fn exact_machine_owner_preserves_public_negative_private_negative_and_positive() {
        let published_false = SymbolHandle::from_arena_index(1);
        let internal_false = SymbolHandle::from_arena_index(2);
        let published_true = SymbolHandle::from_arena_index(3);
        let unknown = SymbolHandle::from_arena_index(4);
        let facts = SuspensionFacts {
            machines: vec![
                MachineSuspensionFact {
                    machine: published_false,
                    plan: SuspensionPlan {
                        interface: SuspensionInterface::PublishedMaySuspend(false),
                        checked_may_suspend: false,
                    },
                },
                MachineSuspensionFact {
                    machine: internal_false,
                    plan: SuspensionPlan {
                        interface: SuspensionInterface::InternalInferred,
                        checked_may_suspend: false,
                    },
                },
                MachineSuspensionFact {
                    machine: published_true,
                    plan: SuspensionPlan {
                        interface: SuspensionInterface::PublishedMaySuspend(true),
                        checked_may_suspend: true,
                    },
                },
            ],
        };

        assert_eq!(
            facts.for_machine(published_false),
            Some(SuspensionPlan {
                interface: SuspensionInterface::PublishedMaySuspend(false),
                checked_may_suspend: false,
            })
        );
        assert_eq!(
            facts.for_machine(internal_false),
            Some(SuspensionPlan {
                interface: SuspensionInterface::InternalInferred,
                checked_may_suspend: false,
            })
        );
        assert_eq!(
            facts.for_machine(published_true),
            Some(SuspensionPlan {
                interface: SuspensionInterface::PublishedMaySuspend(true),
                checked_may_suspend: true,
            })
        );
        assert_eq!(facts.for_machine(unknown), None);
    }
}
