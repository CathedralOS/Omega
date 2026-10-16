//! Constant invocations share checked admission and the hermetic interpreter.
//!
//! The scalar evaluator owns expression order and anonymous landing. This owner
//! only crosses the call boundary: validate every authored argument before any
//! selective execution, preserve its declared carrier, then create fresh value
//! snapshots for the exact selected entry. A private checked probe supplies the
//! body/type and crash evidence missing from the pre-resolution syntax forest.
//! Neither a temporary constant placeholder nor an empty service row establishes
//! that evidence. The caller must first close the declaration dependency graph.
//!
//! Structured probes also retain the selected result carrier before interpretation.
//! Materialization writes into the declared destination; equal encoded payloads
//! cannot authorize changing a nominal owner. Evaluation and receiving replay
//! therefore share the same declaration-backed result check. Constructed integer
//! members also owe their declared range independently: use the shared checker
//! at the actual return statement, including every match arm, before selective
//! interpretation can erase a bad sibling. This proves a predicate, never a
//! domain issuer or carrier from an encoded payload. Direct literals that do not
//! need a structured probe retain their separate declaration-admission route.
//! Scalar probes keep their exact evaluator rather than acquiring runtime
//! return-range obligations.

use std::sync::Arc;

use diagnostics::Diagnostic;
use language_semantics::const_value::{
    CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue,
};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    machine::Machine,
    state::State,
    types::PrimitiveType,
};

use crate::const_evaluation::const_generic_expressions::value::{self, ConstantCalls};
use crate::const_evaluation::const_generic_expressions::{
    scalar_probe_destination, value::ScalarValue,
};
use crate::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue};

pub(crate) struct CheckedInitializers {
    typed: TypedTrees,
    admission: BuildTimeAdmissionPlan,
    authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    crash_causes: Vec<(symbols::SymbolHandle, Vec<checked_trees::CrashCause>)>,
    probe_symbols: Vec<symbols::SymbolHandle>,
}

impl CheckedInitializers {
    pub(crate) fn prepare(
        prepared: &crate::PreparedBuildMachineProgram,
        authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
        probe_symbols: &[symbols::SymbolHandle],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut bodies = prepared.typed().clone();
        let (probes, machines): (Vec<_>, Vec<_>) = bodies
            .machines()
            .iter()
            .cloned()
            .partition(|machine| probe_symbols.contains(&machine.symbol));
        if probes.len() != probe_symbols.len() {
            return Err(super::failure(
                source::SourceSpan::default(),
                "initializer preparation lost an exact probe owner",
            ));
        }
        // Probe expressions are checked by the exact scalar evaluator, not as
        // runtime bodies whose call results have only a declared range. Only
        // their compiler-created roots leave this private body check. Every
        // authored helper body remains unchanged and undergoes ordinary checking.
        bodies.roots.machines = arena::HandleSpan::empty();
        for machine in machines {
            bodies.push_machine(machine);
        }
        let preparation_request = typed_trees_to_typed_trees::PreparationRequest::settled();
        let checking = typed_trees_to_checked_trees::CheckingRequest::new();
        let (bodies, preparation) =
            typed_trees_to_typed_trees::prepare_typed_trees(bodies, &preparation_request)?;
        let checked =
            typed_trees_to_checked_trees::lower_typed_trees(bodies, preparation, &checking)?;
        let crash_causes = typed_trees_to_checked_trees::infer_checked_crash_causes(
            &checked.typed,
            &checked.facts,
        );
        let mut typed = checked.typed.into_inner();
        for probe in probes {
            typed.push_machine(probe);
        }
        let admission = BuildTimeAdmissionPlan::infer(&typed, authority.clone());
        Ok(Self {
            typed,
            admission,
            authority,
            crash_causes,
            probe_symbols: probe_symbols.to_vec(),
        })
    }

    pub(super) fn typed(&self) -> &TypedTrees {
        &self.typed
    }

    pub(super) fn calls<'program>(
        &'program self,
        machine: &'program Machine,
        state: &'program State,
    ) -> Invocation<'program> {
        Invocation {
            program: self,
            machine,
            state,
        }
    }

    pub(crate) fn calls_for_source(
        &self,
        reference: source::SourceSpan,
    ) -> Result<Invocation<'_>, String> {
        let typed = self.typed();
        let mut machines = typed
            .machines()
            .iter()
            .filter(|machine| typed.symbols.symbol_source_span(machine.symbol) == Some(reference));
        let machine = machines
            .next()
            .ok_or("checked initializer probe lost its source owner")?;
        if machines.next().is_some() {
            return Err("checked initializer probe source owner is ambiguous".into());
        }
        let [state] = typed.machine_states(machine) else {
            return Err("checked initializer probe lost its single state".into());
        };
        Ok(self.calls(machine, state))
    }

    pub(crate) fn calls_for_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<Invocation<'_>, String> {
        let typed = self.typed();
        let machine = typed
            .machine(symbol)
            .ok_or("retained initializer probe lost its exact symbol")?;
        let [state] = typed.machine_states(machine) else {
            return Err("retained initializer probe lost its single state".into());
        };
        Ok(self.calls(machine, state))
    }
}

pub(crate) struct Invocation<'program> {
    program: &'program CheckedInitializers,
    machine: &'program Machine,
    state: &'program State,
}

