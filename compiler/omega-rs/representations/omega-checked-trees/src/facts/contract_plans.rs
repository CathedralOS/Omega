//! STR4 checked plans (machine_taxonomy.md): the normalized MACHINE
//! SEMANTIC CONTRACT, independent of syntax and lowering -- component
//! manifests, proof artifacts, provider admission, and hot-swap checks
//! reference this identity, never re-derived booleans. Slice 1 carries the
//! published halves that exist today (supply mode, effect-row ceiling,
//! termination guarantee) plus a deterministic fingerprint over them;
//! requires/ensures fact canonicalization is the recorded follow-up.
//! Prover-independence (acceptance 8: a stronger prover cannot change an
//! exported contract ID) holds BY CONSTRUCTION: only declared/published
//! halves enter the fingerprint, never inferred rows or witnesses.

use omega_core::semantics::{
    BlockingInterface, BlockingPlan, EffectRowId, MachineSupplyMode, SuspensionInterface,
    SuspensionPlan, TerminationGuarantee,
};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineContractPlans {
    /// One entry per machine, in machine order.
    pub machines: Vec<MachineContractPlan>,
    /// Concrete `TaskRuntime::{start,try_start}<M>` specializations elaborated
    /// after checking against the selected target's layouts. These remain
    /// provider-independent demands; runtime admission consumes them later.
    pub task_activations: Vec<TaskActivationPlanFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStartOperation {
    Start,
    TryStart,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskActivationPlanFact {
    pub start_instance: SymbolHandle,
    pub target_machine: SymbolHandle,
    pub target_entry: SymbolHandle,
    pub specialization_fingerprint: u64,
    pub operation: TaskStartOperation,
    pub plan: omega_task_plans::ValidatedActivationPlan,
}

impl MachineContractPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineContractPlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContractPlan {
    pub machine: SymbolHandle,
    /// How the machine is supplied (checked body / requirement / boundary).
    pub supply_mode: MachineSupplyMode,
    /// The authored `effects` clause's normalized row (the published
    /// ceiling; the EMPTY row when no clause).
    pub published_effect_row: EffectRowId,
    /// Independent authored/inferred operational axes. These are never
    /// reconstructed from `published_effect_row`.
    pub suspension: SuspensionPlan,
    pub blocking: BlockingPlan,
    /// The published termination guarantee (never the witness -- the
    /// firewall is the shape).
    pub published_termination: TerminationGuarantee,
    /// Body-derived, state-relative write frames. These are implementation
    /// evidence, not authored contract material, and therefore never enter
    /// `fingerprint` or specialization identity.
    pub inferred_write_frames: Vec<StateWriteFramePlan>,
    /// The deterministic identity over the published halves above. Stable
    /// across prover-strength changes and body edits that keep the declared
    /// surface; NOT yet a full contract identity (facts canonicalization is
    /// the follow-up slice).
    pub fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateWriteFramePlan {
    pub state: SymbolHandle,
    pub frame: omega_facts::NormalizedWriteFrame,
}

/// The slice-1 fingerprint: an FNV-1a fold over the published halves'
/// normalized encodings. Deterministic across programs for the same
/// declared surface (effect-row identity is the sorted member-id set; the
/// termination guarantee and supply mode are closed enums).
pub fn contract_fingerprint(
    supply_mode: MachineSupplyMode,
    published_effect_row: EffectRowId,
    published_effect_members: &[omega_core::semantics::EffectMemberId],
    suspension_interface: SuspensionInterface,
    blocking_interface: BlockingInterface,
    published_termination: &TerminationGuarantee,
    canonical_facts: &[Vec<u8>],
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    let mut fold = |byte: u8| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    };
    fold(match supply_mode {
        MachineSupplyMode::CheckedBody => 1,
        MachineSupplyMode::Requirement => 2,
        MachineSupplyMode::Boundary => 3,
        MachineSupplyMode::Accepted => 4,
        // PRV4: the leaf's supply tag; the binding identity folds separately
        // below so two leaves with different bindings differ.
        MachineSupplyMode::ExternalRealization { .. } => 5,
    });
    if let MachineSupplyMode::ExternalRealization { binding } = supply_mode {
        for byte in binding.0.to_le_bytes() {
            fold(byte);
        }
    }
    // The row's MEMBERS, not its table index -- table indices are
    // program-local; member ids are catalog-fixed.
    let _ = published_effect_row;
    for member in published_effect_members {
        for byte in member.0.to_le_bytes() {
            fold(byte);
        }
    }
    fold(match suspension_interface {
        SuspensionInterface::InternalInferred => 1,
        SuspensionInterface::PublishedMaySuspend(false) => 2,
        SuspensionInterface::PublishedMaySuspend(true) => 3,
    });
    fold(match blocking_interface {
        BlockingInterface::InternalInferred => 1,
        BlockingInterface::PublishedMayBlock(false) => 2,
        BlockingInterface::PublishedMayBlock(true) => 3,
    });
    fold(0xff);
    match published_termination {
        TerminationGuarantee::NoGuarantee => fold(1),
        TerminationGuarantee::EventualTerminal { premises } => {
            fold(2);
            for premise in premises {
                for byte in premise.0.to_le_bytes() {
                    fold(byte);
                }
            }
        }
    }
    // Slice 2: the declared requires/ensures facts, pre-sorted by the
    // caller (clause order never enters the identity).
    fold(0xfd);
    for fact in canonical_facts {
        for byte in fact {
            fold(*byte);
        }
        fold(0xfc);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operational_interfaces_participate_independently_in_contract_identity() {
        let fingerprint = |suspension, blocking| {
            contract_fingerprint(
                MachineSupplyMode::Boundary,
                EffectRowId::NULL,
                &[],
                suspension,
                blocking,
                &TerminationGuarantee::NoGuarantee,
                &[],
            )
        };
        let neither = fingerprint(
            SuspensionInterface::PublishedMaySuspend(false),
            BlockingInterface::PublishedMayBlock(false),
        );
        let suspending = fingerprint(
            SuspensionInterface::PublishedMaySuspend(true),
            BlockingInterface::PublishedMayBlock(false),
        );
        let blocking = fingerprint(
            SuspensionInterface::PublishedMaySuspend(false),
            BlockingInterface::PublishedMayBlock(true),
        );
        assert_ne!(neither, suspending);
        assert_ne!(neither, blocking);
        assert_ne!(suspending, blocking);
    }
}
