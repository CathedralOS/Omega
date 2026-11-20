//! Supply the scalar applications read by the target execution selected in stage 01.

use diagnostics::Diagnostic;
use native_scalar_inputs::EvaluationReceipt;
use psi_scalar_applications::AdmittedScalarApplications;
use std::{borrow::Cow, sync::Arc};
use verified_optimization_input::TerminalNativeRealizationProposal;

pub enum ScalarInputs<'input> {
    Evaluate(&'input mut Vec<EvaluationReceipt>),
    Retained(Option<&'input AdmittedScalarApplications>),
}

impl<'input> ScalarInputs<'input> {
    pub(super) fn resolve(
        &mut self,
        artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
        proposal: &TerminalNativeRealizationProposal,
        demand: &[semantic_vocabulary::ScalarApplicationId],
    ) -> Result<Option<Cow<'input, AdmittedScalarApplications>>, Vec<Diagnostic>> {
        if !verified_optimization_input::requires_scalar_values(artifact, demand) {
            return Ok(None);
        }
        match self {
            Self::Evaluate(receipts) => {
                let values = native_scalar_inputs::complete(
                    artifact,
                    proposal.target_profile().target_name(),
                    demand,
                    proposal.target_provider_proposals(),
                    *receipts,
                )
                .map_err(|reason| {
                    vec![Diagnostic::error(format!("constant checking: {reason}"))]
                })?;
                Ok(Some(Cow::Owned(values)))
            }
            Self::Retained(values) => {
                let values = values.ok_or_else(|| vec![Diagnostic::error(
                    "native compilation needs the scalar values its program reads, completed before it starts",
                )])?;
                Ok(Some(Cow::Borrowed(values)))
            }
        }
    }
}
