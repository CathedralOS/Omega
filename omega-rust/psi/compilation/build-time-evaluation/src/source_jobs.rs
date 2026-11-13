//! Source jobs do not execute through retired tree/checked-program engines.
use crate::BuildTimeSelectionAuthority;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

mod invocation;

fn unavailable(job: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("no product route yet: {job}"))]
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedLayoutTargetScope {
    pub target: String,
    pub facts: Option<layout_plans::LayoutTargetFacts>,
    pub entry_declarations: Option<Vec<symbols::SymbolHandle>>,
    pub provider_declarations: Vec<crate::product::provider_declarations::ProviderSelection>,
}
pub fn has_fixed_layout_applications(typed: &TypedTrees) -> bool {
    typed_trees::references::collect_program_type_occurrences(typed)
        .into_iter()
        .any(|occurrence| {
            typed
                .layout_storage_arguments(occurrence.reference)
                .is_some()
        })
}
pub fn evaluate_fixed_layout_applications(
    typed: &mut TypedTrees,
    _scopes: &[FixedLayoutTargetScope],
    _authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    if has_fixed_layout_applications(typed) {
        return Err(unavailable("source layout-policy evaluation"));
    }
    Ok(())
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
pub fn admit_toolchain_build_activation(_typed: &TypedTrees) -> Result<bool, Vec<Diagnostic>> {
    Err(unavailable("source Build activation admission"))
}

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
        let accepted = self.accept_invocation(arguments)?;
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
        let accepted = self.accept_invocation(arguments)?;
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
    ) -> Result<psi_terminal_production::AcceptedTerminalSource, String> {
        let machine = self
            .source
            .machine_by_symbol(self.symbol)
            .ok_or("source invocation lost its selected declaration")?;
        let (source, caller) = invocation::prepare_value_invocation(
            &self.source,
            self.requester,
            machine,
            arguments,
            None,
        )?;
        let subject = source.symbols.symbol_package_identity(self.requester);
        let source = Arc::new(typed_trees::FrozenTypedTrees::from(source));
        let accepted = psi_terminal_production::AcceptedTerminalSource::judge(
            source,
            psi_terminal_production::TerminalSourceCompilation {
                entries: &[caller],
                subject,
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
        Ok(accepted)
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
        if self.source.machine_by_symbol(symbol).is_none() {
            return Err("no product route yet: source Build entry is unavailable".into());
        }
        Ok(PreparedBuildMachineEntry {
            source: Arc::clone(&self.source),
            symbol,
            requester,
            authority: self.authority.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
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
    fn typed(source: &str) -> TypedTrees {
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
        let another = entry
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
        let accepted = entry
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
        let accepted = entry
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
