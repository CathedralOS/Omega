//! Publish one checked source roster once. Product and policy consumers share
//! the canonical Psi artifact and the source's fixed optimization choices.

use crate::SourceCompilationRoster;
use diagnostics::Diagnostic;
use optimization::PsiOptimizationSelections;
use std::sync::{Arc, OnceLock};
use terminal_codec::CanonicalTerminalArtifact;

pub struct SourcePublication {
    optimizations: PsiOptimizationSelections,
    program: OnceLock<Result<Arc<CanonicalTerminalArtifact>, Vec<Diagnostic>>>,
    accepted: Option<(Arc<crate::AcceptedTerminalSource>, SourceCompilationRoster)>,
}

impl std::fmt::Debug for SourcePublication {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourcePublication")
            .field("optimizations", &self.optimizations)
            .field("published", &self.program.get().map(Result::is_ok))
            .field("accepted", &self.accepted.is_some())
            .finish()
    }
}

impl SourcePublication {
    pub fn new(optimizations: PsiOptimizationSelections) -> Self {
        Self {
            optimizations,
            program: OnceLock::new(),
            accepted: None,
        }
    }

    /// The checker returns the exact lowered roster alongside its own verdicts.
    pub fn from_judgment(
        judgment: checked_program::CheckedPublication<
            lowered_source_publication::LoweredSourcePublication,
        >,
    ) -> Result<Self, Vec<Diagnostic>> {
        let (accepted, roster) = crate::AcceptedTerminalSource::accept_judgment(judgment)?;
        let roster = roster.ok_or_else(|| {
            vec![Diagnostic::error(
                "source publication requires the checked product roster",
            )]
        })?;
        let optimizations = accepted
            .closure()
            .optimization()
            .map(|record| record.selections().clone())
            .unwrap_or_default();
        let mut publication = Self::new(optimizations);
        publication.accepted = Some((Arc::new(accepted), roster));
        Ok(publication)
    }

    /// Read the original roster for a fresh judgment before any publication attempt.
    pub fn unpublished_roster(&self) -> Result<&SourceCompilationRoster, Vec<Diagnostic>> {
        if self.program.get().is_some() {
            return Err(vec![Diagnostic::error(
                "declaration metadata must be judged before Terminal publication",
            )]);
        }
        self.roster()
    }

    /// The roster the retained judgment accepted.
    pub fn roster(&self) -> Result<&SourceCompilationRoster, Vec<Diagnostic>> {
        self.accepted
            .as_ref()
            .map(|(_, roster)| roster)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "no product route yet: this root selection has no accepted source",
                )]
            })
    }

    pub fn accepted(&self) -> Result<&crate::AcceptedTerminalSource, Vec<Diagnostic>> {
        self.accepted_owner().map(Arc::as_ref)
    }

    pub(crate) fn accepted_owner(
        &self,
    ) -> Result<&Arc<crate::AcceptedTerminalSource>, Vec<Diagnostic>> {
        self.accepted
            .as_ref()
            .map(|(accepted, _)| accepted)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "no product route yet: this root selection has no accepted source",
                )]
            })
    }

    /// Check the package selections against the module the source judgment owns.
    /// This stopping point needs neither a portable envelope nor a native target.
    pub fn check_selections(
        &self,
        authority: Option<&dyn declaration_scope::DeclarationSelectionAuthority>,
        timings: &mut artifacts::compile_timings::CompileTimings,
    ) -> Result<(), Vec<Diagnostic>> {
        let accepted = self.accepted()?;
        timings.record_result(artifacts::compile_timings::source::CHECK_SELECTIONS, || {
            let module = accepted.module();
            let selections = crate::source::selection_publication::prepare(accepted, module, None)
                .map_err(|diagnostic| vec![diagnostic])?;
            psi_authorization::selection_questions::admit_selection_questions(
                module,
                &selections.questions,
                &psi_authorization::selection_questions::SelectionAuthorities {
                    build: None,
                    product: authority,
                },
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])
        })
    }

    /// Publish the accepted source once, with the declaration authority admitted
    /// for this request. Consumers receive code and evidence, never a producer
    /// callback capable of replacing a failed result.
    pub fn publish(
        &self,
        optimizations: &PsiOptimizationSelections,
        authority: Option<&dyn declaration_scope::DeclarationSelectionAuthority>,
        timings: &mut artifacts::compile_timings::CompileTimings,
    ) -> Result<Arc<CanonicalTerminalArtifact>, Vec<Diagnostic>> {
        self.retain(optimizations, || {
            let produced = self.accepted()?.produce(Default::default(), timings)?;
            let artifact = produced
                .artifact()
                .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
            let packet = produced.source_selections().ok_or_else(|| {
                vec![Diagnostic::error(
                    "Product publication lost its source declaration selections",
                )]
            })?;
            packet
                .admit(
                    artifact.view().into(),
                    &psi_authorization::selection_questions::SelectionAuthorities {
                        build: None,
                        product: authority,
                    },
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let artifact = produced
                .into_portable_publication()
                .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
            Ok(Arc::new(artifact))
        })
    }

    pub fn optimizations(&self) -> &PsiOptimizationSelections {
        &self.optimizations
    }

    pub(crate) fn require_published(&self) -> Result<&CanonicalTerminalArtifact, Vec<Diagnostic>> {
        self.program
            .get()
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "target compilation requires the shared Terminal program to be published first",
                )]
            })?
            .as_deref()
            .map_err(Clone::clone)
    }

    fn retain(
        &self,
        optimizations: &PsiOptimizationSelections,
        produce: impl FnOnce() -> Result<Arc<CanonicalTerminalArtifact>, Vec<Diagnostic>>,
    ) -> Result<Arc<CanonicalTerminalArtifact>, Vec<Diagnostic>> {
        if &self.optimizations != optimizations {
            return Err(vec![Diagnostic::error(
                "Terminal publication differs from the Product's fixed Psi optimization selection",
            )]);
        }
        self.program.get_or_init(produce).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use optimization::PsiOptimization;

    #[test]
    fn publication_keeps_its_projection_and_one_refusal_without_retry() {
        let publication = SourcePublication::new(PsiOptimizationSelections::default());
        let mut attempts = 0;
        for _ in 0..2 {
            let result = publication.retain(&PsiOptimizationSelections::default(), || {
                attempts += 1;
                Err(vec![Diagnostic::error("fixed source refusal")])
            });
            assert_eq!(result.unwrap_err()[0].message, "fixed source refusal");
        }
        assert_eq!(attempts, 1);
        let different =
            PsiOptimizationSelections::new([PsiOptimization::ControlFlowCleanup]).unwrap();
        let error = publication
            .retain(&different, || {
                attempts += 1;
                Err(Vec::new())
            })
            .unwrap_err();
        assert!(
            error[0]
                .message
                .contains("fixed Psi optimization selection")
        );
        assert_eq!(
            attempts, 1,
            "a different projection cannot replace published code or its refusal"
        );
    }
}
