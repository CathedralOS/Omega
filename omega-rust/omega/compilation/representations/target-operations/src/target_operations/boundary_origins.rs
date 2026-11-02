//! The admitted boundary facts a target plan's native call origins rejoin to.
//!
//! Admission records them beside the optimized source; the target producer
//! hands them on with its plan, so instruction selection and the target audit
//! check `NativeCallOrigin` rows against them without naming either product.

use installation_evidence::InstalledAssumedBindingEvidence;
use terminal_installation::AdmittedTargetMachineSelections;

/// The selected program entry, the sealed assumed-binding relationships, and
/// the deployment's target machine selections over the portable module.
#[derive(Debug, Clone, Copy)]
pub struct BoundaryOriginFacts<'input> {
    pub program_entry: Option<ProgramEntryOrigin>,
    pub assumed_bindings: &'input [InstalledAssumedBindingEvidence],
    pub target_machine_selections: Option<&'input AdmittedTargetMachineSelections>,
    pub module: &'input terminal_psi::TerminalModule,
}

/// The program entry call the admitted provider plan selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramEntryOrigin {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub entry: semantic_vocabulary::MachineId,
    pub source_signature: [u8; 32],
}
