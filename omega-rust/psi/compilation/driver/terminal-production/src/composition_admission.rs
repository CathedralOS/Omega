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
            let entry =
                std::iter::once_with(|| project_root(checked, lowered, selection.machine, sources));
            let callbacks = self
                .callback_roots
                .into_iter()
                .flat_map(|roots| roots.retained_roots(&lowered.semantic_module))
                .map(|root| project_callback_root(checked, lowered, root));
            let adapter = self
                .physical_adapter
                .is_valid()
                .then_some(self.physical_adapter)
                .into_iter()
                .map(|machine| project_root(checked, lowered, machine, sources));
            let mut roots = entry.chain(callbacks).chain(adapter);
            admission.admit(&lowered.semantic_module, &mut roots)?;
        }
        Ok(Some(selection))
    }
}

fn project_callback_root(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
    root: Result<checked_trees_to_lowered_psi::RetainedTerminalRoot<'_>, String>,
) -> Result<TerminalAdmissionRoot, Vec<Diagnostic>> {
    let root = root.map_err(|reason| {
        vec![Diagnostic::error(format!(
            "callback composition root: {reason}"
        ))]
    })?;
    let provenance = checked_trees_to_lowered_psi::lowered_machine_provenance(
        checked,
        lowered,
        root.machine,
        root.name.map(|name| (name, root.span)),
    );
    Ok(TerminalAdmissionRoot {
        machine: root.machine,
        provenance,
    })
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
    let entry_name = checked.typed.machine(machine).map(|body| {
        (
            body.name.as_str(),
            checked.typed.symbols.symbol_source_span(machine),
        )
    });
    let provenance = checked_trees_to_lowered_psi::lowered_machine_provenance(
        checked, lowered, entry.id, entry_name,
    );
    Ok(TerminalAdmissionRoot {
        machine: entry.id,
        provenance,
    })
}
