//! Checker-owned carry facts. The declaration keeps the authored minimum;
//! this plan records the effective policy derived from the complete stored
//! shape so later liveness, runtime-admission, artifact, and model-export
//! passes never need to reinterpret source syntax.

use psi_arena::{Arena, HandleSpan};
use psi_language_semantics::CarryPolicy;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CarryFacts {
    /// One entry per data declaration, in declaration order.
    pub data: Vec<DataCarryFact>,
    /// One topology root per machine. Contained fields and their attached
    /// machine targets live in grouped arenas below; no record owns a leaf
    /// allocation.
    pub machine_topologies: Arena<MachineCarryTopologyFact>,
    pub contained_fields: Arena<ContainedMachineFieldFact>,
    pub contained_targets: Arena<ContainedMachineTargetFact>,
    /// Canonical call sites that may suspend, with the most restrictive
    /// four-axis policy contributed by every value live across that exact
    /// crossing. Activation planning consumes this checked result; it must not
    /// re-run syntax-shaped liveness.
    pub suspension_crossings: Vec<SuspensionCrossingCarryFact>,
    /// One envelope per machine over every typed storage slot and call value
    /// that may be live at an instruction boundary. Activation planning keeps
    /// only the CPU/thread preservation this envelope demands; fixed,
    /// nonmoving stack storage supplies address stability structurally.
    pub activation_wide_carry: Vec<MachineActivationCarryFact>,
    /// Effective carry policy retained by the exact linear claim identity.
    /// This is independent of the carrier's structural policy and survives
    /// path-indexed n-ary transformations through the ownership outcome map.
    pub claim_policies: Vec<ClaimCarryPolicyFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimCarryPolicyFact {
    pub claim_identity: psi_language_semantics::PermissionClaimIdentity,
    pub effective: CarryPolicy,
    /// Number of independent qualification evidence origins intersected to
    /// produce `effective`. Each origin begins strict and is relaxed only by
    /// its own exact positive permissions.
    pub contributing_origins: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineActivationCarryFact {
    pub machine: SymbolHandle,
    /// Intersection of this machine's direct envelope with every machine
    /// reachable through its checked contained-field topology.
    pub effective: CarryPolicy,
    /// False when any machine in that subtree has a call/value shape that
    /// could not be resolved to a checked type. Absence of a type is never
    /// treated as permissive evidence.
    pub analysis_complete: bool,
    /// Canonical type inputs contributing to the envelope, deduplicated by
    /// handle for artifact/debug consumers.
    pub contributing_types: Vec<psi_typed_trees::types::TypeReferenceHandle>,
    /// Some transient reference-like values have no standalone type handle in
    /// typed expressions; their strict contribution remains explicit.
    pub unnamed_strict_values: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspensionCrossingCarryFact {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target: SymbolHandle,
    pub effective: CarryPolicy,
    pub live_values: Vec<SuspensionCrossingLiveValueFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuspensionCrossingLiveValueFact {
    pub type_reference: psi_typed_trees::types::TypeReferenceHandle,
    pub storage: SuspensionCrossingStorage,
    /// Per-value policy after a born-strict claim is relaxed by the exact
    /// compiler-owned permissions still attached to this place. This may be
    /// stricter than the transparent carrier's structural policy.
    pub effective: CarryPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspensionCrossingStorage {
    /// Already resident in the machine's persistent attached/owned layout.
    Persistent,
    /// Entry/state parameter that must survive the parked continuation.
    Parameter,
    /// Lexical local that must survive the parked continuation.
    Local,
    /// Value materialized for the suspending call itself.
    CallArgument,
}

impl CarryFacts {
    pub fn for_data(&self, data: SymbolHandle) -> Option<&DataCarryFact> {
        self.data.iter().find(|fact| fact.data == data)
    }

    pub fn activation_carry_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&MachineActivationCarryFact> {
        self.activation_wide_carry
            .iter()
            .find(|fact| fact.machine == machine)
    }

    pub fn topology_for_machine(&self, machine: SymbolHandle) -> Option<&MachineCarryTopologyFact> {
        self.machine_topologies
            .iter()
            .find_map(|(_, fact)| (fact.machine == machine).then_some(fact))
    }

    pub fn contained_fields_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> &[ContainedMachineFieldFact] {
        self.topology_for_machine(machine)
            .map(|fact| self.contained_fields.span_or_empty(fact.fields))
            .unwrap_or_default()
    }

    pub fn contained_targets_for_field(
        &self,
        field: &ContainedMachineFieldFact,
    ) -> &[ContainedMachineTargetFact] {
        self.contained_targets.span_or_empty(field.targets)
    }

    /// Deterministic, cycle-safe closure over the authored field topology.
    /// The root is always first; each machine symbol appears at most once even
    /// when multiple contained fields have the same data type.
    pub fn machine_subtree_symbols(&self, root: SymbolHandle) -> Vec<SymbolHandle> {
        let mut machines = vec![root];
        let mut cursor = 0;
        while cursor < machines.len() {
            let machine = machines[cursor];
            cursor += 1;
            for field in self.contained_fields_for_machine(machine) {
                for target in self.contained_targets_for_field(field) {
                    if !machines.contains(&target.machine) {
                        machines.push(target.machine);
                    }
                }
            }
        }
        machines
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineCarryTopologyFact {
    pub machine: SymbolHandle,
    pub fields: HandleSpan<ContainedMachineFieldFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContainedMachineFieldFact {
    pub field: SymbolHandle,
    pub data: SymbolHandle,
    pub type_reference: psi_typed_trees::types::TypeReferenceHandle,
    pub targets: HandleSpan<ContainedMachineTargetFact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContainedMachineTargetFact {
    pub machine: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataCarryFact {
    pub data: SymbolHandle,
    /// The optional authored minimum promise retained for diagnostics and
    /// published-contract work. It is not the effective policy.
    pub declared: Option<CarryPolicy>,
    /// The checker-derived policy for this transparent stored shape.
    pub effective: CarryPolicy,
}
