//! Checker-owned carry facts. The declaration keeps the authored minimum;
//! this plan records the effective policy derived from the complete stored
//! shape so later liveness, runtime-admission, artifact, and model-export
//! passes never need to reinterpret source syntax.

use omega_core::semantics::CarryPolicy;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CarryFacts {
    /// One entry per data declaration, in declaration order.
    pub data: Vec<DataCarryFact>,
    /// Canonical call sites that may suspend, with the most restrictive
    /// four-axis policy contributed by every value live across that exact
    /// crossing. Runtime activation admission consumes this checked result;
    /// it must not re-run syntax-shaped liveness.
    pub suspension_crossings: Vec<SuspensionCrossingCarryFact>,
    /// One envelope per machine over every typed storage slot and call value
    /// that may be live at an instruction boundary. Asynchronous runtimes use
    /// this envelope because they may preempt away from canonical safe points.
    pub asynchronous_preemption: Vec<MachinePreemptionCarryFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachinePreemptionCarryFact {
    pub machine: SymbolHandle,
    pub effective: CarryPolicy,
    /// False when a call/value shape could not be resolved to a checked type.
    /// Absence of a type is never treated as permissive evidence.
    pub analysis_complete: bool,
    /// Canonical type inputs contributing to the envelope, deduplicated by
    /// handle for artifact/debug consumers.
    pub contributing_types: Vec<omega_typed_trees::types::TypeReferenceHandle>,
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
    pub type_reference: omega_typed_trees::types::TypeReferenceHandle,
    pub storage: SuspensionCrossingStorage,
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

    pub fn preemption_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&MachinePreemptionCarryFact> {
        self.asynchronous_preemption
            .iter()
            .find(|fact| fact.machine == machine)
    }
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
