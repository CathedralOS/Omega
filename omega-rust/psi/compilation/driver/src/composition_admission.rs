//! Consumer policy over the completed, unoptimized Terminal composition.
//!
//! This is a borrowed admission point, not a stage-order callback. The policy
//! sees portable semantics and diagnostic attribution only; it cannot mutate
//! the program, recover checked trees, select an optimization or resume the
//! pipeline. Psi continues with the same lowering after admission succeeds.

use diagnostics::Diagnostic;
use terminal_psi::{TerminalMachineProvenance, TerminalModule};

pub trait TerminalCompositionAdmission {
    fn admit(
        &self,
        module: &TerminalModule,
        entry: semantic_vocabulary::MachineId,
        provenance: &TerminalMachineProvenance,
    ) -> Result<(), Vec<Diagnostic>>;
}
