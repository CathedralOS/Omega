//! TPR/EFX: normalized termination plans keyed by exact machine identity.
//!
//! Published interface, checked summary, and private ranking witness remain
//! one plan, independent from the aggregate machine-contract carrier.

use psi_language_semantics::MachineTerminationPlan;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminationFacts {
    /// One exact-keyed entry per checked machine, in machine order.
    pub machines: Vec<MachineTerminationFact>,
}

impl TerminationFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineTerminationPlan> {
        self.machines
            .iter()
            .find(|fact| fact.machine == machine)
            .map(|fact| &fact.plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineTerminationFact {
    pub machine: SymbolHandle,
    pub plan: MachineTerminationPlan,
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_language_semantics::{RankingWitness, TerminationGuarantee, TerminationInterface};

    #[test]
    fn exact_machine_owner_preserves_interface_summary_witness_and_unknown() {
        let internal = SymbolHandle::from_arena_index(1);
        let published = SymbolHandle::from_arena_index(2);
        let unknown = SymbolHandle::from_arena_index(3);
        let internal_plan = MachineTerminationPlan {
            interface: TerminationInterface::InternalDerived,
            checked_summary: TerminationGuarantee::Terminates {
                premises: Vec::new(),
            },
            implementation_witness: None,
        };
        let published_plan = MachineTerminationPlan {
            interface: TerminationInterface::Published(TerminationGuarantee::Terminates {
                premises: Vec::new(),
            }),
            checked_summary: TerminationGuarantee::Terminates {
                premises: Vec::new(),
            },
            implementation_witness: Some(RankingWitness {
                view_path: "Nat::Descending".to_owned(),
                ..Default::default()
            }),
        };
        let facts = TerminationFacts {
            machines: vec![
                MachineTerminationFact {
                    machine: internal,
                    plan: internal_plan.clone(),
                },
                MachineTerminationFact {
                    machine: published,
                    plan: published_plan.clone(),
                },
            ],
        };

        assert_eq!(facts.for_machine(internal), Some(&internal_plan));
        assert_eq!(facts.for_machine(published), Some(&published_plan));
        assert_eq!(facts.for_machine(unknown), None);
    }
}
