//! Source jobs do not execute through retired tree/checked-program engines.
use crate::BuildTimeSelectionAuthority;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

mod build_activation;
mod invocation;
mod schema_evaluation;
mod schema_geometry;
mod schema_values;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedLayoutTargetScope {
    pub target: String,
    pub facts: Option<layout_plans::LayoutTargetFacts>,
    pub entry_declarations: Option<Vec<symbols::SymbolHandle>>,
    pub provider_declarations: Vec<crate::product::provider_declarations::ProviderSelection>,
}
/// Nothing evaluates a layout policy yet, so a type applied through one
/// has no layout. Such a type is a request of the declaration that uses it:
/// it is recorded there (`typed_trees::deferred_refusals`) and that
/// declaration is refused where a program demands it. A program that never
/// reaches one is not stopped for a library type it does not use.
pub fn evaluate_fixed_layout_applications(
    typed: &mut TypedTrees,
    _scopes: &[FixedLayoutTargetScope],
    _authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    crate::source_frontier::defer_declared_jobs(typed)
}
pub fn is_exact_toolchain_build_prelude_data(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    expected_name: &str,
) -> bool {
    typed.data_definitions().iter().any(|definition| {
        definition.symbol == symbol
            && definition.name.as_str() == expected_name
            && typed.symbols.is_build_prelude_symbol(symbol)
    })
}
pub use build_activation::{BUILD_VALUE_UNPROTECTED, admit_toolchain_build_activation};

