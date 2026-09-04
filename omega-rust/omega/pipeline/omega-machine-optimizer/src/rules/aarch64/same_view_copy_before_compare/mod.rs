//! Optimizer module role: stage group. Shared mechanics for exact AArch64
//! same-view copies immediately consumed by a comparison.
//!
//! Exact siblings retain their own entrance, declarative pattern, policy
//! contract, and tests. This rung owns only their common deterministic scan,
//! accounting, disposition construction, and separately implemented replay.

mod footprints;
mod proposal;
mod replay;
mod roots;

use omega_selected_instructions::{MachineAlternativeFamily, SelectedInstructionKind};

use crate::Aarch64SameViewCopyElisionPolicy;

pub(super) use proposal::propose;
pub(super) use replay::replay;

#[derive(Clone, Copy)]
pub(super) struct CompareConsumerContract {
    pub policy: Aarch64SameViewCopyElisionPolicy,
    pub kind: SelectedInstructionKind,
    pub family: MachineAlternativeFamily,
    pub operand_count: usize,
    pub external_reads: &'static [u16],
    pub consumed_operand: usize,
    pub provenance: CompareProvenanceContract,
}

#[derive(Clone, Copy)]
pub(super) enum CompareProvenanceContract {
    ExactCopyValue,
    ConsumedOriginAndRetainedValue,
}
