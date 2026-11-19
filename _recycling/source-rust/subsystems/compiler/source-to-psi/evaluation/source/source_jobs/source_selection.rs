#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::{BuildTimeValue, InvocationSourceConsent, SourceCompletionAccounting};
    use terminal_installation::InvocationSourceSelection;

    struct Captured(Arc<SourceMap>);

    impl InvocationSourceConsent for Captured {
        fn allows_selection(
            &self,
            sources: &Arc<SourceMap>,
            requester: SourceSpan,
            declaration: SourceSpan,
            purpose: declaration_contracts::source_identity::DependencyScope,
        ) -> bool {
            Arc::ptr_eq(sources, &self.0)
                && purpose == declaration_contracts::source_identity::DependencyScope::Build
                && requester.source_id == source_text::SourceId(0)
                && declaration.source_id == source_text::SourceId(0)
        }
    }

    fn source(text: &str) -> (crate::source_state::TypedTrees, Arc<SourceMap>) {
        let mut sources = SourceMap::default();
        let id = sources
            .add_checked_instance(
                "/captured/build.omg".into(),
                text,
                "/captured".into(),
                None,
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                declaration_contracts::source_identity::DependencyScope::Build,
            )
            .source_id;
        let sources = Arc::new(sources);
        let tokens = crate::evaluation::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        let mut syntax = syntax_trees::SyntaxTrees::new(id);
        crate::evaluation::frontend_fixture::parse(&mut syntax, id, &tokens).unwrap();
        let mut request = crate::resolution::ResolutionRequest::new(syntax);
        request.sources = Some(Arc::clone(&sources));
        let resolved = crate::resolution::resolve(request).unwrap();
        let typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
        (typed, sources)
    }

    fn target() -> crate::evaluation::InvocationTarget<'static> {
        crate::evaluation::InvocationTarget {
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
        let compilation = crate::production::TerminalSourceCompilation {
            calling: None,
            providers: None,

            declaration_metadata: &[],
            entries: &[request],
            subjects: &[],
            purpose: declaration_contracts::source_identity::DependencyScope::Build,
            selected: &[],
            bindings: &[],
            supplies: &[],
        };
        let mut denied = typed.clone();
        let mut accounting = SourceCompletionAccounting::default();
        let error = crate::evaluation::evaluate_constant_initializers(
            &mut denied,
            compilation,
            None,
            &mut accounting,
        )
        .unwrap_err();
        assert!(diagnostics::join_messages(&error).contains("reconciled package custody"));
        assert!(accounting.receipts().is_empty());

        let mut admitted = typed;
        crate::evaluation::evaluate_constant_initializers_with_source_consent(
            &mut admitted,
            compilation,
            &[],
            None,
            Some(Arc::new(Captured(sources))),
            &mut accounting,
        )
        .unwrap();
        crate::evaluation::validate_constant_evaluations(&admitted, &accounting).unwrap();
        assert_eq!(accounting.receipts().len(), 1);
        let crate::evaluation::PureEvaluationRecord::Invocation(receipt) =
            &accounting.receipts()[0]
        else {
            panic!("the actual interpreter invocation is retained")
        };
        assert_eq!(
            receipt.admission().requester(),
            semantic_vocabulary::compiler_evaluation::DeclarationCustody::UnownedUser,
        );
        assert!(matches!(
            receipt.outcome(),
            crate::evaluation::PureTerminalEvaluationOutcome::Materialized {
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
                declaration_contracts::source_identity::DependencyScope::Build,
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
            declaration_contracts::source_identity::DependencyScope::Build,
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