type ArgumentSnapshot = crate::machine_execution::admission::SnapshotArgument;

impl Invocation<'_> {
    /// A published guarded ceiling has no private whole-body summary, and an
    /// authored `requires` premise has no meaning outside an invocation.
    /// Check the exact call instead of inspecting the body to narrow either
    /// contract, or treating successful interpretation as admission evidence:
    /// the shared snapshot probe's ordinary checking discharges the premise
    /// and the guarded routes at the snapshot arguments.
    fn require_concrete_failure_discharge(
        &self,
        expression: ExpressionHandle,
        callee: String,
        return_type: typed_trees::types::TypeReferenceHandle,
        snapshots: Vec<ArgumentSnapshot>,
    ) -> Result<(), String> {
        let retained: Vec<symbols::SymbolHandle> = self
            .program
            .typed()
            .machines()
            .iter()
            .filter(|machine| !self.program.probe_symbols.contains(&machine.symbol))
            .map(|machine| machine.symbol)
            .collect();
        crate::machine_execution::admission::snapshot_arguments_discharge(
            self.program.typed(),
            &retained,
            self.machine.symbol,
            "@const-invocation",
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
            &typed_trees_to_checked_trees::CheckingRequest::new(),
            expression,
            &callee,
            return_type,
            snapshots,
            &crate::const_evaluation::range_endpoints::deferred_endpoint_stand_ins(
                self.program.typed(),
            ),
        )
    }

    pub(super) fn validate_custody(
        &self,
        original: ExpressionHandle,
        materialized: ExpressionHandle,
    ) -> Result<Vec<crate::const_evaluation::const_generic_expressions::DependencyValue>, String>
    {
        crate::const_evaluation::const_generic_expressions::validate_retained_initializer_call_custody(
            self.program.typed(),
            self.machine,
            self.state,
            original,
            materialized,
        )
    }

    pub(super) fn evaluate(
        &self,
        expression: ExpressionHandle,
        destination: PrimitiveType,
    ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
        let (value, warnings) = self.evaluate_scalar(expression, destination)?;
        Ok((value.into_index()?, warnings))
    }

    pub(super) fn evaluate_scalar(
        &self,
        expression: ExpressionHandle,
        destination: PrimitiveType,
    ) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
        value::evaluate_scalar(
            self.program.typed(),
            self.machine,
            self.state,
            expression,
            destination,
            Some(self),
        )
    }

    /// Resolve the exact closed callee for a call and admit it against the
    /// common floor: either an ordinary machine entry state or a named
    /// boundary operator requirement kept as the call's own declaration. The
    /// entry arm's flag records that the closure carries an authored
    /// `requires` premise: such an invocation still owes the premise,
    /// discharged by the concrete checked probe before interpretation, so the
    /// caller must always run that probe rather than relying on the floor's
    /// conservative closure fence. The operator arm owes only its own
    /// authored `requires` clause, discharged the same concrete way.
    fn selected(&self, expression: ExpressionHandle) -> Result<SelectedCall<'_>, String> {
        let typed = self.program.typed();
        crate::machine_execution::admission::require_call_expression_selection(
            typed,
            expression,
            self.program.authority.as_deref(),
        )?;
        let Some(call) = typed.expression_table.call_node(expression) else {
            return Err("initializer lost its selected call".into());
        };
        if !call.carries_only_positional_arguments()
            || call.static_machine_parameter.is_valid()
            || !call.selects_only_nominal_route()
        {
            return Err(
                "constant invocation needs its complete specialized application context".into(),
            );
        }
        let mut selected = typed.machines().iter().filter(|machine| {
            machine.type_parameters.is_empty()
                && typed
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| typed.call_has_no_runtime_receiver(call, machine, entry))
        });
        let Some(machine) = selected.next() else {
            // No closed ordinary entry: the call may name an exact boundary
            // operator requirement (`F32::square_root`) kept on the call as
            // its own declaration identity. Resolution reuses the same
            // path-and-arity selection checking applies, so a name that is a
            // family rather than an exact overload still declines here.
            let operator = typed_trees::operator::resolve_named_expression_call(typed, call)
                .ok_or("constant call has no exact closed ordinary entry or named requirement")?;
            return Ok(SelectedCall::Operator(operator));
        };
        if selected.next().is_some() {
            return Err("constant call has conflicting exact entries".into());
        }
        let state = typed
            .machine_states(machine)
            .first()
            .ok_or("constant call lost its entry")?;
        let custody =
            BuildTimeInvocationCustody::Source(typed.expression_table.source_span(expression));
        if self
            .program
            .admission
            .closure_includes_authored_requires(typed, machine)
        {
            self.program
                .admission
                .require_common_floor_for_concrete_premise_invocation(typed, machine, custody)?;
            return Ok(SelectedCall::Entry {
                premise_discharge: true,
                machine,
                entry: state,
            });
        }
        self.program
            .admission
            .require_common_floor_for_invocation(typed, machine, custody)?;
        Ok(SelectedCall::Entry {
            premise_discharge: false,
            machine,
            entry: state,
        })
    }
}

