// The source owner judges and publishes one exact root roster. Policy and
// Product consumers share its immutable portable code. `SourcePublication` fixes Psi optimization choices before
// targets settle and memoizes the produced `PortableTerminalProgram` (or its
// refusal) in a `OnceLock` without retrying or permitting a different Psi
// optimization projection to replace it.

use crate::{PortableTerminalProgram, SourceCompilationRoster};
use diagnostics::Diagnostic;
use optimization::PsiOptimizationSelections;
use std::sync::{Arc, OnceLock};

pub struct SourcePublication {
    optimizations: PsiOptimizationSelections,
    program: OnceLock<Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>>>,
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

    /// Judge and retain one exact roster. A caller cannot attach unjudged
    /// supplies to an independently obtained accepted source. Construction is
    /// fresh, so a previously published artifact cannot survive a new judgment.
    pub fn judge(
        source: Arc<crate::source_state::FrozenTypedTrees>,
        roster: SourceCompilationRoster,
        optimizations: PsiOptimizationSelections,
    ) -> Result<Self, Vec<Diagnostic>> {
        let accepted = roster.judge(source)?;
        let mut publication = Self::new(optimizations);
        publication.accepted = Some((accepted, roster));
        Ok(publication)
    }

    /// Judge an executable child's descriptors before publication, preserving
    /// the original roster and every chosen supply on the same frozen owner.
    pub fn with_declaration_metadata(
        &self,
        declarations: &[symbols::SymbolHandle],
    ) -> Result<Self, Vec<Diagnostic>> {
        if self.program.get().is_some() {
            return Err(vec![Diagnostic::error(
                "declaration metadata must be judged before Terminal publication",
            )]);
        }
        let mut roster = self.roster()?.clone();
        for declaration in declarations {
            if !roster.declaration_metadata.contains(declaration) {
                roster.declaration_metadata.push(*declaration);
            }
        }
        Self::judge(
            Arc::clone(self.accepted()?.source()),
            roster,
            self.optimizations.clone(),
        )
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

    pub fn require_source_owner(
        &self,
        source: &Arc<crate::source_state::FrozenTypedTrees>,
    ) -> Result<(), Diagnostic> {
        let (accepted, _) = self.accepted.as_ref().ok_or_else(|| {
            Diagnostic::error("no product route yet: the source has no retained accepted judgment")
        })?;
        if !Arc::ptr_eq(source, accepted.source()) {
            return Err(Diagnostic::error(
                "no product route yet: product publication retains a different frozen source owner",
            ));
        }
        Ok(())
    }

    pub fn accepted(&self) -> Result<&crate::AcceptedTerminalSource, Vec<Diagnostic>> {
        self.accepted_owner().map(Arc::as_ref)
    }

    pub(crate) fn accepted_owner(
        &self,
    ) -> Result<&Arc<crate::AcceptedTerminalSource>, Vec<Diagnostic>> {
        // The factory seals the module the unoptimized check judged. A
        // nonempty selection needs the second lowered-form check, which has
        // no route yet.
        if !self.optimizations.is_empty() {
            return Err(vec![Diagnostic::error(
                "no product route yet: selected Psi optimizations have no second lowered-form check",
            )]);
        }
        self.accepted
            .as_ref()
            .map(|(accepted, _)| accepted)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "no product route yet: this root selection has no accepted source",
                )]
            })
    }

    /// Publish the accepted source once, with the declaration authority admitted
    /// for this request. Consumers receive code and evidence, never a producer
    /// callback capable of replacing a failed result.
    pub fn publish(
        &self,
        optimizations: &PsiOptimizationSelections,
        authority: Option<
            &dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority,
        >,
        observer: &mut impl crate::PsiCompilationObserver,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        self.retain(optimizations, || {
            let program = self
                .accepted()?
                .produce(Default::default(), observer)?
                .into_shared_portable()
                .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
            let packet = program.source_selections().ok_or_else(|| {
                vec![Diagnostic::error(
                    "Product publication lost its source declaration selections",
                )]
            })?;
            packet
                .admit(
                    program.artifact().view().into(),
                    &terminal_installation::selection_questions::SelectionAuthorities {
                        build: None,
                        product: authority,
                    },
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            Ok(Arc::new(program))
        })
    }

    pub fn optimizations(&self) -> &PsiOptimizationSelections {
        &self.optimizations
    }

    pub fn published(&self) -> Option<&PortableTerminalProgram> {
        self.program.get()?.as_ref().ok().map(Arc::as_ref)
    }

    pub fn require_published(
        &self,
        optimizations: &PsiOptimizationSelections,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        if &self.optimizations != optimizations {
            return Err(vec![Diagnostic::error(
                "Terminal publication differs from the Product's fixed Psi optimization selection",
            )]);
        }
        self.program.get().cloned().ok_or_else(|| {
            vec![Diagnostic::error(
                "target compilation requires the shared Terminal program to be published first",
            )]
        })?
    }

    fn retain(
        &self,
        optimizations: &PsiOptimizationSelections,
        produce: impl FnOnce() -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>>,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
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
    #[test]
    fn accepted_publication_refuses_unexecuted_psi_optimizations() {
        let selected =
            PsiOptimizationSelections::new([PsiOptimization::ControlFlowCleanup]).unwrap();
        let publication = SourcePublication::new(selected);
        assert_eq!(
            publication.accepted().unwrap_err()[0].message,
            "no product route yet: selected Psi optimizations have no second lowered-form check"
        );
    }
}
