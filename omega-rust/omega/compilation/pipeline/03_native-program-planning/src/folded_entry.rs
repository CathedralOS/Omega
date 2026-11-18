//! A target-specific canonical artifact commits the exact providers used by scalar evaluation.
use std::sync::Arc;
use terminal_codec::{CanonicalTerminalArtifact, EvidenceSeals};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldedTargetProgram {
    original: terminal_psi::TerminalPsiIdentity,
    artifact: Arc<CanonicalTerminalArtifact>,
    evidence: Option<Arc<EvidenceSeals>>,
}
impl FoldedTargetProgram {
    pub(crate) fn close(
        original: &CanonicalTerminalArtifact,
        accepted_evidence: Option<&EvidenceSeals>,
        folded: terminal_psi::FoldedConstantClosure,
    ) -> Result<Self, String> {
        if original.semantic_module().folded_constants.target.is_some()
            || !original.optimization().selections().is_empty()
        {
            return Err("folded constant custody requires an unbound portable product".into());
        }
        let prior = original.view();
        folded.validate(prior.module()).map_err(str::to_owned)?;
        let mut module = prior.module().clone();
        module.folded_constants = folded;
        let debug = original
            .debug_bytes()
            .map(|bytes| terminal_codec::decode_debug_map(prior.module(), bytes))
            .transpose()
            .map_err(|error| error.to_string())?;
        let closed =
            terminal_codec::ClosedTerminalModule::new(module).map_err(|error| error.to_string())?;
        let evidence = accepted_evidence
            .map(|evidence| {
                if evidence.module != prior.identity() {
                    return Err("accepted evidence belongs to another portable program".to_owned());
                }
                Ok(Arc::new(closed.evidence_seals(evidence.seals.clone())))
            })
            .transpose()?;
        let identity = closed.identity();
        let record = terminal_codec::PsiOptimizationExecutionRecord::new(
            optimization::PsiOptimizationSelections::default(),
            identity,
            identity,
        )
        .map_err(|error| format!("{error:?}"))?;
        let artifact = CanonicalTerminalArtifact::from_closed_parts(closed, record, debug.as_ref())
            .map_err(|error| error.to_string())?;
        Ok(Self {
            original: prior.identity(),
            artifact: Arc::new(artifact),
            evidence,
        })
    }
    pub fn original(&self) -> terminal_psi::TerminalPsiIdentity {
        self.original
    }
    pub fn artifact(&self) -> &Arc<CanonicalTerminalArtifact> {
        &self.artifact
    }
    pub fn evidence(&self) -> Option<&Arc<EvidenceSeals>> {
        self.evidence.as_ref()
    }
}