/// The exact closed callee one admitted call selects: an ordinary machine
/// entry state, or a named boundary operator requirement whose own
/// declaration identity the call retains (`F32::square_root`). A requirement
/// has no body to execute; the checked interpreter's named-requirement route
/// supplies the value through `FloatSemantics`, the checked provider behind
/// the boundary contract, rather than a selected provider body that does not
/// yet exist at pre-resolution evaluation.
enum SelectedCall<'program> {
    Entry {
        premise_discharge: bool,
        machine: &'program Machine,
        entry: &'program State,
    },
    Operator(&'program typed_trees::operator::OperatorDefinition),
}

impl SelectedCall<'_> {
    fn return_type(&self) -> typed_trees::types::TypeReferenceHandle {
        match self {
            Self::Entry { entry, .. } => entry.return_type,
            Self::Operator(operator) => operator.return_type,
        }
    }
}

impl ConstantCalls for Invocation<'_> {
    fn validate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(PrimitiveType, Vec<Diagnostic>), String> {
        let typed = self.program.typed();
        let selected = self.selected(expression)?;
        let destination = scalar_probe_destination(typed, selected.return_type())
            .ok_or("constant call result needs an exact builtin scalar carrier")?;
        let Some(call) = typed.expression_table.call_node(expression) else {
            return Err("constant validation lost its selected call".into());
        };
        let arguments = typed.expression_table.expression_handles(call.arguments);
        // Each explicit operand owes its exact carrier validation.
        let operands: Vec<(ExpressionHandle, &typed_trees::signature::StateParameter)> =
            match selected {
                SelectedCall::Entry { entry, .. } => {
                    let parameters = typed.state_parameters(entry);
                    if arguments.len() != call.arguments.len()
                        || arguments.len() != parameters.len()
                    {
                        return Err(
                            "constant call argument count differs from its exact entry".into()
                        );
                    }
                    arguments.iter().copied().zip(parameters.iter()).collect()
                }
                SelectedCall::Operator(operator) => {
                    let parameters = typed.operator_parameters(operator);
                    let mut operands: Vec<(
                        ExpressionHandle,
                        &typed_trees::signature::StateParameter,
                    )> = Vec::new();
                    let mut argument = arguments.iter().copied();
                    for parameter in parameters {
                        let argument = argument.next().ok_or(
                            "constant call argument count differs from its exact requirement",
                        )?;
                        operands.push((argument, parameter));
                    }
                    if argument.next().is_some() {
                        return Err(
                            "constant call argument count differs from its exact entry".into()
                        );
                    }
                    operands
                }
            };
        let mut warnings = Vec::new();
        for (argument, parameter) in operands {
            let carrier = scalar_probe_destination(typed, parameter.type_reference)
                .ok_or("constant call argument needs an exact builtin scalar carrier")?;
            for warning in value::validate(
                typed,
                self.machine,
                self.state,
                argument,
                carrier,
                Some(self),
            )? {
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
        }
        Ok((destination, warnings))
    }

    fn evaluate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
        let (value, warnings) = self.evaluate_scalar_call(expression)?;
        Ok((value.into_index()?, warnings))
    }

    fn evaluate_scalar_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
        let typed = self.program.typed();
        let AdmittedCall {
            target,
            arguments,
            warnings,
        } = self.admit_call(expression)?;
        let custody =
            BuildTimeInvocationCustody::Source(typed.expression_table.source_span(expression));
        let result = match target {
            SelectedCall::Entry {
                premise_discharge,
                machine,
                ..
            } if premise_discharge => {
                // The probe above re-ran ordinary checked contract proof at the
                // snapshot arguments; its success is what admits the authored
                // `requires` premises here. Interpretation itself proves
                // nothing.
                self.program
                    .admission
                    .evaluate_const_evaluable_machine_symbol_for_concrete_premise_invocation(
                        typed,
                        machine.symbol,
                        arguments,
                        custody,
                    )?
            }
            SelectedCall::Entry { machine, .. } => self
                .program
                .admission
                .evaluate_const_evaluable_machine_symbol_for_invocation(
                    typed,
                    machine.symbol,
                    arguments,
                    custody,
                )?,
            SelectedCall::Operator(operator) => {
                // A named requirement has no body to invoke, but the authored
                // call expression itself is a complete checked program: wrap
                // it in a private probe and let the ordinary checked
                // interpreter's named-requirement route supply the value
                // through `FloatSemantics`. This is not a second resolver —
                // the probe re-runs ordinary checking, which re-derives the
                // same exact operator selection and rejects any call whose
                // authored name is a family rather than an overload.
                let mut probe = typed.clone();
                let owner = append_probe(
                    &mut probe,
                    self.machine.symbol,
                    "@const-requirement".into(),
                    expression,
                    operator.return_type,
                );
                let admission =
                    crate::BuildTimeAdmissionPlan::infer(&probe, self.program.authority.clone());
                admission.evaluate_const_evaluable_machine_symbol_for_invocation(
                    &probe,
                    owner,
                    Vec::new(),
                    custody,
                )?
            }
        };
        let destination = scalar_probe_destination(typed, target.return_type())
            .ok_or("constant result lost its exact carrier")?;
        let value = match result {
            BuildTimeValue::UndeterminedNaN(format)
                if matches!(
                    (format, destination),
                    (numerics::literals::FloatFormat::F32, PrimitiveType::F32)
                        | (numerics::literals::FloatFormat::F64, PrimitiveType::F64)
                ) =>
            {
                ScalarValue::UndeterminedNaN(format)
            }
            BuildTimeValue::Bool(value) if destination == PrimitiveType::Bool => {
                ScalarValue::Index(CanonicalConstValue::boolean(value))
            }
            BuildTimeValue::Int(value) if destination.accepts_integer_literal() => {
                let value = if destination.is_signed_integer() {
                    i128::from(value)
                } else {
                    i128::from(value as u64)
                };
                let identity = CanonicalConstIdentity::integer(destination.name(), value);
                ScalarValue::Index(CanonicalConstValue::new(
                    identity.type_name,
                    identity.encoding,
                    value.to_string(),
                ))
            }
            BuildTimeValue::Float(value)
                if matches!(destination, PrimitiveType::F32 | PrimitiveType::F64) =>
            {
                // The interpreter carries floats in f64 storage, but that is
                // not permission to round an incorrectly landed f32 result.
                // As in ConstMaterializable, NaN needs exact realization
                // custody before an evaluator choice can enter image bytes.
                if value.is_nan() {
                    return Err(
                        "constant result is NaN without an exact raw-NaN realization".into(),
                    );
                }
                match destination {
                    PrimitiveType::F32 if f64::from(value as f32).to_bits() == value.to_bits() => {
                        ScalarValue::Float {
                            format: numerics::literals::FloatFormat::F32,
                            bits: u64::from((value as f32).to_bits()),
                        }
                    }
                    PrimitiveType::F64 => ScalarValue::Float {
                        format: numerics::literals::FloatFormat::F64,
                        bits: value.to_bits(),
                    },
                    _ => {
                        return Err(
                            "constant result does not retain one exact binary32 value".into()
                        );
                    }
                }
            }
            _ => return Err("constant result does not match its declared scalar carrier".into()),
        };
        Ok((value, warnings))
    }
}