pub use crate::evaluation_requests::{
    CapturedFilesystemRoot as BuildMachineCapturedFilesystemRoot,
    FilesystemAccess as BuildMachineFilesystemAccess,
};
pub use evaluation_accounting::{BuildEvaluationSponsor, BuildEvaluationSponsorLimits};
pub use filesystem_authority::{
    FilesystemGrantRoot as BuildMachineFilesystemGrantRoot,
    FilesystemGrantRootIdentity as BuildMachineFilesystemGrantRootIdentity,
    FilesystemMetadataLayout as BuildMachineFilesystemMetadataLayout,
    FilesystemSponsor as BuildMachineFilesystemSponsor,
    FilesystemSponsorLimits as BuildMachineFilesystemSponsorLimits,
    FsGrants as BuildMachineFilesystemGrants,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildMachineExecutionMode {
    Pure,
    Granted {
        filesystem: BuildMachineFilesystemAccess,
        filesystem_metadata_layout: BuildMachineFilesystemMetadataLayout,
    },
}
/// The input source is retained as metadata, never as a checked-program witness.
pub struct PreparedBuildMachineProgram {
    source: Arc<TypedTrees>,
    authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
}
#[derive(Clone)]
pub struct PreparedBuildMachineEntry {
    source: Arc<TypedTrees>,
    symbol: SymbolHandle,
    requester: SymbolHandle,
    call: typed_trees::expression::TableCallExpression,
    call_source: typed_trees::expression::ExpressionHandle,
    authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
}
impl PreparedBuildMachineEntry {
    pub const fn symbol(&self) -> SymbolHandle {
        self.symbol
    }
    pub const fn requester(&self) -> SymbolHandle {
        self.requester
    }

    pub fn source(&self) -> &TypedTrees {
        &self.source
    }

    /// Judge and run the actual call, including construction of its arguments.
    /// Acceptance of the callee under its own preconditions is insufficient.
    pub fn invoke(
        &self,
        target: crate::InvocationTarget<'_>,
        arguments: &[crate::BuildTimeValue],
        meter: &mut psi_work::WorkMeter,
    ) -> Result<crate::BuildTimeValue, String> {
        let (accepted, _) = self.accept_invocation(arguments)?;
        let produced = self.produce_invocation(&accepted)?;
        let admitted = self.admit_invocation(&produced, target)?;
        self.execute_invocation(&produced, &admitted, meter)
    }

    /// Record the value and actual work/storage charges without treating local
    /// invocation code as portable or cached source-completion evidence.
    pub fn invoke_recorded(
        &self,
        target: crate::InvocationTarget<'_>,
        arguments: &[crate::BuildTimeValue],
        accounting: &mut crate::SourceCompletionAccounting,
    ) -> Result<crate::MeasuredEvaluation<crate::BuildTimeValue>, String> {
        let (accepted, call) = self.accept_invocation(arguments)?;
        let produced = Arc::new(self.produce_invocation(&accepted)?);
        let admitted = self.admit_invocation(&produced, target)?;
        let account = crate::evaluation_accounting::pure_terminal_invocation_account()?;
        let mut meter = psi_work::WorkMeter::with_invocation(account.clone(), None)
            .map_err(|error| format!("source invocation meter: {error:?}"))?;
        let result = self
            .execute_invocation(&produced, &admitted, &mut meter)
            .and_then(|value| {
                let usage = account
                    .snapshot()
                    .map_err(|error| format!("source invocation usage: {error:?}"))?;
                Ok(crate::MeasuredEvaluation::new(value, *usage.record()))
            });
        let outcome = match &result {
            Ok(value) => crate::PureTerminalEvaluationOutcome::Materialized {
                value: value.value().clone(),
                usage: value.usage(),
            },
            Err(reason) => crate::PureTerminalEvaluationOutcome::Failed {
                reason: reason.clone(),
            },
        };
        let receipt = crate::evaluation_accounting::PureTerminalEvaluationReceipt::new(
            &accepted,
            call,
            self,
            arguments,
            produced,
            admitted,
            outcome,
            meter.into_usage(),
        )?;
        accounting.append_invocation(receipt);
        result
    }

    fn accept_invocation(
        &self,
        arguments: &[crate::BuildTimeValue],
    ) -> Result<
        (
            psi_terminal_production::AcceptedTerminalSource,
            typed_trees::expression::ExpressionHandle,
        ),
        String,
    > {
        let machine = self
            .source
            .machine_by_symbol(self.symbol)
            .ok_or("source invocation lost its selected declaration")?;
        let (source, caller, call) = invocation::prepare_value_invocation(
            &self.source,
            self.requester,
            machine,
            &self.call,
            arguments,
            self.call_source,
        )?;
        // The call's source reaches the checker the way every program does,
        // through typed preparation; there is no second way to freeze it.
        let (source, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            source,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .map_err(|errors| diagnostics::join_messages(&errors))?;
        let source = Arc::new(source);
        let accepted = psi_terminal_production::AcceptedTerminalSource::judge(
            source,
            psi_terminal_production::TerminalSourceCompilation {
                entries: &[caller],
                // Evaluating this call requests its dependency closure, not a
                // full check of the requester's package or standalone source.
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .map_err(|errors| diagnostics::join_messages(&errors))?;
        let entry = accepted
            .entry()
            .ok_or("source invocation has no accepted entry")?;
        if accepted.closure().elaboration.machines.get(entry) != Some(&caller) {
            return Err("source invocation accepted another caller".into());
        }
        Ok((accepted, call))
    }

    fn produce_invocation(
        &self,
        accepted: &psi_terminal_production::AcceptedTerminalSource,
    ) -> Result<psi_terminal_production::ProducedTerminalProgram, String> {
        accepted
            .produce(
                Default::default(),
                &mut psi_terminal_production::UnmeasuredProduction,
            )
            .map_err(|errors| diagnostics::join_messages(&errors))
    }

    fn admit_invocation(
        &self,
        produced: &psi_terminal_production::ProducedTerminalProgram,
        target: crate::InvocationTarget<'_>,
    ) -> Result<terminal_installation::AdmittedCompilerEvaluation, String> {
        use terminal_interpreter::TerminalProgram;
        let program = TerminalProgram::borrow(produced.view());
        let requirements = program
            .execution_requirements(None)
            .map_err(|error| format!("source invocation demands: {error:?}"))?;
        if !requirements.imports.is_empty()
            || !requirements.missing_providers.is_empty()
            || !requirements.external_completions.is_empty()
            || !requirements.scalar_applications.is_empty()
            || !requirements.scalar_entry_conditions.is_empty()
        {
            return Err("source invocation requires selected target, scalar or external authority that is not supplied".into());
        }
        let view = produced.view();
        let selected = view
            .module()
            .entry
            .ok_or("source invocation lost its emitted root")?;
        let scope = &view.module().scalar_applications.proof_scope;
        let root = scope
            .callable_declaration(selected)
            .ok_or("source invocation has no exact proof declaration")?;
        let mut headers = scope
            .callable_headers
            .iter()
            .filter_map(|(_, header)| (header.declaration == root).then_some(header));
        let header = headers
            .next()
            .ok_or("source invocation lost its accepted root header")?;
        if headers.next().is_some() || !header.supplies_checked_body {
            return Err("source invocation has no unique accepted caller body".into());
        }
        let providers =
            terminal_installation::admit_provider_execution_installation(view, &[], &[])
                .map_err(|error| format!("source invocation provider admission: {error:?}"))?;
        let requester = match (
            self.source.symbols.symbol_package_identity(self.requester),
            self.source.symbols.symbol_source_origin(self.requester),
        ) {
            (Some(package), _) => {
                semantic_vocabulary::compiler_evaluation::DeclarationCustody::Package(package)
            }
            (_, Some(source::SourceOrigin::Toolchain)) => {
                semantic_vocabulary::compiler_evaluation::DeclarationCustody::Toolchain
            }
            (_, Some(source::SourceOrigin::ToolchainContract)) => {
                semantic_vocabulary::compiler_evaluation::DeclarationCustody::ToolchainContract
            }
            (_, Some(source::SourceOrigin::User)) => {
                semantic_vocabulary::compiler_evaluation::DeclarationCustody::UnownedUser
            }
            _ => semantic_vocabulary::compiler_evaluation::DeclarationCustody::Missing,
        };
        let admitted = terminal_installation::admit_compiler_evaluation(
            view,
            target.name,
            root,
            providers,
            requester,
            self.authority.as_deref(),
        )?;
        admitted.require_invocation(view, target.name, selected)?;
        Ok(admitted)
    }

    fn execute_invocation(
        &self,
        produced: &psi_terminal_production::ProducedTerminalProgram,
        admitted: &terminal_installation::AdmittedCompilerEvaluation,
        meter: &mut psi_work::WorkMeter,
    ) -> Result<crate::BuildTimeValue, String> {
        use terminal_interpreter::{
            TerminalExecutionResult, TerminalExecutionStatus, TerminalInstantiation,
            TerminalProgram,
        };
        let selected = produced
            .view()
            .module()
            .entry
            .ok_or("source invocation lost its emitted root")?;
        admitted.require_invocation(produced.view(), admitted.target(), selected)?;
        let installation =
            terminal_interpreter::TerminalRuntimeSupplies::from(admitted.providers());
        let program = TerminalProgram::borrow(produced.view());
        let entry = program
            .entry()
            .map_err(|error| format!("source invocation entry: {error:?}"))?;
        if !entry.scalar_parameters.is_empty() || !entry.structural_parameters.is_empty() {
            return Err("source invocation retained an unmaterialized compiler context".into());
        }
        let result_type = entry
            .result
            .structural()
            .map(|result| result.structural_type);
        let account = meter.invocation_account().cloned();
        let mut instance = program
            .instantiate(TerminalInstantiation {
                account: account.as_ref(),
                installation: Some(&installation),
                ..Default::default()
            })
            .map_err(|error| format!("source invocation preparation: {error:?}"))?;
        let status = instance
            .invoke(meter)
            .map_err(|error| format!("source invocation execution: {error:?}"))?;
        match status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                Ok(crate::BuildTimeValue::Unit)
            }
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(value)) => {
                crate::terminal_values::scalar_result(value)
            }
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(_)) => {
                crate::terminal_values::result_value(
                    &instance,
                    result_type.ok_or("source invocation lost its structured result type")?,
                )
            }
            other => Err(format!(
                "source invocation did not complete normally: {other:?}"
            )),
        }
    }
}
impl PreparedBuildMachineProgram {
    pub fn prepare(
        typed: &TypedTrees,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        // Retaining source is preparation, not acceptance. Each actual job is
        // checked with its materialized arguments before any execution starts.
        Ok(Self {
            source: Arc::new(typed.clone()),
            authority,
        })
    }
    pub fn typed(&self) -> &TypedTrees {
        &self.source
    }
    /// Keep the actual authored call and its complete arguments in the job.
    /// The expression is selected from this retained source owner; its author
    /// must be the actual requesting declaration, not the called declaration.
    pub fn entry_call(
        &self,
        requester: SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Result<PreparedBuildMachineEntry, String> {
        if !self.source.symbols.symbols().nodes().is_valid(requester) {
            return Err("source job requires its actual requesting declaration".into());
        }
        if !self.source.expression_table.expression_is_valid(expression) {
            return Err("source job call does not belong to its retained source".into());
        }
        let span = self
            .source
            .expression_table
            .try_source_span(expression)
            .ok_or("source job call lost its authored source location")?;
        if self.source.symbols.source_file(span).is_none()
            || !self
                .source
                .symbols
                .source_reference_may_select_symbol(span, requester)
        {
            return Err("source job call is outside its requesting source".into());
        }
        let call = self
            .source
            .expression_table
            .call_node(expression)
            .ok_or("source job requires a retained call expression")?;
        if call.authored_machine_application.is_valid()
            && !self
                .source
                .expression_table
                .static_machine_application_is_valid(call.authored_machine_application)
        {
            return Err("source job call has no retained static application".into());
        }
        if typed_trees::references::StaticArgumentCall::Expression(expression, call)
            .lexical_owner(&self.source)
            != requester
        {
            return Err("source job call has another requesting declaration".into());
        }
        let machine = self
            .source
            .machine_by_symbol(call.target_symbol)
            .or_else(|| self.source.machine_holding_state(call.target_symbol))
            .ok_or("source job call has no concrete machine declaration")?;
        let mut entry = self.entry(requester, machine.symbol)?;
        if call.target_symbol != entry.call.target_symbol && call.target_symbol != machine.symbol {
            return Err("source job call does not select the declared entry".into());
        }
        let [state] = self.source.machine_states(machine) else {
            return Err("source job call has no exact entry state".into());
        };
        if self
            .source
            .expression_table
            .expression_handles(call.arguments)
            .len()
            != self.source.state_parameters(state).len()
        {
            return Err("source job call lost its complete argument list".into());
        }
        entry.call = call.clone();
        entry.call_source = expression;
        Ok(entry)
    }

    pub fn entry(
        &self,
        requester: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Result<PreparedBuildMachineEntry, String> {
        if !self.source.symbols.symbols().nodes().is_valid(requester)
            || self
                .source
                .symbols
                .symbol_provenance_source_span(requester)
                .is_none()
        {
            return Err("source job requires its actual requesting declaration".into());
        }
        let machine = self
            .source
            .machine_by_symbol(symbol)
            .ok_or("no product route yet: source Build entry is unavailable")?;
        let call = invocation::direct_call(&self.source, machine)?;
        Ok(PreparedBuildMachineEntry {
            source: Arc::clone(&self.source),
            symbol,
            requester,
            call,
            call_source: typed_trees::expression::ExpressionHandle::invalid(),
            authority: self.authority.clone(),
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::BuildTimeValue;

    fn target() -> crate::InvocationTarget<'static> {
        crate::InvocationTarget {
            name: "linux_x86_64",
            layout_target: None,
            carrier_layout: None,
            address_model: None,
        }
    }

    // The source reader, caller construction, lowered checker, publication and
    // interpreter all see these actual programs. No accepted callee or artifact
    // is fabricated by this helper.
    pub(crate) fn typed(source: &str) -> TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("lex source");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse source");
        let mut sources = source::SourceMap::default();
        sources.add_with_metadata(
            "/job/policy.omg".into(),
            source.into(),
            "/job".into(),
            semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
            source::SourceOrigin::User,
        );
        let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
        request.sources = Some(Arc::new(sources));
        let resolved =
            syntax_trees_to_symbol_resolved_trees::resolve(request).expect("resolve source");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type source")
    }

    fn entry(source: &str) -> PreparedBuildMachineEntry {
        let source = typed(source);
        let symbol = source
            .realized_machine_named("answer")
            .expect("named source entry")
            .symbol;
        PreparedBuildMachineProgram::prepare(&source, None)
            .expect("retain source")
            .entry(symbol, symbol)
            .expect("retain entry")
    }

    fn with_reflection(source: &str) -> TypedTrees {
        let core_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../source/library/core");
        let reflection = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../source/library/core/reflect.omg"
        ));
        let mut sources = source::SourceMap::default();
        let core = sources
            .add_with_metadata(
                core_root.join("reflect.omg"),
                reflection.into(),
                core_root,
                None,
                source::SourceOrigin::Toolchain,
            )
            .source_id;
        let author = sources
            .add_with_metadata(
                "/job/policy.omg".into(),
                source.into(),
                "/job".into(),
                semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
                source::SourceOrigin::User,
            )
            .source_id;
        let mut syntax = syntax_trees::SyntaxTrees::new(author);
        for (id, text) in [(core, reflection), (author, source)] {
            let tokens = source_files_to_tokens::Lexer::new(text)
                .tokenize()
                .expect("lex source");
            tokens_to_syntax_trees::parse(&mut syntax, id, &tokens).expect("parse source");
        }
        let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
        request.sources = Some(Arc::new(sources));
        request.top_level_bindings.push(
            symbols::SourceScopedTopLevelBinding::logical_module_import(
                author,
                core,
                "omega::language::core::reflect",
                3,
            ),
        );
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request)
            .expect("resolve exact core and author");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type exact core and author")
    }

