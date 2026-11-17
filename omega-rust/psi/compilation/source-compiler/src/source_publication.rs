// The source owner judges and publishes one exact root roster. Policy and
// Product consumers share its immutable portable code. `SourcePublication` fixes Psi optimization choices before
// targets settle and memoizes the produced `PortableTerminalProgram` (or its
// refusal) in a `OnceLock` without retrying or permitting a different Psi
// optimization projection to replace it.

use diagnostics::Diagnostic;
use optimization::PsiOptimizationSelections;
use crate::{PortableTerminalProgram, SourceCompilationRoster};
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
            .map(|(accepted, _)| accepted.as_ref())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "no product route yet: this root selection has no accepted source",
                )]
            })
    }

    /// Attach target-specific dependency commitments to already accepted code.
    /// No source preparation or judgment runs here, and the shared source result
    /// remains unchanged for other targets.
    pub fn with_folded_constants(
        &self,
        folded: terminal_psi::FoldedConstantClosure,
    ) -> Result<Self, Vec<Diagnostic>> {
        let program = self.require_published(&self.optimizations)?;
        let derived = program.with_folded_constants(folded)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok(Self {
            optimizations: self.optimizations.clone(),
            program: OnceLock::from(Ok(Arc::new(derived))),
            accepted: self.accepted.clone(),
        })
    }

    /// Publish the accepted source once, with the declaration authority admitted
    /// for this request. Consumers receive code and evidence, never a producer
    /// callback capable of replacing a failed result.
    pub fn publish(
        &self,
        optimizations: &PsiOptimizationSelections,
        authority: Option<&dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority>,
        observer: &mut impl crate::PsiCompilationObserver,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        self.retain(optimizations, || {
            let program = self.accepted()?
                .produce(Default::default(), observer)?
                .into_shared_portable()
                .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
            let packet = program.source_selections().ok_or_else(|| {
                vec![Diagnostic::error("Product publication lost its source declaration selections")]
            })?;
            packet.admit(
                program.artifact().view().into(),
                &terminal_installation::selection_questions::SelectionAuthorities {
                    build: None,
                    product: authority,
                },
            ).map_err(|reason| vec![Diagnostic::error(reason)])?;
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
        if &self.optimizations != optimizations
        {
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
        if &self.optimizations != optimizations
        {
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
    fn folded_dependencies_preserve_accepted_code_and_rebind_companions() {
        let text = "machine main() {}";
        let mut sources = crate::source_inputs::SourceMap::default();
        let id = sources.add_checked_instance(
            "/publication/main.omg".into(), text, "/publication".into(), None,
            source_admission::SourceOrigin::Toolchain, source_admission::SourceResolutionStratum::Base,
            declaration_contracts::source_identity::DependencyScope::Product,
        ).source_id;
        let tokens = crate::frontend_fixture::Lexer::new(text).tokenize().unwrap();
        let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let mut request = crate::resolution::ResolutionRequest::new(syntax);
        request.sources = Some(Arc::new(sources));
        let resolved = crate::resolution::resolve(request).unwrap();
        let typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
        let entry = typed.realized_machine_named("main").unwrap().symbol;
        let (frozen, _) = crate::preparation::prepare_typed_trees(
            typed, &crate::preparation::PreparationRequest::settled(),
        ).unwrap();
        let publication = SourcePublication::judge(
            Arc::new(frozen),
            SourceCompilationRoster { entries: vec![entry], ..Default::default() },
            PsiOptimizationSelections::default(),
        ).unwrap();
        let original = publication.publish(
            publication.optimizations(), None, &mut crate::UnmeasuredProduction,
        ).unwrap();
        let original_bytes = original.artifact().to_bytes();
        let original_view = original.artifact().view();
        let requirement = terminal_psi::TerminalIdentityIndex(
            original_view.module().identities.entries().iter()
                .position(|identity| !identity.is_empty()).unwrap() as u32,
        );
        let folded = terminal_psi::FoldedConstantClosure {
            target: Some("linux_x86_64".into()),
            dependencies: vec![terminal_psi::FoldedConstantDependency {
                target: "linux_x86_64".into(), requirement, plan: [7; 32], candidate: None,
            }],
        };
        let derived_publication = publication.with_folded_constants(folded.clone()).unwrap();
        assert!(Arc::ptr_eq(
            &publication.accepted.as_ref().unwrap().0,
            &derived_publication.accepted.as_ref().unwrap().0,
        ), "dependency publication must reuse the original source judgment");
        let derived = derived_publication.require_published(publication.optimizations()).unwrap();
        let derived_view = derived.artifact().view();
        assert_ne!(original_view.identity(), derived_view.identity());
        let mut without_dependencies = derived_view.module().clone();
        without_dependencies.folded_constants = Default::default();
        assert_eq!(&without_dependencies, original_view.module());
        assert_eq!(original.artifact().to_bytes(), original_bytes);
        assert_eq!(derived.artifact().debug_bytes(), original.artifact().debug_bytes());
        let selections = derived.source_selections().unwrap();
        assert!(selections.questions_for(derived_view.into()).is_some());
        assert!(selections.questions_for(original_view.into()).is_none());
        assert!(derived.build_bindings().calls_for(derived_view.into()).is_some());
        assert!(derived.build_bindings().calls_for(original_view.into()).is_none());
        if let Some(evidence) = derived.evidence() {
            assert_eq!(evidence.module, derived_view.identity());
            assert_eq!(evidence.seals, original.evidence().unwrap().seals);
        }
        assert!(derived_publication.with_folded_constants(folded.clone()).is_err());
        let mut incomplete = folded;
        incomplete.dependencies[0].plan = [0; 32];
        assert!(publication.with_folded_constants(incomplete).is_err());
    }

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
        let different = PsiOptimizationSelections::new([PsiOptimization::ControlFlowCleanup]).unwrap();
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
        let selected = PsiOptimizationSelections::new([PsiOptimization::ControlFlowCleanup]).unwrap();
        let publication = SourcePublication::new(selected);
        assert_eq!(
            publication.accepted().unwrap_err()[0].message,
            "no product route yet: selected Psi optimizations have no second lowered-form check"
        );
    }
}