/// One admitted authored call: exact selected target, evaluated snapshot
/// arguments, and the concrete discharge evidence the scalar evaluator turns
/// into a result.
struct AdmittedCall<'program> {
    target: SelectedCall<'program>,
    arguments: Vec<BuildTimeValue>,
    warnings: Vec<Diagnostic>,
}

impl Invocation<'_> {
    /// Admit one authored call without interpreting it: select its exact
    /// closed entry or named requirement, evaluate every argument snapshot in
    /// this probe's context, and discharge its crash/requires contract at
    /// those concrete arguments.
    fn admit_call(&self, expression: ExpressionHandle) -> Result<AdmittedCall<'_>, String> {
        let typed = self.program.typed();
        let target = self.selected(expression)?;
        // Reuse complete argument-independent evidence when it exists. A
        // fallible or otherwise unsummarized call needs fresh checking
        // snapshots, and an authored `requires` premise in the closure always
        // does: only the concrete probe can decide it at these arguments. A
        // named requirement has no body whose failure routes need the summary
        // — its own authored `requires` clause is the whole premise — but the
        // probe still discharges that clause concretely the same way.
        let needs_concrete_discharge = match target {
            SelectedCall::Entry {
                premise_discharge,
                machine,
                ..
            } => {
                premise_discharge
                    || !self
                        .program
                        .crash_causes
                        .iter()
                        .any(|(symbol, causes)| *symbol == machine.symbol && causes.is_empty())
            }
            SelectedCall::Operator(operator) => {
                typed.operator_contracts(operator).iter().any(|contract| {
                    contract.kind == typed_trees::signature::SignatureContractKind::Requires
                })
            }
        };
        let Some(call) = typed.expression_table.call_node(expression) else {
            return Err("constant execution lost its selected call".into());
        };
        // Pair each evaluated operand expression with its declared carrier.
        let operands: Vec<(ExpressionHandle, typed_trees::types::TypeReferenceHandle)> =
            match target {
                SelectedCall::Entry { entry, .. } => typed
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .zip(
                        typed
                            .state_parameters(entry)
                            .iter()
                            .map(|parameter| parameter.type_reference),
                    )
                    .collect(),
                SelectedCall::Operator(operator) => {
                    let mut operands = Vec::new();
                    let mut arguments = typed
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .copied();
                    for parameter in typed.operator_parameters(operator) {
                        let argument = arguments.next().ok_or(
                            "constant call argument count differs from its exact requirement",
                        )?;
                        operands.push((argument, parameter.type_reference));
                    }
                    if arguments.next().is_some() {
                        return Err(
                            "constant call argument count differs from its exact requirement"
                                .into(),
                        );
                    }
                    operands
                }
            };
        let mut arguments = Vec::new();
        let mut snapshots = Vec::new();
        let mut warnings = Vec::new();
        for (argument, type_reference) in operands {
            let destination = scalar_probe_destination(typed, type_reference)
                .ok_or("constant call argument lost its exact carrier")?;
            let (value, argument_warnings) = value::evaluate_scalar(
                typed,
                self.machine,
                self.state,
                argument,
                destination,
                Some(self),
            )?;
            if needs_concrete_discharge {
                // A payloadless NaN is an evaluable meaning, not a literal
                // representation. Keep its selected computation in this
                // disposable checking probe instead of manufacturing bits.
                // Retaining the handle also retains authored selection custody;
                // cloning just the root node would discard that metadata.
                // The ordinary checker must still discharge every premise
                // and failure route; successful evaluation is not evidence.
                snapshots.push(ArgumentSnapshot {
                    receiver: false,
                    original: argument,
                    literal: match &value {
                        ScalarValue::UndeterminedNaN(_) => None,
                        _ => Some(scalar_snapshot(&value, destination)?),
                    },
                });
            }
            arguments.push(match value {
                ScalarValue::UndeterminedNaN(format) => BuildTimeValue::UndeterminedNaN(format),
                ScalarValue::Float { format, bits } => BuildTimeValue::Float(match format {
                    numerics::literals::FloatFormat::F32 => f64::from(f32::from_bits(
                        u32::try_from(bits).map_err(|_| "invalid binary32 snapshot")?,
                    )),
                    numerics::literals::FloatFormat::F64 => f64::from_bits(bits),
                }),
                ScalarValue::Index(value) => match value.decode_encoding() {
                    Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
                        let bits = if destination.is_signed_integer() {
                            i64::try_from(value).map_err(
                                |_| "constant argument exceeds signed interpreter storage",
                            )?
                        } else {
                            u64::try_from(value).map_err(
                                |_| "constant argument exceeds unsigned interpreter storage",
                            )? as i64
                        };
                        BuildTimeValue::Int(bits)
                    }
                    Some(DecodedCanonicalConstValue::Boolean(value)) => BuildTimeValue::Bool(value),
                    _ => return Err("constant argument is not a canonical scalar snapshot".into()),
                },
            });
            for warning in argument_warnings {
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
        }
        if needs_concrete_discharge {
            let callee = match target {
                SelectedCall::Entry { machine, .. } => machine.name.to_string(),
                SelectedCall::Operator(operator) => {
                    typed.symbols.display_path(operator.symbol, "::")
                }
            };
            self.require_concrete_failure_discharge(
                expression,
                callee,
                target.return_type(),
                snapshots,
            )?;
        }
        Ok(AdmittedCall {
            target,
            arguments,
            warnings,
        })
    }

    /// Admit one authored call inside a structured leaf without interpreting
    /// it: the enclosing probe's checked interpretation supplies the value
    /// itself, while this call's selection, argument snapshots, and concrete
    /// premise/failure discharge remain required evidence.
    pub(super) fn check_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<Vec<Diagnostic>, String> {
        Ok(self.admit_call(expression)?.warnings)
    }

    /// Every expression root in this probe's single state: the return value
    /// plus compiler-hoisted `let` initializers and named-transition arguments
    /// produced by ordinary terminal-call normalization for aggregate results.
    fn probe_roots(&self) -> Result<Vec<ExpressionHandle>, String> {
        let typed = self.program.typed();
        let mut roots = Vec::new();
        for statement in typed.statement_table.statements(self.state.statement_nodes) {
            match statement {
                typed_trees::statement::StatementNode::LocalData(local) => {
                    if local.initial_value.is_valid() {
                        roots.push(local.initial_value);
                    }
                }
                typed_trees::statement::StatementNode::Transition(transition) => {
                    for target in transition.targets() {
                        match typed.statement_table.transition_target(target) {
                            typed_trees::statement::TransitionTargetNode::Value(expression) => {
                                roots.push(*expression);
                            }
                            typed_trees::statement::TransitionTargetNode::Named {
                                arguments,
                                ..
                            } => {
                                let handles = typed.statement_table.expression_handles(*arguments);
                                if handles.len() != arguments.len() {
                                    return Err(
                                        "typed probe transition has a stale argument span".into()
                                    );
                                }
                                roots.extend(handles.iter().copied());
                            }
                            _ => {}
                        }
                    }
                    if let typed_trees::statement::TransitionGuardNode::When(guard) =
                        transition.guard
                    {
                        roots.push(guard);
                    }
                }
                typed_trees::statement::StatementNode::Expression(expression) => {
                    roots.push(*expression);
                }
                typed_trees::statement::StatementNode::Assignment(assignment) => {
                    roots.extend([assignment.target, assignment.value]);
                }
                typed_trees::statement::StatementNode::AssemblyFact(fact) => {
                    roots.extend(typed.tables.proof_fact_expression_roots(fact.fact));
                }
                _ => {}
            }
        }
        if roots.is_empty() {
            return Err("typed probe lost its expression return".into());
        }
        Ok(roots)
    }

    /// The leaf span this probe answers for: its transition statement retains
    /// the authored coordinate through normalization.
    fn probe_reference(&self) -> Result<source::SourceSpan, String> {
        let typed = self.program.typed();
        for statement in typed.statement_table.statements(self.state.statement_nodes) {
            if let Some(transition) = statement.as_transition() {
                return Ok(transition.source_span);
            }
        }
        Err("typed probe lost its expression return".into())
    }

    /// Establish the selected structured result independently of its destination.
    /// These private probes return a value, including a hoisted call temporary;
    /// unrelated local initializers and call arguments are not return values.
    fn require_result_obligations(&self) -> Result<(), String> {
        let typed = self.program.typed();
        let mut returns_value = false;
        for (statement_index, statement) in typed
            .statement_table
            .statements(self.state.statement_nodes)
            .iter()
            .enumerate()
        {
            let Some(transition) = statement.as_transition() else {
                continue;
            };
            for target in [transition.target, transition.continuation] {
                if matches!(
                    typed.statement_table.transition_target(target),
                    typed_trees::statement::TransitionTargetNode::Named { .. }
                ) {
                    return Err("typed initializer probe changed its value return".into());
                }
                let typed_trees::statement::TransitionTargetNode::Value(expression) =
                    typed.statement_table.transition_target(target)
                else {
                    continue;
                };
                returns_value = true;
                self.require_structured_expression_result(
                    *expression,
                    self.state.return_type,
                    statement_index,
                )?;
            }
        }
        if !returns_value {
            return Err("typed initializer probe lost its value return".into());
        }
        Ok(())
    }

    /// Every selected arm and constructed member owes its own carrier. Two arms
    /// can forget different qualifications without having a common existing type
    /// reference; checking each actual avoids guessing a result from the destination.
    /// Primitive components retain the existing concrete scalar admission, including
    /// anonymous landing and proven stores. Integer range predicates owe the
    /// shared checker proof separately; this walk supplies no domain authority.
    fn require_structured_expression_result(
        &self,
        expression: ExpressionHandle,
        destination: typed_trees::types::TypeReferenceHandle,
        statement_index: usize,
    ) -> Result<(), String> {
        let typed = self.program.typed();
        // Each arm owes its result obligations even when selective interpretation
        // will skip it. The destination cannot certify an invalid sibling.
        if let ExpressionNode::Match(dispatch) = typed.expression_table.expression(expression) {
            let arms = typed.expression_table.match_arms(dispatch.arms);
            if arms.is_empty() {
                return Err("constant initializer lost its selected result carrier".into());
            }
            for arm in arms {
                self.require_structured_expression_result(arm.value, destination, statement_index)?;
            }
            return Ok(());
        }
        if let Some(primitive) = typed.primitive_type_reference(destination) {
            if primitive.is_integer() {
                // Carrier correspondence supplies no refinement proof. Reuse the
                // ordinary checker at the exact returning occurrence, before any
                // payload is materialized into a qualified destination.
                let ranges = validation::constrained_range_bounds(typed, destination, false)
                    .ok_or("constant initializer result has an unresolved integer range")?;
                for (minimum, maximum) in ranges {
                    if !validation::occurrence_integer_expression_fits_range(
                        typed,
                        self.machine.symbol,
                        self.state.symbol,
                        statement_index,
                        expression,
                        &minimum,
                        &maximum,
                    ) {
                        return Err(
                            "constant initializer result does not prove its declared integer range"
                                .into(),
                        );
                    }
                }
            }
            return Ok(());
        }
        let carrier = typed
            .type_reference_table
            .unconstrained_type_reference(destination);
        match typed.expression_table.expression(expression) {
            ExpressionNode::ArrayLiteral(elements) => {
                // Substitution can retain a declared array's carrier on literal
                // syntax. Keep that receipt before checking constructed children.
                if let Some(actual) = validation::expression_result_type_reference(
                    typed,
                    self.machine,
                    self.state,
                    expression,
                ) && !result_carriers_match(typed, actual, destination)
                {
                    return Err(
                        "constant initializer result does not match its declared carrier".into(),
                    );
                }
                let typed_trees::types::TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = typed.type_reference_table.type_reference(carrier)
                else {
                    return Err(
                        "constant initializer result does not match its declared carrier".into(),
                    );
                };
                let values = typed.expression_table.expression_handles(*elements);
                if values.len() != elements.len() || values.len() != *length {
                    return Err(
                        "constant initializer result does not match its declared carrier".into(),
                    );
                }
                for value in values {
                    self.require_structured_expression_result(
                        *value,
                        *element_type,
                        statement_index,
                    )?;
                }
                return Ok(());
            }
            ExpressionNode::StructLiteral(literal) => {
                if typed
                    .closed_data_constructor_definition(carrier, literal.type_symbol)
                    .is_none()
                {
                    return Err(
                        "constant initializer result does not match its declared carrier".into(),
                    );
                }
                for field in typed.expression_table.struct_fields(literal.fields) {
                    let field_type = validation::selected_construction_field_type(
                        typed,
                        literal,
                        field.field_symbol,
                    )
                    .or_else(|| {
                        typed
                            .closed_data_member_field(
                                carrier,
                                field.field_symbol,
                                field.name.as_str(),
                            )
                            .map(|field| field.type_reference)
                    })
                    .ok_or("constant initializer lost its selected construction field")?;
                    self.require_structured_expression_result(
                        field.value,
                        field_type,
                        statement_index,
                    )?;
                }
                return Ok(());
            }
            _ => {}
        }
        let actual = validation::expression_result_type_reference(
            typed,
            self.machine,
            self.state,
            expression,
        )
        .ok_or("constant initializer lost its selected result carrier")?;
        if !result_carriers_match(typed, actual, destination) {
            return Err("constant initializer result does not match its declared carrier".into());
        }
        Ok(())
    }

    /// Interpret this probe machine under the common invocation floor. When
    /// the probe's call closure carries an authored `requires` premise, the
    /// per-call concrete discharge evidence produced while admitting each
    /// authored call is what allows the premise-discharge entry point here.
    fn interpret(&self, reference: source::SourceSpan) -> Result<BuildTimeValue, String> {
        self.require_result_obligations()?;
        let typed = self.program.typed();
        let custody = BuildTimeInvocationCustody::Source(reference);
        if self
            .program
            .admission
            .closure_includes_authored_requires(typed, self.machine)
        {
            self.program
                .admission
                .evaluate_const_evaluable_machine_symbol_for_concrete_premise_invocation(
                    typed,
                    self.machine.symbol,
                    Vec::new(),
                    custody,
                )
        } else {
            self.program
                .admission
                .evaluate_const_evaluable_machine_symbol_for_invocation(
                    typed,
                    self.machine.symbol,
                    Vec::new(),
                    custody,
                )
        }
    }

    /// Evaluate one aggregate-producing initializer leaf by interpreting its
    /// private probe machine. Every authored call inside the leaf is admitted
    /// individually at its concrete arguments first; the structured result then
    /// crosses the ordinary ConstEvaluable admission on the declared carrier.
    pub(super) fn evaluate_leaf(
        &self,
        reference: source::SourceSpan,
        expected_origins: &[syntax_trees::types::ConstArgumentOrigin],
        expected_calls: &[(source::SourceSpan, source::SourceSpan)],
        syntax: &syntax_trees::SyntaxTrees,
    ) -> Result<LeafEvaluation, String> {
        let typed = self.program.typed();
        let roots = self.probe_roots()?;
        crate::machine_execution::admission::require_const_expression_selection(
            typed,
            self.machine,
            reference,
            self.program.authority.as_deref(),
        )?;
        let (origins, operators, calls) =
            crate::const_evaluation::const_generic_expressions::leaf_call_custody(
                typed,
                self.machine,
                self.state,
                &roots,
                false,
                syntax,
            )?;
        if calls.len() != expected_calls.len()
            || calls.iter().any(|call| !expected_calls.contains(call))
        {
            return Err("standalone probe changed the original call declaration selection".into());
        }
        if origins.len() != expected_origins.len()
            || origins
                .iter()
                .any(|origin| !expected_origins.contains(origin))
        {
            return Err(
                "standalone probe changed the original constant selection or selected a pending value"
                    .into(),
            );
        }
        let mut warnings = Vec::new();
        for call in
            crate::const_evaluation::const_generic_expressions::call_expressions(typed, &roots)?
        {
            for warning in self.check_call(call)? {
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
        }
        let value = self.interpret(reference)?;
        Ok(LeafEvaluation {
            value,
            destination: self.state.return_type,
            origins,
            operators,
            warnings,
        })
    }

    /// Evaluate a fresh scalar application probe — a const-generic argument
    /// whose expression composes ordinary closed machine calls such as
    /// `sized(4) + 2`. The exact transitive call closure collected beside
    /// evaluation is returned so the caller can rejoin its original lexical
    /// selections, including skipped branches, before replacing the index. Each
    /// authored call still undergoes the same selection, argument snapshot,
    /// and concrete premise/failure discharge as initializer invocations.
    pub(crate) fn evaluate_application_probe(
        &self,
        reference: source::SourceSpan,
        public: bool,
        syntax: &syntax_trees::SyntaxTrees,
        extent: bool,
    ) -> Result<ApplicationProbeEvaluation, String> {
        let typed = self.program.typed();
        let roots = self.probe_roots()?;
        let &[expression] = roots.as_slice() else {
            return Err("application probe lost its single scalar root".into());
        };
        crate::machine_execution::admission::require_const_expression_selection(
            typed,
            self.machine,
            reference,
            self.program.authority.as_deref(),
        )?;
        let destination = crate::const_evaluation::const_generic_expressions::exact_probe_destination(
            typed,
            if extent {
                validation::expression_result_type_reference(typed, self.machine, self.state, expression)
                    .unwrap_or(self.state.return_type)
            } else { self.state.return_type },
        )
        .ok_or("application probe destination requires an unconstrained exact builtin integer or Boolean carrier")?;
        let (origins, operators, calls) =
            crate::const_evaluation::const_generic_expressions::leaf_call_custody(
                typed,
                self.machine,
                self.state,
                &roots,
                public,
                syntax,
            )?;
        let (value, warnings) = self.evaluate(expression, destination)?;
        Ok(ApplicationProbeEvaluation {
            value,
            origins,
            operators,
            calls,
            warnings,
        })
    }

    /// Interpret this retained probe's own transition and return the
    /// structured result with its declared carrier. Receiving-side replay uses
    /// the same checked admission as the original evaluation.
    pub(super) fn evaluate_value(
        &self,
    ) -> Result<(BuildTimeValue, typed_trees::types::TypeReferenceHandle), String> {
        let typed = self.program.typed();
        let roots = self.probe_roots()?;
        let reference = self.probe_reference()?;
        crate::machine_execution::admission::require_const_expression_selection(
            typed,
            self.machine,
            reference,
            self.program.authority.as_deref(),
        )?;
        for call in
            crate::const_evaluation::const_generic_expressions::call_expressions(typed, &roots)?
        {
            self.check_call(call)?;
        }
        Ok((self.interpret(reference)?, self.state.return_type))
    }
}