    fn returned_source_call(
        source: &TypedTrees,
        name: &str,
    ) -> (SymbolHandle, typed_trees::expression::ExpressionHandle) {
        let machine = source.realized_machine_named(name).unwrap();
        let [state] = source.machine_states(machine) else {
            panic!("one authored entry")
        };
        let Some(typed_trees::statement::StatementNode::Transition(transition)) = source
            .statement_table
            .statements(state.statement_nodes)
            .last()
        else {
            panic!("the source returns its query")
        };
        let typed_trees::statement::TransitionTargetNode::Value(expression) =
            source.statement_table.transition_target(transition.target)
        else {
            panic!("the returned value is the authored call")
        };
        (machine.symbol, *expression)
    }

    #[test]
    fn source_call_requests_keep_their_authored_operand_and_owner() {
        let source = with_reflection(
            "use omega::language::core::reflect;
            data Cell<T> { value: T; }
            machine selected<T>(value: T) -> T { value }
            machine answer() -> reflect::CallableInfo { reflect::callable(selected<u32>) }
            machine alternate() -> reflect::CallableInfo { reflect::callable(selected<u64>) }
            machine not_callable() -> reflect::CallableInfo { reflect::callable(Cell<u32>) }",
        );
        let program = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
        let source = program.typed();
        let (requester, first_call) = returned_source_call(source, "answer");
        let (another, second_call) = returned_source_call(source, "alternate");
        let error = program
            .entry_call(another, first_call)
            .err()
            .expect("one declaration cannot adopt another declaration's call");
        assert_eq!(error, "source job call has another requesting declaration");
        let mut descriptions = Vec::new();
        for (requester, expression) in [(requester, first_call), (another, second_call)] {
            let expected = source.expression_table.call_node(expression).unwrap();
            let mut accounting = crate::SourceCompletionAccounting::default();
            let value = program
                .entry_call(requester, expression)
                .unwrap()
                .invoke_recorded(target(), &[], &mut accounting)
                .expect("the original declaration operand is not replaced by a value");
            let Some(crate::PureEvaluationRecord::Invocation(receipt)) =
                accounting.receipts().last()
            else {
                panic!("the actual invocation is recorded")
            };
            assert_eq!(receipt.requester(), requester);
            assert_eq!(receipt.call().unwrap(), expected);
            receipt.validate_charge().unwrap();
            descriptions.push(value.value().clone());
        }
        assert_ne!(descriptions[0], descriptions[1]);
        let (requester, expression) = returned_source_call(source, "not_callable");
        let mut meter = psi_work::WorkMeter::unbounded();
        let error = program
            .entry_call(requester, expression)
            .unwrap()
            .invoke(target(), &[], &mut meter)
            .expect_err("an applied data declaration is not a callable declaration");
        assert!(
            error.contains("an applied declaration operand requires a machine or requirement"),
            "{error}"
        );
        assert_eq!(meter.usage().total_units(), 0);
    }

