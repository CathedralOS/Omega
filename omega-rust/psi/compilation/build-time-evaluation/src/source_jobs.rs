//! Evaluate source computations through the same checker used by programs.
//! Each invocation retains its actual caller, inputs and execution admission.
use crate::BuildTimeSelectionAuthority;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(crate) mod access_plan_values;
mod build_activation;
mod const_argument_jobs;
mod constant_jobs;
mod domain_access_plans;
mod fixed_layouts;
mod invocation;
mod source_selection;

pub use const_argument_jobs::evaluate as evaluate_const_arguments;
pub use constant_jobs::{
    evaluate as evaluate_constant_initializers,
    evaluate_with_source_consent as evaluate_constant_initializers_with_source_consent,
    validate_constant_evaluations,
};
pub use domain_access_plans::evaluate as evaluate_domain_access_plans;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedLayoutTargetScope {
    pub target: String,
    pub facts: Option<layout_plans::LayoutTargetFacts>,
    /// Exact computation roots. An empty list never requests library-wide geometry.
    pub entry_declarations: Vec<symbols::SymbolHandle>,
    pub provider_declarations: Vec<crate::product::provider_declarations::ProviderSelection>,
}
/// Evaluate the layout policies needed by the requested entry declarations.
/// Unused applications remain deferred at their owning declarations.
pub fn evaluate_fixed_layout_applications(
    typed: &mut TypedTrees,
    scopes: &[FixedLayoutTargetScope],
    authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    consent: Option<Arc<dyn crate::InvocationSourceConsent>>,
    purpose: source::DependencyScope,
    accounting: &mut crate::SourceCompletionAccounting,
) -> Result<(), Vec<Diagnostic>> {
    fixed_layouts::evaluate(typed, scopes, authority, consent, purpose, accounting)
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
pub use build_activation::admit_toolchain_build_activation;

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
    purpose: source::DependencyScope,
    source_consent: Option<Arc<dyn crate::InvocationSourceConsent>>,
}
#[derive(Clone)]
pub struct PreparedBuildMachineEntry {
    source: Arc<TypedTrees>,
    symbol: SymbolHandle,
    requester: SymbolHandle,
    call: typed_trees::expression::TableCallExpression,
    call_source: typed_trees::expression::ExpressionHandle,
    authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    purpose: source::DependencyScope,
    source_consent: Option<Arc<dyn crate::InvocationSourceConsent>>,
    accepted: Arc<terminal_production::AcceptedTerminalSource>,
    arguments: Vec<crate::BuildTimeValue>,
    input_values: Vec<crate::BuildTimeValue>,
}
impl PreparedBuildMachineEntry {
    /// The physical callee in the accepted invocation’s source.
    pub const fn symbol(&self) -> SymbolHandle {
        self.symbol
    }
    pub const fn requester(&self) -> SymbolHandle {
        self.requester
    }

    pub fn source(&self) -> &TypedTrees {
        &self.source
    }

    pub(crate) fn retained_source(&self) -> Arc<TypedTrees> {
        Arc::clone(&self.source)
    }

    /// The actual caller and its arguments have already been judged together.
    /// Consumers may inspect that exact source before execution; neither the
    /// source nor a replacement argument list is accepted by the run method.
    pub fn accepted_source(&self) -> &typed_trees::FrozenTypedTrees {
        self.accepted.source()
    }

    pub fn call(&self) -> &typed_trees::expression::TableCallExpression {
        &self.call
    }

    pub fn arguments(&self) -> &[crate::BuildTimeValue] {
        &self.arguments
    }

    pub(crate) fn accepted(&self) -> &terminal_production::AcceptedTerminalSource {
        &self.accepted
    }

    pub(crate) const fn call_handle(&self) -> typed_trees::expression::ExpressionHandle {
        self.call_source
    }

    /// Run the call whose complete arguments were checked during preparation.
    /// Execution admission still checks the actual target and requester.
    pub fn invoke(
        &self,
        target: crate::InvocationTarget<'_>,
        meter: &mut terminal_work::WorkMeter,
    ) -> Result<crate::BuildTimeValue, String> {
        let produced = self.produce_invocation()?;
        let (admitted, requirements) = self.admit_invocation(&produced, target)?;
        self.execute_invocation(&produced, &admitted, requirements, target, meter)
    }

    /// Record the value and actual work/storage charges without treating local
    /// invocation code as portable or cached source-completion evidence.
    pub fn invoke_recorded(
        &self,
        target: crate::InvocationTarget<'_>,
        accounting: &mut crate::SourceCompletionAccounting,
    ) -> Result<crate::MeasuredEvaluation<crate::BuildTimeValue>, String> {
        let produced = Arc::new(self.produce_invocation()?);
        self.invoke_produced_recorded(produced, target, accounting)
    }

    /// Run a constant under each deployment that supplies its required
    /// layout observations or authored storage layouts. Retain one receipt
    /// per actual deployment. A computation with neither runs once portably.
    pub fn invoke_recorded_in_targets(
        &self,
        targets: &[(&str, Option<&terminal_installation::LayoutTargetFacts>)],
        accounting: &mut crate::SourceCompletionAccounting,
    ) -> Result<Vec<crate::BuildTimeValue>, String> {
        let produced = Arc::new(self.produce_invocation()?);
        let module = produced.view().module();
        if module.target_layout_applications.is_empty()
            && module.scalar_applications.declarations().is_empty()
        {
            let target = crate::InvocationTarget {
                name: None,
                layout_target: None,
                carrier_layout: None,
                address_model: None,
            };
            return Ok(vec![
                self.invoke_produced_recorded(produced, target, accounting)?
                    .into_value(),
            ]);
        }
        if targets.is_empty() {
            return Err(
                "the computation stores through an authored layout, and no deployment \
                 that reaches it selects that layout"
                    .into(),
            );
        }
        targets
            .iter()
            .map(|(name, facts)| {
                let facts = facts.ok_or_else(|| {
                    format!("deployment `{name}` has no complete layout facts for the layout it stores through")
                })?;
                let target = crate::InvocationTarget {
                    name: Some(name),
                    layout_target: Some(facts),
                    carrier_layout: None,
                    address_model: None,
                };
                self.invoke_produced_recorded(Arc::clone(&produced), target, accounting)
                    .map(crate::MeasuredEvaluation::into_value)
            })
            .collect()
    }

    fn invoke_produced_recorded(
        &self,
        produced: Arc<terminal_production::ProducedTerminalProgram>,
        target: crate::InvocationTarget<'_>,
        accounting: &mut crate::SourceCompletionAccounting,
    ) -> Result<crate::MeasuredEvaluation<crate::BuildTimeValue>, String> {
        let (admitted, requirements) = self.admit_invocation(&produced, target)?;
        let account = crate::evaluation_accounting::pure_terminal_invocation_account()?;
        let mut meter = terminal_work::WorkMeter::with_invocation(account.clone(), None)
            .map_err(|error| format!("source invocation meter: {error:?}"))?;
        let result = self
            .execute_invocation(&produced, &admitted, requirements, target, &mut meter)
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
            self,
            produced,
            admitted,
            outcome,
            meter.into_usage(),
        )?;
        accounting.append_invocation(receipt);
        result
    }

    fn produce_invocation(&self) -> Result<terminal_production::ProducedTerminalProgram, String> {
        self.accepted
            .produce(
                Default::default(),
                &mut terminal_production::UnmeasuredProduction,
            )
            .map_err(|errors| diagnostics::join_messages(&errors))
    }

    fn admit_invocation(
        &self,
        produced: &terminal_production::ProducedTerminalProgram,
        target: crate::InvocationTarget<'_>,
    ) -> Result<
        (
            terminal_installation::AdmittedCompilerEvaluation,
            terminal_interpreter::TerminalExecutionRequirements,
        ),
        String,
    > {
        if target.name.is_none()
            && (target.layout_target.is_some()
                || target.carrier_layout.is_some()
                || target.address_model.is_some())
        {
            return Err("target-independent evaluation cannot carry target observations".into());
        }
        for value in &self.input_values {
            if let crate::BuildTimeValue::LayoutTarget(facts) = value
                && (target.name != Some(facts.profile.as_str())
                    || target.layout_target != Some(facts))
            {
                return Err(
                    "issued layout context differs from the actual invocation target".into(),
                );
            }
        }
        use terminal_interpreter::TerminalProgram;
        let program = TerminalProgram::borrow(produced.view());
        let unselected =
            terminal_installation::admit_provider_execution_installation(produced.view(), &[], &[])
                .map_err(|error| format!("source invocation provider admission: {error:?}"))?;
        let layouts = Self::layout_supplies(produced, &unselected, target)?;
        let requirements = program
            .execution_requirements(layouts.as_ref())
            .map_err(|error| format!("source invocation demands: {error:?}"))?;
        if !requirements.imports.is_empty()
            || !requirements.missing_providers.is_empty()
            || !requirements.external_completions.is_empty()
            || (target.layout_target.is_none()
                && (!requirements.scalar_applications.is_empty()
                    || !requirements.scalar_entry_conditions.is_empty()))
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
        let source_selection = source_selection::InvocationSelection::new(self, view);
        let admitted = terminal_installation::admit_compiler_evaluation_with_source_consent(
            view,
            target.name,
            root,
            unselected,
            requester,
            self.authority.as_deref(),
            source_selection.as_ref().map(|selection| {
                selection as &dyn terminal_installation::InvocationSourceSelection
            }),
        )?;
        admitted.require_invocation(view, target.name, selected)?;
        Ok((admitted, requirements))
    }

    /// A storage application with an authored layout has bytes only under one
    /// deployment's admitted plan. Install that target's checked layouts so
    /// the invocation runs with the geometry its target closure selected;
    /// without layout facts the program keeps no layout supply and refuses
    /// at its first layout demand.
    fn layout_supplies(
        produced: &terminal_production::ProducedTerminalProgram,
        providers: &terminal_installation::AdmittedProviderSelections,
        target: crate::InvocationTarget<'_>,
    ) -> Result<Option<terminal_interpreter::TerminalRuntimeSupplies>, String> {
        let view = produced.view();
        let (Some(name), Some(facts)) = (target.name, target.layout_target) else {
            return Ok(None);
        };
        if view.module().target_layout_applications.is_empty() {
            return Ok(None);
        }
        let selected = terminal_installation::terminal_target_context(view, name);
        let layouts = terminal_installation::admit_target_layouts(view, &selected, facts)
            .map_err(|error| format!("source invocation target layouts: {error:?}"))?;
        Self::runtime_supplies(view, providers, Some(&selected), Some(&layouts)).map(Some)
    }

    fn runtime_supplies(
        view: terminal_codec::TerminalExecutionView<'_>,
        providers: &terminal_installation::AdmittedProviderSelections,
        selected: Option<&terminal_installation::TerminalTargetContext>,
        layouts: Option<&terminal_installation::AdmittedTargetLayouts>,
    ) -> Result<terminal_interpreter::TerminalRuntimeSupplies, String> {
        terminal_interpreter::TerminalProgram::borrow(view)
            .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                native_code_layouts: None,
                x86_flags: None,
                memory_ordering: None,
                entry: None,
                installation: Some(providers),
                host_imports: &[],
                target_machines: selected,
                target_layouts: layouts,
            })
            .map_err(|error| format!("source invocation target supplies: {error:?}"))
    }

    fn execute_invocation(
        &self,
        produced: &terminal_production::ProducedTerminalProgram,
        admitted: &terminal_installation::AdmittedCompilerEvaluation,
        requirements: terminal_interpreter::TerminalExecutionRequirements,
        target: crate::InvocationTarget<'_>,
        meter: &mut terminal_work::WorkMeter,
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
        let view = produced.view();
        let providers = admitted.providers();
        let mut requested = requirements.scalar_applications;
        requested.extend(requirements.scalar_entry_conditions);
        requested.sort_unstable();
        requested.dedup();
        let scalars = if requested.is_empty() {
            None
        } else {
            let name = target
                .name
                .ok_or("source scalar application has no deployment")?;
            let targets = terminal_installation::terminal_target_context(view, name);
            let declaration = view
                .module()
                .scalar_applications
                .proof_scope
                .callable_declaration(selected)
                .ok_or("source scalar application has no exact invocation root")?;
            Some(crate::complete_invocation_scalar_applications(
                view,
                target,
                &targets,
                providers,
                declaration,
                &requested,
                meter,
            )?)
        };
        let installation = if let Some(scalars) = &scalars {
            let scope = scalars.primary();
            Self::runtime_supplies(view, scope.providers(), scope.targets(), scope.layouts())?
        } else {
            Self::layout_supplies(produced, providers, target)?
                .unwrap_or_else(|| terminal_interpreter::TerminalRuntimeSupplies::from(providers))
        };
        let program = TerminalProgram::borrow(view);
        let entry = program
            .entry()
            .map_err(|error| format!("source invocation entry: {error:?}"))?;
        if !entry.scalar_parameters.is_empty()
            || entry.structural_parameters.len() != self.input_values.len()
        {
            return Err("source invocation retained unmatched host input parameters".into());
        }
        let inputs = entry
            .structural_parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                let declaration = program
                    .module()
                    .structural_type(parameter.structural_type)
                    .ok_or("source input lost its canonical declaration")?;
                let issued = matches!(
                    self.input_values[position],
                    crate::BuildTimeValue::LayoutTarget(_)
                );
                if (issued
                    && (declaration.shape != terminal_psi::StructuralTypeShape::Opaque
                        || parameter.access != terminal_psi::StructuralAccess::Owned))
                    || !matches!(
                        parameter.access,
                        terminal_psi::StructuralAccess::Owned
                            | terminal_psi::StructuralAccess::SharedBorrow
                    )
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || !parameter.additional_input_qualifications.is_empty()
                {
                    return Err("source input changed its checked input contract");
                }
                let identity = u64::try_from(position)
                    .ok()
                    .and_then(|position| position.checked_add(1))
                    .ok_or("source input identity exceeds its host namespace")?;
                Ok(terminal_interpreter::TerminalStructuralValue::minted(
                    identity,
                    parameter.structural_type,
                    Vec::new(),
                ))
            })
            .collect::<Result<Vec<_>, &str>>()?;
        let contents = entry
            .structural_parameters
            .iter()
            .zip(&self.input_values)
            .enumerate()
            .filter(|(_, (_, value))| !matches!(value, crate::BuildTimeValue::LayoutTarget(_)))
            .map(|(position, (parameter, value))| {
                crate::terminal_values::input_contents(
                    &program,
                    u32::try_from(position)
                        .map_err(|_| "source input exceeds its argument namespace")?,
                    parameter.structural_type,
                    value,
                )
            })
            .collect::<Result<Vec<_>, String>>()?;
        let opaque_inputs = inputs.iter().zip(&self.input_values).collect::<Vec<_>>();
        let result_type = entry
            .result
            .structural()
            .map(|result| result.structural_type);
        let account = meter.invocation_account().cloned();
        let mut instance = program
            .instantiate(TerminalInstantiation {
                account: account.as_ref(),
                installation: Some(&installation),
                scalar_admission: scalars.as_ref(),
                structural_inputs: terminal_interpreter::TerminalStructuralInputs {
                    arguments: &inputs,
                    contents: &contents,
                    ..Default::default()
                },
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
                crate::terminal_values::scalar_result(instance.program(), value)
            }
            TerminalExecutionStatus::Complete(
                TerminalExecutionResult::Structural(_)
                | TerminalExecutionResult::ScalarCase(_)
                | TerminalExecutionResult::ScalarArray(_),
            ) => crate::terminal_values::result_value(
                &instance,
                result_type.ok_or("source invocation lost its structured result type")?,
                &opaque_inputs,
            ),
            other => Err(format!(
                "source invocation did not complete normally: {other:?}"
            )),
        }
    }
}
fn invocation_error(reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}