/// Compare already selected closed carriers before payload materialization.
/// Ordinary semantic identity retains applications and arithmetic policies;
/// exact closed declarations additionally prevent equal visible paths from
/// merging distinct owners in this compilation. Array elements keep that same
/// owner check recursively. Qualification establishment remains independent.
fn result_carriers_match(
    typed: &TypedTrees,
    actual: typed_trees::types::TypeReferenceHandle,
    destination: typed_trees::types::TypeReferenceHandle,
) -> bool {
    if let (Some(actual_primitive), Some(destination_primitive)) = (
        typed.primitive_type_reference(actual),
        typed.primitive_type_reference(destination),
    ) {
        return actual_primitive == destination_primitive
            && typed.arithmetic_domain_for_type_reference(actual)
                == typed.arithmetic_domain_for_type_reference(destination);
    }
    let actual = typed
        .type_reference_table
        .unconstrained_type_reference(actual);
    let destination = typed
        .type_reference_table
        .unconstrained_type_reference(destination);
    if let (
        typed_trees::types::TypeReferenceNode::FixedArray {
            element_type: actual_element,
            length: actual_length,
        },
        typed_trees::types::TypeReferenceNode::FixedArray {
            element_type: destination_element,
            length: destination_length,
        },
    ) = (
        typed.type_reference_table.type_reference(actual),
        typed.type_reference_table.type_reference(destination),
    ) {
        return actual_length == destination_length
            && result_carriers_match(typed, *actual_element, *destination_element);
    }
    if typed.normalized_type_identity(actual) != typed.normalized_type_identity(destination) {
        return false;
    }
    match (
        typed.closed_data_definition(actual),
        typed.closed_data_definition(destination),
    ) {
        (Some(actual), Some(destination)) => actual.symbol == destination.symbol,
        (None, None) => true,
        _ => false,
    }
}

