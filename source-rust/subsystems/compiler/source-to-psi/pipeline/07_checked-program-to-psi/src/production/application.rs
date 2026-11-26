//! Observe the entry-receipt check when publishing an application from source.
use crate::{TerminalAdmissionRoot, TerminalApplicationCustody};
use artifacts::compile_timings::{CompileTimings, source as stages};
use diagnostics::Diagnostic;
use psi_publication_records::{CheckedProgramEntryTerminalReceipt, TerminalMachineProvenance};
use terminal_codec::{
    CanonicalTerminalArtifact, CanonicalTerminalArtifactError, TerminalExecutionView,
};

#[derive(Debug, PartialEq, Eq, record_storage::Record)]
pub struct ProducedTerminalApplication {
    pub receipt: Result<Option<CheckedProgramEntryTerminalReceipt>, Vec<Diagnostic>>,
    pub roots: Vec<Result<TerminalAdmissionRoot, Vec<Diagnostic>>>,
}

pub(super) fn attach(
    application: &TerminalApplicationCustody<'_>,
    view: TerminalExecutionView<'_>,
    artifact: Result<&CanonicalTerminalArtifact, CanonicalTerminalArtifactError>,
    provenance: &TerminalMachineProvenance,
    timings: &mut CompileTimings,
) -> Result<ProducedTerminalApplication, Vec<Diagnostic>> {
    let attached = application.attach(view, artifact, provenance)?;
    let receipt = attached
        .entry_receipt
        .map(|request| timings.record_result(stages::ENTRY_RECEIPT, || request.check()))
        .transpose();
    Ok(ProducedTerminalApplication {
        receipt,
        roots: attached.roots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_requested_entry_receipt_records_a_compilation_span() {
        let mut module = terminal_psi::TerminalModule::declarations();
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::for_invocation();
        identities.intern_invocation_local("local_entry").unwrap();
        module.identities = identities.build();
        let code = terminal_codec::InvocationTerminalCode::new(module).unwrap();
        let provenance = TerminalMachineProvenance::default();
        let mut timings = CompileTimings::enabled();
        for identity in [None, Some([4; 32])] {
            let custody = TerminalApplicationCustody::prepare(
                Ok(terminal_psi::PreparedNominalRoot::new("Root", "Root", None).unwrap()),
                identity,
                None,
            );
            let application = attach(
                &custody,
                code.view(),
                Err(CanonicalTerminalArtifactError::Publication(
                    "local only".into(),
                )),
                &provenance,
                &mut timings,
            )
            .unwrap();
            assert!(application.roots.is_empty());
            if identity.is_some() {
                assert!(
                    application.receipt.unwrap_err()[0]
                        .message
                        .contains("local only")
                );
                assert_eq!(timings.phases().len(), 1);
                assert_eq!(timings.phases()[0].phase, stages::ENTRY_RECEIPT.label());
            } else {
                assert!(application.receipt.unwrap().is_none());
                assert!(timings.phases().is_empty());
            }
        }
    }
}
