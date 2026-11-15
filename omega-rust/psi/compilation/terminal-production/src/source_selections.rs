//! Keep accepted source selections attached to the code they describe.
//! Decoding a program never recreates this source-issued companion.

use semantic_vocabulary::SelectionQuestionId;
use semantic_vocabulary::source_selections::SelectionQuestions;
use std::sync::Arc;
use terminal_codec::{TerminalExecutionIdentity, TerminalExecutionView};
use terminal_installation::selection_questions::{
    InvocationSelectionConsent, SelectionAuthorities, admit_selection_questions_with_consent,
};

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalSourceSelections {
    identity: TerminalExecutionIdentity,
    questions: Arc<SelectionQuestions>,
    invocation_consent: Vec<SelectionQuestionId>,
}

impl TerminalSourceSelections {
    pub fn declaration_custody(
        &self,
        artifact: TerminalExecutionView<'_>,
        declaration: &str,
        purpose: semantic_vocabulary::source_selections::SelectionPurpose,
    ) -> Result<semantic_vocabulary::compiler_evaluation::DeclarationCustody, String> {
        let questions = self
            .questions_for(artifact)
            .ok_or("source declaration custody belongs to another program or invocation")?;
        let mut custody = None;
        for (_, source) in questions
            .declarations
            .iter()
            .filter(|(_, source)| source.identity == declaration && source.purpose == purpose)
        {
            if custody
                .replace(source.custody)
                .is_some_and(|previous| previous != source.custody)
            {
                return Err("selected declaration has conflicting source package custody".into());
            }
        }
        custody.ok_or_else(|| "selected declaration has no authenticated source custody".into())
    }

    pub(crate) fn new(
        artifact: TerminalExecutionView<'_>,
        questions: SelectionQuestions,
        invocation_consent: Vec<SelectionQuestionId>,
    ) -> Self {
        Self {
            identity: artifact.custody_identity(),
            questions: Arc::new(questions),
            invocation_consent,
        }
    }

    pub fn questions_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&SelectionQuestions> {
        (self.identity == artifact.custody_identity()).then_some(self.questions.as_ref())
    }

    pub fn admit(
        &self,
        artifact: TerminalExecutionView<'_>,
        authorities: &SelectionAuthorities<'_>,
    ) -> Result<(), String> {
        let questions = self
            .questions_for(artifact)
            .ok_or("source selections belong to a different program or invocation")?;
        admit_selection_questions_with_consent(artifact, questions, authorities, Some(self))
    }
}

