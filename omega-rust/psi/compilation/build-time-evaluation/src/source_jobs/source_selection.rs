//! Join captured-source selection consent to the actual invocation's retained
//! source correspondence. Nothing here gives unowned declarations an owner.
//! One shared capture consent is borrowed; the sorted lookup is admission
//! scratch over this module's real declarations, not a saved authority record.

use std::sync::Arc;

use source::{SourceMap, SourceSpan};
use terminal_psi::{ProofCallableAssociation, ProofDeclarationId, TargetMachineCallable};

use super::PreparedBuildMachineEntry;

pub(super) struct InvocationSelection<'a> {
    entry: &'a PreparedBuildMachineEntry,
    identity: terminal_codec::TerminalExecutionIdentity,
    sources: Arc<SourceMap>,
    requester: SourceSpan,
    declarations: Vec<(ProofDeclarationId, SourceSpan)>,
}

impl<'a> InvocationSelection<'a> {
    pub(super) fn new(
        entry: &'a PreparedBuildMachineEntry,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Option<Self> {
        entry.source_consent.as_ref()?;
        let invocation = artifact.module().identities.invocation_scope()?;
        if entry.accepted.module().identities.invocation_scope() != Some(invocation) {
            return None;
        }
        let source = entry.accepted.source();
        let sources = source.symbols.source_map()?;
        let requester = entry
            .source
            .symbols
            .symbol_provenance_source_span(entry.requester)?;
        let elaboration = &entry.accepted.closure().elaboration;
        let scope = &artifact.module().scalar_applications.proof_scope;
        let mut declarations = scope
            .callables
            .iter()
            .filter_map(|(_, association)| {
                let ProofCallableAssociation::Callable {
                    declaration,
                    callable,
                } = association
                else {
                    return None;
                };
                let symbol = match callable {
                    TargetMachineCallable::Machine(machine) => elaboration.machines.get(*machine),
                    TargetMachineCallable::Boundary(boundary) => {
                        elaboration.boundaries.get(*boundary)
                    }
                }?;
                source
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(*symbol)
                    .then(|| source.symbols.symbol_provenance_source_span(*symbol))
                    .flatten()
                    .map(|span| (*declaration, span))
            })
            .collect::<Vec<_>>();
        declarations.sort_unstable_by_key(|(declaration, _)| declaration.0);
        if declarations
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1)
        {
            return None;
        }
        declarations.dedup_by_key(|(declaration, _)| *declaration);
        Some(Self {
            entry,
            identity: artifact.custody_identity(),
            sources,
            requester,
            declarations,
        })
    }

    fn source(&self, declaration: ProofDeclarationId) -> Option<SourceSpan> {
        self.declarations
            .binary_search_by_key(&declaration.0, |(declaration, _)| declaration.0)
            .ok()
            .map(|position| self.declarations[position].1)
    }
}

