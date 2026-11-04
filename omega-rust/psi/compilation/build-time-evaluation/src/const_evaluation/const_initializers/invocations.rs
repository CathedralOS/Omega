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

use crate::build_time_evaluation::{expression_table, type_reference_table};
use crate::unique::extend_unique;
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
    immediate_call_expressions, leaf_call_custody,
};
use crate::const_evaluation::const_generic_expressions::{
    scalar_probe_destination, value::ScalarValue,
};
use crate::const_evaluation::range_endpoints::deferred_endpoint_stand_ins;
use crate::machine_execution::admission::require_call_expression_selection;
use crate::machine_execution::selected_operators::prepare_selected_execution;
use crate::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue};
use numerics::literals::{FloatFormat, FloatLiteral};
use source::SourceSpan;
use symbols::SymbolHandle;
use typed_trees::constant_recipes::create_evaluation_entry;
use typed_trees::types::TypeReferenceHandle;
use validation::expression_result_type_reference;

pub(crate) struct CheckedInitializers {
    typed: TypedTrees,
    admission: BuildTimeAdmissionPlan,
    authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    probe_symbols: Vec<SymbolHandle>,
    operators: Vec<crate::SelectedBuildTimeBinaryOperator>,
}

impl CheckedInitializers {
    pub(crate) fn prepare(
        prepared: &crate::PreparedBuildMachineProgram,
        authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
        probe_symbols: &[SymbolHandle],
        domain: &crate::build_time_evaluation::EvaluationDomain,
        operators: &[crate::SelectedBuildTimeBinaryOperator],
    ) -> Result<Self, Vec<Diagnostic>> {
        let probes = prepared
            .typed()
            .machines()
            .iter()
            .filter(|machine| probe_symbols.contains(&machine.symbol))
            .count();
        if probes != probe_symbols.len() {
            return Err(super::failure(
                SourceSpan::default(),
                "initializer preparation lost an exact probe owner",
            ));
        }
        let references = typed_trees::references::collect_program_references(prepared.typed());
        let operators = operators
            .iter()
            .filter(|row| {
                references.expressions.contains(&row.expression)
                    && row
                        .origin
                        .machine_symbol()
                        .is_some_and(|owner| prepared.typed().machine(owner).is_some())
            })
            .cloned()
            .collect::<Vec<_>>();
        // Helper checking and result-carrier comparison consume concrete type
        // dependencies too. Keep the caller's layout, Product frontier and
        // execution selection while demanding these exact private probes.
        // Removing a probe must not leave a helper's declared array carrier
        // symbolic beside a literal result.
        let mut domain = domain.clone();
        for &symbol in probe_symbols {
            domain.include(symbol);
        }
        let domain = domain.demanded_roots(prepared.typed(), probe_symbols);
        let settled = if crate::const_evaluation::const_lengths::pending_array_lengths(
            prepared.typed(),
            &domain,
        )
        .is_empty()
        {
            None
        } else {
            let mut execution = prepared.typed().clone();
            crate::const_evaluation::const_lengths::evaluate_selected_array_lengths(
                &mut execution,
                authority.clone(),
                crate::SelectedBuildTimeOperators {
                    operators: &operators,
                    provider_bodies: &[],
                },
                &domain,
            )?;
            Some(execution)
        };
        let execution = settled.as_ref().unwrap_or_else(|| prepared.typed());
        // The private scalar evaluator owns probe expressions. The projection
        // removes their unused generated context as well as their roots, while
        // preserving every helper or descriptive operand that actually uses it.
        let projection = super::probe_projection::ProbeProjection::new(execution, probe_symbols);
        let bodies = projection.helper_bodies();
        let preparation_request = typed_trees_to_typed_trees::PreparationRequest::settled();
        let checking = typed_trees_to_checked_trees::CheckingRequest::new();
        let (bodies, preparation) =
            typed_trees_to_typed_trees::prepare_typed_trees(bodies, &preparation_request)?;
        let checked =
            typed_trees_to_checked_trees::lower_typed_trees(bodies, preparation, &checking)?;
        let typed = projection.rejoin_unjudged(checked.typed.into_inner());
        // Checking may normalize boundary tokens into requirement calls.
        // Admission and interpretation consume the same validated selected
        // meaning on a private graph, never mutate the completed judgment.
        let typed = prepare_selected_execution(&typed, &[], &operators)
            .map_err(|reason| super::failure(SourceSpan::default(), reason))?
            .unwrap_or(typed);
        let admission = BuildTimeAdmissionPlan::infer(&typed, authority.clone())
            .with_accounting(domain.accounting.clone())
            .with_selected_operators(&typed, &operators)
            .map_err(|reason| super::failure(SourceSpan::default(), reason))?;
        Ok(Self {
            typed,
            admission,
            authority,
            probe_symbols: probe_symbols.to_vec(),
            operators,
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
            lexical_owner: machine.symbol,
        }
    }

    pub(crate) fn calls_for_source(&self, reference: SourceSpan) -> Result<Invocation<'_>, String> {
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

    pub(crate) fn calls_for_symbol(&self, symbol: SymbolHandle) -> Result<Invocation<'_>, String> {
        let typed = self.typed();
        let machine = typed
            .machine(symbol)
            .ok_or("retained initializer probe lost its exact symbol")?;
        let [state] = typed.machine_states(machine) else {
            return Err("retained initializer probe lost its single state".into());
        };
        Ok(self.calls(machine, state))
    }

