//! Check application roots and entry receipts against the exact current artifact.
//! This operation requires neither a source tree nor a source producer handle.

use diagnostics::Diagnostic;
use terminal_psi::CheckedProgramEntryTerminalReceipt;

#[derive(Debug, PartialEq, Eq, record_storage::Record)]
pub struct ProducedTerminalApplication {
    pub receipt: Result<Option<CheckedProgramEntryTerminalReceipt>, Vec<Diagnostic>>,
    pub roots: Vec<Result<TerminalAdmissionRoot, Vec<Diagnostic>>>,
}

pub trait EntryReceiptObserver {
    fn record_result<Value, Error>(
        &mut self,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error>;
}

pub struct UnmeasuredEntryReceipt;
impl EntryReceiptObserver for UnmeasuredEntryReceipt {
    fn record_result<Value, Error>(
        &mut self,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        operation()
    }
}

fn entry_diagnostics(
    error: terminal_codec::entry_receipt::ProgramEntryTerminalReceiptError,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "terminal-artifact production failed: EntryReceipt({error:?})"
    ))]
}

use terminal_codec::entry_receipt::prepare_program_entry_receipt;
use terminal_psi::{PreparedNominalRoot, TerminalMachineProvenance};

#[derive(Debug, PartialEq, Eq, record_storage::Record)]
pub struct TerminalAdmissionRoot {
    pub machine: semantic_vocabulary::MachineId,
    pub provenance: TerminalMachineProvenance,
}

#[derive(Clone, Copy, Default)]
pub struct TerminalCompositionRoots<'a> {
    pub callbacks: Option<&'a terminal_psi::PreparedTerminalRoots>,
    pub physical_adapter: Option<&'a PreparedNominalRoot>,
}

struct PreparedComposition<'a> {
    callbacks: Option<&'a terminal_psi::PreparedTerminalRoots>,
    adapter: Option<&'a PreparedNominalRoot>,
}

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

    pub fn attach(
        &self,
        view: terminal_codec::TerminalExecutionView<'_>,
        artifact: Result<
            &terminal_codec::CanonicalTerminalArtifact,
            terminal_codec::CanonicalTerminalArtifactError,
        >,
        provenance: &TerminalMachineProvenance,
        observer: &mut impl EntryReceiptObserver,
    ) -> Result<ProducedTerminalApplication, Vec<Diagnostic>> {
        let Some(entry) = &self.entry else {
            return Ok(ProducedTerminalApplication {
                receipt: Ok(None),
                roots: Vec::new(),
            });
        };
        let entry = entry.as_ref().map_err(Clone::clone)?;
        if let Ok(artifact) = artifact.as_ref() {
            let canonical: terminal_codec::TerminalExecutionView<'_> = artifact.view().into();
            if canonical.custody_identity() != view.custody_identity() {
                return Err(vec![Diagnostic::error(
                    "application roots and receipt belong to different artifacts or invocations",
                )]);
            }
        }

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
            Some(identity) => observer.record_result(|| {
                let artifact =
                    artifact.map_err(|error| vec![Diagnostic::error(error.to_string())])?;
                prepare_program_entry_receipt(entry, identity, artifact)
                    .map(Some)
                    .map_err(entry_diagnostics)
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
    let entry = declaration
        .resolve(module)
        .map_err(|reason| entry_diagnostics(reason.into()))?;
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
