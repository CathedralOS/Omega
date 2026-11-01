//! Source-issued root identities and attribution, not product admission.
//!
//! Psi projects requested entry/callback/adapter roots while source joins are
//! live. Their ordered results travel with the code; the product consumer
//! checks its policy after Terminal publication without a callback into Psi.

use crate::TerminalApplicationCustody;
use crate::TerminalArtifactProductionError;
use crate::observation::{PsiCompilationObserver, PsiCompilationStage};
use diagnostics::Diagnostic;
use terminal_psi::TerminalMachineProvenance;

use checked_trees::CheckedTrees;
use checked_trees_to_lowered_psi::{lowered_machine_provenance, select_terminal_machine};
use lowered_psi::LoweredPsi;
use lowered_psi_to_terminal_psi::retained_source_machine;

/// One exact retained body and its root-specific diagnostic attribution.
/// This is invocation-local input, not a portable grant or a source handle.
#[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalAdmissionRoot {
    pub machine: semantic_vocabulary::MachineId,
    pub provenance: TerminalMachineProvenance,
}

/// Ordered composition scope requested by a product consumer. This requests
/// source projection, never successful exclusion or execution authority.
#[derive(Clone, Copy, Default)]
pub struct TerminalCompositionRoots<'a> {
    pub callbacks: Option<&'a crate::PreparedTerminalRoots>,
    pub physical_adapter: symbols::SymbolHandle,
}

pub(crate) struct PreparedTerminalApplication<'checked> {
    pub selection: Option<&'checked checked_trees::CheckedTerminalMachineSelection>,
    pub roots: Vec<Result<TerminalAdmissionRoot, Vec<Diagnostic>>>,
}

impl TerminalApplicationCustody<'_> {
    /// Capture root results independently. A later missing body remains data
    /// until the consumer reaches it, so it cannot hide an earlier policy refusal.
    pub(crate) fn prepare_roots<'checked>(
        &self,
        checked: &'checked CheckedTrees,
        lowered: &LoweredPsi,
        default_machine: crate::TerminalMachineSelection<'_>,
        sources: &[(source::SourceId, [u8; 32])],
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<PreparedTerminalApplication<'checked>, Vec<Diagnostic>> {
        if self.entry_identity.is_none() && self.composition.is_none() {
            return Ok(PreparedTerminalApplication {
                selection: None,
                roots: Vec::new(),
            });
        }
        let selection = observer
            .record_result(PsiCompilationStage::MachineSelection, || {
                select_terminal_machine(checked, self.entry_machine.unwrap_or(default_machine))
            })
            .map_err(|error| TerminalArtifactProductionError::Lowering(error).diagnostics())?;
        let roots = if let Some(composition) = self.composition {
            let entry =
                std::iter::once_with(|| project_root(checked, lowered, selection.machine, sources));
            let callbacks = composition
                .callbacks
                .into_iter()
                .flat_map(|roots| roots.retained_roots(&lowered.semantic_module))
                .map(|root| project_callback_root(checked, lowered, root));
            let adapter = composition
                .physical_adapter
                .is_valid()
                .then_some(composition.physical_adapter)
                .into_iter()
                .map(|machine| project_root(checked, lowered, machine, sources));
            entry.chain(callbacks).chain(adapter).collect()
        } else {
            Vec::new()
        };
        Ok(PreparedTerminalApplication {
            selection: Some(selection),
            roots,
        })
    }
}

fn project_callback_root(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
    root: Result<checked_trees_to_lowered_psi::RetainedTerminalRoot<'_>, String>,
) -> Result<TerminalAdmissionRoot, Vec<Diagnostic>> {
    let root = root.map_err(|reason| {
        vec![Diagnostic::error(format!(
            "callback composition root: {reason}"
        ))]
    })?;
    let provenance = lowered_machine_provenance(
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
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
    machine: symbols::SymbolHandle,
    sources: &[(source::SourceId, [u8; 32])],
) -> Result<TerminalAdmissionRoot, Vec<Diagnostic>> {
    let entry = retained_source_machine(checked, machine, sources, &lowered.semantic_module)
        .map_err(|error| TerminalArtifactProductionError::EntryReceipt(error).diagnostics())?;
    let entry_name = checked.typed.machine(machine).map(|body| {
        (
            body.name.as_str(),
            checked.typed.symbols.symbol_source_span(machine),
        )
    });
    let provenance = lowered_machine_provenance(checked, lowered, entry.id, entry_name);
    Ok(TerminalAdmissionRoot {
        machine: entry.id,
        provenance,
    })
}