    /// Retained initializer jobs already pair a source declaration with its
    /// exact probe. Keep that source owner for call joins while the probe's
    /// machine and state still govern evaluation and admission.
    pub(super) fn calls_for_retained_initializer(
        &self,
        probe: SymbolHandle,
        declaration: SymbolHandle,
    ) -> Result<Invocation<'_>, String> {
        let mut calls = self.calls_for_symbol(probe)?;
        calls.lexical_owner = declaration;
        Ok(calls)
    }
}

pub(crate) struct Invocation<'program> {
    program: &'program CheckedInitializers,
    machine: &'program Machine,
    state: &'program State,
    lexical_owner: SymbolHandle,
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
        program: &TypedTrees,
        expression: ExpressionHandle,
        callee: String,
        return_type: TypeReferenceHandle,
        snapshots: Vec<ArgumentSnapshot>,
    ) -> Result<(), String> {
        let retained: Vec<SymbolHandle> = self
            .program
            .typed()
            .machines()
            .iter()
            .filter(|machine| !self.program.probe_symbols.contains(&machine.symbol))
            .map(|machine| machine.symbol)
            .collect();
        crate::machine_execution::admission::snapshot_arguments_discharge(
            &self.program.admission,
            program,
            &retained,
            self.machine.symbol,
            "@const-invocation",
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
            &typed_trees_to_checked_trees::CheckingRequest::new(),
            expression,
            &callee,
            return_type,
            snapshots,
            &deferred_endpoint_stand_ins(self.program.typed()),
        )
    }

    pub(super) fn validate_custody(
        &self,
        declaration: &typed_trees::constant::ConstDeclaration,
    ) -> Result<Vec<crate::const_evaluation::const_generic_expressions::DependencyValue>, String>
    {
        crate::const_evaluation::const_generic_expressions::validate_retained_initializer_call_custody(
            self.program.typed(),
            self.machine,
            self.state,
            declaration,
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

    /// Preparation can copy a recipe while preserving its untouched source
    /// root. Execute the retained probe's actual value exit, not that recipe.
    pub(super) fn evaluate_probe_scalar(
        &self,
        destination: PrimitiveType,
    ) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
        let typed = self.program.typed();
        let body = super::super::initializer_probe::value_probe_body(typed, self.state)?;
        crate::machine_execution::admission::require_const_expression_selection(
            typed,
            self.machine,
            body.reference,
            self.program.authority.as_deref(),
        )?;
        if body.roots.len() == 1 {
            return self.evaluate_scalar(body.value, destination);
        }
        if scalar_probe_destination(typed, self.state.return_type) != Some(destination) {
            return Err("constant probe lost its exact scalar carrier".into());
        }
        let mut warnings = Vec::new();
        for call in immediate_call_expressions(typed, &body.roots)? {
            extend_unique(&mut warnings, self.check_call(call)?);
        }
        let value = scalar_result(self.interpret(body.reference)?, destination)?;
        Ok((value, warnings))
    }

    /// A named implementation keeps its public contract after its body is
    /// specialized. Keep that contract, its checked arguments and the exact
    /// executable entry together; the selected application must still match
    /// this caller and body.
    fn selected(&self, expression: ExpressionHandle) -> Result<PreparedCall<'_>, String> {
        let typed = self.program.typed();
        require_call_expression_selection(typed, expression, self.program.authority.as_deref())?;
        let Some(call) = typed.expression_table.call_node(expression) else {
            return Err("initializer lost its selected call".into());
        };
        if call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
        {
            return Err(
                "constant invocation has no ordinary machine or named requirement route".into(),
            );
        }
        let site = validation::CallSite::Expression { expression, call };
        let public_signature = call
            .static_requirement_dispatch
            .as_ref()
            .map(|_| {
                validation::resolve_requirement_contract(typed, self.lexical_owner, &site)
                    .map(|(_, signature)| signature)
                    .map_err(|reason| {
                        format!("constant call has an invalid prepared public contract: {reason}")
                    })
            })
            .transpose()?;
        let mut selected = typed.machines().iter().filter(|machine| {
            machine.type_parameters.is_empty()
                && typed
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| typed.call_has_no_runtime_receiver(call, machine, entry))
        });
        let target = if let Some(machine) = selected.next() {
            if selected.next().is_some() {
                return Err("constant call has conflicting exact entries".into());
            }
            let entry = typed
                .machine_states(machine)
                .first()
                .ok_or("constant call lost its entry")?;
            let custody =
                BuildTimeInvocationCustody::Source(typed.expression_table.source_span(expression));
            let premise_discharge = self
                .program
                .admission
                .closure_includes_authored_requires(typed, machine);
            if premise_discharge {
                self.program
                    .admission
                    .require_common_floor_for_concrete_premise_invocation(
                        typed, machine, custody,
                    )?;
            } else {
                self.program
                    .admission
                    .require_common_floor_for_invocation(typed, machine, custody)?;
            }
            SelectedCall::Entry {
                premise_discharge,
                machine,
                entry,
            }
        } else {
            // No closed ordinary entry: the call may name an exact boundary
            // operator requirement (`F32::square_root`) kept on the call as
            // its own declaration identity. Resolution reuses the same
            // path-and-arity selection checking applies, so a name that is a
            // family rather than an exact overload still declines here.
            let operator = typed_trees::operator::resolve_named_expression_call(typed, call)
                .ok_or("constant call has no exact closed ordinary entry or named requirement")?;
            SelectedCall::Operator(operator)
        };
        Ok(PreparedCall {
            target,
            public_signature,
            operands: self.call_operands(expression, target)?,
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
#[derive(Clone, Copy)]
enum SelectedCall<'program> {
    Entry {
        premise_discharge: bool,
        machine: &'program Machine,
        entry: &'program State,
    },
    Operator(&'program typed_trees::operator::OperatorDefinition),
}

impl SelectedCall<'_> {
    fn return_type(&self) -> TypeReferenceHandle {
        match self {
            Self::Entry { entry, .. } => entry.return_type,
            Self::Operator(operator) => operator.return_type,
        }
    }
}

/// Keep the public method's result and argument types with the exact call.
/// The private entry supplies executable code; it cannot change those types.
struct PreparedCall<'program> {
    target: SelectedCall<'program>,
    public_signature: Option<&'program typed_trees::signature::StateSignature>,
    operands: Vec<(ExpressionHandle, TypeReferenceHandle)>,
}