impl PreparedBuildMachineProgram {
    pub fn prepare(
        typed: &TypedTrees,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::prepare_for(typed, authority, source::DependencyScope::Product, None)
    }

    /// Retain the caller's actual source purpose and captured-source consent.
    /// A private capture remains unowned; it is not a package-selection grant.
    pub fn prepare_for(
        typed: &TypedTrees,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        purpose: source::DependencyScope,
        source_consent: Option<Arc<dyn crate::InvocationSourceConsent>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        // Retaining source is preparation, not acceptance. Each actual job is
        // checked with its materialized arguments before any execution starts.
        Ok(Self {
            source: Arc::new(typed.clone()),
            authority,
            purpose,
            source_consent,
        })
    }
    fn prepare_call(
        &self,
        requester: SymbolHandle,
        declaration: typed_trees::callable_signature_shapes::CallableDeclaration<'_>,
        request: &typed_trees::expression::TableCallExpression,
        request_expression: typed_trees::expression::ExpressionHandle,
        arguments: &[crate::BuildTimeValue],
        selected: Option<(
            &typed_trees::expression::StaticMachineArgument,
            SymbolHandle,
        )>,
        supplies: &[terminal_production::TerminalSourceSupply],
    ) -> Result<PreparedBuildMachineEntry, Vec<Diagnostic>> {
        if !self.source.symbols.symbols().nodes().is_valid(requester)
            || self
                .source
                .symbols
                .symbol_provenance_source_span(requester)
                .is_none()
        {
            return Err(invocation_error(
                "source job requires its actual requesting declaration",
            ));
        }
        let invocation::PreparedSourceInvocation {
            source,
            caller,
            call,
            input_values,
        } = invocation::prepare_value_invocation(
            &self.source,
            requester,
            declaration,
            request,
            arguments,
            request_expression,
            selected,
        )
        .map_err(invocation_error)?;
        // The call's source reaches the checker the way every program does,
        // through typed preparation; there is no second way to freeze it.
        let (source, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            source,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )?;
        let source = Arc::new(source);
        let accepted = terminal_production::AcceptedTerminalSource::judge(
            source,
            terminal_production::TerminalSourceCompilation {
                declaration_metadata: &[],
                entries: &[caller],
                // Evaluating this call requests its dependency closure, not a
                // full check of the requester's package or standalone source.
                subjects: &[],
                purpose: self.purpose,
                selected: &[],
                bindings: &[],
                calling: None,
                providers: None,
                supplies,
            },
            terminal_codec::TerminalPublication::Invocation,
        )?;
        let entry = accepted
            .entry()
            .ok_or_else(|| invocation_error("source invocation has no accepted entry"))?;
        if accepted.closure().elaboration.machines.get(entry) != Some(&caller) {
            return Err(invocation_error(
                "source invocation accepted another caller",
            ));
        }
        let actual_call = accepted
            .source()
            .expression_table
            .call_node(call)
            .cloned()
            .ok_or_else(|| invocation_error("source invocation lost its accepted call"))?;
        let physical = match declaration {
            typed_trees::callable_signature_shapes::CallableDeclaration::Machine(required) => {
                let physical = accepted
                    .source()
                    .machine_by_symbol(actual_call.target_symbol)
                    .or_else(|| {
                        accepted
                            .source()
                            .machine_holding_state(actual_call.target_symbol)
                    })
                    .ok_or_else(|| {
                        invocation_error("source invocation has no accepted concrete body")
                    })?;
                if physical.symbol != required
                    && accepted
                        .source()
                        .machine_instantiation_path(physical.symbol, required)
                        .is_none()
                {
                    return Err(invocation_error(
                        "source invocation changed its required concrete declaration",
                    ));
                }
                physical.symbol
            }
            typed_trees::callable_signature_shapes::CallableDeclaration::Signature(required) => {
                let original = self
                    .source
                    .closed_requirement_application(required.symbol)
                    .map_or(required.symbol, |application| application.template);
                let actual_required = actual_call
                    .static_requirement_dispatch
                    .as_ref()
                    .map(|dispatch| dispatch.requirement)
                    .or_else(|| {
                        accepted
                            .source()
                            .closed_requirement_application(actual_call.target_symbol)
                            .map(|application| application.template)
                    })
                    .unwrap_or(actual_call.target_symbol);
                if actual_required != original {
                    return Err(invocation_error(
                        "selected source invocation changed its public requirement",
                    ));
                }
                let mut sites = accepted.source_call_sites(caller, call);
                let (canonical_caller, site) = sites.next().ok_or_else(|| {
                    invocation_error("selected source invocation has no exact emitted call site")
                })?;
                if sites.next().is_some() {
                    return Err(invocation_error(
                        "selected source invocation has multiple emitted call sites",
                    ));
                }
                let module = &accepted.closure().elaboration.module;
                let invocation = module
                    .machine(canonical_caller)
                    .and_then(|machine| {
                        machine
                            .module_invocations()
                            .find(|invocation| invocation.site == site)
                    })
                    .ok_or_else(|| {
                        invocation_error("selected source invocation lost its canonical operation")
                    })?;
                let required_contract = invocation.inputs.required_contract.ok_or_else(|| {
                    invocation_error("selected source invocation lost its public contract")
                })?;
                let contract = module
                    .invocation_machine_contract(invocation.inputs.callee, Some(required_contract))
                    .ok_or_else(|| {
                        invocation_error(
                            "selected source invocation has no exact public contract and body",
                        )
                    })?;
                let applied = accepted
                    .source()
                    .source_call_requirement_signature(
                        caller,
                        typed_trees::references::StaticArgumentCall::Expression(call, &actual_call),
                    )
                    .map_err(invocation_error)?;
                let (owner, _) = accepted
                    .source()
                    .trait_machine_signature(applied.symbol)
                    .ok_or_else(|| {
                        invocation_error(
                            "selected source invocation lost its public declaring trait",
                        )
                    })?;
                let identity = accepted
                    .source()
                    .trait_requirement_callable_identity_text(owner, applied);
                if contract
                    .declaration
                    .as_ref()
                    .and_then(|declaration| module.identities.text(declaration.callable))
                    != Some(identity.as_str())
                    || contract.selected_application.is_none()
                {
                    return Err(invocation_error(
                        "selected source invocation changed its applied public signature",
                    ));
                }
                *accepted
                    .closure()
                    .elaboration
                    .machines
                    .get(invocation.inputs.callee)
                    .ok_or_else(|| {
                        invocation_error("selected source invocation lost its actual body source")
                    })?
            }
        };
        Ok(PreparedBuildMachineEntry {
            source: Arc::clone(&self.source),
            symbol: physical,
            requester,
            call: actual_call,
            call_source: call,
            authority: self.authority.clone(),
            purpose: self.purpose,
            source_consent: self.source_consent.clone(),
            accepted: Arc::new(accepted),
            arguments: arguments.to_vec(),
            input_values,
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
        arguments: &[crate::BuildTimeValue],
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
        let declaration = self.source.source_call_declaration(
            requester,
            typed_trees::references::StaticArgumentCall::Expression(expression, call),
        )?;
        if self
            .source
            .expression_table
            .expression_handles(call.arguments)
            .len()
            != declaration
                .shape(&self.source)
                .map_err(str::to_owned)?
                .parameters
                .len()
        {
            return Err("source job call lost its complete argument list".into());
        }
        self.prepare_call(
            requester,
            declaration,
            call,
            expression,
            arguments,
            None,
            &[],
        )
        .map_err(|errors| diagnostics::join_messages(&errors))
    }

    pub fn entry(
        &self,
        requester: SymbolHandle,
        symbol: SymbolHandle,
        arguments: &[crate::BuildTimeValue],
    ) -> Result<PreparedBuildMachineEntry, String> {
        self.prepare_entry(requester, symbol, arguments)
            .map_err(|errors| diagnostics::join_messages(&errors))
    }

    fn prepare_entry(
        &self,
        requester: SymbolHandle,
        symbol: SymbolHandle,
        arguments: &[crate::BuildTimeValue],
    ) -> Result<PreparedBuildMachineEntry, Vec<Diagnostic>> {
        if !self.source.symbols.symbols().nodes().is_valid(requester)
            || self
                .source
                .symbols
                .symbol_provenance_source_span(requester)
                .is_none()
        {
            return Err(invocation_error(
                "source job requires its actual requesting declaration",
            ));
        }
        let machine = self.source.machine_by_symbol(symbol).ok_or_else(|| {
            invocation_error("no product route yet: source Build entry is unavailable")
        })?;
        let call = invocation::direct_call(&self.source, machine).map_err(invocation_error)?;
        self.prepare_call(
            requester,
            typed_trees::callable_signature_shapes::CallableDeclaration::Machine(machine.symbol),
            &call,
            typed_trees::expression::ExpressionHandle::invalid(),
            arguments,
            None,
            &[],
        )
    }

    /// Construct an ordinary call through an exact named implementation. The
    /// operand describes a requested choice; the returned entry has still
    /// passed the same actual-argument and independent refinement judgments.
    /// Domain consumers must separately authenticate their owning source
    /// coordinate against this entry before adopting its result as a policy.
    pub fn entry_selected(
        &self,
        requester: SymbolHandle,
        selection_owner: SymbolHandle,
        selection: &typed_trees::expression::StaticMachineArgument,
        requirement: SymbolHandle,
        arguments: &[crate::BuildTimeValue],
    ) -> Result<PreparedBuildMachineEntry, String> {
        use typed_trees::type_identity::{
            DeclaredStaticOperandIdentityRequest, IndexIdentityArgument,
        };
        if !self
            .source
            .symbols
            .symbols()
            .nodes()
            .is_valid(selection_owner)
            || self
                .source
                .symbols
                .symbol_provenance_source_span(selection_owner)
                .is_none()
        {
            return Err("selected source invocation lost its actual supplying declaration".into());
        }
        if !self
            .source
            .symbols
            .symbols()
            .nodes()
            .is_valid(selection.symbol)
            || self.source.symbols.get(selection.symbol).kind != symbols::SymbolKind::Conformance
        {
            return Err(
                "selected source invocation has no retained implementation declaration".into(),
            );
        }
        let mut operands = vec![selection];
        while let Some(operand) = operands.pop() {
            if operand.symbol.is_valid()
                && !self
                    .source
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(operand.symbol)
            {
                return Err("selected source invocation has a stale operand declaration".into());
            }
            if operand.type_reference.is_valid()
                && !self
                    .source
                    .type_reference_table
                    .contains_type_reference(operand.type_reference)
            {
                return Err("selected source invocation has a stale operand reference".into());
            }
            if let Some(application) = &operand.application {
                operands.extend(application.arguments.iter());
            }
        }
        self.source
            .static_operand_region_supplier(selection, selection_owner)?;
        let identity = |operand| {
            self.source
                .declared_static_operand_identity(DeclaredStaticOperandIdentityRequest {
                    operand: IndexIdentityArgument::StaticArgument(operand),
                    lexical_owner: selection_owner,
                    binders: &[],
                    lifetime_binders: &[],
                })
                .ok_or("selected source invocation has no complete operand identity")
        };
        let retained = identity(selection)?;
        let mut explicit = selection.clone();
        explicit.type_reference = Default::default();
        if identity(&explicit)? != retained {
            return Err("selected source invocation changed its complete operand tuple".into());
        }
        // This is the requested public declaration only. The probe closes its
        // exact application and authenticates the supplying row before reading
        // the actual parameter and result types.
        let (_, signature) = self
            .source
            .trait_machine_signature(requirement)
            .ok_or("selected source invocation needs a public requirement declaration")?;
        if selection.application.as_ref().is_some_and(|application| {
            application.member.is_valid() || !application.lifetime_arguments.is_empty()
        }) || selection.const_literal.is_some()
            || selection.evidence_projection.is_some()
        {
            return Err(
                "selected source invocation needs its prepared complete application".into(),
            );
        }
        let call = invocation::declaration_call(signature.symbol, signature.name.clone());
        self.prepare_call(
            requester,
            typed_trees::callable_signature_shapes::CallableDeclaration::Signature(signature),
            &call,
            typed_trees::expression::ExpressionHandle::invalid(),
            arguments,
            Some((selection, selection_owner)),
            &[],
        )
        .map_err(|errors| diagnostics::join_messages(&errors))
    }

    /// Invoke the selected implementation through its original requirement.
    /// The same binding enters judgment with the complete materialized call.
    pub fn entry_for_supply(
        &self,
        requester: SymbolHandle,
        supply: terminal_production::TerminalSourceSupply,
        arguments: &[crate::BuildTimeValue],
    ) -> Result<PreparedBuildMachineEntry, String> {
        self.source
            .trait_machine_signature(supply.slot)
            .filter(|(_, requirement)| requirement.symbol == supply.slot)
            .ok_or("source invocation requires an exact nominal requirement")?;
        let machine = supply.realization(&self.source, supply.slot)?;
        let call = invocation::direct_call(&self.source, machine)?;
        self.prepare_call(
            requester,
            typed_trees::callable_signature_shapes::CallableDeclaration::Machine(machine.symbol),
            &call,
            typed_trees::expression::ExpressionHandle::invalid(),
            arguments,
            None,
            &[supply],
        )
        .map_err(|errors| diagnostics::join_messages(&errors))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::BuildTimeValue;

    fn target() -> crate::InvocationTarget<'static> {
        crate::InvocationTarget {
            name: Some("linux_x86_64"),
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

    fn program(source: &str) -> PreparedBuildMachineProgram {
        PreparedBuildMachineProgram::prepare(&typed(source), None).unwrap()
    }

    fn bind(
        program: &PreparedBuildMachineProgram,
        arguments: &[BuildTimeValue],
    ) -> Result<PreparedBuildMachineEntry, String> {
        let symbol = program
            .typed()
            .realized_machine_named("answer")
            .expect("named source entry")
            .symbol;
        program.entry(symbol, symbol, arguments)
    }

    fn run(
        program: &PreparedBuildMachineProgram,
        arguments: &[BuildTimeValue],
        meter: &mut terminal_work::WorkMeter,
    ) -> Result<BuildTimeValue, String> {
        bind(program, arguments)?.invoke(target(), meter)
    }

    fn with_reflection(source: &str) -> TypedTrees {
        // Reflection imports ordinary core declarations. Keep its complete
        // real source closure and the same test author's package identity.
        let library =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../source/library");
        super::schema_customer_source::actual_library_source(
            &library,
            source,
            semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32])
                .expect("the existing reflection author package"),
        )
        .expect("resolve and type the actual core source and author")
    }

    #[test]
    fn layout_schema_members_keep_their_applied_type_without_lifetime_bindings() {
        // Every repeated schema field uses the same closed Optional<u64>.
        // Its enclosing SchemaField has no lifetime arguments to apply.
        let mut source = with_reflection(
            "use omega::language::core::SchemaField;
             machine retain_schema_member_type(field: SchemaField) {}",
        );
        let machine = source
            .realized_machine_named("retain_schema_member_type")
            .unwrap();
        let Some([entry]) = source.machine_states(machine).as_array() else {
            panic!("one entry state");
        };
        let Some([field]) = source.state_parameters(entry).as_array() else {
            panic!("one schema field parameter");
        };
        let enclosing = field.type_reference;
        let schema = source.closed_data_definition(enclosing).unwrap();
        let identity = source
            .data_members(schema)
            .iter()
            .filter_map(typed_trees::data::DataMember::as_field)
            .find(|field| field.name.as_str() == "identity")
            .unwrap()
            .type_reference;
        assert!(source.closed_data_definition(identity).is_some());

        let applied = source
            .instantiate_data_member_type(identity, enclosing)
            .unwrap();
        assert_eq!(applied, identity);
    }

    #[test]
    fn source_jobs_transport_only_the_issued_layout_context() {
        // Passing the opaque input through another checked call must return
        // that actual input. Neither a record with its name nor target facts
        // from a different invocation may stand in for the supplied context.
        let source = with_reflection(
            "use omega::language::core::LayoutTarget;
             machine forward(value: LayoutTarget) -> LayoutTarget
             reaches none terminates { value }
             machine answer(value: LayoutTarget) -> LayoutTarget
             reaches none terminates { forward(value) }
             data Envelope { context: LayoutTarget; tag: u32; }
             machine transport(value: Envelope) -> Envelope reaches none terminates { value }",
        );
        let facts = layout_plans::LayoutTargetFacts {
            profile: "linux_x86_64".into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        };
        let target = crate::InvocationTarget {
            name: Some(&facts.profile),
            layout_target: Some(&facts),
            carrier_layout: None,
            address_model: None,
        };
        let selected = query_machine(&source, "answer");
        let prepared = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
        let value = BuildTimeValue::LayoutTarget(facts.clone());
        let entry = prepared
            .entry(selected, selected, std::slice::from_ref(&value))
            .unwrap();
        let mut meter = terminal_work::WorkMeter::unbounded();
        assert_eq!(entry.invoke(target, &mut meter).unwrap(), value);
        assert!(meter.usage().total_units() > 0);
        assert_eq!(entry.arguments(), std::slice::from_ref(&value));

        let wrapped = BuildTimeValue::Struct {
            type_name: "Envelope".into(),
            fields: vec![
                ("context".into(), value.clone()),
                ("tag".into(), BuildTimeValue::Int(7)),
            ],
        };
        let transport = query_machine(&source, "transport");
        let entry = prepared
            .entry(transport, transport, std::slice::from_ref(&wrapped))
            .unwrap();
        let returned = entry
            .invoke(target, &mut terminal_work::WorkMeter::unbounded())
            .unwrap();
        let BuildTimeValue::Struct { fields, .. } = returned else {
            panic!("the real enclosing record must remain a record")
        };
        assert_eq!(
            fields,
            vec![
                ("context".into(), value.clone()),
                ("tag".into(), BuildTimeValue::Int(7))
            ]
        );

        let forged = BuildTimeValue::Struct {
            type_name: "LayoutTarget".into(),
            fields: Vec::new(),
        };
        let refusal = prepared.entry(selected, selected, &[forged]).err().unwrap();
        assert!(
            refusal.contains("opaque") && refusal.contains("literal"),
            "{refusal}"
        );

        let mut other = facts.clone();
        other.pointer_size = 4;
        let entry = prepared
            .entry(selected, selected, &[BuildTimeValue::LayoutTarget(other)])
            .unwrap();
        let mut meter = terminal_work::WorkMeter::unbounded();
        assert!(
            entry
                .invoke(target, &mut meter)
                .unwrap_err()
                .contains("differs from the actual invocation target")
        );
        assert_eq!(meter.usage().total_units(), 0);

        let lookalike = program(
            "boundary data LayoutTarget;
             machine answer(value: LayoutTarget) -> LayoutTarget { value }",
        );
        let refusal = bind(&lookalike, std::slice::from_ref(&value))
            .err()
            .unwrap();
        assert!(
            refusal.contains("exact unqualified core declaration"),
            "{refusal}"
        );
    }

    #[test]
    fn an_empty_layout_scope_preserves_the_unrequested_application() {
        // The same kept application must remain a real deferred request when
        // no computation asks for its geometry; no successful plan is invented.
        let mut source = with_reflection(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../samples/cli/facts/fixed_layout_storage/local/main.omg"
        )));
        let entry = query_machine(&source, "main");
        let facts = layout_plans::LayoutTargetFacts {
            profile: "linux_x86_64".into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        };
        let scope = FixedLayoutTargetScope {
            target: facts.profile.clone(),
            facts: Some(facts),
            entry_declarations: Vec::new(),
            provider_declarations: Vec::new(),
        };
        let mut foreign = scope.clone();
        foreign.facts.as_mut().unwrap().profile = "windows_x86_64".into();
        let mut accounting = crate::SourceCompletionAccounting::default();
        let errors = evaluate_fixed_layout_applications(
            &mut source,
            &[foreign],
            None,
            None,
            source::DependencyScope::Product,
            &mut accounting,
        )
        .expect_err("empty roots do not authenticate another target's facts");
        assert!(
            diagnostics::join_messages(&errors)
                .contains("layout scope and immutable target facts disagree")
        );
        assert!(accounting.receipts().is_empty());
        evaluate_fixed_layout_applications(
            &mut source,
            &[scope],
            None,
            None,
            source::DependencyScope::Product,
            &mut accounting,
        )
        .expect("an unused layout application needs no policy invocation");
        assert!(source.plan_laid_layouts.is_empty());
        assert!(source.plan_laid_target_layouts.is_empty());
        assert!(accounting.receipts().is_empty());
        assert!(source.deferred_refusals_of(entry).any(|row| {
            row.request == typed_trees::deferred_refusals::DeferredRequest::LayoutPolicyStorage
        }));
    }

