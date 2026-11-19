//! Prepare the actual source call, with its complete arguments and selected body,
//! through ordinary typed preparation and the checked program publisher.
use crate::evaluation::BuildTimeSelectionAuthority;
use crate::source_state::TypedTrees;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;

#[path = "calls/arguments.rs"]
mod arguments;
#[path = "calls/execution.rs"]
mod execution;
#[path = "calls/source_selection.rs"]
mod source_selection;
#[path = "calls/target.rs"]
mod target;
pub use target::InvocationTarget;

/// A build-time value with the engine-tagged usage record of the evaluation
/// that produced it. Both engines publish `BuildEvaluationUsage`, whose
/// measurement identity names the work and storage schedules, so consumers
/// such as evaluated binding digests never interpret one engine's counts as
/// another's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredEvaluation<T> {
    value: T,
    usage: evaluation_contracts::BuildEvaluationUsage,
}

impl<T> MeasuredEvaluation<T> {
    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn usage(&self) -> evaluation_contracts::BuildEvaluationUsage {
        self.usage
    }

    pub fn into_value(self) -> T {
        self.value
    }

    pub fn into_parts(self) -> (T, evaluation_contracts::BuildEvaluationUsage) {
        (self.value, self.usage)
    }
}

/// The input source is retained as metadata, never as a checked-program witness.
pub struct PreparedBuildMachineProgram {
    source: Arc<TypedTrees>,
    publication: terminal_codec::TerminalPublication,
    exact_toolchain_sources: Box<[(source_text::SourceId, [u8; 32])]>,
    authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    purpose: source_provenance::DependencyScope,
    source_consent: Option<Arc<dyn crate::evaluation::InvocationSourceConsent>>,
}
#[derive(Clone)]
pub struct PreparedBuildMachineEntry {
    source: Arc<TypedTrees>,
    symbol: SymbolHandle,
    requester: SymbolHandle,
    call: crate::source_state::expression::TableCallExpression,
    call_source: crate::source_state::expression::ExpressionHandle,
    authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    purpose: source_provenance::DependencyScope,
    source_consent: Option<Arc<dyn crate::evaluation::InvocationSourceConsent>>,
    accepted: Arc<crate::production::AcceptedTerminalSource>,
    arguments: Arc<[crate::evaluation::BuildTimeValue]>,
    input_values: Arc<[crate::evaluation::BuildTimeValue]>,
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

    /// The actual caller and its arguments have already been judged together.
    /// Consumers may inspect that exact source before execution; neither the
    /// source nor a replacement argument list is accepted by the run method.
    pub fn accepted_source(&self) -> &crate::source_state::FrozenTypedTrees {
        self.accepted.source()
    }

    pub fn call(&self) -> &crate::source_state::expression::TableCallExpression {
        &self.call
    }

    pub fn arguments(&self) -> &[crate::evaluation::BuildTimeValue] {
        &self.arguments
    }

    pub(crate) fn accepted(&self) -> &crate::production::AcceptedTerminalSource {
        &self.accepted
    }

    pub(crate) fn produce_invocation(
        &self,
    ) -> Result<crate::production::ProducedTerminalProgram, String> {
        self.accepted
            .produce(
                Default::default(),
                &mut crate::production::UnmeasuredProduction,
            )
            .map_err(|errors| diagnostics::join_messages(&errors))
    }
}
pub(crate) fn invocation_error(reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}

impl PreparedBuildMachineProgram {
    pub fn prepare(
        typed: &TypedTrees,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::prepare_for(
            typed,
            authority,
            source_provenance::DependencyScope::Product,
            None,
        )
    }

    /// Retain the caller's actual source purpose and captured-source consent.
    /// A private capture remains unowned; it is not a package-selection grant.
    pub fn prepare_for(
        typed: &TypedTrees,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        purpose: source_provenance::DependencyScope,
        source_consent: Option<Arc<dyn crate::evaluation::InvocationSourceConsent>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        // Retaining source is preparation, not acceptance. Each actual job is
        // checked with its materialized arguments before any execution starts.
        let exact_toolchain_sources =
            source_admission::source_identity::toolchain_source_identities(
                typed.symbols.source_files(),
            )
            .map_err(invocation_error)?
            .into_boxed_slice();
        Ok(Self {
            source: Arc::new(typed.clone()),
            publication: terminal_codec::TerminalPublication::Invocation,
            exact_toolchain_sources,
            authority,
            purpose,
            source_consent,
        })
    }
    pub(crate) fn prepare_for_layout(
        typed: &TypedTrees,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        purpose: source_provenance::DependencyScope,
        source_consent: Option<Arc<dyn crate::evaluation::InvocationSourceConsent>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut program = Self::prepare_for(typed, authority, purpose, source_consent)?;
        program.publication = terminal_codec::TerminalPublication::LayoutPolicyInvocation;
        Ok(program)
    }

    pub(crate) fn exact_toolchain_sources(&self) -> &[(source_text::SourceId, [u8; 32])] {
        &self.exact_toolchain_sources
    }

