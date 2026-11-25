//! Keep accepted source selections attached to the code they describe.
//! Decoding a program never recreates this source-issued companion.

use psi_authorization::selection_questions::{
    InvocationSelectionConsent, SelectionAuthorities, admit_selection_questions_with_consent,
};
use semantic_vocabulary::source_selections::SelectionQuestions;
use semantic_vocabulary::{
    CallableInvocationSite, MachineId, SelectionQuestionId, TargetMachineCallable,
};
use std::sync::Arc;
use terminal_codec::{TerminalExecutionIdentity, TerminalExecutionView};

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalSourceSelections {
    identity: TerminalExecutionIdentity,
    questions: Arc<SelectionQuestions>,
    invocation_consent: Vec<SelectionQuestionId>,
    captured_entry: Option<TargetMachineCallable>,
}

impl TerminalSourceSelections {
    pub(crate) fn new(
        artifact: TerminalExecutionView<'_>,
        questions: SelectionQuestions,
        invocation_consent: Vec<SelectionQuestionId>,
        captured_entry: Option<TargetMachineCallable>,
    ) -> Self {
        Self {
            identity: artifact.custody_identity(),
            questions: Arc::new(questions),
            invocation_consent,
            captured_entry,
        }
    }

    pub fn questions_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&SelectionQuestions> {
        (self.identity == artifact.custody_identity()).then_some(self.questions.as_ref())
    }

