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
    /// TPR6: call-specific provider-receiver progress demands retained by the
    /// checked fixed point. These are composition obligations, not public
    /// caller premises and not evidence that the selected provider satisfies
    /// the profile.
    pub build_bound_progress: Vec<MachineBuildBoundProgressDemands>,
}

impl TerminationFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineTerminationPlan> {
        self.machines
            .iter()
            .find(|fact| fact.machine == machine)
            .map(|fact| &fact.plan)
    }

    pub fn build_bound_for_machine(&self, machine: SymbolHandle) -> &[BuildBoundProgressDemand] {
        self.build_bound_progress
            .iter()
            .find(|fact| fact.machine == machine)
            .map_or(&[], |fact| fact.demands.as_slice())
    }
}

/// Exact build-bound progress obligations reachable from one checked machine.
/// Private-helper propagation copies the original call coordinate unchanged,
/// so the selected entry retains the real requirement invocation rather than
/// a reconstructed service-level approximation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineBuildBoundProgressDemands {
    pub machine: SymbolHandle,
    pub demands: Vec<BuildBoundProgressDemand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildBoundProgressDemand {
    /// Exact boundary service whose selected provider occurrence is the
    /// premise subject at composition.
    pub provider_service_identity: String,
    /// Compiler-derived package owner of that exact boundary service.
    pub provider_service_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// Normalized exact trait-requirement overload identity.
    pub requirement_identity: String,
    /// Compiler-derived package owner of the requirement overload.
    pub requirement_owner_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// Canonical progress-profile identity.
    pub profile_identity: String,
    /// Exact field path below the provider receiver.
    pub subject_projections: Vec<String>,
    /// Original checked invocation that instantiated this demand.
    pub origin: ProgressDemandCallSite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressDemandCallSite {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: usize,
    pub call_ordinal: usize,
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
            build_bound_progress: Vec::new(),
        };

        assert_eq!(facts.for_machine(internal), Some(&internal_plan));
        assert_eq!(facts.for_machine(published), Some(&published_plan));
        assert_eq!(facts.for_machine(unknown), None);
    }
}