    fn prepare_call(
        &self,
        requester: SymbolHandle,
        declaration: crate::source_state::callable_signature_shapes::CallableDeclaration<'_>,
        request: &crate::source_state::expression::TableCallExpression,
        request_expression: crate::source_state::expression::ExpressionHandle,
        arguments: &[crate::evaluation::BuildTimeValue],
        selected: Option<(
            &crate::source_state::expression::StaticMachineArgument,
            SymbolHandle,
        )>,
        supplies: &[crate::production::TerminalSourceSupply],
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
        let arguments::PreparedSourceInvocation {
            source,
            caller,
            call,
            input_values,
        } = arguments::prepare_value_invocation(
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
        let (source, _) = crate::preparation::prepare_typed_trees(
            source,
            &crate::preparation::PreparationRequest::settled(),
        )?;
        let source = Arc::new(source);
        let accepted = crate::production::AcceptedTerminalSource::judge(
            source,
            crate::production::TerminalSourceCompilation {
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
            self.publication,
        )?;
        let entry = accepted
            .entry()
            .ok_or_else(|| invocation_error("source invocation has no accepted entry"))?;
        if accepted.closure().elaboration().machines.get(entry) != Some(&caller) {
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
            crate::source_state::callable_signature_shapes::CallableDeclaration::Machine(
                required,
            ) => {
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
            crate::source_state::callable_signature_shapes::CallableDeclaration::Signature(
                required,
            ) => {
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
                let module = &accepted.closure().elaboration().module;
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
                        crate::source_state::references::StaticArgumentCall::Expression(
                            call,
                            &actual_call,
                        ),
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
                    .elaboration()
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
            arguments: Arc::from(arguments),
            input_values: Arc::from(input_values),
        })
    }

    /// Keep the actual authored call and its complete arguments in the job.
    /// The expression is selected from this retained source owner; its author
    /// must be the actual requesting declaration, not the called declaration.
    #[cfg(test)]
    pub(crate) fn entry_call(
        &self,
        requester: SymbolHandle,
        expression: crate::source_state::expression::ExpressionHandle,
        arguments: &[crate::evaluation::BuildTimeValue],
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
        if crate::source_state::references::StaticArgumentCall::Expression(expression, call)
            .lexical_owner(&self.source)
            != requester
        {
            return Err("source job call has another requesting declaration".into());
        }
        let declaration = self.source.source_call_declaration(
            requester,
            crate::source_state::references::StaticArgumentCall::Expression(expression, call),
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
        arguments: &[crate::evaluation::BuildTimeValue],
    ) -> Result<PreparedBuildMachineEntry, String> {
        self.prepare_entry(requester, symbol, arguments)
            .map_err(|errors| diagnostics::join_messages(&errors))
    }

    pub(crate) fn prepare_entry(
        &self,
        requester: SymbolHandle,
        symbol: SymbolHandle,
        arguments: &[crate::evaluation::BuildTimeValue],
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
        let call = arguments::direct_call(&self.source, machine).map_err(invocation_error)?;
        self.prepare_call(
            requester,
            crate::source_state::callable_signature_shapes::CallableDeclaration::Machine(
                machine.symbol,
            ),
            &call,
            crate::source_state::expression::ExpressionHandle::invalid(),
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
        selection: &crate::source_state::expression::StaticMachineArgument,
        requirement: SymbolHandle,
        arguments: &[crate::evaluation::BuildTimeValue],
    ) -> Result<PreparedBuildMachineEntry, String> {
        use crate::source_state::type_identity::{
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
        let identity = |operand: &crate::source_state::expression::StaticMachineArgument, form| {
            self.source
                .declared_static_operand_identity(DeclaredStaticOperandIdentityRequest {
                    operand: IndexIdentityArgument::StaticArgument(operand),
                    lexical_owner: selection_owner,
                    binders: &[],
                    lifetime_binders: &[],
                    exact_toolchain_sources: &self.exact_toolchain_sources,
                })
                .ok_or_else(|| {
                    format!(
                        "selected source invocation has no complete operand identity: {form} \
                         operand `{}` supplied by `{}`",
                        self.source.symbols.display_path(operand.symbol, "::"),
                        self.source.symbols.display_path(selection_owner, "::"),
                    )
                })
        };
        let retained = identity(selection, "retained")?;
        let mut explicit = selection.clone();
        explicit.type_reference = Default::default();
        if identity(&explicit, "explicit")? != retained {
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
        let call = arguments::declaration_call(signature.symbol, signature.name.clone());
        self.prepare_call(
            requester,
            crate::source_state::callable_signature_shapes::CallableDeclaration::Signature(
                signature,
            ),
            &call,
            crate::source_state::expression::ExpressionHandle::invalid(),
            arguments,
            Some((selection, selection_owner)),
            &[],
        )
        .map_err(|errors| diagnostics::join_messages(&errors))
    }
}
