//! Ephemeral source joins retained alongside, never inside, portable Terminal artifacts.

use super::LoweredPsi;
use semantic_vocabulary::{BlockId, MachineId, OperationId};
use terminal_psi::ValueDeclaration;

/// Exact checked-to-Terminal join for the first bounded callback body cohort.
/// Source handles remain target-owned sidecar evidence; they are never encoded
/// into the canonical Terminal artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackTerminalLoweringReceipt {
    pub source_machine: symbols::SymbolHandle,
    pub source_entry: symbols::SymbolHandle,
    pub terminal_machine: MachineId,
    pub terminal_entry: BlockId,
}

/// Isolated callback body and the exact checked coordinate that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredCallbackPsi {
    pub terminal: LoweredPsi,
    pub receipt: CallbackTerminalLoweringReceipt,
}

/// One exact checked source call joined to its emitted Terminal operation.
/// Source handles are deliberately confined to the producer result and never
/// enter the canonical Terminal artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSourceCallOccurrence {
    pub source_site: Option<checked_trees::NominalMachineUseSite>,
    pub source_state: symbols::SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub terminal_operation: OperationId,
    pub source_target: symbols::SymbolHandle,
    /// Scalar environment immediately before this call, ordered as checked
    /// state parameters followed by established local bindings. Empty means
    /// this lowering route does not expose an exact scalar frontier mapping.
    pub source_values_before_call: Vec<ValueDeclaration>,
}

/// One exact selected checked IEEE FMA use joined to its emitted Terminal
/// operation. This sidecar is target-neutral custody, not hardware admission:
/// native realization must independently rejoin its plan evidence to an
/// admitted target provider before selecting an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoweredSelectedIeeeFloatFmaOccurrence {
    pub source_state: symbols::SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub terminal_operation: OperationId,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub format: semantic_vocabulary::IeeeFloatFormat,
}
