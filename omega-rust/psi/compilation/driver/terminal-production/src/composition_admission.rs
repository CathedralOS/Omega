//! Consumer policy over the completed, unoptimized Terminal composition.
//!
//! This is a borrowed admission point, not a stage-order callback. The policy
//! sees portable semantics and diagnostic attribution only; it cannot mutate
//! the program, recover checked trees, select an optimization or resume the
//! pipeline. Psi continues with the same lowering after admission succeeds.

use crate::TerminalApplicationCustody;
use crate::TerminalArtifactProductionError;
use crate::observation::{PsiCompilationObserver, PsiCompilationStage};
use diagnostics::Diagnostic;
use terminal_psi::{TerminalMachineProvenance, TerminalModule};

/// One exact retained body and its root-specific diagnostic attribution.
/// This is invocation-local input, not a portable grant or a source handle.
pub struct TerminalAdmissionRoot {
    pub machine: semantic_vocabulary::MachineId,
    pub provenance: TerminalMachineProvenance,
}

/// Admit one application's ordered roots against the unoptimized composition.
///
/// Projection is lazy so an earlier policy refusal precedes a later missing
/// body. Consumers visit every root on success, including when their policy
/// imposes no exclusions, and stop at the first projection or policy refusal.
/// Duplicate roots and their individual attribution are intentional.
pub trait TerminalCompositionAdmission {
    fn admit(
        &self,
        module: &TerminalModule,
        roots: &mut dyn Iterator<Item = Result<TerminalAdmissionRoot, Vec<Diagnostic>>>,
    ) -> Result<(), Vec<Diagnostic>>;
}

impl TerminalApplicationCustody<'_> {
    /// Project this application's source subjects before optimization, then
    /// lend their canonical bodies to its policy in original request order.
    pub(crate) fn admit<'checked>(
        &self,
        checked: &'checked checked_trees::CheckedTrees,
        lowered: &lowered_psi::LoweredPsi,
        default_machine: crate::TerminalMachineSelection<'_>,
        sources: &[(source::SourceId, [u8; 32])],
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Option<&'checked checked_trees::CheckedTerminalMachineSelection>, Vec<Diagnostic>>
    {
        if self.entry_identity.is_none() && self.admission.is_none() {
            return Ok(None);
        }
        let selection = observer
            .record_result(PsiCompilationStage::MachineSelection, || {
                checked_trees_to_lowered_psi::select_terminal_machine(
                    checked,
                    self.entry_machine.unwrap_or(default_machine),
                )
            })
            .map_err(|error| TerminalArtifactProductionError::Lowering(error).diagnostics())?;
        if let Some(admission) = self.admission {
            let mut roots = std::iter::once(selection.machine)
                .chain(self.admission_roots.iter().copied())
                .map(|machine| project_root(checked, lowered, machine, sources));
            admission.admit(&lowered.semantic_module, &mut roots)?;
        }
        Ok(Some(selection))
    }
}

fn project_root(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
    machine: symbols::SymbolHandle,
    sources: &[(source::SourceId, [u8; 32])],
) -> Result<TerminalAdmissionRoot, Vec<Diagnostic>> {
    let entry = lowered_psi_to_terminal_psi::retained_source_machine(
        checked,
        machine,
        sources,
        &lowered.semantic_module,
    )
    .map_err(|error| TerminalArtifactProductionError::EntryReceipt(error).diagnostics())?;
    let provenance = checked_trees_to_lowered_psi::lowered_machine_provenance(
        checked, lowered, machine, sources,
    );
    Ok(TerminalAdmissionRoot {
        machine: entry.id,
        provenance,
    })
}