    #[test]
    fn a_layout_storage_constant_folds_one_snapshot_from_every_deployment() {
        // `SAVED` is computed by storing through `LayoutStorage<Pair,
        // PairLayout>`, so its bytes exist only under a selected plan. It runs
        // once per deployment that reads it and keeps the one semantic value
        // every target agrees on; with no deployment it has no layout at all.
        let mut source = with_reflection(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../samples/cli/facts/fixed_layout_storage/const_name/main.omg"
        )));
        let entry = query_machine(&source, "main");
        let scopes = ["linux_x86_64", "macos_arm64"].map(|target| FixedLayoutTargetScope {
            target: target.into(),
            facts: Some(layout_plans::LayoutTargetFacts {
                profile: target.into(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            }),
            entry_declarations: vec![entry],
            provider_declarations: Vec::new(),
        });
        let mut accounting = crate::SourceCompletionAccounting::default();
        evaluate_fixed_layout_applications(
            &mut source,
            &scopes,
            None,
            None,
            source::DependencyScope::Product,
            &mut accounting,
        )
        .unwrap();
        // Source preparation keeps each initializer as an ordinary machine.
        let initializers = source
            .const_declarations()
            .iter()
            .filter(|constant| source.tables.machines.is_valid(constant.evaluation_entry))
            .map(|constant| {
                source
                    .tables
                    .machines
                    .get(constant.evaluation_entry)
                    .clone()
            })
            .filter(|machine| source.machine_by_symbol(machine.symbol).is_none())
            .collect::<Vec<_>>();
        for machine in initializers {
            source.push_machine(machine);
        }
        let request = terminal_production::TerminalSourceCompilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        };
        let errors = crate::evaluate_constant_initializers_with_source_consent(
            &mut source.clone(),
            request,
            &[],
            None,
            None,
            &mut crate::SourceCompletionAccounting::default(),
        )
        .expect_err("a layout storage constant has no bytes without a deployment");
        assert!(
            diagnostics::join_messages(&errors).contains("no deployment that reaches it"),
            "{}",
            diagnostics::join_messages(&errors)
        );
        crate::evaluate_constant_initializers_with_source_consent(
            &mut source,
            request,
            &scopes,
            None,
            None,
            &mut accounting,
        )
        .unwrap();
        crate::validate_constant_evaluations(&source, &accounting).unwrap();
        let saved = source
            .const_declarations()
            .iter()
            .find(|constant| source.symbols.name(constant.symbol) == "SAVED")
            .expect("the program declares SAVED");
        let value = typed_trees::canonical_const_declaration_value(&source, saved.symbol)
            .and_then(|value| value.decode_encoding())
            .expect("SAVED keeps its folded value");
        let language_semantics::const_value::DecodedCanonicalConstValue::Record {
            type_name,
            fields,
        } = value
        else {
            panic!("SAVED is the semantic Pair record, not its bytes: {value:?}");
        };
        assert_eq!(type_name, "Pair");
        let integers = fields
            .iter()
            .map(|(name, value)| match value {
                language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                    value,
                    ..
                } => (name.as_str(), value.to_string()),
                other => panic!("Pair holds integers: {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(integers, [("lo", "42".to_owned()), ("hi", "9".to_owned())]);
        let initializer = source.tables.machines.get(saved.evaluation_entry).symbol;
        let mut targets = accounting
            .receipts()
            .iter()
            .filter_map(|record| match record {
                crate::PureEvaluationRecord::Invocation(receipt)
                    if receipt.requester() == saved.symbol && receipt.machine() == initializer =>
                {
                    receipt.admission().target().map(str::to_owned)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort();
        assert_eq!(targets, ["linux_x86_64", "macos_arm64"]);
    }

    #[test]
    fn fixed_layout_policy_installs_the_retained_pairs_authored_geometry() {
        // The kept program deliberately puts lo after hi. Evaluate that same
        // policy and retain its geometry, rather than assigning default offsets.
        verify_fixed_layout_geometry(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../samples/cli/facts/fixed_layout_storage/local/main.omg"
        )));
    }

    #[test]
    fn fixed_layout_policy_keeps_the_retained_pair_in_a_record_field() {
        verify_fixed_layout_geometry(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../samples/cli/facts/fixed_layout_storage/field/main.omg"
        )));
    }

    #[test]
    fn fixed_layout_policy_reaches_the_retained_field_cast_refusal() {
        let result = judge_fixed_layout_pair(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../samples/cli/facts/fixed_layout_storage/invalid_field/main.omg"
        )));
        let Err(errors) = result else {
            panic!("the retained cast of 41 to Delivered must fail");
        };
        let refusal = diagnostics::join_messages(&errors);
        assert!(
            refusal.contains(
                "cannot prove the value of this cast is in the domain it adds in machine `make` state `entry`: domain `u64::Delivered` requires `self >= 42 && self <= 42`"
            ),
            "{refusal}"
        );
    }

    fn judge_fixed_layout_pair(
        authored: &str,
    ) -> Result<terminal_production::AcceptedTerminalSource, Vec<diagnostics::Diagnostic>> {
        let mut source = with_reflection(authored);
        let entry = query_machine(&source, "main");
        let pair = source
            .data_definitions()
            .iter()
            .filter(|data| data.name.as_str() == "Pair")
            .map(|data| data.symbol)
            .collect::<Vec<_>>();
        let [pair] = pair.as_slice() else {
            panic!("the retained program declares exactly one Pair")
        };
        let pair = *pair;
        let facts = layout_plans::LayoutTargetFacts {
            profile: "linux_x86_64".into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        };
        // Two applications may share a target profile, including a nested
        // field's policy and its containing policy. Conflicting facts must
        // refuse before either can run and contaminate the other's schema.
        let scope = FixedLayoutTargetScope {
            target: facts.profile.clone(),
            facts: Some(facts.clone()),
            entry_declarations: vec![entry],
            provider_declarations: Vec::new(),
        };
        let mut conflicting = scope.clone();
        conflicting.facts.as_mut().unwrap().pointer_size = 4;
        conflicting.facts.as_mut().unwrap().pointer_alignment = 4;
        let mut refused = source.clone();
        let mut accounting = crate::SourceCompletionAccounting::default();
        let errors = evaluate_fixed_layout_applications(
            &mut refused,
            &[scope.clone(), conflicting],
            None,
            None,
            source::DependencyScope::Product,
            &mut accounting,
        )
        .unwrap_err();
        assert!(
            diagnostics::join_messages(&errors)
                .contains("disagree on one target's immutable facts")
        );
        assert!(accounting.receipts().is_empty());
        assert!(refused.plan_laid_layouts.is_empty());
        evaluate_fixed_layout_applications(
            &mut source,
            &[scope],
            None,
            None,
            source::DependencyScope::Product,
            &mut crate::SourceCompletionAccounting::default(),
        )
        .unwrap();
        let plan = source
            .plan_laid_layouts
            .iter()
            .find(|plan| plan.data_symbol == pair)
            .expect("the demanded Pair policy must produce its actual layout");
        let geometry = plan
            .geometry_for_target(&source, Some("linux_x86_64"))
            .expect("the plan must retain the requested target's geometry");
        assert_eq!(geometry.validated_layout.size, Some(16));
        assert_eq!(geometry.validated_layout.align, 8);
        assert_eq!(geometry.validated_layout.offsets, Some(vec![8, 0]));

        // The same program must keep those placements in its checked module.
        // Evaluating the policy alone would miss a later return to defaults.
        let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            source,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .unwrap();
        terminal_production::AcceptedTerminalSource::judge(
            Arc::new(prepared),
            terminal_production::TerminalSourceCompilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Portable,
        )
    }

    fn verify_fixed_layout_geometry(authored: &str) {
        let accepted = judge_fixed_layout_pair(authored).unwrap();
        let module = accepted.module();
        let [application] = module.target_layout_applications.as_slice() else {
            panic!("the program retains its one actual layout application");
        };
        assert_ne!(application.record, application.schema);
        let [realization] = application.realizations.as_slice() else {
            panic!("the program retains its one requested target");
        };
        assert_eq!(realization.target, "linux_x86_64");
        assert_eq!(realization.storage.byte_size, 16);
        assert_eq!(realization.storage.byte_alignment, 8);
        assert_eq!(
            realization
                .fields
                .iter()
                .map(|field| field.storage.byte_offset())
                .collect::<Vec<_>>(),
            [Some(8), Some(0)]
        );
        let native = module.native_layouts.record(application.record).unwrap();
        let [native] = module.native_layouts.realizations(native).unwrap() else {
            panic!("native emission retains that same target realization");
        };
        assert_eq!(native.report.layout.offsets, Some(vec![8, 0]));
        let bytes = terminal_codec::encode_module(module).unwrap();
        assert_eq!(terminal_codec::decode_module(&bytes).unwrap(), *module);

        // A byte view covers its captured region. Shifting that whole view
        // cannot borrow another byte beyond the region's established extent.
        let mut shifted = module.clone();
        let (machine, offset) = shifted
            .machines
            .iter()
            .find_map(|machine| {
                machine.operations().find_map(|operation| {
                    let terminal_psi::OperationKind::EstablishReference {
                        storage: Some(storage),
                        ..
                    } = operation.kind
                    else {
                        return None;
                    };
                    Some((machine.id, storage.byte_offset))
                })
            })
            .expect("the retained byte oracle forms an actual storage view");
        let producer = shifted
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == machine)
            .unwrap()
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| {
                operation
                    .result
                    .as_scalar()
                    .is_some_and(|result| result.id == offset)
            })
            .unwrap();
        producer.kind = terminal_psi::OperationKind::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Unsigned(1),
        };
        let refusal =
            terminal_codec::decode_module(&terminal_codec::encode_module(&shifted).unwrap())
                .unwrap_err();
        assert!(
            refusal.to_string().contains("captured zero offset"),
            "{refusal}"
        );

        // Changing the issuer's target input cannot relabel the retained
        // realization as a result for different representation facts.
        let mut forged = module.clone();
        forged.target_layout_applications[0].target_inputs[0].pointer_size = 4;
        let refusal =
            terminal_codec::decode_module(&terminal_codec::encode_module(&forged).unwrap())
                .unwrap_err();
        assert!(
            refusal
                .to_string()
                .contains("independent issuer target input"),
            "{refusal}"
        );
    }

    #[test]
    fn generic_layout_policy_retains_its_declared_machine_and_publishes_its_plan() {
        // The same closed layout plan must use the declared member in source
        // metadata and its captured body in the actual invocation receipt.
        let authored = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../samples/cli/facts/fixed_layout_storage/local/main.omg"
        ))
        .replace(
            "PairLayout satisfies FixedLayout<Pair>",
            "public data OtherPair [copy] { lo: u64 in Delivered; hi: u64; }\nPairLayout<T> satisfies FixedLayout<T>",
        );
        let evaluated = |argument: &str| {
            let mut source = with_reflection(&authored.replace(
                "LayoutStorage<Pair, PairLayout>",
                &format!("LayoutStorage<Pair, PairLayout<{argument}>>"),
            ));
            let root = query_machine(&source, "main");
            let facts = layout_plans::LayoutTargetFacts {
                profile: "linux_x86_64".into(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            };
            let mut accounting = crate::SourceCompletionAccounting::default();
            let result = evaluate_fixed_layout_applications(
                &mut source,
                &[FixedLayoutTargetScope {
                    target: facts.profile.clone(),
                    facts: Some(facts),
                    entry_declarations: vec![root],
                    provider_declarations: Vec::new(),
                }],
                None,
                None,
                source::DependencyScope::Product,
                &mut accounting,
            );
            (source, root, accounting, result)
        };

        // Geometry alone cannot identify a policy's semantic argument.
        let (wrong, _, accounting, refusal) = evaluated("OtherPair");
        let refusal = diagnostics::join_messages(&refusal.unwrap_err());
        assert!(
            refusal.contains("selected layout policy belongs to another semantic type"),
            "{refusal}"
        );
        assert!(accounting.receipts().is_empty());
        assert!(wrong.plan_laid_layouts.is_empty());

        let (source, root, accounting, result) = evaluated("Pair");
        result.unwrap();
        let [plan] = source.plan_laid_layouts.as_slice() else {
            panic!("one exact layout for Pair");
        };
        let policy = source.conformance_by_symbol(plan.policy_symbol).unwrap();
        let [member] = policy.rows.as_slice() else {
            panic!("the authored plan member");
        };
        assert_eq!(plan.policy_plan_machine_symbol, member.realization_machine);
        let [layout] = plan.target_layouts(&source) else {
            panic!("one plan for the selected target");
        };
        assert_eq!(layout.machine, member.realization_machine);
        let [crate::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
            panic!("one physical policy invocation");
        };
        assert_ne!(receipt.machine(), member.realization_machine);
        assert!(source.machine_by_symbol(receipt.machine()).is_none());
        assert!(
            receipt
                .source()
                .machine_instantiation_path(receipt.machine(), member.realization_machine)
                .is_some()
        );
        accounting.validate_charges().unwrap();

        let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            source,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .unwrap();
        let accepted = terminal_production::AcceptedTerminalSource::judge(
            Arc::new(prepared),
            terminal_production::TerminalSourceCompilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[root],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .unwrap();
        let module = accepted.module();
        let [application] = module.target_layout_applications.as_slice() else {
            panic!("the checked module retains the applied generic policy");
        };
        let record = module.native_layouts.record(application.record).unwrap();
        let [layout] = module.native_layouts.realizations(record).unwrap() else {
            panic!("one published native layout");
        };
        assert_eq!(layout.report.layout.size, Some(16));
        assert_eq!(layout.report.layout.align, 8);
        assert_eq!(layout.report.layout.offsets, Some(vec![8, 0]));
        let bytes = terminal_codec::encode_module(module).unwrap();
        assert_eq!(terminal_codec::decode_module(&bytes).unwrap(), *module);
    }

    #[test]
    fn the_library_c_layout_policy_keeps_its_existing_scalar_record() {
        // The bundled generic policy must compute the kept C-layout example's
        // real field placements. Its alias, operations and byte oracle stay
        // unchanged while the compiler supplies the actual type application.
        let mut source = with_reflection(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../samples/cli/facts/c_layout_scalar_record/local/main.omg"
        )));
        let entry = query_machine(&source, "main");
        let records = source
            .data_definitions()
            .iter()
            .filter(|data| data.name.as_str() == "Record")
            .map(|data| data.symbol)
            .collect::<Vec<_>>();
        let [record] = records.as_slice() else {
            panic!("the kept example declares one Record")
        };
        let record = *record;
        let facts = layout_plans::LayoutTargetFacts {
            profile: "linux_x86_64".into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        };
        evaluate_fixed_layout_applications(
            &mut source,
            &[FixedLayoutTargetScope {
                target: facts.profile.clone(),
                facts: Some(facts),
                entry_declarations: vec![entry],
                provider_declarations: Vec::new(),
            }],
            None,
            None,
            source::DependencyScope::Product,
            &mut crate::SourceCompletionAccounting::default(),
        )
        .unwrap();
        let plans = source
            .plan_laid_layouts
            .iter()
            .filter(|plan| plan.data_symbol == record)
            .collect::<Vec<_>>();
        let [plan] = plans.as_slice() else {
            panic!("the one C policy retains one actual record layout")
        };
        let geometry = plan
            .geometry_for_target(&source, Some("linux_x86_64"))
            .unwrap();
        assert_eq!(geometry.validated_layout.size, Some(24));
        assert_eq!(geometry.validated_layout.align, 8);
        assert_eq!(geometry.validated_layout.offsets, Some(vec![0, 8, 16]));
        let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            source,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .unwrap();
        let accepted = terminal_production::AcceptedTerminalSource::judge(
            Arc::new(prepared),
            terminal_production::TerminalSourceCompilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .unwrap();
        let bytes = terminal_codec::encode_module(accepted.module()).unwrap();
        assert_eq!(
            terminal_codec::decode_module(&bytes).unwrap(),
            *accepted.module()
        );
    }

    #[test]
    fn actual_float_bit_views_keep_their_declared_scalar_homes() {
        // Exposing a float's integer bits must borrow its original float storage.
        // Check the actual core helpers with their declared inputs; the
        // application's overloaded from_bits calls are a separate unresolved path.
        let source = with_reflection(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../samples/cli/facts/core_float_representation/local/main.omg"
        )));
        let requester = query_machine(&source, "main");
        let span = source.symbols.symbol_source_span(requester).unwrap();
        let candidates = source.symbols.top_level_candidates_from_source(
            "float::to_bits",
            &[symbols::SymbolKind::Machine],
            span,
        );
        assert_eq!(candidates.len(), 2, "both actual public float declarations");
        let declarations = candidates
            .into_iter()
            .map(|symbol| {
                let machine = source.machine_by_symbol(symbol).unwrap();
                let declaration_span = source.symbols.symbol_source_span(symbol).unwrap();
                assert!(
                    source
                        .symbols
                        .source_file(declaration_span)
                        .unwrap()
                        .path
                        .ends_with("core/float.omg")
                );
                let Some([state]) = source.machine_states(machine).as_array() else {
                    panic!("one original helper entry")
                };
                let Some([formal]) = source.state_parameters(state).as_array() else {
                    panic!("one declared float input")
                };
                let typed_trees::types::TypeReferenceNode::Named { symbol: scalar, .. } = source
                    .type_reference_table
                    .type_reference(formal.type_reference)
                else {
                    panic!("the declared builtin float")
                };
                let format = match source.symbols.builtin_type_atom(*scalar) {
                    Some(symbols::BuiltinTypeAtom::F32) => {
                        semantic_vocabulary::IeeeFloatFormat::Binary32
                    }
                    Some(symbols::BuiltinTypeAtom::F64) => {
                        semantic_vocabulary::IeeeFloatFormat::Binary64
                    }
                    other => panic!("unexpected public input {other:?}"),
                };
                (
                    symbols::RetainedDeclaration::capture(&source.symbols, symbol).unwrap(),
                    format,
                )
            })
            .collect::<Vec<_>>();
        for format in [
            semantic_vocabulary::IeeeFloatFormat::Binary32,
            semantic_vocabulary::IeeeFloatFormat::Binary64,
        ] {
            assert!(declarations.iter().any(|(_, actual)| *actual == format));
        }
        let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            source,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .unwrap();
        let prepared = Arc::new(prepared);
        for (declaration, format) in declarations {
            let symbol = declaration
                .rejoin(
                    &prepared.symbols,
                    prepared.machines().iter().map(|machine| machine.symbol),
                )
                .unwrap();
            let accepted = terminal_production::AcceptedTerminalSource::judge(
                Arc::clone(&prepared),
                terminal_production::TerminalSourceCompilation {
                    calling: None,
                    providers: None,
                    declaration_metadata: &[],
                    entries: &[symbol],
                    subjects: &[],
                    purpose: source::DependencyScope::Product,
                    selected: &[],
                    bindings: &[],
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Portable,
            )
            .unwrap();
            let module = accepted.module();
            let scalar = semantic_vocabulary::ScalarType::IeeeFloat(format);
            assert!(
                module.structural_types.iter().any(|declaration| {
                    declaration.shape == terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar)
                }),
                "the helper retains its original IEEE storage identity"
            );
            assert!(module.machines.iter().flat_map(|machine| machine.operations()).any(|operation| {
                matches!(operation.kind, terminal_psi::OperationKind::EstablishReference {
                    storage: Some(terminal_psi::ReferenceStorageView { stored_scalar: Some(stored), .. }), ..
                } if stored == scalar)
            }), "the actual helper presents its IEEE storage as integer bits");
            let bytes = terminal_codec::encode_module(module).unwrap();
            assert_eq!(terminal_codec::decode_module(&bytes).unwrap(), *module);
        }
    }

    /// The machine `name` that a `with_reflection` query declares. The real
    /// core is typed beside the query and its nested machines take ordinary
    /// names (`core/wire.omg` declares a `payload`), so a lookup by name
    /// alone can select a library machine instead of the test's own.
    fn query_machine(source: &TypedTrees, name: &str) -> SymbolHandle {
        let mut declared = source.machines().iter().filter(|machine| {
            machine.target.is_none()
                && machine.name.as_str() == name
                && source
                    .symbols
                    .symbol_source_span(machine.symbol)
                    .and_then(|span| source.symbols.source_file(span))
                    .is_some_and(|file| file.path.ends_with("query.omg"))
        });
        let (Some(machine), None) = (declared.next(), declared.next()) else {
            panic!("the query declares exactly one machine named `{name}`")
        };
        machine.symbol
    }

    fn returned_source_call(
        source: &TypedTrees,
        name: &str,
    ) -> (SymbolHandle, typed_trees::expression::ExpressionHandle) {
        let machine = source.realized_machine_named(name).unwrap();
        let Some([state]) = source.machine_states(machine).as_array() else {
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
    fn source_jobs_retain_the_selected_implementation_member() {
        // Selecting an implementation names its exact private member; it does
        // not select another map with the same requirement or attached name.
        let source = typed(
            "trait Policy {
             machine plan(value: u64) -> u64 requires value > 0;
             }
             public First satisfies Policy {
                 machine plan(value: u64) -> u64 requires value > 0 { value }
             }
             public Second satisfies Policy {
                 machine plan(value: u64) -> u64 requires value > 0 { 23u64 }
             }
             public Applied<T> satisfies Policy {
                 machine plan(value: u64) -> u64 requires value > 0 { value }
             }
             machine first() -> u64 { First::plan(7u64) }
             machine second() -> u64 { Second::plan(7u64) }
             machine narrow() -> u64 { Applied<u32>::plan(7u64) }
             machine wide() -> u64 { Applied<u64>::plan(7u64) }",
        );
        let program = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
        for (name, expected) in [("first", 7), ("second", 23), ("narrow", 7), ("wide", 7)] {
            let (requester, call) = returned_source_call(program.typed(), name);
            let entry = program
                .entry_call(requester, call, &[BuildTimeValue::Int(7)])
                .unwrap();
            let mut accounting = crate::SourceCompletionAccounting::default();
            let result = entry
                .invoke_recorded(target(), &mut accounting)
                .expect("the exact ordinary selected member executes");
            assert_eq!(result.value(), &BuildTimeValue::Int(expected));
            let [crate::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
                panic!("one actual selected call")
            };
            assert_eq!(receipt.requester(), requester);
            let authored = source.expression_table.call_node(call).unwrap();
            let retained = receipt.call().unwrap();
            assert_eq!(retained.target_symbol, authored.target_symbol);
            assert_eq!(retained.receiver, authored.receiver);
            assert_ne!(
                retained.authored_machine_application, authored.authored_machine_application,
                "the actual caller owns a distinct copy of this application"
            );
            let accepted = entry.accepted_source();
            let original = source
                .expression_table
                .static_machine_application(authored.authored_machine_application);
            assert_eq!(
                accepted
                    .expression_table
                    .static_machine_application(authored.authored_machine_application,),
                original,
                "constructing another invocation does not mutate its authored source row"
            );
            let copied = accepted
                .expression_table
                .static_machine_application(retained.authored_machine_application);
            assert_eq!(
                copied.registration_operation,
                original.registration_operation
            );
            assert_eq!(copied.binding_declaration, original.binding_declaration);
            let canonical_caller = entry.accepted().entry().unwrap();
            let actual_caller = entry
                .accepted()
                .closure()
                .elaboration
                .machines
                .get(canonical_caller)
                .unwrap();
            assert_eq!(copied.lexical_owner, *actual_caller);
            assert_eq!(original.lexical_owner, requester);
            assert_eq!(
                accepted
                    .expression_table
                    .static_machine_application_arguments(retained.authored_machine_application,),
                source
                    .expression_table
                    .static_machine_application_arguments(authored.authored_machine_application,),
                "every original static operand survives the owned application copy"
            );
            assert_eq!(
                retained.requirement_application,
                authored.requirement_application
            );
            assert_eq!(retained.machine_arguments, authored.machine_arguments);
            let error = match program.entry_call(requester, call, &[BuildTimeValue::Int(0)]) {
                Ok(_) => panic!("the public requirement must establish its actual input"),
                Err(error) => error,
            };
            assert!(error.contains("requires"), "{error}");
        }
    }

    #[test]
    fn selected_entries_close_the_original_map_tuple_and_public_signature() {
        // The trait's semantic argument follows the complete map application,
        // including a binder unused by its public signature. A fixed trait
        // argument must not be mistaken for the map's differently typed binder.
        let source = typed(
            "trait Policy<T> {
                 machine plan(value: u64) -> u64 requires value > 0 ensures result > 0;
             }
             public Applied<T, Unused> satisfies Policy<T> {
                 machine plan(value: u64) -> u64 requires true
                 ensures result == 41u64 { 41u64 }
             }
             public FixedSubject<Unused> satisfies Policy<u32> {
                 machine plan(value: u64) -> u64 requires true
                 ensures result == 41u64 { 41u64 }
             }
             machine narrow() -> u64 { Applied<u32, u64>::plan(7u64) }
             machine wide() -> u64 { Applied<u64, bool>::plan(7u64) }
             machine fixed() -> u64 { FixedSubject<u64>::plan(7u64) }
             machine hidden() -> u64 ensures result == 41u64 {
                 Applied<u32, u64>::plan(7u64)
             }",
        );
        let program = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
        for (name, semantic_type) in [("narrow", "u32"), ("wide", "u64"), ("fixed", "u32")] {
            let (requester, expression) = returned_source_call(&source, name);
            let call = source.expression_table.call_node(expression).unwrap();
            let (operand, owner) = source
                .source_call_selected_operand(
                    requester,
                    typed_trees::references::StaticArgumentCall::Expression(expression, call),
                )
                .unwrap();
            let scope = source
                .applied_requirement_scope(owner, call.requirement_application)
                .unwrap();
            let environment = scope.environment();
            let mut materialized = source.clone();
            let trait_arguments = materialized
                .materialize_trait_application_arguments(&environment)
                .unwrap();
            assert_eq!(trait_arguments.len(), 1);
            assert_eq!(
                materialized.primitive_type_reference(trait_arguments[0]),
                Some(typed_trees::types::PrimitiveType::Integer(
                    numerics::integer_carrier::FixedInteger::from_decimal_spelling(semantic_type)
                        .unwrap()
                )),
                "the selected trait's semantic T is resolved through its own map environment"
            );
            if name != "wide" {
                assert_ne!(
                    materialized.primitive_type_reference(trait_arguments[0]),
                    Some(typed_trees::types::PrimitiveType::U64),
                    "a genuine closed selection does not establish a different semantic subject"
                );
            }
            let entry = program
                .entry_selected(
                    requester,
                    owner,
                    &operand,
                    call.target_symbol,
                    &[BuildTimeValue::Int(7)],
                )
                .expect("the exact generic application uses the shared source-call route");
            assert_eq!(
                entry.call().requirement_application,
                call.requirement_application
            );
            let mut accounting = crate::SourceCompletionAccounting::default();
            let result = entry.invoke_recorded(target(), &mut accounting).unwrap();
            assert_eq!(result.value(), &BuildTimeValue::Int(41));
            let [crate::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
                panic!("one exact selected invocation")
            };
            let (_, invocation) = receipt.invocation().unwrap().unwrap();
            assert!(invocation.inputs.required_contract.is_some());
            let error = program
                .entry_selected(
                    requester,
                    owner,
                    &operand,
                    call.target_symbol,
                    &[BuildTimeValue::Int(0)],
                )
                .err()
                .expect("the public input bound survives the total private body");
            assert!(error.contains("requires"), "{error}");

            let (_, other_expression) =
                returned_source_call(&source, if name == "narrow" { "wide" } else { "narrow" });
            let other = source.expression_table.call_node(other_expression).unwrap();
            let typed_trees::types::TypeReferenceNode::Generic { arguments, .. } = source
                .type_reference_table
                .type_reference(other.requirement_application)
            else {
                panic!("the other authored map has its complete tuple")
            };
            let other_type = source
                .type_reference_table
                .type_reference_handles(*arguments)[0];
            let mut changed = operand.clone().into_owned();
            changed.application.as_mut().unwrap().arguments[0].type_reference = other_type;
            let error = program
                .entry_selected(
                    requester,
                    owner,
                    &changed,
                    call.target_symbol,
                    &[BuildTimeValue::Int(7)],
                )
                .err()
                .expect("a different semantic argument cannot replace the retained tuple");
            assert!(
                matches!(
                    error.as_str(),
                    "selected source invocation changed its complete operand tuple"
                        | "static operand changed its retained selected application"
                ),
                "{error}"
            );

            let mut changed = operand.into_owned();
            let application = changed.application.as_mut().unwrap();
            let original = application.arguments[0].type_reference;
            application.arguments[0].type_reference =
                typed_trees::types::TypeReferenceHandle::from_parts(
                    original.arena_index(),
                    original.generation() + 1,
                );
            let error = program
                .entry_selected(
                    requester,
                    owner,
                    &changed,
                    call.target_symbol,
                    &[BuildTimeValue::Int(7)],
                )
                .err()
                .expect("a stale child cannot be replaced from its remaining symbol");
            assert_eq!(
                error,
                "selected source invocation has a stale operand reference"
            );
        }
        let (hidden, _) = returned_source_call(&source, "hidden");
        let error = program
            .entry(hidden, hidden, &[])
            .err()
            .expect("the selected body's private promise is unavailable to its public caller");
        assert!(error.contains("ensures"), "{error}");
    }

    #[test]
    fn selected_calls_keep_public_promises_through_source_free_roundtrip() {
        // A selected body's total implementation cannot erase a public input
        // requirement or lend its stronger private result promise to the caller.
        // Both selections survive in the artifact so same-arity substitutions
        // can be rejected without consulting the source trees.
        let source = typed(
            "trait Policy {
                 machine plan(value: u64) -> u64 requires value > 0 ensures result > 0;
             }
             public First satisfies Policy {
                 machine plan(value: u64) -> u64 requires true
                 ensures result == 23u64 { 23u64 }
             }
             public Second satisfies Policy {
                 machine plan(value: u64) -> u64 requires true
                 ensures result == 41u64 { 41u64 }
             }
             machine request() -> u64 {
                 let first: u64 = First::plan(7u64);
                 Second::plan(first)
             }
             machine hidden() -> u64 ensures result == 41u64 {
                 Second::plan(7u64)
             }",
        );
        let prepared = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
        let (requester, expression) = returned_source_call(prepared.typed(), "request");
        let selected = prepared
            .entry_call(requester, expression, &[BuildTimeValue::Int(7)])
            .unwrap();
        let mut accounting = crate::SourceCompletionAccounting::default();
        assert_eq!(
            selected
                .invoke_recorded(target(), &mut accounting)
                .unwrap()
                .value(),
            &BuildTimeValue::Int(41)
        );
        let [crate::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
            panic!("one selected call retains its exact execution receipt")
        };
        let (canonical_caller, invocation) = receipt.invocation().unwrap().unwrap();
        assert_eq!(
            Some(semantic_vocabulary::TargetMachineCallable::Machine(
                canonical_caller
            )),
            receipt.program().view().module().entry
        );
        assert!(invocation.inputs.required_contract.is_some());
        let refusal = prepared
            .entry_call(requester, expression, &[BuildTimeValue::Int(0)])
            .err()
            .expect("the public input requirement still applies to a total implementation");
        assert!(refusal.contains("requires"), "{refusal}");
        let (hidden, _) = returned_source_call(prepared.typed(), "hidden");
        let refusal = prepared
            .entry(hidden, hidden, &[])
            .err()
            .expect("a private implementation postcondition is not a public promise");
        assert!(refusal.contains("ensures"), "{refusal}");

        // The temporary actual-argument probe deliberately has no portable
        // identity. Publish the same authored, packaged request closure through
        // the ordinary source route; never relabel invocation code as portable.
        let (portable_source, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            source,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .unwrap();
        let accepted = terminal_production::AcceptedTerminalSource::judge(
            Arc::new(portable_source),
            terminal_production::TerminalSourceCompilation {
                declaration_metadata: &[],
                entries: &[requester],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                calling: None,
                providers: None,
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .unwrap();
        let module = accepted.module();
        let calls = module
            .machines
            .iter()
            .flat_map(|machine| machine.module_invocations())
            .filter(|call| call.inputs.required_contract.is_some())
            .map(|call| (call.inputs.required_contract.unwrap(), call.inputs.callee))
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2, "both exact source selections are reachable");
        assert_ne!(calls[0].0, calls[1].0);
        assert_ne!(calls[0].1, calls[1].1);
        for (required, body) in &calls {
            let public = module
                .invocation_machine_contract(*body, Some(*required))
                .unwrap();
            assert_eq!(public.requires.len(), 1);
            assert_eq!(public.ensures.len(), 1);
            assert!(public.selected_application.is_some());
        }
        let bytes = terminal_codec::encode_module(module).unwrap();
        assert_eq!(terminal_codec::decode_module(&bytes).unwrap(), *module);
        for suspension in [false, true] {
            for change_declared in [false, true] {
                let mut forged = module.clone();
                let public = forged
                    .declaration_contracts
                    .iter_mut()
                    .find(|contract| contract.id == calls[0].0)
                    .unwrap();
                if suspension {
                    public.published_may_suspend = !public.published_may_suspend;
                    if change_declared {
                        public.declared_suspension = Some(public.published_may_suspend);
                    }
                } else {
                    public.published_may_block = !public.published_may_block;
                    if change_declared {
                        public.declared_blocking = Some(public.published_may_block);
                    }
                }
                assert_eq!(
                    terminal_codec::decode_module(&terminal_codec::encode_module(&forged).unwrap())
                        .unwrap_err(),
                    terminal_codec::CodecError::MalformedStructuralFoundation(if change_declared {
                        "public operational contract differs from its source declaration"
                    } else {
                        "authored callable effects differ from their published promises"
                    })
                );
            }
        }
        let (caller, call) = module
            .machines
            .iter()
            .find_map(|machine| {
                machine
                    .module_invocations()
                    .find(|call| call.inputs.required_contract == Some(calls[0].0))
                    .map(|call| (machine.id, call))
            })
            .unwrap();
        let header = module.declaration_contract(calls[0].0).unwrap();
        let mut forged = module.clone();
        forged
            .proof_output_calls
            .push(terminal_psi::ProofOutputCall {
                caller,
                ordinal: 0,
                target_identity: header.declaration.as_ref().unwrap().callable,
                static_requirement_dispatch: None,
                runtime_result: Some(terminal_psi::ProofOutputRuntimeResult::Scalar(
                    header.result.scalar_ref().unwrap().scalar_type,
                )),
                runtime_call: Some(terminal_psi::ProofOutputRuntimeCall {
                    site: call.site,
                    target: terminal_psi::EvidenceCallableTarget::Machine(call.inputs.callee),
                }),
                evidence_arguments: Vec::new(),
                outputs: Vec::new(),
            });
        assert_eq!(
            terminal_codec::decode_module(&terminal_codec::encode_module(&forged).unwrap())
                .unwrap_err(),
            terminal_codec::CodecError::MalformedStructuralFoundation(
                "selected proof call differs from its operation's public contract"
            )
        );
        let mut previous_format = bytes.clone();
        previous_format[8..10].copy_from_slice(&263u16.to_le_bytes());
        assert_eq!(
            terminal_codec::decode_module(&previous_format).unwrap_err(),
            terminal_codec::CodecError::UnsupportedFormatMarker(263)
        );
        for (required, replacement) in [(calls[0].0, calls[1].1), (calls[1].0, calls[0].1)] {
            let mut forged = module.clone();
            forged
                .declaration_contracts
                .iter_mut()
                .find(|contract| contract.id == required)
                .unwrap()
                .selected_application
                .as_mut()
                .unwrap()
                .realization = replacement;
            let mut changed = 0;
            for operation in forged
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.blocks)
                .flat_map(|block| &mut block.operations)
                .filter(|operation| operation.required_contract == Some(required))
            {
                let terminal_psi::OperationKind::Call { callee, .. } = &mut operation.kind else {
                    panic!("this source selection is the ordinary scalar call")
                };
                *callee = replacement;
                changed += 1;
            }
            assert_eq!(
                changed, 1,
                "change the exact header and physical call together"
            );
            let bytes = terminal_codec::encode_module(&forged).unwrap();
            let refusal = terminal_codec::decode_module(&bytes).unwrap_err();
            assert_eq!(
                refusal,
                terminal_codec::CodecError::MalformedStructuralFoundation(
                    "static requirement dispatch has no exact executable method application"
                ),
                "equal physical/header bodies must still match the closed selection"
            );
        }
    }

    #[test]
    fn nominal_invocation_bindings_retain_the_requirement_and_implementation() {
        let source = typed(
            "trait A { machine plan(value: u32) -> u32; }
             trait B { machine plan(value: u32) -> u32; }
             P satisfies A { machine plan(value: u32) -> u32 { value } }
             Q satisfies B { machine plan(value: u32) -> u32 { value } }",
        );
        let implementation = |name: &str| {
            source
                .conformances()
                .iter()
                .find(|row| {
                    row.alias
                        .as_ref()
                        .is_some_and(|alias| alias.as_str() == name)
                })
                .unwrap()
        };
        let requirement = |name: &str| {
            let owner = source
                .traits()
                .iter()
                .find(|row| row.name.as_str() == name)
                .unwrap();
            source.trait_machine_signatures(owner)[0].symbol
        };
        let binding = terminal_production::TerminalSourceSupply {
            slot: requirement("A"),
            provider: implementation("P").symbol,
        };
        let machine = binding.realization(&source, binding.slot).unwrap();
        let (owner, row) = source
            .owned_conformance_realization(machine.symbol)
            .unwrap();
        assert_eq!(owner.symbol, binding.provider);
        assert_eq!(row.requirement, binding.slot);
        assert!(
            !source
                .trait_machine_signature(binding.slot)
                .unwrap()
                .0
                .is_boundary
        );
        let program = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
        let entry = program
            .entry_for_supply(machine.symbol, binding, &[BuildTimeValue::Int(7)])
            .unwrap();
        assert!(entry.accepted.module().boundary_machines.is_empty());
        assert!(
            entry
                .accepted
                .boundary_requirements_declared()
                .unwrap()
                .is_empty()
        );
        let value = entry
            .invoke(target(), &mut terminal_work::WorkMeter::unbounded())
            .unwrap();
        assert_eq!(value, BuildTimeValue::Int(7));

        assert!(
            terminal_production::TerminalSourceSupply {
                provider: implementation("Q").symbol,
                ..binding
            }
            .realization(&source, binding.slot)
            .is_err()
        );
        assert!(binding.realization(&source, requirement("B")).is_err());
        assert!(
            terminal_production::TerminalSourceSupply {
                provider: SymbolHandle::invalid(),
                ..binding
            }
            .realization(&source, binding.slot)
            .is_err()
        );
    }

    #[test]
    fn source_case_values_select_only_members_of_the_actual_formal() {
        let program = program(
            "data Envelope [copy] { case Value(value: u64); }
             data Other [copy] { case Value(value: u64); }
             machine answer(value: Envelope) -> Envelope { value }",
        );
        let value = BuildTimeValue::Case {
            variant: "Value".into(),
            payload: vec![("value".into(), BuildTimeValue::Int(7))],
        };
        let entry = bind(&program, std::slice::from_ref(&value)).unwrap();
        assert_eq!(
            entry
                .invoke(target(), &mut terminal_work::WorkMeter::unbounded())
                .unwrap(),
            value
        );
        for variant in ["Other::Value", "Missing"] {
            let wrong = BuildTimeValue::Case {
                variant: variant.into(),
                payload: vec![("value".into(), BuildTimeValue::Int(7))],
            };
            let error = bind(&program, &[wrong])
                .err()
                .expect("only actual formal members bind");
            assert!(
                error.contains(variant) && error.contains("formal"),
                "{error}"
            );
        }
    }

    #[test]
    fn source_array_results_keep_the_actual_extent_and_elements() {
        let program = program("machine answer(value: [u64; 2]) -> [u64; 2] { value }");
        let value = BuildTimeValue::Array(vec![BuildTimeValue::Int(3), BuildTimeValue::Int(5)]);
        let entry = bind(&program, std::slice::from_ref(&value)).unwrap();
        assert_eq!(
            entry
                .invoke(target(), &mut terminal_work::WorkMeter::unbounded())
                .unwrap(),
            value
        );
        let error = bind(
            &program,
            &[BuildTimeValue::Array(vec![BuildTimeValue::Int(3)])],
        )
        .err()
        .expect("the declared extent is retained");
        assert!(error.contains("declared extent"), "{error}");
    }

    #[test]
    fn subject_requirements_keep_their_full_headers_without_becoming_bodies() {
        let source = typed(
            "trait Description {
                machine closed(value: u64) -> u64
                    requires value > 0u64
                    ensures result >= value
                    reaches none crashes none terminates;
                machine open(value: u64) -> u64;
            }
            machine answer() -> u64 { 17u64 }",
        );
        let entry = source.realized_machine_named("answer").unwrap().symbol;
        let package = source.symbols.symbol_package_identity(entry).unwrap();
        let (source, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            source,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .expect("prepare the actual source declarations");
        let accepted = terminal_production::AcceptedTerminalSource::judge(
            Arc::new(source),
            terminal_production::TerminalSourceCompilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[Some(package)],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                calling: None,
                providers: None,
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .expect("requirements publish contracts, not executable obligations");
        let module = accepted.module();
        assert_eq!(
            module.machines.len(),
            1,
            "only the actual source body is executable"
        );
        let [closed, open] = module.declaration_contracts.as_slice() else {
            panic!(
                "both authored requirements must survive: {:?}",
                module.declaration_contracts
            );
        };
        let owners = accepted.source().traits();
        let (1, Some(owner)) = (owners.len(), owners.first()) else {
            panic!("one actual source trait")
        };
        let requirements = accepted.source().trait_machine_signatures(owner);
        assert_eq!(requirements.len(), 2);
        for (contract, requirement) in [closed, open].into_iter().zip(requirements) {
            assert_eq!(contract.parameters.len(), 1);
            assert_eq!(
                contract.parameter_order,
                [terminal_psi::BoundaryParameterKind::Scalar]
            );
            assert!(contract.result.scalar_ref().is_some());
            let declaration = contract
                .declaration
                .as_ref()
                .expect("exact source declaration");
            assert_eq!(declaration.parameter_types.len(), 1);
            assert_ne!(declaration.contract, [0; 32]);
            assert_eq!(
                module.identities.text(declaration.semantic_declaration),
                Some(
                    accepted
                        .source()
                        .normalized_hermetic_symbol_identity(requirement.symbol)
                        .unwrap()
                        .as_str(),
                )
            );
            assert_eq!(
                module.identities.text(declaration.callable),
                Some(
                    accepted
                        .source()
                        .trait_requirement_callable_identity_text(owner, requirement)
                        .as_str(),
                )
            );
        }
        assert_eq!(closed.declared_service_reach, Some(Vec::new()));
        assert_eq!(closed.declared_crash_routes, Some(Vec::new()));
        assert_eq!(
            closed.termination,
            semantic_vocabulary::Termination::Terminates
        );
        assert_eq!(closed.requires.len(), 1);
        assert_eq!(closed.ensures.len(), 1);
        assert_eq!(open.declared_service_reach, None);
        assert_eq!(open.declared_crash_routes, None);
        assert_eq!(
            open.termination,
            semantic_vocabulary::Termination::NoGuarantee
        );
        assert_ne!(
            closed.declaration.as_ref().unwrap().contract,
            open.declaration.as_ref().unwrap().contract
        );
        let bytes = terminal_codec::encode_module(module).expect("encode checked declarations");
        assert_eq!(terminal_codec::decode_module(&bytes).unwrap(), *module);
        let mut forged = module.clone();
        forged.machines[0].contract.id = closed.id;
        assert!(
            terminal_codec::decode_module(&terminal_codec::encode_module(&forged).unwrap())
                .is_err(),
            "a body cannot adopt a bodyless contract coordinate"
        );
        let mut forged = module.clone();
        forged.declaration_contracts[0].declaration = None;
        assert!(
            terminal_codec::decode_module(&terminal_codec::encode_module(&forged).unwrap())
                .is_err(),
            "a public contract needs its own source declaration"
        );
    }

    #[test]
    fn field_queries_return_the_selected_declaration_and_its_complete_application() {
        fn field<'a>(value: &'a crate::BuildTimeValue, name: &str) -> &'a crate::BuildTimeValue {
            let crate::BuildTimeValue::Struct { fields, .. } = value else {
                panic!("the query returns the ordinary FieldInfo record")
            };
            let mut matches = fields.iter().filter(|(key, _)| key == name);
            let (_, value) = matches.next().expect("declared description field");
            assert!(matches.next().is_none());
            value
        }

        let source = with_reflection(
            "use omega::language::core::reflect;
            data Cell<T> { #7 value: T; }
            data Packet { case Ready(#9 value: u32); }
            machine first() -> reflect::FieldInfo { reflect::field(Cell<u32>::value) }
            machine second() -> reflect::FieldInfo { reflect::field(Cell<u64>::value) }
            machine payload() -> reflect::FieldInfo { reflect::field(Packet::Ready::value) }",
        );
        let prepared = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
        let mut accounting = crate::SourceCompletionAccounting::default();
        let mut values = Vec::new();
        for name in ["first", "second", "payload"] {
            let symbol = query_machine(&source, name);
            values.push(
                prepared
                    .entry(symbol, symbol, &[])
                    .unwrap()
                    .invoke_recorded(target(), &mut accounting)
                    .unwrap_or_else(|reason| panic!("{name}: {reason}"))
                    .value()
                    .clone(),
            );
        }
        for value in &values {
            assert_eq!(
                field(value, "name"),
                &crate::BuildTimeValue::Text(b"value".to_vec())
            );
            assert_eq!(
                field(value, "declaration_position"),
                &crate::BuildTimeValue::Int(0)
            );
            assert_eq!(
                field(value, "has_stable_number"),
                &crate::BuildTimeValue::Bool(true)
            );
            assert_eq!(
                field(value, "is_erased"),
                &crate::BuildTimeValue::Bool(false)
            );
            assert!(
                matches!(field(value, "source_revision"), crate::BuildTimeValue::Text(bytes) if !bytes.is_empty())
            );
        }
        assert_eq!(
            field(&values[0], "stable_number"),
            &crate::BuildTimeValue::Int(7)
        );
        assert_eq!(
            field(&values[2], "stable_number"),
            &crate::BuildTimeValue::Int(9)
        );
        assert_eq!(
            field(&values[0], "case_identity"),
            &crate::BuildTimeValue::Text(Vec::new())
        );
        assert!(
            matches!(field(&values[2], "case_identity"), crate::BuildTimeValue::Text(bytes) if !bytes.is_empty())
        );
        assert_eq!(
            field(&values[0], "member_identity"),
            field(&values[1], "member_identity")
        );
        assert_eq!(
            field(&values[0], "owner_identity"),
            field(&values[1], "owner_identity")
        );
        assert_ne!(
            field(&values[0], "owner_application"),
            field(&values[1], "owner_application")
        );
        assert_ne!(
            field(&values[0], "qualified_type"),
            field(&values[1], "qualified_type")
        );
        assert_eq!(
            field(&values[0], "source_revision"),
            field(&values[1], "source_revision")
        );
        accounting.validate_charges().unwrap();
    }

    #[test]
    fn field_queries_reject_values_and_data_heads_without_reclassifying_ordinary_calls() {
        let source = with_reflection(
            "use omega::language::core::reflect;
            data Cell { value: u32; }
            machine held(value: u32) -> reflect::FieldInfo { reflect::field(value) }
            machine wrong() -> reflect::FieldInfo { reflect::field(Cell) }
            machine observed() -> reflect::FieldInfo {
                let cell: Cell = Cell { value: 1 };
                reflect::field(cell.value)
            }
            machine field(value: u32) -> u32 { value }
            machine ordinary() -> u32 { field(7) }",
        );
        let prepared = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
        let mut accounting = crate::SourceCompletionAccounting::default();
        for (name, arguments, expected) in [
            (
                "held",
                vec![crate::BuildTimeValue::Int(3)],
                "exact resolved field declaration",
            ),
            ("wrong", Vec::new(), "exact resolved field declaration"),
            ("observed", Vec::new(), "uninvoked path"),
        ] {
            let symbol = query_machine(&source, name);
            let reason = prepared
                .entry(symbol, symbol, &arguments)
                .err()
                .expect("a runtime value is not a field selector");
            assert!(reason.contains(expected), "{name}: {reason}");
        }
        let symbol = query_machine(&source, "ordinary");
        assert_eq!(
            prepared
                .entry(symbol, symbol, &[])
                .unwrap()
                .invoke_recorded(target(), &mut accounting)
                .unwrap()
                .value(),
            &crate::BuildTimeValue::Int(7)
        );
    }

    #[test]
    fn callable_queries_use_the_same_landed_application_as_invocation() {
        // The value exceeds i128, but its declared carrier admits it. Both
        // selecting this declaration and invoking it must retain that value;
        // another declaration's same-spelled N is not the same binder.
        let source = with_reflection(
            "use omega::language::core::reflect;
            machine selected<const N: u128>() -> u32 { 7u32 }
            machine other<const N: u128>() -> u32 { 9u32 }
            machine answer() -> reflect::CallableInfo {
                reflect::callable(selected<170141183460469231731687303715884105728>)
            }
            machine alternate() -> reflect::CallableInfo {
                reflect::callable(other<170141183460469231731687303715884105728>)
            }
            machine used() -> u32 { selected<170141183460469231731687303715884105728>() }",
        );
        let program = PreparedBuildMachineProgram::prepare(&source, None).unwrap();
        let mut descriptions = Vec::new();
        for name in ["answer", "alternate"] {
            let symbol = query_machine(&source, name);
            let value = program
                .entry(symbol, symbol, &[])
                .unwrap()
                .invoke(target(), &mut terminal_work::WorkMeter::unbounded())
                .unwrap();
            descriptions.push(value);
        }
        assert_ne!(descriptions[0], descriptions[1]);
        let used = query_machine(&source, "used");
        let used_entry = program.entry(used, used, &[]).unwrap();
        assert_eq!(
            used_entry
                .invoke(target(), &mut terminal_work::WorkMeter::unbounded())
                .unwrap(),
            BuildTimeValue::Int(7)
        );
        let (owner, call) = returned_source_call(&source, "answer");
        let call = source.expression_table.call_node(call).unwrap();
        let [operand] = source.expression_table.expression_handles(call.arguments) else {
            panic!("one declaration operand")
        };
        let selected = source
            .declaration_operand_from_expression(*operand)
            .unwrap();
        let original = typed_trees::reflection::construction::callable_description_identity(
            &source, &selected, owner,
        )
        .unwrap();
        let mut crossed = selected.clone();
        crossed.symbol = query_machine(&source, "other");
        assert!(
            typed_trees::reflection::construction::callable_description_identity(
                &source, &crossed, owner
            )
            .is_err()
        );
        assert!(!original.0);
        let selected_machine = source.machine_by_symbol(selected.symbol).unwrap();
        let other_machine = source.machine_by_symbol(crossed.symbol).unwrap();
        let selected_binder = source.machine_type_parameters(selected_machine)[0].symbol;
        let other_binder = source.machine_type_parameters(other_machine)[0].symbol;
        assert_eq!(
            source.symbols.name(selected_binder),
            source.symbols.name(other_binder)
        );
        assert_ne!(selected_binder, other_binder);
        let (_, used_call) = returned_source_call(&source, "used");
        let application = source
            .expression_table
            .call_node(used_call)
            .unwrap()
            .authored_machine_application;
        let [binding] = source
            .expression_table
            .static_machine_application_bindings(application)
        else {
            panic!("ordinary formation retains its exact binder")
        };
        assert_eq!(binding.parameter, selected_binder);
        let source_application = typed_trees::expression::StaticMachineArgument {
            symbol: selected.symbol,
            application: Some(Box::new(typed_trees::expression::StaticSymbolApplication {
                member: Default::default(),
                lifetime_arguments: Box::default(),
                arguments: source
                    .expression_table
                    .static_machine_application_arguments(application)
                    .to_vec()
                    .into_boxed_slice(),
            })),
            ..Default::default()
        };
        let invoked_identity = typed_trees::type_identity::portable_applied_index_identity(
            &source,
            typed_trees::type_identity::IndexIdentityArgument::StaticArgument(&source_application),
            used,
            &[],
        )
        .and_then(|identity| source.type_identity_text(identity))
        .unwrap();
        assert_eq!(original.1[1], invoked_identity.as_bytes());
        // Use the actual generated body and its checked argument account.
        // A matching template or argument count cannot hide a changed value.
        let entries = [used];
        let request = terminal_production::TerminalSourceCompilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &entries,
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        };
        let (portable, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            source.clone(),
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .expect("prepare the same original source for ordinary portable publication");
        let portable = terminal_production::AcceptedTerminalSource::judge(
            std::sync::Arc::new(portable),
            request,
            terminal_codec::TerminalPublication::Portable,
        )
        .expect("normally judge the same selected call as an ordinary product");
        let bytes = terminal_codec::encode_module(portable.module())
            .expect("encode the actual typed const account with the ordinary source program");
        let mut module = terminal_codec::decode_module(&bytes)
            .expect("the independent receiver checks the same actual application");
        let received = terminal_interpreter::TerminalProgram::load(&bytes)
            .expect("execute the actual received bytes rather than the source product");
        let mut instance = received
            .instantiate(Default::default())
            .expect("the original scalar program needs no external supply");
        let terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Scalar(value),
        ) = instance
            .invoke(&mut terminal_work::WorkMeter::unbounded())
            .expect("run the same original selected call after transport")
        else {
            panic!("the original selected call completes with its scalar result")
        };
        assert_eq!(
            crate::terminal_values::scalar_result(instance.program(), value).unwrap(),
            BuildTimeValue::Int(7)
        );
        let mut bodies = module.machines.iter().filter(|machine| {
            machine
                .declaration
                .as_ref()
                .and_then(|declaration| declaration.specialization.as_ref())
                .is_some_and(|application| {
                    module
                        .identities
                        .text(application.template_semantic_declaration)
                        .map(str::as_bytes)
                        == Some(original.1[0].as_slice())
                })
        });
        let body = bodies.next().expect("one actual generated selected body");
        assert!(bodies.next().is_none(), "the actual application is unique");
        let current = body.id;
        let application = body
            .declaration
            .as_ref()
            .unwrap()
            .specialization
            .as_ref()
            .unwrap();
        let declaration = application.template.clone();
        let identity = terminal_psi::CheckedMachineDeclaration::callable_evidence_identity(
            module
                .identities
                .text(application.template_callable)
                .unwrap(),
            Some(&application.commitment),
        );
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let machine_identity = identities.intern(&identity);
        module.identities = identities.build();
        let caller = (
            original.clone(),
            terminal_psi::StructuralEstablishmentRoute::ExactMachine {
                declaration,
                machine_identity,
            },
        );
        assert!(
            terminal_semantics::structural_qualifications::caller_matches_machine(
                &module, current, &caller
            )
            .expect("the same declaration, entry, signature and actual u128 argument match")
        );
        let mut changed_value = caller;
        let applied = std::str::from_utf8(&changed_value.0.1[1]).unwrap();
        let value = "170141183460469231731687303715884105728";
        assert_eq!(applied.matches(value).count(), 1);
        changed_value.0.1[1] = applied
            .replace(value, "170141183460469231731687303715884105729")
            .into_bytes();
        assert!(
            !terminal_semantics::structural_qualifications::caller_matches_machine(
                &module,
                current,
                &changed_value
            )
            .expect("a different value cannot borrow the checked application's caller identity")
        );
        // Changing both descriptions cannot reuse the old application's hash,
        // even when its body, source identity and route are left untouched.
        let mut changed_account = module.clone();
        let body = changed_account
            .machines
            .iter_mut()
            .find(|body| body.id == current)
            .unwrap();
        let terminal_psi::ClosedReachParameter::Const { argument } =
            &mut body.closed_reach_application.as_mut().unwrap().telescope[0]
        else {
            panic!("the exact generated body retains its scalar argument")
        };
        assert_eq!(argument.encoding.matches(value).count(), 1);
        argument.encoding = argument
            .encoding
            .replace(value, "170141183460469231731687303715884105729");
        let error = terminal_semantics::structural_qualifications::caller_matches_machine(
            &changed_account,
            current,
            &changed_value,
        )
        .expect_err("matching altered descriptions cannot authenticate an unchanged commitment");
        assert_eq!(
            error.detail,
            "machine application arguments differ from their retained commitment"
        );
        assert_eq!(
            terminal_semantics::closed_reach::validate_closed_reach_applications(&changed_account),
            Err("machine application arguments differ from their retained commitment"),
            "the independent receiving owner rejects the same unused argument inconsistency",
        );
        let typed_trees::static_applications::StaticMachineBindingValue::Const(value) =
            binding.value
        else {
            panic!("the binding retains its value, not its carrier")
        };
        assert_eq!(
            source
                .static_const_literal_from_type_reference(value)
                .unwrap()
                .value_bignum()
                .unwrap()
                .to_string(),
            "170141183460469231731687303715884105728"
        );
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
            .entry_call(another, first_call, &[])
            .err()
            .expect("one declaration cannot adopt another declaration's call");
        assert_eq!(error, "source job call has another requesting declaration");
        let mut descriptions = Vec::new();
        for (requester, expression) in [(requester, first_call), (another, second_call)] {
            let expected = source.expression_table.call_node(expression).unwrap();
            let mut accounting = crate::SourceCompletionAccounting::default();
            let value = program
                .entry_call(requester, expression, &[])
                .unwrap()
                .invoke_recorded(target(), &mut accounting)
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
        let meter = terminal_work::WorkMeter::unbounded();
        let error = program
            .entry_call(requester, expression, &[])
            .err()
            .expect("an applied data declaration is not a callable declaration");
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
        let Some([state]) = source.machine_states(machine).as_array() else {
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
            .entry(selected, selected, &[])
            .unwrap()
            .invoke(target(), &mut terminal_work::WorkMeter::unbounded())
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
        let error = program.entry_call(another, expression, &[]).err().unwrap();
        assert_eq!(error, "source job call has another requesting declaration");
        let mut accounting = crate::SourceCompletionAccounting::default();
        let result = program
            .entry_call(requester, expression, &[])
            .unwrap()
            .invoke_recorded(target(), &mut accounting)
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
            .entry_call(requester, expression, &[])
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
        let meter = terminal_work::WorkMeter::unbounded();
        let error = PreparedBuildMachineProgram::prepare(&stale_source, None)
            .unwrap()
            .entry_call(requester, expression, &[])
            .err()
            .expect("malformed supplied references remain unavailable");
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
                .entry_call(requester, expression, &[])
                .unwrap()
                .invoke_recorded(target(), &mut accounting)
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
            let symbol = query_machine(&source, name);
            program
                .entry(symbol, symbol, &[])
                .expect("query entry")
                .invoke(target(), &mut terminal_work::WorkMeter::unbounded())
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
        let symbol = query_machine(&source, "answer");
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
            .entry(symbol, symbol, &[value])
            .err()
            .expect("data descriptions do not supply lexical declaration selection");
        assert!(
            error
                .contains("a callable description selects a concrete machine or trait requirement"),
            "{error}"
        );
    }

    #[test]
    fn inferred_boolean_subjects_execute_without_relaxing_operand_checks() {
        // The caller keeps both outcomes of each predicate. A Boolean result
        // type neither assumes the predicate nor grants a case or domain fact.
        let cases = program(
            "data Choice { case A; case B; }
             machine answer(value: Choice) -> u64 {
                 transition value in Choice::A { true -> 41u64 _ -> 0u64 }
             }",
        );
        for (case, expected) in [("A", 41), ("B", 0)] {
            assert_eq!(
                run(
                    &cases,
                    &[BuildTimeValue::Case {
                        variant: case.into(),
                        payload: Vec::new(),
                    }],
                    &mut terminal_work::WorkMeter::unbounded(),
                )
                .expect("the actual case test chooses its arm"),
                BuildTimeValue::Int(expected),
            );
        }
        let mut refused = terminal_work::WorkMeter::unbounded();
        let error = run(
            &cases,
            &[BuildTimeValue::Case {
                variant: "Other".into(),
                payload: Vec::new(),
            }],
            &mut refused,
        )
        .expect_err("a Boolean case test cannot create another case");
        assert!(error.contains("not declared by formal"), "{error}");
        assert_eq!(refused.usage().total_units(), 0);

        let membership = program(
            "domain u64::Small requires self <= 8;
             machine answer(value: u64) -> u64 {
                 transition value in Small { true -> 41u64 _ -> 0u64 }
             }",
        );
        for (value, expected) in [(0, 41), (8, 41), (9, 0)] {
            assert_eq!(
                run(
                    &membership,
                    &[BuildTimeValue::Int(value)],
                    &mut terminal_work::WorkMeter::unbounded(),
                )
                .expect("the actual domain predicate chooses its arm"),
                BuildTimeValue::Int(expected),
            );
        }
        let negation_source = "machine answer(value: bool) -> u64 {
            transition !value { true -> 41u64 _ -> 0u64 }
        }";
        let negated = program(negation_source);
        for value in [false, true] {
            assert_eq!(
                run(
                    &negated,
                    &[BuildTimeValue::Bool(value)],
                    &mut terminal_work::WorkMeter::unbounded(),
                )
                .expect("the captured negation chooses its actual arm"),
                BuildTimeValue::Int(if value { 0 } else { 41 }),
            );
        }
        let invalid = program(&negation_source.replacen("value: bool", "value: u64", 1));
        let mut refused = terminal_work::WorkMeter::unbounded();
        let error = run(&invalid, &[BuildTimeValue::Int(0)], &mut refused)
            .expect_err("a Boolean result does not turn an integer operand into Bool");
        assert!(error.contains("unary operand type"), "{error}");
        assert_eq!(refused.usage().total_units(), 0);
    }

    #[test]
    fn source_jobs_prove_requires_at_the_actual_arguments_before_execution() {
        let entry = program("machine answer(value: u64) -> u64 requires value > 0 { value }");
        let mut meter = terminal_work::WorkMeter::unbounded();
        assert_eq!(
            run(&entry, &[BuildTimeValue::Int(1)], &mut meter).expect("proved actual call"),
            BuildTimeValue::Int(1)
        );
        assert!(meter.usage().total_units() > 0);
        let mut refused = terminal_work::WorkMeter::unbounded();
        let error = run(&entry, &[BuildTimeValue::Int(0)], &mut refused)
            .expect_err("callee requires are not assumptions of its caller");
        assert!(error.contains("requires"), "{error}");
        assert_eq!(refused.usage().total_units(), 0);
    }

    #[test]
    fn source_jobs_execute_the_exact_visible_overload_and_refuse_incompatible_actuals() {
        let prepared = program(
            r#"
            module kit;
            public machine pick(value: u32) -> u32 { 17u32 }
            public machine pick(value: u64) -> u64 { 64u64 }
            machine answer32(value: u32) -> u32 { kit::pick(value) }
            machine answer64(value: u64) -> u64 { kit::pick(value) }
            machine incompatible(value: bool) -> u32 { kit::pick(value) }
            "#,
        );
        for (name, expected) in [("answer32", 17), ("answer64", 64)] {
            let symbol = prepared
                .typed()
                .realized_machine_named(name)
                .unwrap()
                .symbol;
            let entry = prepared
                .entry(symbol, symbol, &[BuildTimeValue::Int(9)])
                .expect("complete actual types select one ordinary visible header");
            let mut meter = terminal_work::WorkMeter::unbounded();
            assert_eq!(
                entry.invoke(target(), &mut meter).unwrap(),
                BuildTimeValue::Int(expected)
            );
        }
        let wrong = prepared
            .typed()
            .realized_machine_named("incompatible")
            .unwrap()
            .symbol;
        assert_eq!(
            prepared.typed().deferred_refusal(wrong).unwrap().request,
            typed_trees::deferred_refusals::DeferredRequest::UnresolvedCallTarget,
        );
        let error = prepared
            .entry(wrong, wrong, &[BuildTimeValue::Bool(true)])
            .err()
            .expect("no visible integer header accepts the Boolean operand");
        assert!(error.contains("call"), "{error}");
    }

    #[test]
    fn prepared_calls_keep_actual_arguments_with_their_checked_source() {
        let program = program("machine answer(value: u64) -> u64 requires value > 0 { value }");
        let first = bind(&program, &[BuildTimeValue::Int(7)]).expect("first actual call");
        let second = bind(&program, &[BuildTimeValue::Int(8)]).expect("second actual call");
        assert_eq!(first.symbol(), second.symbol());
        assert!(!std::ptr::eq(
            first.accepted_source(),
            second.accepted_source()
        ));
        for (entry, expected) in [
            (first, BuildTimeValue::Int(7)),
            (second, BuildTimeValue::Int(8)),
        ] {
            assert_eq!(entry.arguments(), std::slice::from_ref(&expected));
            assert_eq!(
                entry
                    .accepted_source()
                    .expression_table
                    .call_node(entry.call_handle()),
                Some(entry.call())
            );
            let mut accounting = crate::SourceCompletionAccounting::default();
            assert!(accounting.receipts().is_empty());
            let result = entry
                .invoke_recorded(target(), &mut accounting)
                .expect("run the bound actual call");
            assert_eq!(result.value(), &expected);
            let [crate::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
                panic!("one actual invocation receipt")
            };
            assert!(std::ptr::eq(receipt.source(), entry.accepted_source()));
            assert_eq!(receipt.arguments(), std::slice::from_ref(&expected));
            assert_eq!(receipt.call().unwrap(), entry.call());
        }
    }

    // The real interpreter must replace exactly the selected owned element.
    // A neighboring element and sibling field retain their supplied values;
    // the same source still refuses an index beyond its declared extent.
    #[test]
    fn source_jobs_execute_indexed_record_replacement_without_changing_siblings() {
        let program = program(
            "data Item { value: u64; }\n\
             data Table { items: [Item; 2]; side: Item; }\n\
             machine answer(mut table: Table, index: u64) -> Table requires index < 2 {\n\
                 table.items[index] = Item { value: 7 };\n\
                 table\n\
             }",
        );
        let item = |value| BuildTimeValue::Struct {
            type_name: "Item".into(),
            fields: vec![("value".into(), BuildTimeValue::Int(value))],
        };
        let supplied = BuildTimeValue::Struct {
            type_name: "Table".into(),
            fields: vec![
                (
                    "items".into(),
                    BuildTimeValue::Array(vec![item(1), item(2)]),
                ),
                ("side".into(), item(9)),
            ],
        };
        let arguments = [supplied.clone(), BuildTimeValue::Int(1)];
        let entry = bind(&program, &arguments).expect("judge the actual owned caller");
        let mut accounting = crate::SourceCompletionAccounting::default();
        let result = entry
            .invoke_recorded(target(), &mut accounting)
            .expect("execute the indexed record store and ordinary owned return");
        fn field<'a>(value: &'a BuildTimeValue, name: &str) -> &'a BuildTimeValue {
            let BuildTimeValue::Struct { fields, .. } = value else {
                panic!("actual source record")
            };
            &fields
                .iter()
                .find(|(key, _)| key == name)
                .expect("complete returned field roster")
                .1
        }
        let BuildTimeValue::Array(values) = field(result.value(), "items") else {
            panic!("complete returned array")
        };
        assert_eq!(values.len(), 2);
        assert_eq!(field(&values[0], "value"), &BuildTimeValue::Int(1));
        assert_eq!(field(&values[1], "value"), &BuildTimeValue::Int(7));
        assert_eq!(
            field(field(result.value(), "side"), "value"),
            &BuildTimeValue::Int(9)
        );
        let [crate::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
            panic!("one exact call was interpreted")
        };
        assert_eq!(receipt.arguments(), &arguments);
        receipt.validate_charge().unwrap();
        accounting.validate_charges().unwrap();
        let error = bind(&program, &[supplied, BuildTimeValue::Int(2)])
            .err()
            .expect("the same source requires the actual index bound before execution");
        assert!(
            error.contains("cannot prove requires contract") && error.contains("index < 2"),
            "{error}"
        );
        assert_eq!(
            accounting.receipts().len(),
            1,
            "the refused actual never executes"
        );
    }

    #[test]
    fn source_jobs_establish_record_domains_without_discarding_extra_fields() {
        let entry = program(
            "data Positive requires value > 0 { value: u64; }
            machine answer(input: Positive) -> u64 { input.value }",
        );
        let argument = |value| BuildTimeValue::Struct {
            type_name: "Positive".into(),
            fields: vec![("value".into(), BuildTimeValue::Int(value))],
        };
        assert_eq!(
            run(
                &entry,
                &[argument(1)],
                &mut terminal_work::WorkMeter::unbounded()
            )
            .expect("established record"),
            BuildTimeValue::Int(1)
        );
        assert!(
            run(
                &entry,
                &[argument(0)],
                &mut terminal_work::WorkMeter::unbounded()
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
            run(&entry, &[extra], &mut terminal_work::WorkMeter::unbounded())
                .expect_err("all supplied fields remain obligations")
                .contains("field")
        );
    }

    #[test]
    fn source_jobs_preserve_immutable_byte_backing_and_refuse_mutable_fabrication() {
        let bytes = BuildTimeValue::Text(vec![0, 255, b'x']);
        let shared = program("machine answer(bytes: &[u8]) -> &[u8] { bytes }");
        assert_eq!(
            run(
                &shared,
                std::slice::from_ref(&bytes),
                &mut terminal_work::WorkMeter::unbounded()
            )
            .expect("real shared backing"),
            bytes
        );
        assert_eq!(
            run(
                &shared,
                &[BuildTimeValue::Text(Vec::new())],
                &mut terminal_work::WorkMeter::unbounded()
            )
            .expect("empty real backing"),
            BuildTimeValue::Text(Vec::new())
        );
        let mutable = program("machine answer(bytes: &mut [u8]) -> &mut [u8] { bytes }");
        assert!(
            run(
                &mutable,
                &[bytes],
                &mut terminal_work::WorkMeter::unbounded()
            )
            .is_err()
        );
    }

    #[test]
    fn source_jobs_construct_copy_and_return_byte_fields_with_their_real_contents() {
        let entry = program(
            "data Packet<'bytes> [copy] { bytes: &'bytes [u8]; }
            machine answer(input: Packet) -> Packet { input }",
        );
        let value = BuildTimeValue::Struct {
            type_name: "Packet".into(),
            fields: vec![("bytes".into(), BuildTimeValue::Text(vec![0, 255, b'x']))],
        };
        let result = run(
            &entry,
            std::slice::from_ref(&value),
            &mut terminal_work::WorkMeter::unbounded(),
        )
        .expect("byte field keeps its backing");
        let BuildTimeValue::Struct { type_name, fields } = &result else {
            panic!("record result")
        };
        let machine = entry
            .typed()
            .machine_by_symbol(
                entry
                    .typed()
                    .realized_machine_named("answer")
                    .unwrap()
                    .symbol,
            )
            .unwrap();
        let state = &entry.typed().machine_states(machine)[0];
        assert_eq!(
            type_name,
            &entry
                .typed()
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
            run(
                &entry,
                std::slice::from_ref(&result),
                &mut terminal_work::WorkMeter::unbounded()
            )
            .expect("readback retains its complete formal identity"),
            result
        );
    }

    #[test]
    fn source_jobs_round_trip_numbered_members_with_their_exact_formal() {
        // Numbered members keep their schema identity when materialized.
        // An authored name and its returned number select the same member.
        let entry = program(
            "data Choice<'bytes> [copy] { #7 case Ready(#9 bytes: &'bytes [u8]); #8 case Empty; }
             machine answer(value: Choice) -> Choice { value }",
        );
        let value = BuildTimeValue::Case {
            variant: "Ready".into(),
            payload: vec![("bytes".into(), BuildTimeValue::Text(vec![0, 255, b'x']))],
        };
        let returned = run(&entry, &[value], &mut terminal_work::WorkMeter::unbounded())
            .expect("authored member selects its exact numbered case");
        assert_eq!(
            returned,
            BuildTimeValue::Case {
                variant: "#7".into(),
                payload: vec![("#9".into(), BuildTimeValue::Text(vec![0, 255, b'x']))],
            }
        );
        assert_eq!(
            run(
                &entry,
                std::slice::from_ref(&returned),
                &mut terminal_work::WorkMeter::unbounded()
            )
            .expect("returned identities rejoin the same formal"),
            returned
        );
        let mut meter = terminal_work::WorkMeter::unbounded();
        let unknown = BuildTimeValue::Case {
            variant: "#99".into(),
            payload: vec![("#9".into(), BuildTimeValue::Text(vec![]))],
        };
        let error = run(&entry, &[unknown], &mut meter)
            .expect_err("unknown case is not supplied by a number");
        assert!(
            error.contains("invocation case `#99` is not declared by formal"),
            "{error}"
        );
        assert_eq!(meter.usage().total_units(), 0);
        let wrong_field = BuildTimeValue::Case {
            variant: "#7".into(),
            payload: vec![("#10".into(), BuildTimeValue::Text(vec![]))],
        };
        let error = run(&entry, &[wrong_field], &mut meter)
            .expect_err("a field number names only its declared payload");
        assert!(error.contains("missing a declared field"), "{error}");
        assert_eq!(meter.usage().total_units(), 0);
    }

    #[test]
    fn source_jobs_record_actual_bytes_and_charges_without_exporting_local_code() {
        let source_program = program("machine answer(bytes: &[u8]) -> &[u8] { bytes }");
        let bytes = BuildTimeValue::Text(vec![0, 255, b'x']);
        let entry =
            bind(&source_program, std::slice::from_ref(&bytes)).expect("checked actual bytes");
        let mut accounting = crate::SourceCompletionAccounting::default();
        let measured = entry
            .invoke_recorded(target(), &mut accounting)
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
        let another_entry =
            bind(&source_program, receipt.arguments()).expect("another actual caller");
        let another = another_entry
            .produce_invocation()
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
    fn source_jobs_run_the_kept_ranked_control_loop() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../tests/fixtures/run/views/ranked_slice_iteration/main.omg"
        ));
        let ranked = fixture
            .split_once("machine parity(")
            .and_then(|(_, source)| source.split_once("data Packet"))
            .map(|(tail, _)| format!("machine parity({tail}"))
            .expect("the kept fixture declares parity before Packet");
        let arguments = [BuildTimeValue::Int(3), BuildTimeValue::Bool(false)];

        let trees = with_reflection(&ranked);
        let parity = query_machine(&trees, "parity");
        let entry = PreparedBuildMachineProgram::prepare(&trees, None)
            .unwrap()
            .entry(parity, parity, &arguments)
            .expect("the original rank proves the local control cycle");
        assert_eq!(
            entry.invoke(target(), &mut terminal_work::WorkMeter::unbounded()),
            Ok(BuildTimeValue::Bool(true)),
        );

        let trees = with_reflection(&ranked.replace("remaining - 1", "remaining"));
        let parity = query_machine(&trees, "parity");
        let Err(refusal) = PreparedBuildMachineProgram::prepare(&trees, None)
            .unwrap()
            .entry(parity, parity, &arguments)
        else {
            panic!("a nondecreasing local cycle must fail source checking");
        };
        assert!(
            refusal.contains("removing strict rank edges leaves a control cycle"),
            "{refusal}",
        );
    }

    #[test]
    fn source_jobs_require_progress_before_the_interpreter_starts() {
        // Same direct recursive call as the structural reader's loop controls.
        // The runtime program may be silent about termination; a compiler job
        // must establish it before consuming any interpreter work.
        let source_program = program("machine answer(value: u64) -> u64 { answer(value) }");
        let entry = bind(&source_program, &[BuildTimeValue::Int(1)])
            .expect("ordinary source is accepted without promising progress");
        let produced = entry
            .produce_invocation()
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
        let mut meter = terminal_work::WorkMeter::with_budget(100);
        let error = entry
            .invoke(target(), &mut meter)
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
            // A declaration the full check of a subject does not read yet.
            let unused = "data NotReady<'held, T> { value: &'held T; }";
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
            let entry = program.entry(requester, callee, &[]).unwrap();
            if owner.is_some() {
                assert_eq!(
                    entry
                        .invoke(target(), &mut terminal_work::WorkMeter::unbounded())
                        .expect("one invocation does not claim the unused generic declaration"),
                    BuildTimeValue::Int(17)
                );
            } else {
                let produced = entry
                    .produce_invocation()
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
                terminal_production::AcceptedTerminalSource::judge(
                    Arc::clone(&source),
                    terminal_production::TerminalSourceCompilation {
                        declaration_metadata: &[],
                        entries: &[callee],
                        subjects: &[subject],
                        purpose: source::DependencyScope::Product,
                        selected: &[],
                        calling: None,
                        providers: None,
                        bindings: &[],
                        supplies: &[],
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

    fn packaged_selection_source(
        texts: &[&str],
        bindings: &[(usize, usize, &str)],
    ) -> (Vec<semantic_vocabulary::PackageKeyIdentity>, TypedTrees) {
        let packages = (1..=texts.len())
            .map(|byte| {
                semantic_vocabulary::PackageKeyIdentity::from_digest(
                    [u8::try_from(byte).unwrap(); 32],
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let mut sources = source::SourceMap::default();
        let mut syntax = None;
        let mut ids = Vec::new();
        for (index, text) in texts.iter().copied().enumerate() {
            let root = format!("/selection-{index}");
            let source = sources
                .add_with_metadata(
                    format!("{root}/source.omg").into(),
                    text.into(),
                    root.into(),
                    Some(packages[index]),
                    source::SourceOrigin::User,
                )
                .source_id;
            ids.push(source);
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            if let Some(syntax) = &mut syntax {
                tokens_to_syntax_trees::parse(syntax, source, &tokens).unwrap();
            } else {
                syntax = Some(
                    tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap(),
                );
            }
        }
        let mut request =
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
        request.sources = Some(Arc::new(sources));
        request.top_level_bindings = bindings
            .iter()
            .map(|(from, to, name)| {
                symbols::SourceScopedTopLevelBinding::new(ids[*from], ids[*to], *name)
            })
            .collect();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
        let source =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        (packages, source)
    }

    struct SelectionAuthorityEdges(
        Vec<(
            semantic_vocabulary::PackageKeyIdentity,
            semantic_vocabulary::PackageKeyIdentity,
        )>,
    );
    impl BuildTimeSelectionAuthority for SelectionAuthorityEdges {
        fn allows_declaration_selection(
            &self,
            requester: semantic_vocabulary::PackageKeyIdentity,
            owner: semantic_vocabulary::PackageKeyIdentity,
        ) -> bool {
            self.0.contains(&(requester, owner))
        }
        fn package_label(&self, package: semantic_vocabulary::PackageKeyIdentity) -> String {
            format!("{package:?}")
        }
    }

    #[test]
    fn selected_calls_keep_the_suppliers_declaration_authority_requests() {
        let (packages, source) = packaged_selection_source(
            &[
                "machine request() -> u64 { 0u64 }",
                "public First satisfies Policy { machine plan(value: u64) -> u64 requires value > 0 { value } }",
                "public trait Policy { machine plan(value: u64) -> u64 requires value > 0; }",
            ],
            &[(1, 2, "Policy")],
        );
        let requester = source
            .machines()
            .iter()
            .find(|machine| {
                source.symbols.symbol_package_identity(machine.symbol) == Some(packages[0])
            })
            .unwrap()
            .symbol;
        let selected = source
            .conformances()
            .iter()
            .find(|selected| {
                source.symbols.symbol_package_identity(selected.symbol) == Some(packages[1])
            })
            .unwrap();
        let [row] = selected.rows.as_slice() else {
            panic!("one actual owned member")
        };
        let operand = typed_trees::expression::StaticMachineArgument {
            symbol: selected.symbol,
            ..Default::default()
        };
        let limited = PreparedBuildMachineProgram::prepare(
            &source,
            Some(Arc::new(SelectionAuthorityEdges(vec![(
                packages[0],
                packages[1],
            )]))),
        )
        .unwrap();
        let entry = limited
            .entry_selected(
                requester,
                requester,
                &operand,
                row.requirement,
                &[BuildTimeValue::Int(1)],
            )
            .unwrap();
        let produced = entry.produce_invocation().unwrap();
        let error = entry
            .admit_invocation(&produced, target())
            .expect_err("the implementation's separate trait selection needs its own package edge");
        assert!(
            error.contains("without direct dependency authority"),
            "{error}"
        );
        assert!(
            error.contains(&format!("{:?}", packages[2]))
                && error.contains(&format!("{:?}", packages[1])),
            "{error}"
        );
        let mut meter = terminal_work::WorkMeter::unbounded();
        assert_eq!(entry.invoke(target(), &mut meter).unwrap_err(), error);
        assert_eq!(meter.usage().total_units(), 0);
        let complete = PreparedBuildMachineProgram::prepare(
            &source,
            Some(Arc::new(SelectionAuthorityEdges(vec![
                (packages[0], packages[1]),
                (packages[1], packages[2]),
            ]))),
        )
        .unwrap();
        assert!(
            complete
                .entry(
                    requester,
                    row.realization_machine,
                    &[BuildTimeValue::Int(1)]
                )
                .err()
                .unwrap()
                .contains("private machine of another package")
        );
        let entry = complete
            .entry_selected(
                requester,
                requester,
                &operand,
                row.requirement,
                &[BuildTimeValue::Int(1)],
            )
            .unwrap();
        let mut accounting = crate::SourceCompletionAccounting::default();
        let value = entry.invoke_recorded(target(), &mut accounting).unwrap();
        assert_eq!(value.value(), &BuildTimeValue::Int(1));
        let [crate::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts() else {
            panic!("one actual selected receipt")
        };
        let (_, call) = receipt.invocation().unwrap().unwrap();
        assert!(call.inputs.required_contract.is_some());
        assert_eq!(receipt.requester(), requester);
        assert_eq!(receipt.machine(), row.realization_machine);
    }

    #[test]
    fn a_reference_row_keeps_its_suppliers_body_dependency() {
        let (packages, source) = packaged_selection_source(
            &[
                "machine request() -> u64 { 0u64 }",
                "public First satisfies Policy { Policy::plan = implementation; }",
                "public trait Policy { machine plan(value: u64) -> u64 requires value > 0; }",
                "public machine implementation(value: u64) -> u64 requires value > 0 { helper(value) } machine helper(value: u64) -> u64 { value }",
            ],
            &[(1, 2, "Policy"), (1, 3, "implementation")],
        );
        let requester = source
            .machines()
            .iter()
            .find(|machine| {
                source.symbols.symbol_package_identity(machine.symbol) == Some(packages[0])
            })
            .unwrap()
            .symbol;
        let selected = source
            .conformances()
            .iter()
            .find(|selected| {
                source.symbols.symbol_package_identity(selected.symbol) == Some(packages[1])
            })
            .unwrap();
        let [row] = selected.rows.as_slice() else {
            panic!("one exact reference row")
        };
        assert_eq!(
            row.source,
            typed_trees::trait_definition::ConformanceRowSource::Reference
        );
        let operand = typed_trees::expression::StaticMachineArgument {
            symbol: selected.symbol,
            ..Default::default()
        };
        for has_body_edge in [false, true] {
            let mut edges = vec![(packages[0], packages[1]), (packages[1], packages[2])];
            if has_body_edge {
                edges.push((packages[1], packages[3]));
            }
            let program = PreparedBuildMachineProgram::prepare(
                &source,
                Some(Arc::new(SelectionAuthorityEdges(edges.clone()))),
            )
            .unwrap();
            let entry = program
                .entry_selected(
                    requester,
                    requester,
                    &operand,
                    row.requirement,
                    &[BuildTimeValue::Int(1)],
                )
                .unwrap();
            let mut meter = terminal_work::WorkMeter::unbounded();
            let result = entry.invoke(target(), &mut meter);
            if has_body_edge {
                assert_eq!(result.unwrap(), BuildTimeValue::Int(1));
                let produced = entry.produce_invocation().unwrap();
                let module = produced.view().module();
                let authority = SelectionAuthorityEdges(edges);
                let admit = |module: terminal_psi::TerminalModule| {
                    // Keep the original invocation custody. These are receiving
                    // forgeries, never a promotion to portable publication.
                    let code = terminal_codec::InvocationTerminalCode::new(module)
                        .map_err(|error| format!("artifact refusal: {error:?}"))?;
                    let view = code.view();
                    let root = view
                        .module()
                        .scalar_applications
                        .proof_scope
                        .callable_declaration(view.module().entry.unwrap())
                        .unwrap();
                    let providers = terminal_installation::admit_provider_execution_installation(
                        view,
                        &[],
                        &[],
                    )
                    .map_err(|error| format!("provider refusal: {error:?}"))?;
                    terminal_installation::admit_compiler_evaluation(
                        view,
                        target().name,
                        root,
                        providers,
                        semantic_vocabulary::compiler_evaluation::DeclarationCustody::Package(
                            packages[0],
                        ),
                        Some(&authority),
                    )
                };
                assert!(admit(module.clone()).is_ok());
                let selected = module
                    .machines
                    .iter()
                    .flat_map(|machine| {
                        machine.module_invocations().filter_map(move |call| {
                            call.inputs.required_contract.map(|required| {
                                (machine.id, call.site, call.inputs.callee, required)
                            })
                        })
                    })
                    .collect::<Vec<_>>();
                let [(caller, site, body, required)] = selected.as_slice() else {
                    panic!("one actual selected call")
                };
                let semantic_vocabulary::InvocationSite::Operation(operation) = site else {
                    panic!("the source call returns normally")
                };
                let mut stripped = module.clone();
                stripped
                    .machines
                    .iter_mut()
                    .find(|machine| machine.id == *caller)
                    .unwrap()
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|row| row.id == *operation)
                    .unwrap()
                    .required_contract = None;
                let error = admit(stripped).unwrap_err();
                assert!(
                    error.contains("without direct dependency authority")
                        && error.contains(&format!("{:?}", packages[0]))
                        && error.contains(&format!("{:?}", packages[3])),
                    "{error}"
                );

                let mut missing = module.clone();
                let scope = &mut missing.scalar_applications.proof_scope;
                let declaration = scope.callable_declaration((*caller).into()).unwrap();
                let (header, _) = scope
                    .callable_headers
                    .iter()
                    .find(|(_, header)| header.declaration == declaration)
                    .unwrap();
                scope
                    .callable_headers
                    .get_mut(header)
                    .compiler_evaluation
                    .as_mut()
                    .unwrap()
                    .selections
                    .as_mut()
                    .unwrap()
                    .retain(|request| {
                        request.requester != packages[1] || request.owner != packages[3]
                    });
                let error = admit(missing).unwrap_err();
                assert!(
                    error.contains("supplier's admitted source-selection requests"),
                    "{error}"
                );

                let body_machine = module.machine(*body).unwrap();
                let inner = body_machine.module_invocations().next().unwrap();
                let semantic_vocabulary::InvocationSite::Operation(inner_operation) = inner.site
                else {
                    panic!("the helper call returns normally")
                };
                let mut copied = module.clone();
                copied
                    .machines
                    .iter_mut()
                    .find(|machine| machine.id == *body)
                    .unwrap()
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|row| row.id == inner_operation)
                    .unwrap()
                    .required_contract = Some(*required);
                let error = admit(copied).unwrap_err();
                assert!(
                    error.contains("selected invocation lost its exact public/body association"),
                    "{error}"
                );
            } else {
                let error = result.unwrap_err();
                assert!(
                    error.contains("without direct dependency authority")
                        && error.contains(&format!("{:?}", packages[1]))
                        && error.contains(&format!("{:?}", packages[3])),
                    "{error}"
                );
                assert_eq!(meter.usage().total_units(), 0);
            }
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
            .entry(requester, callee, &[BuildTimeValue::Int(1)])
            .unwrap();
        let accepted = &entry.accepted;
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
        let produced = entry.produce_invocation().unwrap();
        let error = entry
            .admit_invocation(&produced, target())
            .expect_err("the dependency cannot request itself for the customer");
        assert!(
            error.contains("no direct dependency selection authority"),
            "{error}"
        );
        let mut meter = terminal_work::WorkMeter::unbounded();
        assert_eq!(entry.invoke(target(), &mut meter).unwrap_err(), error);
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
            .entry(requester, callee, &[BuildTimeValue::Int(1)])
            .unwrap();
        let mut accounting = crate::SourceCompletionAccounting::default();
        let value = entry
            .invoke_recorded(target(), &mut accounting)
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

#[cfg(test)]
#[path = "source_jobs/schema_customer_source.rs"]
mod schema_customer_source;

#[cfg(test)]
#[path = "source_jobs/access_plan_construction.rs"]
mod access_plan_construction;

#[cfg(test)]
#[path = "source_jobs/sequence_values.rs"]
mod sequence_values;

#[cfg(test)]
#[path = "source_jobs/borrowed_views.rs"]
mod borrowed_views;