impl InvocationSelectionConsent for TerminalSourceSelections {
    fn allows_question(
        &self,
        artifact: TerminalExecutionView<'_>,
        question: SelectionQuestionId,
    ) -> bool {
        self.identity == artifact.custody_identity()
            && self.invocation_consent.binary_search(&question).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation_code() -> terminal_codec::InvocationTerminalCode {
        let mut module = terminal_psi::TerminalModule::declarations();
        module.identities = terminal_psi::TerminalIdentityTableBuilder::for_invocation().build();
        terminal_codec::InvocationTerminalCode::new(module).unwrap()
    }

    #[test]
    fn equal_code_in_another_invocation_cannot_reuse_source_selections() {
        let first = invocation_code();
        let second = invocation_code();
        assert_eq!(first.view().identity(), second.view().identity());
        let selections =
            TerminalSourceSelections::new(first.view(), SelectionQuestions::default(), Vec::new());
        assert!(
            selections
                .questions_for(first.retain_code().view())
                .is_some()
        );
        assert!(selections.questions_for(second.view()).is_none());
        assert!(
            selections
                .admit(
                    second.view(),
                    &SelectionAuthorities {
                        build: None,
                        product: None
                    }
                )
                .is_err()
        );
    }

    #[test]
    fn captured_build_consent_stays_with_its_source_and_invocation() {
        struct Capture(Arc<source::SourceMap>);
        impl source::InvocationSourceConsent for Capture {
            fn allows_selection(
                &self,
                sources: &Arc<source::SourceMap>,
                requester: source::SourceSpan,
                declaration: source::SourceSpan,
                purpose: source::DependencyScope,
            ) -> bool {
                Arc::ptr_eq(&self.0, sources)
                    && purpose == source::DependencyScope::Build
                    && [requester, declaration].into_iter().all(|span| {
                        sources.get(span.source_id).is_some_and(|file| {
                            file.dependency_scope == purpose
                                && file.package_identity.is_none()
                                && span.span.start <= span.span.end
                                && span.span.end <= file.source.len()
                        })
                    })
            }
        }

        let text = "machine helper() { } machine build() { helper(); }";
        let mut sources = source::SourceMap::default();
        let id = sources
            .add_checked_instance(
                "/captured/build.omg".into(),
                text,
                "/captured".into(),
                None,
                source::SourceOrigin::User,
                source::SourceResolutionStratum::Base,
                source::DependencyScope::Build,
            )
            .source_id;
        let sources = Arc::new(sources);
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
        request.sources = Some(Arc::clone(&sources));
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let entry = typed.realized_machine_named("build").unwrap().symbol;
        let (frozen, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            typed,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .unwrap();
        let accepted = crate::AcceptedTerminalSource::judge(
            Arc::new(frozen),
            crate::TerminalSourceCompilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[None],
                purpose: source::DependencyScope::Build,
                selected: &[],
                calling: None,
                providers: None,
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .unwrap();
        let authorities = SelectionAuthorities {
            build: None,
            product: None,
        };
        let produce = |consent: Option<&dyn source::InvocationSourceConsent>| {
            accepted
                .produce(
                    crate::TerminalProductionCustody {
                        invocation_sources: consent,
                        ..Default::default()
                    },
                    &mut crate::UnmeasuredProduction,
                )
                .unwrap()
        };
        let ordinary = produce(None);
        assert!(
            ordinary
                .source_selections()
                .unwrap()
                .admit(ordinary.view(), &authorities)
                .is_err()
        );
        let captured = Capture(Arc::clone(&sources));
        let produced = produce(Some(&captured));
        let selections = produced.source_selections().unwrap();
        selections.admit(produced.view(), &authorities).unwrap();
        let helper = accepted.source().realized_machine_named("helper").unwrap();
        let helper_machine = accepted.machine_of(helper.symbol).unwrap();
        let helper_identity = produced
            .view()
            .module()
            .machine(helper_machine)
            .unwrap()
            .declaration
            .as_ref()
            .unwrap()
            .semantic_declaration;
        let helper_identity = produced
            .view()
            .module()
            .identities
            .text(helper_identity)
            .unwrap();
        let mut helper_rows = selections.questions.declarations.iter().filter(|(_, row)| {
            row.identity == helper_identity
                && row.purpose == semantic_vocabulary::source_selections::SelectionPurpose::Build
        });
        let (helper_declaration, _) = helper_rows.next().unwrap();
        assert!(
            helper_rows.next().is_none(),
            "entry and machine share one declaration row"
        );
        for role in [
            semantic_vocabulary::source_selections::SelectionRole::Declaration,
            semantic_vocabulary::source_selections::SelectionRole::Executable,
        ] {
            assert!(
                selections.questions.questions.values().any(|row| {
                    row.selected
                        == semantic_vocabulary::source_selections::SelectionReference::Declaration(
                            helper_declaration,
                        )
                        && row.role == role
                }),
                "the raw selection and physical invocation rejoin one helper declaration"
            );
        }
        assert!(!selections.invocation_consent.is_empty());
        assert!(
            selections
                .questions
                .declarations
                .values()
                .all(|declaration| {
                    declaration.custody
                        == semantic_vocabulary::compiler_evaluation::DeclarationCustody::UnownedUser
                })
        );
        let foreign = Capture(Arc::new(sources.as_ref().clone()));
        let denied = produce(Some(&foreign));
        assert!(
            denied
                .source_selections()
                .unwrap()
                .admit(denied.view(), &authorities)
                .is_err()
        );

        let other = invocation_code();
        assert!(!selections.allows_question(other.view(), selections.invocation_consent[0]));
    }
}
