use omega_calling_conventions::{StateFootprintEvidence, compose_state_footprints};

/// Provenance of one independently derived boundary-code footprint fragment.
/// The closed set grows only when the corresponding lowering stage can derive
/// exact evidence from the same target implementation that emits the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryFootprintFragmentOrigin {
    EntryStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryFootprintFragment {
    pub origin: BoundaryFootprintFragmentOrigin,
    pub evidence: StateFootprintEvidence,
}

/// Retained implementation evidence for compiler-owned boundary code. A plan
/// remains explicitly incomplete until body, exit, veneer, thunk, and admitted
/// leaf enumeration are all represented after final placement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundaryFootprintPlan {
    pub fragments: Vec<BoundaryFootprintFragment>,
    pub enumeration_complete: bool,
}

impl BoundaryFootprintPlan {
    pub fn composed_evidence(&self) -> StateFootprintEvidence {
        compose_state_footprints(self.fragments.iter().map(|fragment| &fragment.evidence))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};

    #[test]
    fn partial_plan_composes_fragment_evidence_without_claiming_completeness() {
        let plan = BoundaryFootprintPlan {
            fragments: vec![BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::EntryStorage,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86R15]),
                    MachineStateSet::new([MachineState::Flags]),
                ),
            }],
            enumeration_complete: false,
        };

        assert!(!plan.enumeration_complete);
        assert_eq!(
            plan.composed_evidence().registers().as_slice(),
            &[MachineRegister::X86R15]
        );
        assert!(
            plan.composed_evidence()
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }
}