impl PreparedCall<'_> {
    fn return_type(&self) -> TypeReferenceHandle {
        self.public_signature.map_or_else(
            || self.target.return_type(),
            |signature| signature.return_type,
        )
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
        let warnings = self.validate_call_arguments(&selected)?;
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
        let (result, reference, warnings) = self.evaluate_admitted_call(expression)?;
        let destination = scalar_probe_destination(self.program.typed(), reference)
            .ok_or("constant result needs an exact builtin scalar type")?;
        Ok((scalar_result(result, destination)?, warnings))
    }
}

/// Convert an admitted result without losing its exact scalar representation.
fn scalar_result(
    result: BuildTimeValue,
    destination: PrimitiveType,
) -> Result<ScalarValue, String> {
    let value = match result {
        BuildTimeValue::UndeterminedNaN(format)
            if matches!(
                (format, destination),
                (FloatFormat::F32, PrimitiveType::F32) | (FloatFormat::F64, PrimitiveType::F64)
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
            // Semantic arithmetic publishes UndeterminedNaN above. This
            // branch carries presented bits, including exact raw NaNs.
            let format = if destination == PrimitiveType::F32 {
                FloatFormat::F32
            } else {
                FloatFormat::F64
            };
            let bits = format
                .capture_wide_representation(value)
                .ok_or("constant result does not retain its exact floating carrier")?;
            ScalarValue::Float { format, bits }
        }
        _ => return Err("constant result does not match its declared scalar carrier".into()),
    };
    Ok(value)
}

