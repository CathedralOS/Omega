//! Ordinary scalar calls share checked admission and the hermetic interpreter.
//!
//! The scalar evaluator owns expression order and anonymous landing. This owner
//! only crosses the call boundary: validate every authored argument before any
//! selective execution, preserve its declared carrier, then create fresh value
//! snapshots for the exact selected entry. A private checked probe supplies the
//! body/type and crash evidence missing from the pre-resolution syntax forest.
//! Neither a temporary constant placeholder nor an empty service row establishes
//! that evidence. The caller must first close the declaration dependency graph.

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
        typed: &TypedTrees,
        authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
        probe_symbols: &[symbols::SymbolHandle],
    ) -> Result<Self, Vec<Diagnostic>> {
        let prepared = crate::PreparedBuildMachineProgram::prepare(typed)?;
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
        let checked = typed_trees_to_checked_trees::lower_typed_trees(bodies)?;
        let crash_causes = typed_trees_to_checked_trees::infer_checked_crash_causes(
            &checked.typed,
            &checked.facts,
        );
        let mut typed = checked.typed;
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

    pub(super) fn calls_for_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<Invocation<'_>, String> {
        let typed = self.typed();
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == symbol)
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

impl Invocation<'_> {
    fn require_concrete_failure_discharge(
        &self,
        expression: ExpressionHandle,
        machine: &Machine,
        entry: &State,
        snapshots: Vec<ExpressionNode>,
    ) -> Result<(), String> {
        // A published guarded ceiling has no private whole-body summary, and an
        // authored `requires` premise has no meaning outside an invocation.
        // Check the exact call instead of inspecting the body to narrow either
        // contract, or treating successful interpretation as admission
        // evidence: the probe's ordinary checking discharges the premise and
        // the guarded routes at the snapshot arguments.
        // All original initializer probes must leave this private body check:
        // their detached expressions use exact evaluation, not runtime ranges.
        let mut probe = self.program.typed().clone();
        let machines = probe
            .machines()
            .iter()
            .filter(|candidate| !self.program.probe_symbols.contains(&candidate.symbol))
            .cloned()
            .collect::<Vec<_>>();
        probe.roots.machines = arena::HandleSpan::empty();
        for candidate in machines {
            probe.push_machine(candidate);
        }
        let ExpressionNode::Call(mut call) = probe.expression_table.expression(expression).clone()
        else {
            return Err("constant invocation lost its selected call".into());
        };
        let arguments = snapshots
            .into_iter()
            .map(|snapshot| probe.expression_table.insert(snapshot))
            .collect::<Vec<_>>();
        call.arguments = probe.expression_table.insert_expression_handles(arguments);
        let concrete = probe.expression_table.insert(ExpressionNode::Call(call));
        probe.expression_table.set_source_span(
            concrete,
            self.program
                .typed()
                .expression_table
                .source_span(expression),
        );
        let owner = append_probe(
            &mut probe,
            self.machine.symbol,
            "@const-invocation".into(),
            concrete,
            entry.return_type,
        );
        let checked =
            typed_trees_to_checked_trees::lower_typed_trees(probe).map_err(|diagnostics| {
                format!(
                    "constant invocation of `{}` failed checking: {diagnostics:?}",
                    machine.name
                )
            })?;
        let causes = typed_trees_to_checked_trees::infer_checked_machine_crash_causes(
            &checked.typed,
            &checked.facts,
            owner,
        )
        .ok_or("constant invocation has no complete checked failure summary")?;
        if !causes.is_empty() {
            return Err(format!(
                "constant invocation of `{}` retains unhandled {causes:?} routes at its concrete arguments",
                machine.name
            ));
        }
        Ok(())
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

    /// Resolve the exact closed ordinary entry for a call and admit it against
    /// the common floor. The returned flag records that the closure carries an
    /// authored `requires` premise: such an invocation still owes the premise,
    /// discharged by the concrete checked probe before interpretation, so the
    /// caller must always run that probe rather than relying on the floor's
    /// conservative closure fence.
    fn selected(&self, expression: ExpressionHandle) -> Result<(bool, &Machine, &State), String> {
        let typed = self.program.typed();
        crate::machine_execution::admission::require_call_expression_selection(
            typed,
            expression,
            self.program.authority.as_deref(),
        )?;
        let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
            return Err("initializer lost its selected call".into());
        };
        if !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.static_machine_parameter.is_valid()
            || call.static_requirement_dispatch.is_some()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
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
        let machine = selected
            .next()
            .ok_or("constant call has no exact closed ordinary entry")?;
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
            return Ok((true, machine, state));
        }
        self.program
            .admission
            .require_common_floor_for_invocation(typed, machine, custody)?;
        Ok((false, machine, state))
    }
}