/// The evaluated result of one scalar application probe: the canonical value,
/// the selection custody gathered beside it, the exact transitive call
/// closure, and any evaluation warnings.
pub(crate) struct ApplicationProbeEvaluation {
    pub(crate) value: CanonicalConstValue,
    pub(crate) origins: Vec<syntax_trees::types::ConstArgumentOrigin>,
    pub(crate) operators: Vec<source::SourceSpan>,
    pub(crate) calls: Vec<(source::SourceSpan, source::SourceSpan)>,
    pub(crate) warnings: Vec<Diagnostic>,
}

/// The evaluated result of one aggregate-producing initializer leaf: the
/// checked interpreter's structured value, its declared carrier, and the same
/// custody receipts the scalar leaf path collects.
pub(super) struct LeafEvaluation {
    pub(super) value: BuildTimeValue,
    pub(super) destination: typed_trees::types::TypeReferenceHandle,
    pub(super) origins: Vec<syntax_trees::types::ConstArgumentOrigin>,
    pub(super) operators: Vec<source::SourceSpan>,
    pub(super) warnings: Vec<Diagnostic>,
}

fn scalar_snapshot(value: &ScalarValue, carrier: PrimitiveType) -> Result<ExpressionNode, String> {
    use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix};
    let value = match value {
        ScalarValue::UndeterminedNaN(_) => {
            return Err("concrete contract snapshot requires determined floating representation; NaN has no payload bits".into());
        }
        ScalarValue::Index(value) => value,
        ScalarValue::Float { format, bits } => {
            let value = match (format, carrier) {
                (numerics::literals::FloatFormat::F32, PrimitiveType::F32) => f64::from(
                    f32::from_bits(u32::try_from(*bits).map_err(|_| "invalid binary32 snapshot")?),
                ),
                (numerics::literals::FloatFormat::F64, PrimitiveType::F64) => f64::from_bits(*bits),
                _ => return Err("constant argument lost its exact floating carrier".into()),
            };
            return Ok(ExpressionNode::Float(
                numerics::literals::FloatLiteral::from_f64(value).with_landing(*format),
            ));
        }
    };
    let value = match value.decode_encoding() {
        Some(DecodedCanonicalConstValue::Boolean(value)) if carrier == PrimitiveType::Bool => {
            return Ok(ExpressionNode::Boolean(value));
        }
        Some(DecodedCanonicalConstValue::Integer { value, .. }) => value,
        _ => return Err("constant argument lost its canonical scalar snapshot".into()),
    };
    let landed_type = carrier
        .landed_landing_fixed()
        .ok_or("constant argument lost its builtin integer carrier")?;
    let magnitude = value.unsigned_abs().to_string();
    Ok(ExpressionNode::Integer(
        IntegerLiteral::from_parts(value < 0, IntegerRadix::Decimal, &magnitude)?.with_landing(
            IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    ))
}

/// Disposable expression roots share the same ordinary checking context for
/// concrete admission and receiving replay; they never become source declarations.
pub(super) fn append_probe(
    program: &mut TypedTrees,
    owner: symbols::SymbolHandle,
    name: String,
    expression: ExpressionHandle,
    destination: typed_trees::types::TypeReferenceHandle,
) -> symbols::SymbolHandle {
    let symbol =
        program
            .symbols
            .insert_generated_root_from(owner, symbols::SymbolKind::Machine, &name);
    let children = program
        .symbols
        .insert_generated_children(symbol, [(symbols::SymbolKind::State, name.as_str())]);
    let target = program.statement_table.insert_transition_target(
        typed_trees::statement::TransitionTargetNode::Value(expression),
    );
    let mut state = State {
        symbol: children.start(),
        name: typed_trees::name::Identifier::generated(name.clone()),
        return_type: destination,
        ..Default::default()
    };
    let source_span = program.expression_table.source_span(expression);
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        typed_trees::statement::StatementNode::Transition(
            typed_trees::statement::TableTransition {
                target,
                source_span,
                ..Default::default()
            },
        ),
    );
    let mut machine = Machine {
        symbol,
        name: typed_trees::name::Identifier::generated(name),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    symbol
}
