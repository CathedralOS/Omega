//! Retained application descriptions, attached after code publication.
//!
//! Preparation receives closed entry and adapter identities. Attachment reads
//! those descriptions, the callback declaration roster and the sealed program;
//! it neither reopens source nor executes product admission policy.

use crate::observation::{PsiCompilationObserver, PsiCompilationStage};
use crate::{ProducedTerminalApplication, TerminalArtifactProductionError};
use diagnostics::Diagnostic;
use lowered_psi_to_terminal_psi::prepare_program_entry_receipt;
use terminal_psi::{PreparedNominalRoot, TerminalMachineProvenance};

#[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalAdmissionRoot {
    pub machine: semantic_vocabulary::MachineId,
    pub provenance: TerminalMachineProvenance,
}

/// Already-closed root descriptions; none interprets a source coordinate.
#[derive(Clone, Copy, Default)]
pub struct TerminalCompositionRoots<'a> {
    pub callbacks: Option<&'a crate::PreparedTerminalRoots>,
    pub physical_adapter: Option<&'a PreparedNominalRoot>,
}

struct PreparedComposition<'a> {
    callbacks: Option<&'a crate::PreparedTerminalRoots>,
    adapter: Option<&'a PreparedNominalRoot>,
}

/// Declaration descriptions of one application's requested judgments.
/// These private records contain no checked trees, source handles or lowering
/// products. Failed preparation remains data until the application is reached.
pub struct TerminalApplicationCustody<'a> {
    entry: Option<Result<PreparedNominalRoot, Vec<Diagnostic>>>,
    entry_identity: Option<[u8; 32]>,
    composition: Option<PreparedComposition<'a>>,
}

impl<'a> TerminalApplicationCustody<'a> {
    pub fn prepare(
        entry: Result<PreparedNominalRoot, Vec<Diagnostic>>,
        entry_identity: Option<[u8; 32]>,
        composition: Option<TerminalCompositionRoots<'a>>,
    ) -> Self {
        if entry_identity.is_none() && composition.is_none() {
            return Self {
                entry: None,
                entry_identity,
                composition: None,
            };
        }
        let composition = composition.map(|composition| PreparedComposition {
            callbacks: composition.callbacks,
            adapter: composition.physical_adapter,
        });
        Self {
            entry: Some(entry),
            entry_identity,
            composition,
        }
    }

    /// Attach exact root correspondences and the optional Unit entry receipt to
    /// the final artifact. Root failures remain ordered ahead of receipt failure.
    pub(crate) fn attach(
        &self,
        view: terminal_codec::TerminalExecutionView<'_>,
        artifact: Result<
            &terminal_codec::CanonicalTerminalArtifact,
            terminal_codec::CanonicalTerminalArtifactError,
        >,
        provenance: &TerminalMachineProvenance,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<ProducedTerminalApplication, Vec<Diagnostic>> {
        let Some(entry) = &self.entry else {
            return Ok(ProducedTerminalApplication {
                receipt: Ok(None),
                roots: Vec::new(),
            });
        };
        let entry = entry.as_ref().map_err(Clone::clone)?;
        let module = view.module();
        let mut roots = Vec::new();
        if let Some(composition) = &self.composition {
            roots.push(project_root(module, provenance, entry));
            if let Some(callbacks) = composition.callbacks {
                roots.extend(callbacks.retained_roots(module).map(|root| {
                    let root = root.map_err(|reason| {
                        vec![Diagnostic::error(format!(
                            "callback composition root: {reason}"
                        ))]
                    })?;
                    Ok(TerminalAdmissionRoot {
                        machine: root.machine,
                        provenance: provenance.for_root(
                            module,
                            root.machine,
                            root.name.map(|name| (name, root.span)),
                        ),
                    })
                }));
            }
            if let Some(adapter) = &composition.adapter {
                roots.push(project_root(module, provenance, adapter));
            }
        }
        let receipt = match self.entry_identity {
            Some(identity) => observer.record_result(PsiCompilationStage::EntryReceipt, || {
                let artifact =
                    artifact.map_err(|error| vec![Diagnostic::error(error.to_string())])?;
                prepare_program_entry_receipt(entry, identity, artifact)
                    .map(Some)
                    .map_err(|error| {
                        TerminalArtifactProductionError::EntryReceipt(error).diagnostics()
                    })
            }),
            None => Ok(None),
        };
        Ok(ProducedTerminalApplication { receipt, roots })
    }
}

fn project_root(
    module: &terminal_psi::TerminalModule,
    provenance: &TerminalMachineProvenance,
    declaration: &PreparedNominalRoot,
) -> Result<TerminalAdmissionRoot, Vec<Diagnostic>> {
    let entry = declaration.resolve(module).map_err(|reason| {
        TerminalArtifactProductionError::EntryReceipt(reason.into()).diagnostics()
    })?;
    Ok(TerminalAdmissionRoot {
        machine: entry.id,
        provenance: provenance.for_root(
            module,
            entry.id,
            declaration
                .diagnostic_name()
                .map(|(name, span)| (name, span.or_else(|| provenance.source_span(entry.id)))),
        ),
    })
}