impl ConstantCalls for Invocation<'_> {
    fn validate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(PrimitiveType, Vec<Diagnostic>), String> {
        let typed = self.program.typed();
        let (_, _, entry) = self.selected(expression)?;
        let destination = scalar_probe_destination(typed, entry.return_type)
            .ok_or("constant call result needs an exact builtin scalar carrier")?;
        let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
            return Err("constant validation lost its selected call".into());
        };
        let arguments = typed.expression_table.expression_handles(call.arguments);
        let parameters = typed.state_parameters(entry);
        if arguments.len() != call.arguments.len() || arguments.len() != parameters.len() {
            return Err("constant call argument count differs from its exact entry".into());
        }
        let mut warnings = Vec::new();
        for (argument, parameter) in arguments.iter().zip(parameters) {
            let carrier = scalar_probe_destination(typed, parameter.type_reference)
                .ok_or("constant call argument needs an exact builtin scalar carrier")?;
            for warning in value::validate(
                typed,
                self.machine,
                self.state,
                *argument,
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
            premise_discharge,
            machine,
            entry,
            arguments,
            warnings,
        } = self.admit_call(expression)?;
        let custody =
            BuildTimeInvocationCustody::Source(typed.expression_table.source_span(expression));
        let result = if premise_discharge {
            // The probe above re-ran ordinary checked contract proof at the
            // snapshot arguments; its success is what admits the authored
            // `requires` premises here. Interpretation itself proves nothing.
            self.program
                .admission
                .evaluate_const_evaluable_machine_symbol_for_concrete_premise_invocation(
                    typed,
                    machine.symbol,
                    arguments,
                    custody,
                )?
        } else {
            self.program
                .admission
                .evaluate_const_evaluable_machine_symbol_for_invocation(
                    typed,
                    machine.symbol,
                    arguments,
                    custody,
                )?
        };
        let destination = scalar_probe_destination(typed, entry.return_type)
            .ok_or("constant result lost its exact carrier")?;
        let value = match result {
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

/// One admitted authored call: exact entry, evaluated snapshot arguments, and
/// the concrete discharge evidence the scalar evaluator turns into a result.
struct AdmittedCall<'program> {
    premise_discharge: bool,
    machine: &'program Machine,
    entry: &'program State,
    arguments: Vec<BuildTimeValue>,
    warnings: Vec<Diagnostic>,
}

impl Invocation<'_> {
    /// Admit one authored call without interpreting it: select its exact
    /// closed entry, evaluate every argument snapshot in this probe's context,
    /// and discharge its crash/requires contract at those concrete arguments.
    fn admit_call(&self, expression: ExpressionHandle) -> Result<AdmittedCall<'_>, String> {
        let typed = self.program.typed();
        let (premise_discharge, machine, entry) = self.selected(expression)?;
        // Reuse complete argument-independent evidence when it exists. A
        // fallible or otherwise unsummarized call needs fresh checking
        // snapshots, and an authored `requires` premise in the closure always
        // does: only the concrete probe can decide it at these arguments.
        let needs_concrete_discharge = premise_discharge
            || !self
                .program
                .crash_causes
                .iter()
                .any(|(symbol, causes)| *symbol == machine.symbol && causes.is_empty());
        let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
            return Err("constant execution lost its selected call".into());
        };
        let mut arguments = Vec::new();
        let mut snapshots = Vec::new();
        let mut warnings = Vec::new();
        for (argument, parameter) in typed
            .expression_table
            .expression_handles(call.arguments)
            .iter()
            .zip(typed.state_parameters(entry))
        {
            let destination = scalar_probe_destination(typed, parameter.type_reference)
                .ok_or("constant call argument lost its exact carrier")?;
            let (value, argument_warnings) = value::evaluate_scalar(
                typed,
                self.machine,
                self.state,
                *argument,
                destination,
                Some(self),
            )?;
            if needs_concrete_discharge {
                snapshots.push(scalar_snapshot(&value, destination)?);
            }
            arguments.push(match value {
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
            self.require_concrete_failure_discharge(expression, machine, entry, snapshots)?;
        }
        Ok(AdmittedCall {
            premise_discharge,
            machine,
            entry,
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
                    for target in [transition.target, transition.continuation] {
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
                    roots.push(fact.expression);
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
            if let typed_trees::statement::StatementNode::Transition(transition) = statement {
                return Ok(transition.source_span);
            }
        }
        Err("typed probe lost its expression return".into())
    }

    /// Interpret this probe machine under the common invocation floor. When
    /// the probe's call closure carries an authored `requires` premise, the
    /// per-call concrete discharge evidence produced while admitting each
    /// authored call is what allows the premise-discharge entry point here.
    fn interpret(&self, reference: source::SourceSpan) -> Result<BuildTimeValue, String> {
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
    /// whose whole expression is an ordinary closed machine call such as
    /// `sized(4)`. There is no earlier phase roster to rejoin; the exact
    /// transitive call closure collected beside the evaluation is returned so
    /// the caller can confirm every authored call survived into custody. Each
    /// authored call still undergoes the same selection, argument snapshot,
    /// and concrete premise/failure discharge as initializer invocations.
    pub(crate) fn evaluate_application_probe(
        &self,
        reference: source::SourceSpan,
        public: bool,
        syntax: &syntax_trees::SyntaxTrees,
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
            self.state.return_type,
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
    use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};
    let value = match value {
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
    let landed_type = match carrier {
        PrimitiveType::I8 => LandedIntegerType::I8,
        PrimitiveType::I16 => LandedIntegerType::I16,
        PrimitiveType::I32 => LandedIntegerType::I32,
        PrimitiveType::I64 => LandedIntegerType::I64,
        PrimitiveType::U8 => LandedIntegerType::U8,
        PrimitiveType::U16 => LandedIntegerType::U16,
        PrimitiveType::U32 => LandedIntegerType::U32,
        PrimitiveType::U64 => LandedIntegerType::U64,
        _ => return Err("constant argument lost its builtin integer carrier".into()),
    };
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