    // The selector's call runs once before the replacement changes index.
    // The returned fields distinguish a redirected store and a repeated call.
    #[test]
    fn source_jobs_capture_selectors_before_the_replacement_runs() {
        let source = typed(
            "data Store { items: [u64; 2]; }
            data Report { first: u64; second: u64; index: u64; calls: u64; }
            machine select(index: u64, calls: &mut u64) -> u64
            requires calls < 2
            ensures result == index
            { calls = calls + 1; index }
            machine replace(index: &mut u64) -> u64
            ensures index == 1
            { index = 1; 9u64 }
            machine answer() -> Report {
                let mut index: u64 = 0;
                let mut calls: u64 = 0;
                let mut store: Store = Store { items: [3u64, 4u64] };
                store.items[select(index, &mut calls)] = replace(&mut index);
                Report { first: store.items[0], second: store.items[1], index: index, calls: calls }
            }",
        );
        let machine = source.realized_machine_named("answer").unwrap();
        let [state] = source.machine_states(machine) else {
            panic!("one source state")
        };
        assert!(
            source
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| matches!(statement,
            typed_trees::statement::StatementNode::LocalData(local) if local.is_operand_capture))
        );
        let selected = machine.symbol;
        let result = PreparedBuildMachineProgram::prepare(&source, None)
            .unwrap()
            .entry(selected, selected)
            .unwrap()
            .invoke(target(), &[], &mut psi_work::WorkMeter::unbounded())
            .expect("the store uses the selector captured before replacement");
        let BuildTimeValue::Struct { fields, .. } = result else {
            panic!("actual report")
        };
        assert_eq!(
            fields.as_slice(),
            &[
                ("first".into(), BuildTimeValue::Int(9)),
                ("second".into(), BuildTimeValue::Int(4)),
                ("index".into(), BuildTimeValue::Int(1)),
                ("calls".into(), BuildTimeValue::Int(1)),
            ]
        );
    }

    #[test]
    fn source_schema_query_materializes_its_bare_type_operand() {
        let source = with_reflection(
            "use omega::language::core::reflect;
            data Cell { value: u32; }
            machine answer() -> reflect::TypeSchema { reflect::schema<Cell>() }
            machine other() -> reflect::TypeSchema { reflect::schema<Cell>() }",
        );
        let (requester, expression) = returned_source_call(&source, "answer");
        let (another, _) = returned_source_call(&source, "other");
        let original = source.expression_table.call_node(expression).unwrap();
        let request = typed_trees::references::StaticArgumentCall::Expression(expression, original);
        let [operand] = request.arguments(&source) else {
            panic!("one actual type operand")
        };
        assert_eq!(
            operand.type_reference,
            typed_trees::types::TypeReferenceHandle::invalid()
        );
        let selected = source.data_definition_by_symbol(operand.symbol).unwrap();
        assert_eq!(selected.name.as_str(), "Cell");
        let selected_symbol = selected.symbol;
        let application = request.application();
        let program = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
        let error = program.entry_call(another, expression).err().unwrap();
        assert_eq!(error, "source job call has another requesting declaration");
        let mut accounting = crate::SourceCompletionAccounting::default();
        let result = program
            .entry_call(requester, expression)
            .unwrap()
            .invoke_recorded(target(), &[], &mut accounting)
            .expect("the actual bare declaration produces its schema");
        let [crate::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
            panic!("one actual query invocation")
        };
        receipt.validate_charge().unwrap();
        assert_eq!(receipt.requester(), requester);
        let call = receipt.call().unwrap();
        let retained = typed_trees::references::StaticArgumentCall::Expression(
            typed_trees::expression::ExpressionHandle::invalid(),
            call,
        );
        assert_eq!(retained.lexical_owner(receipt.source()), requester);
        let [operand] = retained.arguments(receipt.source()) else {
            panic!("one normalized type operand")
        };
        assert_eq!(operand.symbol, selected_symbol);
        assert!(
            receipt
                .source()
                .type_reference_table
                .contains_type_reference(operand.type_reference)
        );
        assert!(
            matches!(receipt.source().type_reference_table.type_reference(operand.type_reference),
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } if *symbol == selected_symbol)
        );
        assert_eq!(
            receipt
                .source()
                .closed_data_definition(operand.type_reference)
                .unwrap()
                .symbol,
            selected_symbol
        );
        if !call.machine_arguments.is_empty() {
            assert_eq!(
                call.machine_arguments.as_ref(),
                retained.arguments(receipt.source())
            );
            assert_eq!(
                receipt
                    .source()
                    .expression_table
                    .call_node(expression)
                    .unwrap()
                    .machine_arguments,
                call.machine_arguments
            );
        }
        let rows = receipt
            .program()
            .source_records()
            .iter()
            .filter(|row| {
                row.declaration == selected_symbol
                    && row
                        .application
                        .as_ref()
                        .is_some_and(|application| application.reference == operand.type_reference)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows.len(),
            1,
            "the same request registers its actual structural home"
        );
        let BuildTimeValue::Struct { fields, .. } = result.value() else {
            panic!("the ordinary schema result")
        };
        assert!(
            matches!(fields.as_slice(), [(field, BuildTimeValue::Text(bytes))]
            if field == "storage" && !bytes.is_empty())
        );

        let mut stale_application_source = source.clone();
        let stale_application =
            typed_trees::static_applications::StaticMachineApplicationHandle::from_parts(
                application.arena_index(),
                application.generation() + 1,
            );
        assert!(
            !stale_application_source
                .expression_table
                .static_machine_application_is_valid(stale_application)
        );
        let typed_trees::expression::ExpressionNode::Call(call) = stale_application_source
            .expression_table
            .expression_mut(expression)
        else {
            panic!("the same source call")
        };
        call.authored_machine_application = stale_application;
        let error = PreparedBuildMachineProgram::prepare(&stale_application_source, None)
            .unwrap()
            .entry_call(requester, expression)
            .err()
            .expect("a stale application cannot supply an argument tuple or lexical owner");
        assert_eq!(error, "source job call has no retained static application");

        // A supplied reference with the wrong generation must not be rebuilt
        // from the still-correct declaration symbol, or reach execution.
        let mut stale_source = source.clone();
        let (reference, _) = source.type_reference_table.references().next().unwrap();
        let stale = typed_trees::types::TypeReferenceHandle::from_parts(
            reference.arena_index(),
            reference.generation() + 1,
        );
        assert!(
            !stale_source
                .type_reference_table
                .contains_type_reference(stale)
        );
        stale_source
            .expression_table
            .static_machine_application_arguments_mut(application)[0]
            .type_reference = stale;
        let typed_trees::expression::ExpressionNode::Call(call) =
            stale_source.expression_table.expression_mut(expression)
        else {
            panic!("the same source occurrence")
        };
        if !call.machine_arguments.is_empty() {
            call.machine_arguments[0].type_reference = stale;
        }
        let mut meter = psi_work::WorkMeter::unbounded();
        let error = PreparedBuildMachineProgram::prepare(&stale_source, None)
            .unwrap()
            .entry_call(requester, expression)
            .unwrap()
            .invoke(target(), &[], &mut meter)
            .expect_err("malformed supplied references remain unavailable");
        assert_eq!(
            error,
            "source invocation type argument has no retained reference"
        );
        assert_eq!(meter.usage().total_units(), 0);
    }

    #[test]
    fn source_schema_query_retains_the_requested_application_and_structural_home() {
        let source = with_reflection(
            "use omega::language::core::reflect;
            data Cell<T> { value: T; }
            machine answer() -> reflect::TypeSchema { reflect::schema<Cell<u32>>() }
            machine alternate() -> reflect::TypeSchema { reflect::schema<Cell<u64>>() }",
        );
        let program = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
        let mut descriptions = Vec::new();
        let mut homes = Vec::new();
        for name in ["answer", "alternate"] {
            let (requester, expression) = returned_source_call(program.typed(), name);
            let mut accounting = crate::SourceCompletionAccounting::default();
            let result = program
                .entry_call(requester, expression)
                .unwrap()
                .invoke_recorded(target(), &[], &mut accounting)
                .expect("the actual selected type produces its schema");
            let [crate::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
                panic!("one actual query request")
            };
            let request = typed_trees::references::StaticArgumentCall::Expression(
                typed_trees::expression::ExpressionHandle::invalid(),
                receipt.call().unwrap(),
            );
            assert_eq!(request.lexical_owner(receipt.source()), requester);
            let [operand] = request.arguments(receipt.source()) else {
                panic!("the receipt retains the complete selected argument")
            };
            let selected = receipt
                .source()
                .closed_data_definition(operand.type_reference)
                .unwrap();
            let rows = receipt
                .program()
                .source_records()
                .iter()
                .filter(|row| {
                    row.application
                        .as_ref()
                        .is_some_and(|application| application.reference == operand.type_reference)
                })
                .collect::<Vec<_>>();
            let [row] = rows.as_slice() else {
                panic!("the same query registers one exact actual application")
            };
            assert_eq!(row.declaration, selected.symbol);
            assert_eq!(row.fields.len(), 1);
            homes.push(row.storage_identity.clone());
            let BuildTimeValue::Struct { fields, .. } = result.value() else {
                panic!("the source returns an ordinary TypeSchema record")
            };
            let [(field, BuildTimeValue::Text(bytes))] = fields.as_slice() else {
                panic!("the result has its real immutable image")
            };
            assert_eq!(field, "storage");
            assert!(!bytes.is_empty());
            descriptions.push(bytes.clone());
        }
        assert_ne!(descriptions[0], descriptions[1]);
        assert_ne!(homes[0], homes[1]);
    }

    #[test]
    fn source_callable_queries_execute_with_exact_declarations_and_immutable_fields() {
        let source = with_reflection(
            "use omega::language::core::reflect;
            machine selected() -> u64 { 17u64 }
            machine other() -> u64 { 17u64 }
            machine answer() -> reflect::CallableInfo { reflect::callable(selected) }
            machine alternate() -> reflect::CallableInfo { reflect::callable(other) }",
        );
        let program = PreparedBuildMachineProgram::prepare(&source, None).expect("retain source");
        let query = |name| {
            let symbol = source
                .realized_machine_named(name)
                .expect("query declaration")
                .symbol;
            program
                .entry(symbol, symbol)
                .expect("query entry")
                .invoke(target(), &[], &mut psi_work::WorkMeter::unbounded())
                .expect("actual source query execution")
        };
        let first = query("answer");
        let second = query("alternate");
        let fields = |value| {
            let BuildTimeValue::Case { variant, payload } = value else {
                panic!("callable description alternative")
            };
            assert_eq!(variant, "Machine");
            payload
        };
        let first = fields(first);
        let second = fields(second);
        let bytes = |fields: &[(String, BuildTimeValue)], name: &str| {
            let mut matches = fields.iter().filter(|(field, _)| field == name);
            let (_, BuildTimeValue::Text(bytes)) = matches.next().expect("identity field") else {
                panic!("readable byte backing")
            };
            assert!(matches.next().is_none());
            assert!(!bytes.is_empty());
            bytes.clone()
        };
        assert_eq!(first.len(), 3);
        assert_eq!(second.len(), 3);
        assert_ne!(
            bytes(&first, "declaration_identity"),
            bytes(&second, "declaration_identity")
        );
        assert_ne!(
            bytes(&first, "callable_identity"),
            bytes(&second, "callable_identity")
        );
        assert_eq!(
            bytes(&first, "source_revision"),
            bytes(&second, "source_revision")
        );
    }

    #[test]
    fn source_callable_queries_refuse_a_value_holding_a_description() {
        let source = with_reflection("use omega::language::core::reflect;
            machine answer(value: reflect::CallableInfo) -> reflect::CallableInfo { reflect::callable(value) }");
        let symbol = source
            .realized_machine_named("answer")
            .expect("query declaration")
            .symbol;
        let value = BuildTimeValue::Case {
            variant: "Machine".into(),
            payload: [
                "declaration_identity",
                "callable_identity",
                "source_revision",
            ]
            .into_iter()
            .map(|name| (name.into(), BuildTimeValue::Text(vec![1])))
            .collect(),
        };
        let error = PreparedBuildMachineProgram::prepare(&source, None)
            .expect("retain source")
            .entry(symbol, symbol)
            .expect("query entry")
            .invoke(target(), &[value], &mut psi_work::WorkMeter::unbounded())
            .expect_err("data descriptions do not supply lexical declaration selection");
        assert!(
            error
                .contains("a callable description selects a concrete machine or trait requirement"),
            "{error}"
        );
    }

    #[test]
    fn source_jobs_prove_requires_at_the_actual_arguments_before_execution() {
        let entry = entry("machine answer(value: u64) -> u64 requires value > 0 { value }");
        let mut meter = psi_work::WorkMeter::unbounded();
        assert_eq!(
            entry
                .invoke(target(), &[BuildTimeValue::Int(1)], &mut meter)
                .expect("proved actual call"),
            BuildTimeValue::Int(1)
        );
        assert!(meter.usage().total_units() > 0);
        let mut refused = psi_work::WorkMeter::unbounded();
        let error = entry
            .invoke(target(), &[BuildTimeValue::Int(0)], &mut refused)
            .expect_err("callee requires are not assumptions of its caller");
        assert!(error.contains("requires"), "{error}");
        assert_eq!(refused.usage().total_units(), 0);
    }

    #[test]
    fn source_jobs_establish_record_domains_without_discarding_extra_fields() {
        let entry = entry(
            "data Positive requires value > 0 { value: u64; }
            machine answer(input: Positive) -> u64 { input.value }",
        );
        let argument = |value| BuildTimeValue::Struct {
            type_name: "Positive".into(),
            fields: vec![("value".into(), BuildTimeValue::Int(value))],
        };
        assert_eq!(
            entry
                .invoke(
                    target(),
                    &[argument(1)],
                    &mut psi_work::WorkMeter::unbounded()
                )
                .expect("established record"),
            BuildTimeValue::Int(1)
        );
        assert!(
            entry
                .invoke(
                    target(),
                    &[argument(0)],
                    &mut psi_work::WorkMeter::unbounded()
                )
                .expect_err("the zero field cannot establish Positive")
                .contains("requires")
        );
        let mut extra = argument(1);
        let BuildTimeValue::Struct { fields, .. } = &mut extra else {
            unreachable!()
        };
        fields.push(("extra".into(), BuildTimeValue::Int(2)));
        assert!(
            entry
                .invoke(target(), &[extra], &mut psi_work::WorkMeter::unbounded())
                .expect_err("all supplied fields remain obligations")
                .contains("field")
        );
    }

    #[test]
    fn source_jobs_preserve_immutable_byte_backing_and_refuse_mutable_fabrication() {
        let bytes = BuildTimeValue::Text(vec![0, 255, b'x']);
        let shared = entry("machine answer(bytes: &[u8]) -> &[u8] { bytes }");
        assert_eq!(
            shared
                .invoke(
                    target(),
                    std::slice::from_ref(&bytes),
                    &mut psi_work::WorkMeter::unbounded()
                )
                .expect("real shared backing"),
            bytes
        );
        assert_eq!(
            shared
                .invoke(
                    target(),
                    &[BuildTimeValue::Text(Vec::new())],
                    &mut psi_work::WorkMeter::unbounded()
                )
                .expect("empty real backing"),
            BuildTimeValue::Text(Vec::new())
        );
        let mutable = entry("machine answer(bytes: &mut [u8]) -> &mut [u8] { bytes }");
        assert!(
            mutable
                .invoke(target(), &[bytes], &mut psi_work::WorkMeter::unbounded())
                .is_err()
        );
    }

    #[test]
    fn source_jobs_construct_copy_and_return_byte_fields_with_their_real_contents() {
        let entry = entry(
            "data Packet [copy] { bytes: &[u8]; }
            machine answer(input: Packet) -> Packet { input }",
        );
        let value = BuildTimeValue::Struct {
            type_name: "Packet".into(),
            fields: vec![("bytes".into(), BuildTimeValue::Text(vec![0, 255, b'x']))],
        };
        let result = entry
            .invoke(
                target(),
                std::slice::from_ref(&value),
                &mut psi_work::WorkMeter::unbounded(),
            )
            .expect("byte field keeps its backing");
        let BuildTimeValue::Struct { type_name, fields } = &result else {
            panic!("record result")
        };
        let machine = entry.source().machine_by_symbol(entry.symbol()).unwrap();
        let state = &entry.source().machine_states(machine)[0];
        assert_eq!(
            type_name,
            &entry
                .source()
                .normalized_type_identity_text(state.return_type)
        );
        let BuildTimeValue::Struct {
            fields: original, ..
        } = &value
        else {
            unreachable!()
        };
        assert_eq!(fields, original);
        assert_eq!(
            entry
                .invoke(
                    target(),
                    std::slice::from_ref(&result),
                    &mut psi_work::WorkMeter::unbounded()
                )
                .expect("readback retains its complete formal identity"),
            result
        );
    }

    #[test]
    fn source_jobs_round_trip_numbered_members_with_their_exact_formal() {
        // Numbered members keep their schema identity when materialized.
        // An authored name and its returned number select the same member.
        let entry = entry(
            "data Choice [copy] { #7 case Ready(#9 bytes: &[u8]); #8 case Empty; }
             machine answer(value: Choice) -> Choice { value }",
        );
        let value = BuildTimeValue::Case {
            variant: "Ready".into(),
            payload: vec![("bytes".into(), BuildTimeValue::Text(vec![0, 255, b'x']))],
        };
        let returned = entry
            .invoke(target(), &[value], &mut psi_work::WorkMeter::unbounded())
            .expect("authored member selects its exact numbered case");
        assert_eq!(
            returned,
            BuildTimeValue::Case {
                variant: "#7".into(),
                payload: vec![("#9".into(), BuildTimeValue::Text(vec![0, 255, b'x']))],
            }
        );
        assert_eq!(
            entry
                .invoke(
                    target(),
                    std::slice::from_ref(&returned),
                    &mut psi_work::WorkMeter::unbounded()
                )
                .expect("returned identities rejoin the same formal"),
            returned
        );
        let mut meter = psi_work::WorkMeter::unbounded();
        let unknown = BuildTimeValue::Case {
            variant: "#99".into(),
            payload: vec![("#9".into(), BuildTimeValue::Text(vec![]))],
        };
        let error = entry
            .invoke(target(), &[unknown], &mut meter)
            .expect_err("unknown case is not supplied by a number");
        assert!(
            error.contains("case is not declared by its formal"),
            "{error}"
        );
        assert_eq!(meter.usage().total_units(), 0);
        let wrong_field = BuildTimeValue::Case {
            variant: "#7".into(),
            payload: vec![("#10".into(), BuildTimeValue::Text(vec![]))],
        };
        let error = entry
            .invoke(target(), &[wrong_field], &mut meter)
            .expect_err("a field number names only its declared payload");
        assert!(error.contains("missing a declared field"), "{error}");
        assert_eq!(meter.usage().total_units(), 0);
    }

    #[test]
    fn source_jobs_record_actual_bytes_and_charges_without_exporting_local_code() {
        let entry = entry("machine answer(bytes: &[u8]) -> &[u8] { bytes }");
        let bytes = BuildTimeValue::Text(vec![0, 255, b'x']);
        let mut accounting = crate::SourceCompletionAccounting::default();
        let measured = entry
            .invoke_recorded(target(), std::slice::from_ref(&bytes), &mut accounting)
            .expect("actual measured byte invocation");
        assert_eq!(measured.value(), &bytes);
        assert!(measured.usage().work_units > 0);
        assert!(measured.usage().result_payload_bytes >= 3);
        assert!(accounting.has_complete_usage());
        accounting
            .validate_charges()
            .expect("complete measured charges");
        let [crate::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
            panic!("one actual invocation receipt")
        };
        assert_eq!(receipt.machine(), entry.symbol());
        assert_eq!(receipt.requester(), entry.requester());
        assert_ne!(receipt.caller(), entry.symbol());
        assert_eq!(receipt.arguments(), &[bytes]);
        assert_eq!(receipt.usage().total_units(), measured.usage().work_units);
        assert!(receipt.program().artifact().is_err());
        assert_eq!(receipt.admission().target(), target().name);
        let selected = receipt.program().view().module().entry.unwrap();
        assert!(
            receipt
                .admission()
                .require_invocation(receipt.program().view(), "windows_x86_64", selected)
                .is_err()
        );
        let (another, _) = entry
            .accept_invocation(receipt.arguments())
            .expect("another actual caller");
        let another = entry
            .produce_invocation(&another)
            .expect("another invocation-local code owner");
        assert!(
            receipt
                .admission()
                .require_invocation(
                    another.view(),
                    target().name,
                    another.view().module().entry.unwrap()
                )
                .is_err()
        );
        let error =
            compiler_storage::encode(&accounting, compiler_storage::RecordLimits::default())
                .expect_err("temporary callers cannot become portable cached evidence");
        assert!(error.to_string().contains("invocation-local"), "{error}");
    }

    #[test]
    fn source_jobs_require_progress_before_the_interpreter_starts() {
        // Same direct recursive call as the structural reader's loop controls.
        // The runtime program may be silent about termination; a compiler job
        // must establish it before consuming any interpreter work.
        let entry = entry("machine answer(value: u64) -> u64 { answer(value) }");
        let (accepted, _) = entry
            .accept_invocation(&[BuildTimeValue::Int(1)])
            .expect("ordinary source is accepted without promising progress");
        let produced = entry
            .produce_invocation(&accepted)
            .expect("accepted ordinary body is produced");
        let intended = entry
            .admit_invocation(&produced, target())
            .expect_err("compiler evaluation independently requires progress");
        assert!(
            intended.contains("ordinary checked `Terminates` guarantee")
                || intended
                    .contains("recursive machine-call cycle has no ordinary termination proof"),
            "{intended}"
        );
        let mut meter = psi_work::WorkMeter::with_budget(100);
        let error = entry
            .invoke(target(), &[BuildTimeValue::Int(1)], &mut meter)
            .expect_err("a work ceiling cannot substitute for progress admission");
        assert_eq!(error, intended);
        assert_eq!(meter.usage().total_units(), 0);
    }

    #[test]
    fn source_jobs_check_only_the_requested_closure_and_keep_full_subjects_explicit() {
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0xA1; 32]).unwrap();
        for (owner, unused_owner) in [
            (Some(package), Some(package)),
            (None, None),
            (Some(package), None),
        ] {
            let mut sources = source::SourceMap::default();
            let policy = "machine answer() -> u64 { 17u64 } machine request() -> u64 { 0u64 }";
            let unused = "data NotReady<T> { value: T; }";
            let policy_id = sources
                .add_with_metadata(
                    "/policy-package/policy.omg".into(),
                    policy.into(),
                    "/policy-package".into(),
                    owner,
                    source::SourceOrigin::User,
                )
                .source_id;
            let unused_id = sources
                .add_with_metadata(
                    "/policy-package/not_ready.omg".into(),
                    unused.into(),
                    "/policy-package".into(),
                    unused_owner,
                    source::SourceOrigin::User,
                )
                .source_id;
            let mut syntax = syntax_trees::SyntaxTrees::new(policy_id);
            for (id, text) in [(policy_id, policy), (unused_id, unused)] {
                let tokens = source_files_to_tokens::Lexer::new(text)
                    .tokenize()
                    .expect("lex source");
                tokens_to_syntax_trees::parse(&mut syntax, id, &tokens).expect("parse source");
            }
            let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
            request.sources = Some(Arc::new(sources));
            let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request)
                .expect("resolve actual source owners");
            let typed =
                symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                    .expect("prepare declarations");
            let requester = typed.realized_machine_named("request").unwrap().symbol;
            let callee = typed.realized_machine_named("answer").unwrap().symbol;
            assert_eq!(typed.symbols.symbol_package_identity(requester), owner);
            let program =
                PreparedBuildMachineProgram::prepare(&typed, None).expect("retain source");
            let entry = program.entry(requester, callee).unwrap();
            if owner.is_some() {
                assert_eq!(
                    entry
                        .invoke(target(), &[], &mut psi_work::WorkMeter::unbounded())
                        .expect("one invocation does not claim the unused generic declaration"),
                    BuildTimeValue::Int(17)
                );
            } else {
                let (accepted, _) = entry
                    .accept_invocation(&[])
                    .expect("the standalone call closure excludes the unused declaration");
                let produced = entry
                    .produce_invocation(&accepted)
                    .expect("the checked standalone call has a concrete program");
                let error = entry
                    .admit_invocation(&produced, target())
                    .expect_err("source checking cannot supply missing package custody");
                assert!(
                    error.contains(
                        "build-time invocation has user source without reconciled package custody"
                    ),
                    "{error}"
                );
            }
            let (source, _) = typed_trees_to_typed_trees::prepare_typed_trees(
                typed,
                &typed_trees_to_typed_trees::PreparationRequest::settled(),
            )
            .expect("typed preparation refuses nothing in this source");
            let source = Arc::new(source);
            let full_subject = |subject| {
                psi_terminal_production::AcceptedTerminalSource::judge(
                    Arc::clone(&source),
                    psi_terminal_production::TerminalSourceCompilation {
                        entries: &[callee],
                        subjects: &[subject],
                        purpose: source::DependencyScope::Product,
                        selected: &[],
                        bindings: &[],
                    },
                    terminal_codec::TerminalPublication::Portable,
                )
            };
            if owner != unused_owner {
                full_subject(owner)
                    .expect("the package request excludes unrelated standalone source");
            }
            let full = full_subject(unused_owner)
                .expect_err("the explicit subject still owes every declaration");
            assert!(
                diagnostics::join_messages(&full).contains("type generic data"),
                "{full:?}"
            );
        }
    }

    #[test]
    fn source_jobs_authenticate_the_requesters_direct_dependency_selection() {
        use semantic_vocabulary::PackageKeyIdentity;
        let requester_package = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let dependency_package = PackageKeyIdentity::from_digest([2; 32]).unwrap();
        let caller_text = "module customer; machine request() -> u64 { 0u64 }";
        let callee_text = "module dependency; public machine answer(value: u64) -> u64 requires value > 0 { value }";
        let mut sources = source::SourceMap::default();
        let caller_source = sources
            .add_with_metadata(
                "/job/request.omg".into(),
                caller_text.into(),
                "/job".into(),
                Some(requester_package),
                source::SourceOrigin::User,
            )
            .source_id;
        let callee_source = sources
            .add_with_metadata(
                "/dependency/answer.omg".into(),
                callee_text.into(),
                "/dependency".into(),
                Some(dependency_package),
                source::SourceOrigin::User,
            )
            .source_id;
        let mut syntax = syntax_trees::SyntaxTrees::new(caller_source);
        for (source, text) in [(caller_source, caller_text), (callee_source, callee_text)] {
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            tokens_to_syntax_trees::parse(&mut syntax, source, &tokens).unwrap();
        }
        let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
        request.sources = Some(Arc::new(sources));
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request)
            .expect("two actual source owners");
        let source = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type both source declarations");
        let selected = |package, name: &str| {
            source
                .machines()
                .iter()
                .find(|machine| {
                    source.symbols.symbol_package_identity(machine.symbol) == Some(package)
                        && source.symbols.name(machine.symbol) == name
                })
                .unwrap()
                .symbol
        };
        let requester = selected(requester_package, "request");
        let callee = selected(dependency_package, "answer");
        let entry = PreparedBuildMachineProgram::prepare(&source, None)
            .unwrap()
            .entry(requester, callee)
            .unwrap();
        let (accepted, _) = entry
            .accept_invocation(&[BuildTimeValue::Int(1)])
            .expect("actual caller checks the arguments");
        let caller = *accepted
            .closure()
            .elaboration
            .machines
            .get(accepted.entry().unwrap())
            .unwrap();
        assert_eq!(
            accepted.source().symbols.get(caller).generated_from,
            requester
        );
        assert_eq!(
            accepted.source().symbols.symbol_package_identity(caller),
            Some(requester_package)
        );
        let produced = entry.produce_invocation(&accepted).unwrap();
        let error = entry
            .admit_invocation(&produced, target())
            .expect_err("the dependency cannot request itself for the customer");
        assert!(
            error.contains("no direct dependency selection authority"),
            "{error}"
        );
        let mut meter = psi_work::WorkMeter::unbounded();
        assert_eq!(
            entry
                .invoke(target(), &[BuildTimeValue::Int(1)], &mut meter)
                .unwrap_err(),
            error
        );
        assert_eq!(meter.usage().total_units(), 0);

        struct DirectDependency {
            requester: PackageKeyIdentity,
            dependency: PackageKeyIdentity,
        }
        impl BuildTimeSelectionAuthority for DirectDependency {
            fn allows_declaration_selection(
                &self,
                requester: PackageKeyIdentity,
                owner: PackageKeyIdentity,
            ) -> bool {
                requester == self.requester && owner == self.dependency
            }
            fn package_label(&self, package: PackageKeyIdentity) -> String {
                format!("{package:?}")
            }
        }
        let authority = Arc::new(DirectDependency {
            requester: requester_package,
            dependency: dependency_package,
        });
        let entry = PreparedBuildMachineProgram::prepare(&source, Some(authority))
            .unwrap()
            .entry(requester, callee)
            .unwrap();
        let mut accounting = crate::SourceCompletionAccounting::default();
        let value = entry
            .invoke_recorded(target(), &[BuildTimeValue::Int(1)], &mut accounting)
            .expect("the actual direct edge is selected");
        assert_eq!(value.value(), &BuildTimeValue::Int(1));
        let [crate::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
            panic!("one actual receipt")
        };
        assert_eq!(receipt.requester(), requester);
        assert_eq!(receipt.machine(), callee);
        assert_eq!(
            receipt.admission().requester(),
            semantic_vocabulary::compiler_evaluation::DeclarationCustody::Package(
                requester_package
            )
        );
        assert_eq!(receipt.admission().target(), target().name);
    }
}
