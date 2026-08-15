//! EFX: normalized worker-blocking contracts keyed by exact machine identity.
//!
//! Blocking stays independent from service reach and suspension. The interface
//! preserves public omission as an explicit negative guarantee; checked
//! inference remains implementation evidence.

use psi_language_semantics::BlockingPlan;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockingFacts {
    /// One exact-keyed entry per checked machine, in machine order.
    pub machines: Vec<MachineBlockingFact>,
}

impl BlockingFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<BlockingPlan> {
        self.machines
            .iter()
            .find(|fact| fact.machine == machine)
            .map(|fact| fact.plan)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineBlockingFact {
    pub machine: SymbolHandle,
    pub plan: BlockingPlan,
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_language_semantics::BlockingInterface;

    #[test]
    fn exact_machine_owner_preserves_public_negative_private_negative_and_positive() {
        let published_false = SymbolHandle::from_arena_index(1);
        let internal_false = SymbolHandle::from_arena_index(2);
        let published_true = SymbolHandle::from_arena_index(3);
        let unknown = SymbolHandle::from_arena_index(4);
        let facts = BlockingFacts {
            machines: vec![
                MachineBlockingFact {
                    machine: published_false,
                    plan: BlockingPlan {
                        interface: BlockingInterface::PublishedMayBlock(false),
                        checked_may_block: false,
                    },
                },
                MachineBlockingFact {
                    machine: internal_false,
                    plan: BlockingPlan {
                        interface: BlockingInterface::InternalInferred,
                        checked_may_block: false,
                    },
                },
                MachineBlockingFact {
                    machine: published_true,
                    plan: BlockingPlan {
                        interface: BlockingInterface::PublishedMayBlock(true),
                        checked_may_block: true,
                    },
                },
            ],
        };

        assert_eq!(
            facts.for_machine(published_false),
            Some(BlockingPlan {
                interface: BlockingInterface::PublishedMayBlock(false),
                checked_may_block: false,
            })
        );
        assert_eq!(
            facts.for_machine(internal_false),
            Some(BlockingPlan {
                interface: BlockingInterface::InternalInferred,
                checked_may_block: false,
            })
        );
        assert_eq!(
            facts.for_machine(published_true),
            Some(BlockingPlan {
                interface: BlockingInterface::PublishedMayBlock(true),
                checked_may_block: true,
            })
        );
        assert_eq!(facts.for_machine(unknown), None);
    }
}