/// One admitted authored call: exact selected target, evaluated snapshot
/// arguments, and concrete discharge evidence for the shared call executor.
struct AdmittedCall<'program> {
    target: PreparedCall<'program>,
    arguments: Vec<BuildTimeValue>,
    warnings: Vec<Diagnostic>,
}

impl Invocation<'_> {
    /// Reuse evaluated arguments when entering a machine. Named operator
    /// requirements keep their existing checked expression probe. Result
    /// conversion follows this shared execution step.
    fn evaluate_admitted_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(BuildTimeValue, TypeReferenceHandle, Vec<Diagnostic>), String> {
        let typed = self.program.typed();
        let AdmittedCall {
            target,
            arguments,
            warnings,
        } = self.admit_call(expression)?;
        let custody =
            BuildTimeInvocationCustody::Source(typed.expression_table.source_span(expression));
        let result = match target.target {
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
                    crate::BuildTimeAdmissionPlan::infer(&probe, self.program.authority.clone())
                        .with_accounting(self.program.admission.accounting.clone())
                        .with_selected_operators(&probe, &self.program.operators)?;
                admission.evaluate_const_evaluable_machine_symbol_for_invocation(
                    &probe,
                    owner,
                    Vec::new(),
                    custody,
                )?
            }
        };
        Ok((result, target.return_type(), warnings))
    }

    /// Admit one authored call without interpreting it: select its exact
    /// closed entry or named requirement, evaluate every argument snapshot in
    /// this probe's context, and discharge its crash/requires contract at
    /// those concrete arguments.
    fn admit_call(&self, expression: ExpressionHandle) -> Result<AdmittedCall<'_>, String> {
        let typed = self.program.typed();
        let target = self.selected(expression)?;
        // Every source call owes its complete application and public premises,
        // even when its selected body has no private precondition or failure.
        let operands = &target.operands;
        let mut arguments = vec![None; operands.len()];
        let mut snapshots = (0..operands.len()).map(|_| None).collect::<Vec<_>>();
        let mut warnings = self.validate_call_arguments(&target)?;
        let mut snapshot_program = None;
        let call = typed
            .expression_table
            .call_node(expression)
            .ok_or("constant execution lost its selected call")?;
        let order = (validation::CallSite::Expression { expression, call })
            .argument_evaluation_order(typed)
            .map_err(str::to_owned)?;
        for position in order {
            let (argument, type_reference) = *operands
                .get(position)
                .ok_or("constant argument schedule names a missing operand")?;
            let Some(destination) = scalar_probe_destination(typed, type_reference) else {
                let (value, argument_warnings) =
                    self.evaluate_owned_argument(argument, type_reference)?;
                let canonical = super::materialize::canonical_value(typed, type_reference, &value)?;
                let probe = snapshot_program.get_or_insert_with(|| typed.clone());
                let literal = probe
                    .materialize_canonical_const_value(type_reference, &canonical)
                    .ok_or(
                        "cannot represent the evaluated constant argument as its declared type",
                    )?;
                snapshots[position] = Some(ArgumentSnapshot {
                    original: argument,
                    literal,
                });
                arguments[position] = Some(value);
                extend_unique(&mut warnings, argument_warnings);
                continue;
            };
            let (value, argument_warnings) = value::evaluate_scalar(
                typed,
                self.machine,
                self.state,
                argument,
                destination,
                Some(self),
            )?;
            snapshots[position] = Some(ArgumentSnapshot {
                original: argument,
                // This semantic node preserves NaN classification without
                // choosing payload bits or replaying its producing computation.
                literal: match &value {
                    ScalarValue::UndeterminedNaN(format) => {
                        ExpressionNode::UndeterminedNaN(*format)
                    }
                    _ => scalar_snapshot(&value, destination)?,
                },
            });
            arguments[position] = Some(match value {
                ScalarValue::UndeterminedNaN(format) => BuildTimeValue::UndeterminedNaN(format),
                ScalarValue::Float { format, bits } => BuildTimeValue::Float(
                    format
                        .widen_representation(bits)
                        .ok_or("invalid floating snapshot")?,
                ),
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
            extend_unique(&mut warnings, argument_warnings);
        }
        let callee = match target.target {
            SelectedCall::Entry { machine, .. } => machine.name.to_string(),
            SelectedCall::Operator(operator) => typed.symbols.display_path(operator.symbol, "::"),
        };
        self.require_concrete_failure_discharge(
            snapshot_program.as_ref().unwrap_or(typed),
            expression,
            callee,
            target.return_type(),
            snapshots.into_iter().collect::<Option<Vec<_>>>().ok_or(
                "constant call did not evaluate every argument before checking its preconditions",
            )?,
        )?;
        Ok(AdmittedCall {
            target,
            arguments: arguments
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or("constant call did not produce every argument value")?,
            warnings,
        })
    }

    /// Call selection fixes one complete argument roster, independent of the
    /// result carrier. Validation and evaluation use these same formal types.
    fn call_operands(
        &self,
        expression: ExpressionHandle,
        target: SelectedCall<'_>,
    ) -> Result<Vec<(ExpressionHandle, TypeReferenceHandle)>, String> {
        let typed = self.program.typed();
        let call = typed
            .expression_table
            .call_node(expression)
            .ok_or("constant execution lost its selected call")?;
        let operands = match target {
            SelectedCall::Entry { .. } if call.static_requirement_dispatch.is_some() => {
                let site = validation::CallSite::Expression { expression, call };
                validation::selected_call_arguments(typed, self.lexical_owner, &site)
                    .map_err(|reason| {
                        format!("constant call has invalid prepared arguments: {reason}")
                    })?
                    .into_iter()
                    .map(|argument| (argument.expression, argument.logical_type_reference))
                    .collect()
            }
            SelectedCall::Entry { entry, .. } => {
                let arguments = expression_table(typed).expression_handles(call.arguments);
                let parameters = typed.state_parameters(entry);
                if arguments.len() != call.arguments.len() || arguments.len() != parameters.len() {
                    return Err("constant call argument count differs from its exact entry".into());
                }
                arguments
                    .iter()
                    .copied()
                    .zip(parameters.iter().map(|parameter| parameter.type_reference))
                    .collect()
            }
            SelectedCall::Operator(operator) => {
                let mut operands = Vec::new();
                let mut arguments = expression_table(typed)
                    .expression_handles(call.arguments)
                    .iter()
                    .copied();
                for parameter in typed.operator_parameters(operator) {
                    let argument = arguments
                        .next()
                        .ok_or("constant call argument count differs from its exact requirement")?;
                    operands.push((argument, parameter.type_reference));
                }
                if arguments.next().is_some() {
                    return Err(
                        "constant call argument count differs from its exact requirement".into(),
                    );
                }
                operands
            }
        };
        Ok(operands)
    }

    fn validate_call_arguments(
        &self,
        target: &PreparedCall<'_>,
    ) -> Result<Vec<Diagnostic>, String> {
        let typed = self.program.typed();
        let mut warnings = Vec::new();
        for &(argument, carrier) in &target.operands {
            if let Some(primitive) = scalar_probe_destination(typed, carrier) {
                extend_unique(
                    &mut warnings,
                    value::validate(
                        typed,
                        self.machine,
                        self.state,
                        argument,
                        primitive,
                        Some(self),
                    )?,
                );
            } else {
                self.owned_argument_probe(argument, carrier)?;
                for call in immediate_call_expressions(typed, &[argument])? {
                    extend_unique(
                        &mut warnings,
                        self.validate_call_arguments(&self.selected(call)?)?,
                    );
                }
            }
        }
        Ok(warnings)
    }

    /// A record or array argument keeps its declared carrier while it becomes
    /// an owned snapshot. Temporary borrows may run inside its producer; the
    /// ordinary ConstEvaluable result check forbids them from escaping here.
    fn evaluate_owned_argument(
        &self,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
    ) -> Result<(BuildTimeValue, Vec<Diagnostic>), String> {
        if self
            .program
            .typed()
            .expression_table
            .call_node(expression)
            .is_some()
        {
            let (value, actual, warnings) = self.evaluate_admitted_call(expression)?;
            if !result_carriers_match(self.program.typed(), actual, destination) {
                return Err("constant argument result does not match its declared type".into());
            }
            return Ok((value, warnings));
        }
        let (probe, owner) = self.owned_argument_probe(expression, destination)?;
        let admission = BuildTimeAdmissionPlan::infer(&probe, self.program.authority.clone())
            .with_accounting(self.program.admission.accounting.clone())
            .with_selected_operators(&probe, &self.program.operators)?;
        let custody =
            BuildTimeInvocationCustody::Source(probe.expression_table.source_span(expression));
        // This wrapper has no entry parameters. Ordinary checked closure
        // admission must prove every nested call before the interpreter runs.
        let value = admission.evaluate_const_evaluable_machine_symbol_for_invocation(
            &probe,
            owner,
            Vec::new(),
            custody,
        )?;
        Ok((value, Vec::new()))
    }

    fn owned_argument_probe(
        &self,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
    ) -> Result<(TypedTrees, SymbolHandle), String> {
        let mut probe = self.program.typed().clone();
        let owner = append_probe(
            &mut probe,
            self.machine.symbol,
            "@const-argument".into(),
            expression,
            destination,
        );
        let machine = probe
            .machine(owner)
            .ok_or("constant argument probe has no owning machine")?;
        let [state] = probe.machine_states(machine) else {
            return Err("constant argument probe must have exactly one state".into());
        };
        Self::require_expression_carrier(&probe, machine, state, expression, destination, 0)?;
        Ok((probe, owner))
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

    /// Source custody and checked replay observe the same normalized captures
    /// and original value exit, including an ordinary hoisted direct call.
    fn probe_roots(&self) -> Result<Vec<ExpressionHandle>, String> {
        Ok(
            super::super::initializer_probe::value_probe_body(self.program.typed(), self.state)?
                .roots,
        )
    }

    fn probe_reference(&self) -> Result<SourceSpan, String> {
        Ok(
            super::super::initializer_probe::value_probe_body(self.program.typed(), self.state)?
                .reference,
        )
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
                Self::require_expression_carrier(
                    typed,
                    self.machine,
                    self.state,
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
    fn require_expression_carrier(
        typed: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
        statement_index: usize,
    ) -> Result<(), String> {
        // Each arm owes its result obligations even when selective interpretation
        // will skip it. The destination cannot certify an invalid sibling.
        if let ExpressionNode::Match(dispatch) = typed.expression_table.expression(expression) {
            let arms = typed.expression_table.match_arms(dispatch.arms);
            if arms.is_empty() {
                return Err("constant initializer lost its selected result carrier".into());
            }
            for arm in arms {
                Self::require_expression_carrier(
                    typed,
                    machine,
                    state,
                    arm.value,
                    destination,
                    statement_index,
                )?;
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
                        machine.symbol,
                        state.symbol,
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
        let carrier = type_reference_table(typed).unconstrained_type_reference(destination);
        match typed.expression_table.expression(expression) {
            ExpressionNode::ArrayLiteral(elements) => {
                // Substitution can retain a declared array's carrier on literal
                // syntax. Keep that receipt before checking constructed children.
                if let Some(actual) =
                    expression_result_type_reference(typed, machine, state, expression)
                    && !result_carriers_match(typed, actual, destination)
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
                    Self::require_expression_carrier(
                        typed,
                        machine,
                        state,
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
                        expression,
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
                    Self::require_expression_carrier(
                        typed,
                        machine,
                        state,
                        field.value,
                        field_type,
                        statement_index,
                    )?;
                }
                return Ok(());
            }
            _ => {}
        }
        let actual = expression_result_type_reference(typed, machine, state, expression)
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
    fn interpret(&self, reference: SourceSpan) -> Result<BuildTimeValue, String> {
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
        reference: SourceSpan,
        authored: &crate::const_evaluation::const_generic_expressions::AuthoredProbeCustody,
        syntax: &syntax_trees::SyntaxTrees,
    ) -> Result<LeafEvaluation, String> {
        authored.rejoin(reference, self.machine.symbol)?;
        let typed = self.program.typed();
        let roots = self.probe_roots()?;
        crate::machine_execution::admission::require_const_expression_selection(
            typed,
            self.machine,
            reference,
            self.program.authority.as_deref(),
        )?;
        let (origins, operators, calls) =
            leaf_call_custody(typed, self.machine, self.state, &roots, false, syntax)?;
        let mut warnings = Vec::new();
        for call in immediate_call_expressions(typed, &roots)? {
            extend_unique(&mut warnings, self.check_call(call)?);
        }
        let value = self.interpret(reference)?;
        Ok(LeafEvaluation {
            value,
            destination: self.state.return_type,
            origins,
            operators,
            calls,
            warnings,
        })
    }

    /// Evaluate a fresh application probe — a const-generic argument
    /// whose expression composes ordinary closed machine calls such as
    /// `sized(4) + 2`. The exact transitive call closure collected beside
    /// evaluation is returned so the caller can rejoin its original lexical
    /// selections, including skipped branches, before replacing the index. Each
    /// authored call still undergoes the same selection, argument snapshot,
    /// and concrete premise/failure discharge as initializer invocations.
    pub(crate) fn evaluate_application_probe(
        &self,
        reference: SourceSpan,
        public: bool,
        syntax: &mut syntax_trees::SyntaxTrees,
        extent: bool,
    ) -> Result<ApplicationProbeEvaluation, String> {
        let typed = self.program.typed();
        let roots = self.probe_roots()?;
        let &[expression] = roots.as_slice() else {
            return Err("application probe lost its single result root".into());
        };
        crate::machine_execution::admission::require_const_expression_selection(
            typed,
            self.machine,
            reference,
            self.program.authority.as_deref(),
        )?;
        let destination_type = if extent {
            expression_result_type_reference(typed, self.machine, self.state, expression)
                .unwrap_or(self.state.return_type)
        } else {
            self.state.return_type
        };
        let destination =
            crate::const_evaluation::const_generic_expressions::exact_probe_destination(
                typed,
                destination_type,
            );
        let (origins, operators, calls) =
            leaf_call_custody(typed, self.machine, self.state, &roots, public, syntax)?;
        let (value, warnings, materialized_expression) = match destination {
            Some(destination) => {
                let (value, warnings) = self.evaluate(expression, destination)?;
                // A scalar result has the same computation custody as an
                // aggregate result. Retain both sides of materialization;
                // replacing only the atom loses selected calls inside types.
                let literal =
                    match value.decode_encoding() {
                        Some(DecodedCanonicalConstValue::Integer { value, type_name }) => {
                            let spelling = value.to_string();
                            syntax_trees::expression::ExpressionNode::Integer(
                                numerics::literals::IntegerLiteral::from_parts(
                                    spelling.starts_with('-'),
                                    numerics::literals::IntegerRadix::Decimal,
                                    spelling.strip_prefix('-').unwrap_or(&spelling),
                                )
                                .map_err(str::to_owned)?
                                .with_landing(numerics::literals::IntegerLanding {
                                    landed_type: numerics::literals::LandedIntegerType::from_name(
                                        &type_name,
                                    )
                                    .ok_or("scalar application lost its checked integer carrier")?,
                                    domain: numerics::arithmetic::ArithmeticDomain::Exact,
                                }),
                            )
                        }
                        Some(DecodedCanonicalConstValue::Boolean(value)) => {
                            syntax_trees::expression::ExpressionNode::Boolean(value)
                        }
                        _ => return Err("scalar application has no canonical scalar value".into()),
                    };
                let materialized = syntax.expressions.insert(literal);
                syntax.expressions.set_source_span(materialized, reference);
                (value, warnings, materialized)
            }
            None => {
                // The initializer's ordinary checked invocation owns aggregate
                // results too. Admit every authored call before interpretation,
                // then replay its complete result against the receiving carrier.
                // Equal field bytes cannot supply nominal identity, predicates,
                // copying rights, or eligibility as a canonical static index.
                let mut warnings = Vec::new();
                for call in immediate_call_expressions(typed, &roots)? {
                    extend_unique(&mut warnings, self.check_call(call)?);
                }
                let result = self.interpret(reference)?;
                let decoded =
                    super::materialize::canonical_value(typed, destination_type, &result)?;
                let value = CanonicalConstValue::from_decoded(
                    &decoded,
                    expression_table(typed).display_name(expression),
                )
                .ok_or(
                    "application result is not a canonical static value within encoding limits",
                )?;
                validation::validate_exact_const_value_encoding(
                    typed,
                    destination_type,
                    &value.encoding,
                )?;
                // Source normalization retains the canonical schema label;
                // the independently checked destination above owns nominal
                // authority, rather than a rendered typed-identity string.
                let materialized = super::materialize::literal(
                    syntax,
                    typed,
                    destination_type,
                    &result,
                    reference,
                )?;
                (value, warnings, materialized)
            }
        };
        Ok(ApplicationProbeEvaluation {
            value,
            materialized_expression,
            origins,
            operators,
            calls,
            warnings,
        })
    }

    /// Interpret this retained probe's own transition and return the
    /// structured result with its declared carrier. Receiving-side replay uses
    /// the same checked admission as the original evaluation.
    pub(super) fn evaluate_value(&self) -> Result<(BuildTimeValue, TypeReferenceHandle), String> {
        let typed = self.program.typed();
        let roots = self.probe_roots()?;
        let reference = self.probe_reference()?;
        crate::machine_execution::admission::require_const_expression_selection(
            typed,
            self.machine,
            reference,
            self.program.authority.as_deref(),
        )?;
        for call in immediate_call_expressions(typed, &roots)? {
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
    actual: TypeReferenceHandle,
    destination: TypeReferenceHandle,
) -> bool {
    if let (Some(actual_primitive), Some(destination_primitive)) = (
        typed.primitive_type_reference(actual),
        typed.primitive_type_reference(destination),
    ) {
        return actual_primitive == destination_primitive
            && typed.arithmetic_domain_for_type_reference(actual)
                == typed.arithmetic_domain_for_type_reference(destination);
    }
    let actual = type_reference_table(typed).unconstrained_type_reference(actual);
    let destination = type_reference_table(typed).unconstrained_type_reference(destination);
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

/// The evaluated result of one application probe: the canonical value,
/// the selection custody gathered beside it, the exact transitive call
/// closure, and any evaluation warnings.
pub(crate) struct ApplicationProbeEvaluation {
    pub(crate) value: CanonicalConstValue,
    pub(crate) materialized_expression: syntax_trees::expression::ExpressionHandle,
    pub(crate) origins: Vec<syntax_trees::types::ConstArgumentOrigin>,
    pub(crate) operators: Vec<SourceSpan>,
    pub(crate) calls: Vec<(SourceSpan, SourceSpan)>,
    pub(crate) warnings: Vec<Diagnostic>,
}

/// The evaluated result of one aggregate-producing initializer leaf: the
/// checked interpreter's structured value, its declared carrier, and the same
/// custody receipts the scalar leaf path collects.
pub(super) struct LeafEvaluation {
    pub(super) value: BuildTimeValue,
    pub(super) destination: TypeReferenceHandle,
    pub(super) origins: Vec<syntax_trees::types::ConstArgumentOrigin>,
    pub(super) operators: Vec<SourceSpan>,
    pub(super) calls: Vec<(SourceSpan, SourceSpan)>,
    pub(super) warnings: Vec<Diagnostic>,
}

pub(super) fn scalar_snapshot(
    value: &ScalarValue,
    carrier: PrimitiveType,
) -> Result<ExpressionNode, String> {
    use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix};
    let value = match value {
        ScalarValue::UndeterminedNaN(_) => {
            return Err("concrete contract snapshot requires determined floating representation; NaN has no payload bits".into());
        }
        ScalarValue::Index(value) => value,
        ScalarValue::Float { format, bits } => {
            if !matches!(
                (format, carrier),
                (FloatFormat::F32, PrimitiveType::F32) | (FloatFormat::F64, PrimitiveType::F64)
            ) {
                return Err("constant argument lost its exact floating carrier".into());
            }
            return Ok(ExpressionNode::Float(
                FloatLiteral::from_bits(*format, *bits).ok_or("invalid floating snapshot")?,
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
    owner: SymbolHandle,
    name: String,
    expression: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> SymbolHandle {
    let machine = create_evaluation_entry(program, owner, name, expression, destination);
    let symbol = machine.symbol;
    program.push_machine(machine);
    symbol
}