    /// Discharge the captured-source choices made reachable by the installed
    /// Build providers. A direct call needs its own source occurrence. A machine
    /// value needs an actual descriptor constructed by that caller for that body.
    pub fn admit_captured_execution(
        &self,
        artifact: TerminalExecutionView<'_>,
        entry: Option<TargetMachineCallable>,
        calls: &[(
            MachineId,
            Option<CallableInvocationSite>,
            TargetMachineCallable,
        )],
    ) -> Result<(), String> {
        use semantic_vocabulary::source_selections::{
            SelectionCallTarget, SelectionJoin, SelectionReference,
        };
        let packet = self
            .questions_for(artifact)
            .ok_or("captured Build selections belong to another invocation")?;
        if entry.is_some() && (entry != self.captured_entry || entry != artifact.module().entry) {
            return Err("Build entry has no consent from this source capture".into());
        }
        if calls.is_empty() {
            return Ok(());
        }
        let mut admitted = Vec::new();
        for join in &packet.joins {
            let (question, caller, site, target) = match join {
                SelectionJoin::Invocation {
                    question,
                    machine,
                    site,
                    target: SelectionCallTarget::Declared(target),
                    ..
                } => (*question, *machine, Some(*site), *target),
                SelectionJoin::Descriptor {
                    question,
                    machine,
                    descriptor,
                    ..
                } => {
                    let value = artifact
                        .module()
                        .execution
                        .callables
                        .descriptor(*descriptor)
                        .ok_or("captured Build descriptor has no emitted declaration")?;
                    (
                        *question,
                        *machine,
                        None,
                        TargetMachineCallable::Machine(value.machine),
                    )
                }
                _ => continue,
            };
            let row = packet
                .questions
                .get(question)
                .ok_or("captured Build call lost its source question")?;
            if matches!(row.selected, SelectionReference::Declaration(_))
                && self.invocation_consent.binary_search(&question).is_ok()
            {
                admitted.push((caller, site, target));
            }
        }
        admitted.sort_unstable();
        if let Some(missing) = calls
            .iter()
            .find(|call| admitted.binary_search(call).is_err())
        {
            return Err(format!(
                "captured Build call has no source consent for its exact occurrence and selected declaration: {missing:?}"
            ));
        }
        Ok(())
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

    fn allows_supplied_library(
        &self,
        artifact: TerminalExecutionView<'_>,
        packet: &SelectionQuestions,
        question: SelectionQuestionId,
    ) -> bool {
        // An owned target enters this list only after the source producer checked
        // the actual requester and selected declaration against a supplied library.
        std::ptr::eq(self.questions.as_ref(), packet)
            && self.allows_question(artifact, question)
            && self.questions.questions.get(question).is_some_and(|row| {
                let semantic_vocabulary::source_selections::SelectionReference::Declaration(target) = &row.selected else { return false; };
                self.questions.declarations.get(*target).is_some_and(|target| matches!(target.custody, semantic_vocabulary::compiler_evaluation::DeclarationCustody::Package(_)))
            })
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
        let selections = TerminalSourceSelections::new(
            first.view(),
            SelectionQuestions::default(),
            Vec::new(),
            None,
        );
        assert!(
            selections
                .questions_for(first.retain_code().view())
                .is_some()
        );
        assert!(selections.questions_for(second.view()).is_none());
        assert!(
            selections
                .admit_captured_execution(second.view(), None, &[])
                .is_err()
        );
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
    fn ordinary_overloads_keep_distinct_source_selection_declarations() {
        // Both calls must retain the overload they selected, including when
        // their declarations come from the same package or toolchain source.
        let text = "public machine choose(value: u32) {} \
                    public machine choose(value: bool) {} \
                    machine main() { choose(1u32); choose(true); }";
        for package in [
            None,
            semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]),
        ] {
            let mut sources = crate::source_inputs::SourceMap::default();
            let id = sources
                .add_checked_instance(
                    "/overloads/main.omg".into(),
                    text,
                    "/overloads".into(),
                    package,
                    if package.is_some() {
                        source_admission::SourceOrigin::User
                    } else {
                        source_admission::SourceOrigin::Toolchain
                    },
                    source_admission::SourceResolutionStratum::Base,
                    source_provenance::DependencyScope::Product,
                )
                .source_id;
            let tokens = crate::frontend_fixture::Lexer::new(text)
                .tokenize()
                .unwrap();
            let syntax =
                crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens)
                    .unwrap();
            let mut request = crate::resolution::ResolutionRequest::new(syntax);
            request.sources = Some(Arc::new(sources));
            let resolved = crate::resolution::resolve(request).unwrap();
            let typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
            let identities = typed
                .machines()
                .iter()
                .filter(|machine| machine.name.as_str() == "choose")
                .map(|machine| {
                    typed
                        .normalized_hermetic_symbol_identity(machine.symbol)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(identities.len(), 2);
            assert_ne!(identities[0], identities[1]);
            let entry = typed.realized_machine_named("main").unwrap().symbol;
            let frozen = crate::preparation::prepare_typed_trees(typed, &[], &[]).unwrap();
            let accepted = crate::judge_fixture(
                frozen,
                crate::TerminalSourceCompilation {
                    declaration_metadata: &[],
                    entries: &[entry],
                    subjects: &[None],
                    purpose: source_provenance::DependencyScope::Product,
                    selected: &[],
                    calling: None,
                    providers: None,
                    bindings: &[],
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Invocation,
            )
            .unwrap();
            let produced = accepted
                .produce(
                    Default::default(),
                    &mut artifacts::compile_timings::CompileTimings::default(),
                )
                .unwrap();
            let questions = &produced.source_selections().unwrap().questions;
            psi_authorization::selection_questions::validate_selection_questions(
                produced.view().module(),
                questions,
            )
            .unwrap();
            let overloads = questions
                .declarations
                .iter()
                .filter(|(_, declaration)| declaration.name == "choose")
                .collect::<Vec<_>>();
            assert_eq!(overloads.len(), 2);
            assert_ne!(overloads[0].1.identity, overloads[1].1.identity);
            let mut duplicate = questions.as_ref().clone();
            duplicate
                .declarations
                .get_mut(overloads[1].0)
                .unwrap()
                .identity = overloads[0].1.identity.clone();
            assert!(
                psi_authorization::selection_questions::validate_selection_questions(
                    produced.view().module(),
                    &duplicate,
                )
                .unwrap_err()
                .contains("repeats a declaration")
            );
        }
    }

    #[test]
    fn captured_build_consent_stays_with_its_source_and_invocation() {
        struct Capture(Arc<crate::source_inputs::SourceMap>);
        impl crate::source_inputs::InvocationSourceConsent for Capture {
            fn allows_selection(
                &self,
                sources: &Arc<crate::source_inputs::SourceMap>,
                requester: source_text::SourceSpan,
                declaration: source_text::SourceSpan,
                purpose: source_provenance::DependencyScope,
            ) -> bool {
                Arc::ptr_eq(&self.0, sources)
                    && purpose == source_provenance::DependencyScope::Build
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

        let text = "const CHOSEN: bool = false; machine helper(value: bool) { } machine build() { helper(CHOSEN); }";
        let mut sources = crate::source_inputs::SourceMap::default();
        let id = sources
            .add_checked_instance(
                "/captured/build.omg".into(),
                text,
                "/captured".into(),
                None,
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                source_provenance::DependencyScope::Build,
            )
            .source_id;
        let sources = Arc::new(sources);
        let tokens = crate::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        let syntax =
            crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let mut request = crate::resolution::ResolutionRequest::new(syntax);
        request.sources = Some(Arc::clone(&sources));
        let resolved = crate::resolution::resolve(request).unwrap();
        let typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
        let entry = typed.realized_machine_named("build").unwrap().symbol;
        let frozen = crate::preparation::prepare_typed_trees(typed, &[], &[]).unwrap();
        let accepted = crate::judge_fixture(
            frozen,
            crate::TerminalSourceCompilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[None],
                purpose: source_provenance::DependencyScope::Build,
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
        let produce = |consent: Option<&dyn crate::source_inputs::InvocationSourceConsent>| {
            accepted
                .produce(
                    crate::TerminalProductionCustody {
                        invocation_sources: consent,
                        ..Default::default()
                    },
                    &mut artifacts::compile_timings::CompileTimings::default(),
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
        let mut constants = selections
            .questions
            .declarations
            .iter()
            .filter(|(_, row)| row.name == "CHOSEN");
        let (constant, _) = constants
            .next()
            .expect("the folded constant remains a source selection");
        assert!(constants.next().is_none());
        let (question, _) = selections
            .questions
            .questions
            .iter()
            .find(|(_, row)| {
                row.selected
                    == semantic_vocabulary::source_selections::SelectionReference::Declaration(
                        constant,
                    )
            })
            .expect("the source occurrence selects its captured constant");
        assert!(selections.allows_question(produced.view(), question));
        let view = produced.view();
        let root = view.module().entry.unwrap();
        let providers =
            psi_provider_selections::admit_provider_execution_installation(view, &[], &[]).unwrap();
        let closure = evaluation_admission::admit_declaration_selections(
            view, None, root, &providers, None, None,
        )
        .unwrap();
        assert_eq!(closure.entry, Some(root));
        assert_eq!(closure.calls.len(), 1);
        selections
            .admit_captured_execution(view, closure.entry, &closure.calls)
            .unwrap();
        let (caller, site, target) = closure.calls[0];
        assert!(site.is_some());
        // Naming the helper at this call did not construct a callable descriptor.
        assert!(
            selections
                .admit_captured_execution(view, closure.entry, &[(caller, None, target)])
                .is_err()
        );
        assert!(
            selections
                .admit_captured_execution(view, Some(target), &closure.calls)
                .is_err()
        );
        assert!(
            ordinary
                .source_selections()
                .unwrap()
                .admit_captured_execution(ordinary.view(), Some(root), &closure.calls)
                .is_err()
        );
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
            !denied
                .source_selections()
                .unwrap()
                .allows_question(denied.view(), question)
        );
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