impl terminal_installation::InvocationSourceSelection for InvocationSelection<'_> {
    fn allows_selection(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
        requester: Option<ProofDeclarationId>,
        declaration: ProofDeclarationId,
    ) -> bool {
        if artifact.custody_identity() != self.identity {
            return false;
        }
        let requester = match requester {
            Some(requester) => self.source(requester),
            None => Some(self.requester),
        };
        let Some((requester, declaration)) = requester.zip(self.source(declaration)) else {
            return false;
        };
        self.entry.source_consent.as_ref().is_some_and(|consent| {
            consent.allows_selection(&self.sources, requester, declaration, self.entry.purpose)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildTimeValue, InvocationSourceConsent, SourceCompletionAccounting};
    use terminal_installation::InvocationSourceSelection;

    struct Captured(Arc<SourceMap>);

    impl InvocationSourceConsent for Captured {
        fn allows_selection(
            &self,
            sources: &Arc<SourceMap>,
            requester: SourceSpan,
            declaration: SourceSpan,
            purpose: source::DependencyScope,
        ) -> bool {
            Arc::ptr_eq(sources, &self.0)
                && purpose == source::DependencyScope::Build
                && requester.source_id == source::SourceId(0)
                && declaration.source_id == source::SourceId(0)
        }
    }

    fn source(text: &str) -> (typed_trees::TypedTrees, Arc<SourceMap>) {
        let mut sources = SourceMap::default();
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
        let mut syntax = syntax_trees::SyntaxTrees::new(id);
        tokens_to_syntax_trees::parse(&mut syntax, id, &tokens).unwrap();
        let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
        request.sources = Some(Arc::clone(&sources));
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        (typed, sources)
    }

    fn target() -> crate::InvocationTarget<'static> {
        crate::InvocationTarget {
            name: None,
            layout_target: None,
            carrier_layout: None,
            address_model: None,
        }
    }

    #[test]
    fn a_build_constant_uses_capture_consent_without_acquiring_an_owner() {
        let (typed, sources) = source(
            "const VALUE:u64=answer(); machine answer()->u64 { 4u64 } machine request()->u64 { VALUE }",
        );
        let request = typed.realized_machine_named("request").unwrap().symbol;
        let compilation = psi_terminal_production::TerminalSourceCompilation {
            calling: None,
            providers: None,

            declaration_metadata: &[],
            entries: &[request],
            subjects: &[],
            purpose: source::DependencyScope::Build,
            selected: &[],
            bindings: &[],
            supplies: &[],
        };
        let mut denied = typed.clone();
        let mut accounting = SourceCompletionAccounting::default();
        let error =
            crate::evaluate_constant_initializers(&mut denied, compilation, None, &mut accounting)
                .unwrap_err();
        assert!(diagnostics::join_messages(&error).contains("reconciled package custody"));
        assert!(accounting.receipts().is_empty());

        let mut admitted = typed;
        crate::evaluate_constant_initializers_with_source_consent(
            &mut admitted,
            compilation,
            None,
            Some(Arc::new(Captured(sources))),
            &mut accounting,
        )
        .unwrap();
        crate::validate_constant_evaluations(&admitted, &accounting).unwrap();
        assert_eq!(accounting.receipts().len(), 1);
        let crate::PureEvaluationRecord::Invocation(receipt) = &accounting.receipts()[0] else {
            panic!("the actual interpreter invocation is retained")
        };
        assert_eq!(
            receipt.admission().requester(),
            semantic_vocabulary::compiler_evaluation::DeclarationCustody::UnownedUser,
        );
        assert!(matches!(
            receipt.outcome(),
            crate::PureTerminalEvaluationOutcome::Materialized {
                value: BuildTimeValue::Int(4),
                ..
            }
        ));
        assert!(admitted.symbols.symbol_package_identity(request).is_none());
    }

    #[test]
    fn capture_consent_does_not_authorize_another_source_or_invocation() {
        let (typed, sources) =
            source("machine answer()->u64 { 4u64 } machine request()->u64 { 0u64 }");
        let requester = typed.realized_machine_named("request").unwrap().symbol;
        let answer = typed.realized_machine_named("answer").unwrap().symbol;
        let prepare = |consent: Arc<dyn InvocationSourceConsent>| {
            super::super::PreparedBuildMachineProgram::prepare_for(
                &typed,
                None,
                source::DependencyScope::Build,
                Some(consent),
            )
            .unwrap()
        };
        let foreign = prepare(Arc::new(Captured(Arc::new(sources.as_ref().clone()))));
        let error = foreign
            .entry(requester, answer, &[])
            .unwrap()
            .invoke_recorded(target(), &mut SourceCompletionAccounting::default())
            .unwrap_err();
        assert!(error.contains("reconciled package custody"), "{error}");

        let program = prepare(Arc::new(Captured(sources)));
        let first = program.entry(requester, answer, &[]).unwrap();
        let second = program.entry(requester, answer, &[]).unwrap();
        let produced = first.produce_invocation().unwrap();
        let other = second.produce_invocation().unwrap();
        assert_ne!(
            produced.view().custody_identity(),
            other.view().custody_identity()
        );
        let selection = InvocationSelection::new(&first, produced.view()).unwrap();
        let declaration = produced
            .view()
            .module()
            .scalar_applications
            .proof_scope
            .callable_declaration(produced.view().module().entry.unwrap())
            .unwrap();
        assert!(selection.allows_selection(produced.view(), None, declaration));
        assert!(!selection.allows_selection(other.view(), None, declaration));
        assert!(!selection.allows_selection(produced.view(), None, ProofDeclarationId(u32::MAX),));
        assert!(InvocationSelection::new(&first, other.view()).is_none());
    }

    #[test]
    fn capture_consent_keeps_the_written_service_ceiling() {
        let (typed, sources) = source(
            "boundary trait Requests { machine observe() reaches none crashes none; } machine answer()->u64 reaches Requests { 4u64 } machine request()->u64 { 0u64 }",
        );
        let requester = typed.realized_machine_named("request").unwrap().symbol;
        let answer = typed.realized_machine_named("answer").unwrap().symbol;
        let program = super::super::PreparedBuildMachineProgram::prepare_for(
            &typed,
            None,
            source::DependencyScope::Build,
            Some(Arc::new(Captured(sources))),
        )
        .unwrap();
        let mut accounting = SourceCompletionAccounting::default();
        let error = program
            .entry(requester, answer, &[])
            .unwrap()
            .invoke_recorded(target(), &mut accounting)
            .unwrap_err();
        assert!(error.contains("service reach"), "{error}");
        assert!(accounting.receipts().is_empty());
    }
}
