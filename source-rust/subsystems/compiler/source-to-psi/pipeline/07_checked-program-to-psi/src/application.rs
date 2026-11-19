//! Observe the entry-receipt check when publishing an application from source.
use super::{
    PsiCompilationObserver, PsiCompilationStage, TerminalAdmissionRoot, TerminalApplicationCustody,
};
use diagnostics::Diagnostic;
use terminal_codec::{
    CanonicalTerminalArtifact, CanonicalTerminalArtifactError, TerminalExecutionView,
};
use terminal_psi::{CheckedProgramEntryTerminalReceipt, TerminalMachineProvenance};

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
    observer: &mut impl PsiCompilationObserver,
) -> Result<ProducedTerminalApplication, Vec<Diagnostic>> {
    let attached = application.attach(view, artifact, provenance)?;
    let receipt = attached
        .entry_receipt
        .map(|request| {
            observer.record_result(PsiCompilationStage::EntryReceipt, || request.check())
        })
        .transpose();
    Ok(ProducedTerminalApplication {
        receipt,
        roots: attached.roots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Observed(Vec<PsiCompilationStage>);

    impl PsiCompilationObserver for Observed {
        fn record_result<Value, Error>(
            &mut self,
            stage: PsiCompilationStage,
            operation: impl FnOnce() -> Result<Value, Error>,
        ) -> Result<Value, Error> {
            self.0.push(stage);
            operation()
        }
    }

    #[test]
    fn only_a_requested_entry_receipt_records_a_compilation_span() {
        let mut module = terminal_psi::TerminalModule::declarations();
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::for_invocation();
        identities.intern_invocation_local("local_entry").unwrap();
        module.identities = identities.build();
        let code = terminal_codec::InvocationTerminalCode::new(module).unwrap();
        let provenance = TerminalMachineProvenance::default();
        let mut observed = Observed::default();
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
                &mut observed,
            )
            .unwrap();
            assert!(application.roots.is_empty());
            if identity.is_some() {
                assert!(
                    application.receipt.unwrap_err()[0]
                        .message
                        .contains("local only")
                );
                assert_eq!(observed.0, [PsiCompilationStage::EntryReceipt]);
            } else {
                assert!(application.receipt.unwrap().is_none());
                assert!(observed.0.is_empty());
            }
        }
    }
}
