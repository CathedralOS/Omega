//! Exact body-derived mutation summaries. These frames are implementation
//! evidence and remain independent from the published machine contract.

use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MutationFacts {
    /// One entry per checked machine, in machine-table order.
    pub machines: Vec<MachineMutationFact>,
}

impl MutationFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineMutationFact> {
        self.machines.iter().find(|fact| fact.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineMutationFact {
    pub machine: SymbolHandle,
    /// Exact state-symbol frames in that machine's state-table order.
    pub state_write_frames: Vec<StateWriteFramePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateWriteFramePlan {
    pub state: SymbolHandle,
    pub frame: psi_facts::NormalizedWriteFrame,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_facts_use_exact_machine_identity_and_default_to_absence() {
        let machine = SymbolHandle::from_arena_index(3);
        let state = SymbolHandle::from_arena_index(4);
        let facts = MutationFacts {
            machines: vec![MachineMutationFact {
                machine,
                state_write_frames: vec![StateWriteFramePlan {
                    state,
                    frame: psi_facts::NormalizedWriteFrame::complete(vec!["self.value".to_owned()]),
                }],
            }],
        };

        let retained = facts.for_machine(machine).expect("exact machine fact");
        assert_eq!(retained.state_write_frames[0].state, state);
        assert_eq!(retained.state_write_frames[0].frame.paths(), ["self.value"]);
        assert!(
            facts
                .for_machine(SymbolHandle::from_arena_index(9))
                .is_none()
        );
    }
}
