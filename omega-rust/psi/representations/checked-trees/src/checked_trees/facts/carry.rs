//! Checker-owned carry facts. The declaration keeps the authored minimum;
//! this plan records the effective policy derived from the complete stored
//! shape so later liveness, runtime-admission, artifact, and model-export
//! passes never need to reinterpret source syntax.

use arena::{Arena, HandleSpan};
use language_semantics::{CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use symbols::SymbolHandle;

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
    pub claim_identity: language_semantics::PermissionClaimIdentity,
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
    pub contributing_types: Vec<typed_trees::types::TypeReferenceHandle>,
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
    /// Exact method receiver when the call is not receiver-free. The first
    /// Terminal carrier fails closed on this cohort until it can bind the
    /// receiver to a source-free structural place.
    pub receiver: Option<SymbolHandle>,
    pub effective: CarryPolicy,
    pub live_values: Vec<SuspensionCrossingLiveValueFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspensionCrossingLiveValueFact {
    pub type_reference: typed_trees::types::TypeReferenceHandle,
    pub storage: SuspensionCrossingStorage,
    /// Exact source coordinate for this value. The coordinate is retained by
    /// the checker because neither its storage class nor its type identifies a
    /// unique value at the crossing.
    pub origin: SuspensionCrossingValueOrigin,
    /// Complete live linear-claim identities attached to this exact place.
    /// An empty roster means no compiler-owned live claim was established; it
    /// must never be reconstructed from the type or storage class.
    pub claims: Vec<language_semantics::PermissionClaimIdentity>,
    /// Per-value policy after a born-strict claim is relaxed by the exact
    /// compiler-owned permissions still attached to this place. This may be
    /// stricter than the transparent carrier's structural policy.
    pub effective: CarryPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspensionCrossingValueOrigin {
    Persistent {
        symbol: SymbolHandle,
    },
    Parameter {
        symbol: SymbolHandle,
        /// Dense ordinal among non-`self` state parameters.
        position: usize,
    },
    Local {
        symbol: SymbolHandle,
        /// Source statement that establishes the local.
        statement_index: usize,
        /// Exact position in the checked scalar environment: non-`self`
        /// parameters followed by preceding local-data bindings.
        environment_position: usize,
    },
    CallArgument {
        /// Dense ordinary argument ordinal at the exact call coordinate.
        position: usize,
    },
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

/// Reconstruct the established suspension-crossing identity used by Omega's
/// activation planning. Exact live-place and claim coordinates remain
/// separately replayable frontier data; extending that frontier does not
/// silently rename already-published crossing identities.
pub fn canonical_suspension_crossing_id(
    program: &typed_trees::TypedTrees,
    crossing: &SuspensionCrossingCarryFact,
) -> Option<semantic_vocabulary::SuspensionCrossingId> {
    let mut hash = StableCrossingHash::new();
    hash.byte(0x73);
    hash.string(symbol_identity(program, crossing.machine)?);
    hash.string(symbol_identity(program, crossing.state)?);
    hash.usize(crossing.statement_index);
    hash.usize(crossing.call_ordinal);
    hash.string(symbol_identity(program, crossing.target)?);
    hash.policy(crossing.effective);
    for live in &crossing.live_values {
        hash.string(
            program
                .normalized_type_identity(live.type_reference)
                .as_str(),
        );
        hash.byte(match live.storage {
            SuspensionCrossingStorage::Persistent => 1,
            SuspensionCrossingStorage::Parameter => 2,
            SuspensionCrossingStorage::Local => 3,
            SuspensionCrossingStorage::CallArgument => 4,
        });
        hash.policy(live.effective);
    }
    semantic_vocabulary::SuspensionCrossingId::new(hash.finish())
}

fn symbol_identity(program: &typed_trees::TypedTrees, symbol: SymbolHandle) -> Option<&str> {
    for machine in program.machines() {
        if machine.symbol == symbol {
            return Some(machine.name.as_str());
        }
        if let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == symbol)
        {
            return Some(state.name.as_str());
        }
    }
    None
}

struct StableCrossingHash(u64);

impl StableCrossingHash {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn string(&mut self, value: &str) {
        for byte in value.as_bytes() {
            self.byte(*byte);
        }
        self.byte(0);
    }

    fn usize(&mut self, value: usize) {
        for byte in (value as u64).to_le_bytes() {
            self.byte(byte);
        }
    }

    fn policy(&mut self, policy: CarryPolicy) {
        self.byte(u8::from(policy.suspension == CarrySuspension::Allowed));
        self.byte(u8::from(policy.cpu == CarryCpu::Origin));
        self.byte(u8::from(policy.host_thread == CarryHostThread::Origin));
        self.byte(match policy.address {
            CarryAddress::Movable => 1,
            CarryAddress::Stable => 2,
        });
    }

    fn finish(self) -> u64 {
        self.0.max(1)
    }
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
    pub type_reference: typed_trees::types::TypeReferenceHandle,
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
