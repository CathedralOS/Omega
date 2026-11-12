/*
    Dependency-ordered evaluation, materialization, target-layout observation,
    and receiving-program replay of computed scalar and aggregate constant
    initializers.

    We index authored constant dependencies without substituting pending values,
    evaluate ready scalar leaves and helper invocations through admitted probes,
    resume target-layout-dependent initializers after layout closure, and replay
    saved initializer receipts against final typed programs.
*/

use crate::syntax_probes::extend_normalization_owners;
use crate::unique::{extend_unique, push_unique};
use diagnostics::Diagnostic;
use language_core::constant_value::EvaluatedConstant;
use language_semantics::const_value::DecodedCanonicalConstValue;
use source::SourceSpan;
use std::collections::HashMap;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::item::{
    ConstDefinition, ConstInitializerCustody, ConstInitializerNormalization, Item, ItemHandle,
};
use syntax_trees::types::ConstArgumentOrigin;
use syntax_trees::{FloatSpelling, SyntaxTrees};
use syntax_trees_to_symbol_resolved_trees::pre_resolution::{
    ConstInitializerSelection, InitializerCustodyPurpose, resolve_deferred_initializer_probe,
};

mod invocations {
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
            let projection =
                super::probe_projection::ProbeProjection::new(execution, probe_symbols);
            let bodies = projection.helper_bodies();
            let preparation_request = typed_trees_to_typed_trees::PreparationRequest::settled();
            let checking = typed_trees_to_checked_trees::CheckingRequest::new();
            let (bodies, preparation) =
                typed_trees_to_typed_trees::prepare_typed_trees(bodies, &preparation_request)?;
            let checked =
                typed_trees_to_checked_trees::lower_typed_trees(bodies, preparation, &checking)?;
            let typed = projection.rejoin_unjudged(checked.typed.into_inner());
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

        pub(crate) fn calls_for_source(
            &self,
            reference: SourceSpan,
        ) -> Result<Invocation<'_>, String> {
            let typed = self.typed();
            let mut machines = typed.machines().iter().filter(|machine| {
                typed.symbols.symbol_source_span(machine.symbol) == Some(reference)
            });
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
            symbol: SymbolHandle,
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

        pub(super) fn evaluate_probe_scalar(
            &self,
            destination: PrimitiveType,
        ) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
            let typed = self.program.typed();
            let body =
                crate::const_evaluation::initializer_probe::value_probe_body(typed, self.state)?;
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

        fn selected(&self, expression: ExpressionHandle) -> Result<PreparedCall<'_>, String> {
            let typed = self.program.typed();
            require_call_expression_selection(
                typed,
                expression,
                self.program.authority.as_deref(),
            )?;
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
            let public_signature =
                call.static_requirement_dispatch
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
                    && typed.machine_states(machine).first().is_some_and(|entry| {
                        typed.call_has_no_runtime_receiver(call, machine, entry)
                    })
            });
            let target = if let Some(machine) = selected.next() {
                if selected.next().is_some() {
                    return Err("constant call has conflicting exact entries".into());
                }
                let entry = typed
                    .machine_states(machine)
                    .first()
                    .ok_or("constant call lost its entry")?;
                let custody = BuildTimeInvocationCustody::Source(
                    typed.expression_table.source_span(expression),
                );
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
                let operator = typed_trees::operator::resolve_named_expression_call(typed, call)
                    .ok_or(
                        "constant call has no exact closed ordinary entry or named requirement",
                    )?;
                SelectedCall::Operator(operator)
            };
            Ok(PreparedCall {
                target,
                public_signature,
                operands: self.call_operands(expression, target)?,
            })
        }
    }

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

    struct AdmittedCall<'program> {
        target: PreparedCall<'program>,
        arguments: Vec<BuildTimeValue>,
        warnings: Vec<Diagnostic>,
    }

    impl Invocation<'_> {
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
                } if premise_discharge => self
                    .program
                    .admission
                    .evaluate_const_evaluable_machine_symbol_for_concrete_premise_invocation(
                        typed,
                        machine.symbol,
                        arguments,
                        custody,
                    )?,
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
                    let mut probe = typed.clone();
                    let owner = append_probe(
                        &mut probe,
                        self.machine.symbol,
                        "@const-requirement".into(),
                        expression,
                        operator.return_type,
                    );
                    let admission = crate::BuildTimeAdmissionPlan::infer(
                        &probe,
                        self.program.authority.clone(),
                    )
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

        fn admit_call(&self, expression: ExpressionHandle) -> Result<AdmittedCall<'_>, String> {
            let typed = self.program.typed();
            let target = self.selected(expression)?;
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
                    let canonical =
                        super::materialize::canonical_value(typed, type_reference, &value)?;
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
                        Some(DecodedCanonicalConstValue::Boolean(value)) => {
                            BuildTimeValue::Bool(value)
                        }
                        _ => {
                            return Err(
                                "constant argument is not a canonical scalar snapshot".into()
                            );
                        }
                    },
                });
                extend_unique(&mut warnings, argument_warnings);
            }
            let callee = match target.target {
                SelectedCall::Entry { machine, .. } => machine.name.to_string(),
                SelectedCall::Operator(operator) => {
                    typed.symbols.display_path(operator.symbol, "::")
                }
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
                    if arguments.len() != call.arguments.len()
                        || arguments.len() != parameters.len()
                    {
                        return Err(
                            "constant call argument count differs from its exact entry".into()
                        );
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

        pub(super) fn check_call(
            &self,
            expression: ExpressionHandle,
        ) -> Result<Vec<Diagnostic>, String> {
            Ok(self.admit_call(expression)?.warnings)
        }

        fn probe_roots(&self) -> Result<Vec<ExpressionHandle>, String> {
            Ok(
                crate::const_evaluation::initializer_probe::value_probe_body(
                    self.program.typed(),
                    self.state,
                )?
                .roots,
            )
        }

        fn probe_reference(&self) -> Result<SourceSpan, String> {
            Ok(
                crate::const_evaluation::initializer_probe::value_probe_body(
                    self.program.typed(),
                    self.state,
                )?
                .reference,
            )
        }

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

        fn require_expression_carrier(
            typed: &TypedTrees,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            destination: TypeReferenceHandle,
            _statement_index: usize,
        ) -> Result<(), String> {
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
                        _statement_index,
                    )?;
                }
                return Ok(());
            }
            if typed.primitive_type_reference(destination).is_some() {
                return Ok(());
            }
            let carrier = type_reference_table(typed).unconstrained_type_reference(destination);
            match typed.expression_table.expression(expression) {
                ExpressionNode::ArrayLiteral(elements) => {
                    if let Some(actual) =
                        expression_result_type_reference(typed, machine, state, expression)
                        && !result_carriers_match(typed, actual, destination)
                    {
                        return Err(
                            "constant initializer result does not match its declared carrier"
                                .into(),
                        );
                    }
                    let typed_trees::types::TypeReferenceNode::FixedArray {
                        element_type,
                        length: typed_trees::types::FixedArrayLength::Literal(length),
                    } = typed.type_reference_table.type_reference(carrier)
                    else {
                        return Err(
                            "constant initializer result does not match its declared carrier"
                                .into(),
                        );
                    };
                    let values = typed.expression_table.expression_handles(*elements);
                    if values.len() != elements.len() || values.len() != *length {
                        return Err(
                            "constant initializer result does not match its declared carrier"
                                .into(),
                        );
                    }
                    for value in values {
                        Self::require_expression_carrier(
                            typed,
                            machine,
                            state,
                            *value,
                            *element_type,
                            _statement_index,
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
                            "constant initializer result does not match its declared carrier"
                                .into(),
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
                            _statement_index,
                        )?;
                    }
                    return Ok(());
                }
                _ => {}
            }
            let actual = expression_result_type_reference(typed, machine, state, expression)
                .ok_or("constant initializer lost its selected result carrier")?;
            if !result_carriers_match(typed, actual, destination) {
                return Err(
                    "constant initializer result does not match its declared carrier".into(),
                );
            }
            Ok(())
        }

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
                    let literal = match value.decode_encoding() {
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

        pub(super) fn evaluate_value(
            &self,
        ) -> Result<(BuildTimeValue, TypeReferenceHandle), String> {
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

    pub(crate) struct ApplicationProbeEvaluation {
        pub(crate) value: CanonicalConstValue,
        pub(crate) materialized_expression: syntax_trees::expression::ExpressionHandle,
        pub(crate) origins: Vec<syntax_trees::types::ConstArgumentOrigin>,
        pub(crate) operators: Vec<SourceSpan>,
        pub(crate) calls: Vec<(SourceSpan, SourceSpan)>,
        pub(crate) warnings: Vec<Diagnostic>,
    }

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
}
mod materialize {
    use crate::BuildTimeValue;
    use crate::const_evaluation::const_generic_expressions::scalar_probe_destination;
    use language_semantics::const_value::DecodedCanonicalConstValue;
    use source::SourceSpan;
    use symbols::{SymbolHandle, SymbolKind};
    use syntax_trees::expression::{
        ExpressionHandle, ExpressionNode, TableStructLiteral, TableStructLiteralField,
    };
    use syntax_trees::identifier::Identifier;
    use syntax_trees::{FloatSpelling, SyntaxTrees};
    use typed_trees::TypedTrees;
    use typed_trees::data::{DataMember, DataVariant};
    use typed_trees::types::{
        FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
    };

    pub(super) fn literal(
        syntax: &mut SyntaxTrees,
        program: &TypedTrees,
        destination: TypeReferenceHandle,
        value: &BuildTimeValue,
        reference: SourceSpan,
    ) -> Result<ExpressionHandle, String> {
        if let Some(storage) = program.layout_storage_arguments(destination) {
            return literal(syntax, program, storage.semantic_type, value, reference);
        }
        let node = match program.type_reference_table.type_reference(destination) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                return literal(syntax, program, *base_type, value, reference);
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let BuildTimeValue::Array(elements) = value else {
                    return Err(
                        "evaluated aggregate leaf did not produce its declared array carrier"
                            .to_owned(),
                    );
                };
                if elements.len() != *length {
                    return Err("evaluated aggregate leaf array shape drifted".to_owned());
                }
                let mut materialized = Vec::with_capacity(elements.len());
                for element in elements {
                    materialized.push(literal(syntax, program, *element_type, element, reference)?);
                }
                let elements = syntax.expressions.insert_expression_handles(materialized);
                ExpressionNode::ArrayLiteral(elements)
            }
            TypeReferenceNode::Named { symbol, .. } => {
                if let Some(primitive) = scalar_probe_destination(program, destination) {
                    match (primitive, value) {
                        (
                            PrimitiveType::F32 | PrimitiveType::F64,
                            BuildTimeValue::UndeterminedNaN(_),
                        ) => {
                            return Err(
                                "constant result is NaN without an exact raw-NaN realization"
                                    .into(),
                            );
                        }
                        (PrimitiveType::Bool, BuildTimeValue::Bool(value)) => {
                            ExpressionNode::Boolean(*value)
                        }
                        (primitive, BuildTimeValue::Int(value))
                            if primitive.accepts_integer_literal() =>
                        {
                            let digits = if primitive.is_signed_integer() {
                                value.unsigned_abs().to_string()
                            } else {
                                (*value as u64).to_string()
                            };
                            ExpressionNode::Integer(
                                numerics::literals::IntegerLiteral::from_parts(
                                    primitive.is_signed_integer() && *value < 0,
                                    numerics::literals::IntegerRadix::Decimal,
                                    &digits,
                                )
                                .map_err(str::to_owned)?,
                            )
                        }
                        (
                            primitive @ (PrimitiveType::F32 | PrimitiveType::F64),
                            BuildTimeValue::Float(value),
                        ) => {
                            let (format, bits) = floating_bits(primitive, *value)?;
                            let literal = numerics::literals::FloatLiteral::from_bits(format, bits)
                                .ok_or("invalid floating representation")?;
                            ExpressionNode::Float(FloatSpelling::materialized(literal, reference))
                        }
                        _ => {
                            return Err(
                            "evaluated aggregate leaf field does not match its declared scalar carrier"
                                .to_owned(),
                        );
                        }
                    }
                } else {
                    nominal_literal(syntax, program, *symbol, value, reference)?
                }
            }
            TypeReferenceNode::Generic { .. } => {
                let definition = program
                    .closed_data_definition(destination)
                    .ok_or("evaluated aggregate leaf lost its exact closed data application")?;
                nominal_literal_for_definition(syntax, program, definition, value, reference)?
            }
            _ => {
                return Err("evaluated aggregate leaf has no materializable carrier".to_owned());
            }
        };
        let handle = syntax.expressions.insert(node);
        syntax.expressions.set_source_span(handle, reference);
        Ok(handle)
    }

    fn nominal_literal(
        syntax: &mut SyntaxTrees,
        program: &TypedTrees,
        symbol: SymbolHandle,
        value: &BuildTimeValue,
        reference: SourceSpan,
    ) -> Result<ExpressionNode, String> {
        let mut definitions = program
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == symbol);
        let definition = definitions
            .next()
            .ok_or("evaluated aggregate leaf has no exact nominal declaration")?;
        if definitions.next().is_some() {
            return Err("evaluated aggregate leaf has ambiguous nominal identity".to_owned());
        }
        nominal_literal_for_definition(syntax, program, definition, value, reference)
    }

    fn nominal_literal_for_definition(
        syntax: &mut SyntaxTrees,
        program: &TypedTrees,
        definition: &typed_trees::data::DataDefinition,
        value: &BuildTimeValue,
        reference: SourceSpan,
    ) -> Result<ExpressionNode, String> {
        if definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
            return Err("evaluated aggregate leaf has no source-owned constructor".into());
        }
        let symbol = definition.symbol;
        let owner_path = program.symbols.display_path(symbol, "::");
        if program
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                &owner_path,
                &[SymbolKind::Data],
                reference,
            )
            != Some(symbol)
        {
            return Err(format!(
                "evaluated constructor `{owner_path}` cannot reselect its exact carrier from the constant's source"
            ));
        }
        let owner_path = if let Some(origin) = definition.generic_instance {
            let TypeReferenceNode::Generic { base_symbol, .. } =
                program.type_reference_table.type_reference(origin)
            else {
                return Err("evaluated generic constructor lost its template origin".to_owned());
            };
            let template_path = program.symbols.display_path(*base_symbol, "::");
            if !base_symbol.is_valid()
                || program
                    .symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        &template_path,
                        &[SymbolKind::Data],
                        reference,
                    )
                    != Some(*base_symbol)
            {
                return Err(
                    "evaluated generic constructor cannot reselect its exact template".to_owned(),
                );
            }
            template_path
        } else {
            owner_path
        };
        let members = program.data_members(definition);
        let (constructor, declared) = match value {
            BuildTimeValue::Struct { type_name, fields } => {
                if type_name != definition.name.as_str() {
                    return Err(format!(
                        "evaluated aggregate leaf expected record `{}`, found record `{type_name}`",
                        definition.name
                    ));
                }
                (
                    owner_path,
                    members
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Field(field) => Some((field, fields)),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                )
            }
            BuildTimeValue::Case { variant, payload } => {
                let case = variant_member_by_name(members, variant.as_str()).ok_or_else(|| {
                    format!("evaluated aggregate leaf case `{variant}` is absent from its owner")
                })?;
                (
                    format!("{owner_path}::{variant}"),
                    program
                        .data_payload_fields(case)
                        .iter()
                        .map(|field| (field, payload))
                        .collect::<Vec<_>>(),
                )
            }
            _ => {
                return Err(
                    "evaluated aggregate leaf is not a nominal constructor value".to_owned(),
                );
            }
        };
        let mut literal_fields = Vec::with_capacity(declared.len());
        for (field, supplied) in declared {
            let value = supplied
                .iter()
                .find(|(name, _)| name.as_str() == field.name.as_str())
                .map(|(_, value)| value)
                .ok_or_else(|| {
                    format!(
                        "evaluated aggregate leaf lost declared field `{}`",
                        field.name
                    )
                })?;
            let value = literal(syntax, program, field.type_reference, value, reference)?;
            literal_fields.push(TableStructLiteralField {
                name: Identifier::generated(field.name.as_str()),
                value,
            });
        }
        let fields = syntax.expressions.insert_struct_fields(literal_fields);
        Ok(ExpressionNode::StructLiteral(TableStructLiteral {
            application_type: Default::default(),
            constructor_name: Identifier::new(
                constructor,
                SourceSpan::new(
                    reference.source_id,
                    source::Span::new(reference.span.start, reference.span.start),
                ),
            ),
            fields,
        }))
    }

    pub(super) fn canonical_value(
        program: &TypedTrees,
        destination: TypeReferenceHandle,
        value: &BuildTimeValue,
    ) -> Result<DecodedCanonicalConstValue, String> {
        if let Some(storage) = program.layout_storage_arguments(destination) {
            return canonical_value(program, storage.semantic_type, value);
        }
        match program.type_reference_table.type_reference(destination) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                canonical_value(program, *base_type, value)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let BuildTimeValue::Array(elements) = value else {
                    return Err(
                        "evaluated leaf did not produce its declared array carrier".to_owned()
                    );
                };
                if elements.len() != *length {
                    return Err("evaluated leaf array shape drifted".to_owned());
                }
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    values.push(canonical_value(program, *element_type, element)?);
                }
                Ok(DecodedCanonicalConstValue::Array {
                    type_name: type_label(program, destination)?,
                    values,
                })
            }
            TypeReferenceNode::Named { symbol, .. } => {
                if let Some(primitive) = scalar_probe_destination(program, destination) {
                    match (primitive, value) {
                        (
                            PrimitiveType::F32 | PrimitiveType::F64,
                            BuildTimeValue::UndeterminedNaN(_),
                        ) => Err(
                            "constant result is NaN without an exact raw-NaN realization".into(),
                        ),
                        (PrimitiveType::Bool, BuildTimeValue::Bool(value)) => {
                            Ok(DecodedCanonicalConstValue::Boolean(*value))
                        }
                        (primitive, BuildTimeValue::Int(value))
                            if primitive.accepts_integer_literal() =>
                        {
                            Ok(DecodedCanonicalConstValue::Integer {
                                type_name: primitive.name().to_owned(),
                                value: if primitive.is_signed_integer() {
                                    i128::from(*value)
                                } else {
                                    i128::from(*value as u64)
                                },
                            })
                        }
                        (
                            primitive @ (PrimitiveType::F32 | PrimitiveType::F64),
                            BuildTimeValue::Float(value),
                        ) => {
                            let (format, bits) = floating_bits(primitive, *value)?;
                            Ok(DecodedCanonicalConstValue::Float { format, bits })
                        }
                        _ => {
                            Err("evaluated leaf does not match its declared scalar carrier"
                                .to_owned())
                        }
                    }
                } else {
                    nominal_value(program, *symbol, value)
                }
            }
            TypeReferenceNode::Generic { .. } => nominal_value_for_definition(
                program,
                program
                    .closed_data_definition(destination)
                    .ok_or("evaluated leaf lost its exact closed data application")?,
                value,
            ),
            _ => Err("evaluated leaf has no encodable carrier".to_owned()),
        }
    }

    fn nominal_value(
        program: &TypedTrees,
        symbol: SymbolHandle,
        value: &BuildTimeValue,
    ) -> Result<DecodedCanonicalConstValue, String> {
        let mut definitions = program
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == symbol);
        let definition = definitions
            .next()
            .ok_or("evaluated leaf has no exact nominal declaration")?;
        if definitions.next().is_some() {
            return Err("evaluated leaf has ambiguous nominal identity".to_owned());
        }
        nominal_value_for_definition(program, definition, value)
    }

    fn nominal_value_for_definition(
        program: &TypedTrees,
        definition: &typed_trees::data::DataDefinition,
        value: &BuildTimeValue,
    ) -> Result<DecodedCanonicalConstValue, String> {
        if definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
            return Err("evaluated leaf has no source-owned structural value".into());
        }
        let members = program.data_members(definition);
        let type_name = definition.name.as_str().to_owned();
        let encode_fields = |declared: &[&typed_trees::data::DataField],
                             supplied: &[(String, BuildTimeValue)]|
         -> Result<Vec<(String, DecodedCanonicalConstValue)>, String> {
            let mut fields = Vec::with_capacity(declared.len());
            for field in declared {
                let value = supplied
                    .iter()
                    .find(|(name, _)| name.as_str() == field.name.as_str())
                    .map(|(_, value)| value)
                    .ok_or_else(|| {
                        format!("evaluated leaf lost declared field `{}`", field.name)
                    })?;
                fields.push((
                    field.name.as_str().to_owned(),
                    canonical_value(program, field.type_reference, value)?,
                ));
            }
            Ok(fields)
        };
        match value {
            BuildTimeValue::Struct {
                type_name: actual,
                fields,
            } => {
                if actual != &type_name {
                    return Err(format!(
                        "evaluated leaf expected record `{type_name}`, found record `{actual}`"
                    ));
                }
                let declared = members
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some(field),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                Ok(DecodedCanonicalConstValue::Record {
                    type_name,
                    fields: encode_fields(&declared, fields)?,
                })
            }
            BuildTimeValue::Case { variant, payload } => {
                let case = variant_member_by_name(members, variant.as_str()).ok_or_else(|| {
                    format!("evaluated leaf case `{variant}` is absent from its owner")
                })?;
                let declared = program.data_payload_fields(case).iter().collect::<Vec<_>>();
                Ok(DecodedCanonicalConstValue::Variant {
                    type_name,
                    case_name: variant.clone(),
                    fields: encode_fields(&declared, payload)?,
                })
            }
            _ => Err("evaluated leaf is not a nominal constructor value".to_owned()),
        }
    }

    pub(super) fn type_label(
        program: &TypedTrees,
        destination: TypeReferenceHandle,
    ) -> Result<String, String> {
        match program.type_reference_table.type_reference(destination) {
            TypeReferenceNode::Named { symbol, name } => {
                if program.symbols.builtin_type_atom(*symbol).is_some() {
                    return Ok(name.as_str().to_owned());
                }
                let mut definitions = program
                    .data_definitions()
                    .iter()
                    .filter(|definition| definition.symbol == *symbol);
                let definition = definitions
                    .next()
                    .ok_or("aggregate leaf carrier has no exact nominal declaration")?;
                if definitions.next().is_some() {
                    return Err("aggregate leaf carrier has ambiguous nominal identity".to_owned());
                }
                Ok(definition.name.as_str().to_owned())
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => Ok(format!(
                "[{}; {length}]",
                type_label(program, *element_type)?
            )),
            TypeReferenceNode::Constrained { base_type, .. } => type_label(program, *base_type),
            TypeReferenceNode::Unit => Ok("()".to_owned()),
            _ => Err("aggregate leaf carrier has no canonical label".to_owned()),
        }
    }

    fn floating_bits(
        primitive: PrimitiveType,
        value: f64,
    ) -> Result<(numerics::literals::FloatFormat, u64), String> {
        use numerics::literals::FloatFormat;
        let format = match primitive {
            PrimitiveType::F32 => FloatFormat::F32,
            PrimitiveType::F64 => FloatFormat::F64,
            _ => return Err("constant result has no floating carrier".into()),
        };
        let bits = format
            .capture_wide_representation(value)
            .ok_or("constant result does not retain one exact floating representation")?;
        Ok((format, bits))
    }

    fn variant_member_by_name<'m>(
        members: &'m [DataMember],
        name: &str,
    ) -> Option<&'m DataVariant> {
        members.iter().find_map(|member| match member {
            DataMember::Variant(case) if case.name.as_str() == name => Some(case),
            _ => None,
        })
    }
}
mod probe_projection {
    use crate::build_time_evaluation::expression_table;
    use std::collections::HashSet;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::references::{DeclarationSelection, collect_declaration_symbol_dependencies};
    use typed_trees::typed_trees::{EvidenceForwarding, MachineSpecialization, ProofOutputCall};

    pub(super) struct ProbeProjection<'program> {
        original: &'program TypedTrees,
        removed: DeclarationSelection,
    }

    impl<'program> ProbeProjection<'program> {
        pub(super) fn new(original: &'program TypedTrees, probes: &[SymbolHandle]) -> Self {
            let probe_roots = DeclarationSelection::from_symbols(probes.iter().copied());
            let mut candidates = DeclarationSelection::from_symbols([]);
            loop {
                let before = candidates.clone();
                let private_owner = |owner| {
                    owned_by(original, &probe_roots, owner)
                        || owned_by(original, &candidates, owner)
                };
                let mut discovered = Vec::new();
                for (_, capture) in original.captured_machine_applications.iter() {
                    if capture.instance != capture.template
                        && capture.supplying_application.is_valid()
                        && private_owner(
                            expression_table(original)
                                .static_machine_application(capture.supplying_application)
                                .lexical_owner,
                        )
                    {
                        discovered.push(capture.instance);
                    }
                }
                for specialization in &original.machine_specializations {
                    if specialization.instance != specialization.template
                        && specialization.binding_application.is_valid()
                        && private_owner(
                            expression_table(original)
                                .static_machine_application(specialization.binding_application)
                                .lexical_owner,
                        )
                    {
                        discovered.push(specialization.instance);
                    }
                }
                for machine in original.machines() {
                    let declaration = original.symbols.get(machine.symbol);
                    if (declaration.generated_from.is_valid() && private_owner(declaration.parent))
                        || machine
                            .lexical_region_origin
                            .is_some_and(|origin| private_owner(origin.machine))
                    {
                        discovered.push(machine.symbol);
                    }
                }
                for symbol in discovered {
                    candidates.include(symbol);
                }
                if candidates == before {
                    break;
                }
            }

            let dependencies = collect_declaration_symbol_dependencies(original);
            let mut retained = DeclarationSelection::from_symbols([]);
            loop {
                let before = retained.clone();
                for (owner, references) in &dependencies {
                    if probe_roots.contains(*owner)
                        || (candidates.contains(*owner) && !retained.contains(*owner))
                    {
                        continue;
                    }
                    for &reference in references {
                        let mut symbol = reference;
                        while symbol.is_valid() {
                            if candidates.contains(symbol) {
                                retained.include(symbol);
                            }
                            symbol = original.symbols.get(symbol).parent;
                        }
                    }
                }
                if retained == before {
                    break;
                }
            }
            let removed = DeclarationSelection::from_symbols(
                original.machines().iter().filter_map(|machine| {
                    (probe_roots.contains(machine.symbol)
                        || (candidates.contains(machine.symbol)
                            && !retained.contains(machine.symbol)))
                    .then_some(machine.symbol)
                }),
            );
            Self { original, removed }
        }

        pub(super) fn helper_bodies(&self) -> TypedTrees {
            let mut program = self.original.clone();
            program.identity = Default::default();
            let removed = program
                .machines()
                .iter()
                .filter_map(|machine| {
                    self.removed
                        .contains(machine.symbol)
                        .then_some(machine.symbol)
                })
                .collect::<HashSet<_>>();
            let retained = program
                .machines()
                .iter()
                .filter_map(|machine| {
                    (!self.removed.contains(machine.symbol)).then_some(machine.symbol)
                })
                .collect::<HashSet<_>>();
            validation::execution_projection::retain_machine_roster(
                &mut program,
                &retained,
                &removed,
            );
            program
        }

        pub(super) fn rejoin_unjudged(&self, mut program: TypedTrees) -> TypedTrees {
            program.identity = Default::default();
            for machine in self
                .original
                .machines()
                .iter()
                .filter(|machine| self.removed.contains(machine.symbol))
            {
                program.push_machine(machine.clone());
            }
            macro_rules! restore_vector {
                ($field:ident, $owner:expr) => {
                    for row in &self.original.$field {
                        if owned_by(self.original, &self.removed, ($owner)(row))
                            && !program.$field.contains(row)
                        {
                            program.$field.push(row.clone());
                        }
                    }
                };
            }
            restore_vector!(machine_specializations, |row: &MachineSpecialization| row
                .instance);
            restore_vector!(
                authored_service_reach_rows,
                |row: &typed_trees::signature::AuthoredServiceReachRow| row.owner
            );
            restore_vector!(evidence_forwardings, |row: &EvidenceForwarding| row
                .machine_symbol);
            restore_vector!(proof_output_calls, |row: &ProofOutputCall| row
                .machine_symbol);
            restore_vector!(
                ranking_expression_custody,
                |row: &typed_trees::ranking::RankingExpressionCustody| row.machine
            );

            for (_, row) in self.original.captured_machine_applications.iter() {
                if self.removed.contains(row.instance)
                    && !program
                        .captured_machine_applications
                        .iter()
                        .any(|(_, current)| current.instance == row.instance)
                {
                    program.captured_machine_applications.insert(row.clone());
                }
            }
            for (_, row) in self.original.data_implementation_applications.iter() {
                if owned_by(self.original, &self.removed, row.lexical_owner)
                    && !program
                        .data_implementation_applications
                        .iter()
                        .any(|(_, current)| current == row)
                {
                    program.data_implementation_applications.insert(row.clone());
                }
            }
            for (_, row) in self.original.trait_implementation_applications.iter() {
                let source_owner = row.source_scope.supplying_owner();
                let supplying_owner = if row.supplying_application.is_valid() {
                    {
                        expression_table(self.original)
                            .static_machine_application(row.supplying_application)
                            .lexical_owner
                    }
                } else {
                    Default::default()
                };
                if (owned_by(self.original, &self.removed, source_owner)
                    || owned_by(self.original, &self.removed, supplying_owner))
                    && !program
                        .trait_implementation_applications
                        .iter()
                        .any(|(_, current)| current == row)
                {
                    program
                        .trait_implementation_applications
                        .insert(row.clone());
                }
            }
            program
        }
    }

    fn owned_by(
        program: &TypedTrees,
        owners: &DeclarationSelection,
        mut symbol: SymbolHandle,
    ) -> bool {
        while symbol.is_valid() {
            if owners.contains(symbol) {
                return true;
            }
            symbol = program.symbols.get(symbol).parent;
        }
        false
    }
}
mod replay {
    use crate::build_time_evaluation::{expression_table, type_reference_table};
    use crate::unique::push_unique;
    use std::sync::Arc;

    use crate::const_evaluation::const_generic_expressions::value::ConstantCalls;
    use crate::const_evaluation::const_generic_expressions::{
        initializer_contains_call, scalar_probe_destination,
        validate_retained_initializer_call_custody,
    };
    use crate::machine_execution::selected_operators::prepare_selected_execution;
    use crate::syntax_probes::PreparedEvaluationInputs;
    use diagnostics::Diagnostic;
    use language_semantics::const_value::{CanonicalConstIdentity, DecodedCanonicalConstValue};
    use typed_trees::{
        TypedTrees,
        data::DataMember,
        expression::{ExpressionHandle, ExpressionNode},
        types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode},
    };
    use validation::has_builtin_binary_expression_meaning;

    struct Leaf {
        owner: symbols::SymbolHandle,
        original: ExpressionHandle,
        materialized: ExpressionHandle,
        destination: TypeReferenceHandle,
        structured: bool,
        expected: DecodedCanonicalConstValue,
    }

    pub(crate) fn validate(
        typed: &TypedTrees,
        authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
        domain: &crate::build_time_evaluation::EvaluationDomain,
        selected: crate::SelectedBuildTimeOperators<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut replay_domain = typed_trees::references::DeclarationSelection::from_symbols(
            typed
                .const_declarations()
                .iter()
                .filter(|declaration| domain.contains(declaration.symbol))
                .map(|declaration| declaration.symbol),
        );
        loop {
            let references =
                typed_trees::references::collect_selected_program_references(typed, &replay_domain);
            let mut changed = false;
            for declaration in typed.const_declarations() {
                if references.symbols.contains(&declaration.symbol)
                    && !replay_domain.contains(declaration.symbol)
                {
                    replay_domain.include(declaration.symbol);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut leaves = Vec::new();
        let mut replay_declarations = Vec::new();
        let mut floating_declarations = Vec::new();
        for declaration in typed.const_declarations() {
            if !replay_domain.contains(declaration.symbol) {
                continue;
            }
            let reference = declaration.initializer_source_span;
            if !expression_table(typed).expression_is_valid(declaration.materialized_initializer)
                || expression_table(typed).source_span(declaration.materialized_initializer)
                    != reference
            {
                return Err(super::failure(
                    reference,
                    "constant declaration lost its exact materialized initializer root",
                ));
            }
            validation::validate_constant_literal_fields(
                typed,
                declaration.declared_type,
                declaration.materialized_initializer,
            )
            .map_err(|reason| super::failure(reference, reason))?;
            let mut retained_call = false;
            for occurrence in expression_table(typed)
                .authored_selection_occurrences(declaration.materialized_initializer)
            {
                let selection = typed
                    .authored_declaration_selections()
                    .get(occurrence)
                    .ok_or_else(|| {
                        super::failure(
                            reference,
                            "constant initializer lost its attached selection occurrence",
                        )
                    })?;
                retained_call |= selection.kind()
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call;
            }
            let authored_call = if declaration.authored_initializer.is_valid() {
                initializer_contains_call(typed, declaration.authored_initializer)
                    .map_err(|reason| super::failure(reference, reason))?
            } else {
                false
            };
            let destination = float_destination(typed, declaration.declared_type);
            let encoded_float = declaration
                .evaluated_value
                .as_ref()
                .and_then(|value| value.encoding())
                .is_some_and(|encoding| encoding.starts_with("float:"));
            let semantic_float = matches!(
                declaration.evaluated_value.as_ref(),
                Some(language_core::constant_value::EvaluatedConstant::UndeterminedNaN(_))
            ) || matches!(
                expression_table(typed).expression(declaration.materialized_initializer),
                ExpressionNode::UndeterminedNaN(_)
            );
            if destination.is_some() || encoded_float || semantic_float {
                let destination = destination.ok_or_else(|| {
                    super::failure(
                        reference,
                        "floating constant result encoding lost its exact declared format",
                    )
                })?;
                validate_replay_roots(typed, declaration, retained_call || authored_call)
                    .map_err(|reason| super::failure(reference, reason))?;
                if retained_call || authored_call {
                    replay_declarations.push(declaration);
                    floating_declarations.push((declaration, destination));
                } else {
                    let execution = domain.execution_program(typed);
                    validate_anonymous_float(
                        &execution,
                        declaration,
                        destination,
                        authority.clone(),
                    )
                    .map_err(|reason| super::failure(reference, reason))?;
                }
                continue;
            }
            let has_calls = retained_call || authored_call;
            if !has_calls && !contains_floating_component(typed, declaration.declared_type) {
                continue;
            }
            validate_replay_roots(typed, declaration, has_calls)
                .map_err(|reason| super::failure(reference, reason))?;
            let first_leaf = leaves.len();
            let identity = CanonicalConstIdentity {
                type_name: String::new(),
                encoding: declaration
                    .evaluated_value
                    .as_ref()
                    .and_then(|value| value.encoding())
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        super::failure(
                            declaration.initializer_source_span,
                            "retained invocation lost its canonical result",
                        )
                    })?,
            };
            let expected = identity.decode_declaration_encoding().ok_or_else(|| {
                super::failure(
                    declaration.initializer_source_span,
                    "retained invocation has invalid canonical result",
                )
            })?;
            pair(
                typed,
                Leaf {
                    owner: declaration.symbol,
                    original: declaration.authored_initializer,
                    materialized: declaration.materialized_initializer,
                    destination: declaration.declared_type,
                    structured: false,
                    expected,
                },
                &mut Vec::new(),
                &mut leaves,
            )
            .map_err(|reason| super::failure(declaration.initializer_source_span, reason))?;
            if leaves.len() != first_leaf {
                replay_declarations.push(declaration);
            } else if has_calls {
                return Err(super::failure(
                    reference,
                    "retained call declaration has no replayable leaf",
                ));
            }
        }
        if leaves.is_empty() && floating_declarations.is_empty() {
            return Ok(());
        }
        let mut probe = typed.clone();
        let mut owners = Vec::new();
        for (ordinal, leaf) in leaves.iter().enumerate() {
            let original = super::invocations::append_probe(
                &mut probe,
                leaf.owner,
                format!("@const-replay-{ordinal}"),
                leaf.original,
                leaf.destination,
            );
            let mut materialized_destination = leaf.destination;
            while let Some(storage) = probe.layout_storage_arguments(materialized_destination) {
                materialized_destination = storage.semantic_type;
            }
            let materialized = super::invocations::append_probe(
                &mut probe,
                leaf.owner,
                format!("@const-replay-materialized-{ordinal}"),
                leaf.materialized,
                materialized_destination,
            );
            owners.push((original, materialized));
        }
        let mut replay_owners = Vec::new();
        for (leaf, (owner, _)) in leaves.iter().zip(&owners) {
            if !replay_owners
                .iter()
                .any(|(declaration, _)| *declaration == leaf.owner)
            {
                replay_owners.push((leaf.owner, *owner));
            }
        }
        let mut probe_symbols = owners
            .iter()
            .flat_map(|(original, materialized)| [*original, *materialized])
            .collect::<Vec<_>>();
        for (ordinal, (declaration, _)) in floating_declarations.iter().enumerate() {
            let owner = super::invocations::append_probe(
                &mut probe,
                declaration.symbol,
                format!("@const-float-custody-{ordinal}"),
                declaration.authored_initializer,
                declaration.declared_type,
            );
            replay_owners.push((declaration.symbol, owner));
            probe_symbols.push(owner);
        }
        while !validation::resolve_named_result_overloads(&mut probe)?.is_empty() {}
        typed_trees_to_typed_trees::bind_pre_specialization_authored_selections(&mut probe)
            .map_err(|diagnostic| vec![diagnostic])?;
        let source_roots = replay_declarations
            .iter()
            .flat_map(|declaration| {
                [
                    declaration.authored_initializer,
                    declaration.materialized_initializer,
                ]
                .map(|root| (declaration.symbol, root))
            })
            .collect::<Vec<_>>();
        typed_trees_to_typed_trees::prepare_requirement_expression_views(
            &mut probe,
            &source_roots,
        )?;
        for declaration in &replay_declarations {
            let owner = replay_owners
                .iter()
                .find(|(symbol, _)| *symbol == declaration.symbol)
                .ok_or_else(|| {
                    super::failure(
                        declaration.initializer_source_span,
                        "constant lost its authored replay owner",
                    )
                })?
                .1;
            validate_authored_custody(&probe, owner, declaration)
            .map_err(|reason| {
                super::failure(
                    declaration.initializer_source_span,
                    format!(
                        "authored call custody for declaration {:?}, roots {:?}/{:?}, probe {owner:?}: {reason}",
                        declaration.symbol,
                        declaration.authored_initializer,
                        declaration.materialized_initializer,
                    ),
                )
            })?;
        }
        let probe =
            prepare_selected_execution(&probe, selected.provider_bodies, selected.operators)
                .map_err(|reason| super::failure(source::SourceSpan::default(), reason))?
                .unwrap_or(probe);
        // A replayed initializer's type can name a constant only through a
        // fixed-array extent, as `SIZE` in `[u64; SIZE]`, which no probe
        // declaration references. The realized program must still retain it.
        let referenced_constants = {
            let mut constants = TypeExtentConstants {
                program: &probe,
                visited: Vec::new(),
                constants: Vec::new(),
            };
            for declaration in &replay_declarations {
                typed_trees::references::visit_type_reference(
                    &probe,
                    &mut constants,
                    declaration.declared_type,
                );
            }
            constants.constants
        };
        let replay_roots = probe_symbols
            .iter()
            .copied()
            .chain(referenced_constants)
            .collect::<Vec<_>>();
        let replay_execution =
            PreparedEvaluationInputs::domain_for_program(&probe, &replay_roots, &[]);
        let prepared = crate::PreparedBuildMachineProgram::prepare_in(&probe, &replay_execution)?;
        let checked = super::invocations::CheckedInitializers::prepare(
            &prepared,
            authority.clone(),
            &probe_symbols,
            domain,
            selected.operators,
        )?;
        let program = checked.typed();
        let mut dependency_jobs = Vec::new();
        for declaration in &replay_declarations {
            let owner = replay_owners
                .iter()
                .find(|(symbol, _)| *symbol == declaration.symbol)
                .ok_or_else(|| {
                    super::failure(
                        declaration.initializer_source_span,
                        "constant lost its realized replay owner",
                    )
                })?
                .1;
            let dependencies = checked
            .calls_for_retained_initializer(owner, declaration.symbol)
            .and_then(|calls| calls.validate_custody(declaration))
            .map_err(|reason| {
                super::failure(
                    declaration.initializer_source_span,
                    format!(
                        "realized call custody for declaration {:?}, roots {:?}/{:?}, probe {owner:?}: {reason}",
                        declaration.symbol,
                        declaration.authored_initializer,
                        declaration.materialized_initializer,
                    ),
                )
            })?;
            for dependency in &dependencies {
                let owner = program
                    .const_declarations()
                    .iter()
                    .find(|owner| owner.symbol == dependency.declaration)
                    .ok_or_else(|| {
                        super::failure(
                            declaration.initializer_source_span,
                            "dependency capsule lost its declaration owner",
                        )
                    })?;
                let has_calls = program.expression_table.authored_selection_occurrences(owner.materialized_initializer)
                .any(|occurrence| program.authored_declaration_selections().get(occurrence)
                    .is_some_and(|selection| selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call))
                || initializer_contains_call(program, owner.authored_initializer)
                    .map_err(|reason| super::failure(declaration.initializer_source_span, reason))?;
                if has_calls
                    && !replay_declarations
                        .iter()
                        .any(|candidate| candidate.symbol == owner.symbol)
                {
                    return Err(super::failure(
                        declaration.initializer_source_span,
                        "call-bearing dependency has no independently checked replay owner",
                    ));
                }
            }
            dependency_jobs.push((declaration, owner, dependencies));
        }
        for (declaration, owner, dependencies) in dependency_jobs {
            let calls = checked
                .calls_for_retained_initializer(owner, declaration.symbol)
                .map_err(|reason| super::failure(declaration.initializer_source_span, reason))?;
            if let Some((_, destination)) = floating_declarations
                .iter()
                .find(|(candidate, _)| candidate.symbol == declaration.symbol)
            {
                validate_float_value(
                    program,
                    declaration,
                    *destination,
                    owner,
                    authority.as_deref(),
                    Some(&calls),
                    &dependencies,
                )
                .map_err(|reason| super::failure(declaration.initializer_source_span, reason))?;
            } else {
                validate_dependency_values(program, &dependencies, |expression, destination| {
                    calls
                        .evaluate_scalar(expression, destination)
                        .map(|(value, _)| value)
                })
                .map_err(|reason| super::failure(declaration.initializer_source_span, reason))?;
            }
        }
        for (leaf, (original_owner, materialized_owner)) in leaves.iter().zip(owners) {
            let result = (|| {
                let calls = checked.calls_for_retained_initializer(original_owner, leaf.owner)?;
                let materialized_calls =
                    checked.calls_for_retained_initializer(materialized_owner, leaf.owner)?;
                if let Some(destination) = scalar_probe_destination(program, leaf.destination) {
                    let original = decode_scalar(calls.evaluate_probe_scalar(destination)?.0)?;
                    let materialized =
                        decode_scalar(materialized_calls.evaluate_probe_scalar(destination)?.0)?;
                    if original != leaf.expected || materialized != leaf.expected {
                        return Err(
                        "retained constant invocation, materialized value, or canonical result drifted"
                            .to_owned(),
                    );
                    }
                    return Ok(());
                }
                let (original, _) = calls.evaluate_value()?;
                let (materialized, _) = materialized_calls.evaluate_value()?;
                let original =
                    super::materialize::canonical_value(program, leaf.destination, &original)?;
                let materialized =
                    super::materialize::canonical_value(program, leaf.destination, &materialized)?;
                if original != leaf.expected || materialized != leaf.expected {
                    return Err(
                    "retained constant invocation, materialized value, or canonical result drifted"
                        .to_owned(),
                );
                }
                Ok(())
            })();
            result.map_err(|reason| {
                super::failure(program.expression_table.source_span(leaf.original), reason)
            })?;
        }
        Ok(())
    }

    fn contains_floating_component(program: &TypedTrees, root: TypeReferenceHandle) -> bool {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(reference) = crate::worklist::unvisited(&mut pending, &mut visited) {
            if float_destination(program, reference).is_some() {
                return true;
            }
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
                TypeReferenceNode::FixedArray { element_type, .. } => pending.push(*element_type),
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some(definition) = program.data_definition_by_symbol(*symbol) {
                        for member in program.data_members(definition) {
                            match member {
                                DataMember::Field(field) => pending.push(field.type_reference),
                                DataMember::Variant(variant) => pending.extend(
                                    program
                                        .data_payload_fields(variant)
                                        .iter()
                                        .map(|field| field.type_reference),
                                ),
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn validate_replay_roots(
        program: &TypedTrees,
        declaration: &typed_trees::constant::ConstDeclaration,
        has_calls: bool,
    ) -> Result<(), String> {
        let retained = program
            .const_declaration_by_symbol(declaration.symbol)
            .ok_or("constant call receipt has no exact source declaration")?;
        if retained.authored_initializer != declaration.authored_initializer
            || retained.materialized_initializer != declaration.materialized_initializer
            || retained.declared_type != declaration.declared_type
        {
            return Err(
                "constant call receipt names another declaration's initializer roots".into(),
            );
        }
        let table = &program.expression_table;
        if !table.expression_is_valid(declaration.authored_initializer)
            || !table.expression_is_valid(declaration.materialized_initializer)
            || (has_calls
                && declaration.authored_initializer == declaration.materialized_initializer)
            || table.source_span(declaration.authored_initializer)
                != declaration.initializer_source_span
            || table.source_span(declaration.materialized_initializer)
                != declaration.initializer_source_span
        {
            return Err("retained declaration lost its source-owned initializer roots".into());
        }
        Ok(())
    }

    fn validate_authored_custody(
        program: &TypedTrees,
        owner: symbols::SymbolHandle,
        declaration: &typed_trees::constant::ConstDeclaration,
    ) -> Result<(), String> {
        let machine = program
            .machine(owner)
            .ok_or("constant lost its authored replay machine")?;
        let [state] = program.machine_states(machine) else {
            return Err("constant lost its authored replay state".into());
        };
        crate::const_evaluation::const_generic_expressions::validate_authored_initializer_call_custody(
        program,
        machine,
        state,
        declaration,
    )
    .map(|_| ())
    }

    fn float_destination(
        program: &TypedTrees,
        destination: TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        match program.type_reference_table.primitive_type(destination)? {
            PrimitiveType::F32 => Some(PrimitiveType::F32),
            PrimitiveType::F64 => Some(PrimitiveType::F64),
            _ => None,
        }
    }

    fn validate_anonymous_float(
        typed: &TypedTrees,
        declaration: &typed_trees::constant::ConstDeclaration,
        destination: PrimitiveType,
        authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    ) -> Result<(), String> {
        use crate::const_evaluation::const_generic_expressions::value::ScalarValue;
        use numerics::literals::FloatFormat;

        let original = declaration.authored_initializer;
        let materialized = declaration.materialized_initializer;
        if !typed.expression_table.expression_is_valid(original)
            || typed.expression_table.source_span(original) != declaration.initializer_source_span
        {
            return Err(
                "floating constant lost its distinct source-owned initializer roots".into(),
            );
        }
        if matches!(
            typed.expression_table.expression(original),
            ExpressionNode::Float(_) | ExpressionNode::Integer(_)
        ) && expression_table(typed)
            .authored_selection_occurrences(original)
            .next()
            .is_none()
            && expression_table(typed)
                .authored_selection_occurrences(materialized)
                .next()
                .is_none()
        {
            let format = match destination {
                PrimitiveType::F32 => FloatFormat::F32,
                PrimitiveType::F64 => FloatFormat::F64,
                _ => return Err("floating constant lost its exact declared format".into()),
            };
            let bits = float_literal_bits(typed, original, format)?;
            if float_literal_bits(typed, materialized, format)? != bits
                || declaration
                    .evaluated_value
                    .as_ref()
                    .and_then(|value| value.encoding())
                    != Some(ScalarValue::Float { format, bits }.encoding()?.as_str())
            {
                return Err("floating constant literal bits or result encoding drifted".into());
            }
            return Ok(());
        }
        if original == materialized {
            return Err("floating constant computation lost its distinct materialized root".into());
        }
        let mut probe = typed.clone();
        let owner = super::invocations::append_probe(
            &mut probe,
            declaration.symbol,
            "@const-float-replay".to_owned(),
            original,
            declaration.declared_type,
        );
        let machine = probe
            .machine(owner)
            .ok_or("floating constant lost its receiving probe")?;
        let [state] = probe.machine_states(machine) else {
            return Err("floating constant lost its receiving expression state".into());
        };
        let dependencies =
        validate_retained_initializer_call_custody(&probe, machine, state, declaration).map_err(
            |reason| {
                format!(
                    "floating call custody for declaration {:?}, roots {original:?}/{materialized:?}, probe {owner:?}: {reason}",
                    declaration.symbol,
                )
            },
        )?;
        validate_float_value(
            &probe,
            declaration,
            destination,
            owner,
            authority.as_deref(),
            None,
            &dependencies,
        )
    }

    fn validate_float_value(
        probe: &TypedTrees,
        declaration: &typed_trees::constant::ConstDeclaration,
        destination: PrimitiveType,
        owner: symbols::SymbolHandle,
        authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
        scalar_calls: Option<&dyn ConstantCalls>,
        dependencies: &[crate::const_evaluation::const_generic_expressions::DependencyValue],
    ) -> Result<(), String> {
        use crate::const_evaluation::const_generic_expressions::value::{
            ScalarValue, evaluate_scalar,
        };
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionIntrinsic as Intrinsic,
            AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
        };
        use numerics::literals::FloatFormat;

        let original = declaration.authored_initializer;
        let materialized = declaration.materialized_initializer;
        let machine = probe
            .machine(owner)
            .ok_or("floating constant lost its receiving probe")?;
        let [state] = probe.machine_states(machine) else {
            return Err("floating constant lost its receiving expression state".into());
        };
        crate::machine_execution::admission::require_const_expression_selection(
            probe,
            machine,
            declaration.initializer_source_span,
            authority,
        )?;
        let dependency_roots =
            validate_dependency_values(probe, dependencies, |expression, destination| {
                evaluate_scalar(probe, machine, state, expression, destination, scalar_calls)
                    .map(|(value, _)| value)
            })?;
        let (evaluated, _) =
            evaluate_scalar(probe, machine, state, original, destination, scalar_calls)?;

        let mut operators = Vec::new();
        let mut visited = Vec::new();
        let mut pending = vec![original];
        pending.extend(dependency_roots);
        while let Some(expression) = crate::worklist::unvisited(&mut pending, &mut visited) {
            if let Some(binary) = probe.expression_table.binary_node(expression) {
                if !has_builtin_binary_expression_meaning(probe, machine, Some(state), expression) {
                    return Err("floating constant replay lost selected builtin meaning".into());
                }
                let reference = probe.expression_table.source_span(expression);
                push_unique(&mut operators, reference);
                pending.push(binary.right);
                pending.push(binary.left);
            } else if let ExpressionNode::Match(dispatch) =
                probe.expression_table.expression(expression)
            {
                pending.push(dispatch.subject);
                for arm in probe.expression_table.match_arms(dispatch.arms) {
                    pending.push(arm.value);
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                }
            } else if let ExpressionNode::Call(call) = probe.expression_table.expression(expression)
            {
                if call.receiver.is_valid() {
                    pending.push(call.receiver);
                }
                pending.extend(
                    expression_table(probe)
                        .expression_handles(call.arguments)
                        .iter()
                        .copied(),
                );
            }
        }
        let mut retained_operators = Vec::new();
        for occurrence in expression_table(probe).authored_selection_occurrences(materialized) {
            let selection = probe
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or("floating constant lost its retained operator selection")?;
            if selection.kind() == Kind::Call {
                continue;
            }
            if selection.kind() == Kind::StaticPathSegment
                && let Target::Resolved(selected) = selection.target()
                && dependencies
                    .iter()
                    .any(|dependency| dependency.declaration == selected.selected_symbol())
            {
                continue;
            }
            if selection.kind() != Kind::Operator
                || selection.target() != Target::Intrinsic(Intrinsic::BuiltinOperator)
            {
                return Err("anonymous floating constant retained a non-builtin selection".into());
            }
            if !retained_operators.contains(&selection.source_span()) {
                retained_operators.push(selection.source_span());
            }
        }
        if operators.len() != retained_operators.len()
            || operators
                .iter()
                .any(|reference| !retained_operators.contains(reference))
        {
            return Err("floating constant authored and retained operator custody drifted".into());
        }
        if let ScalarValue::UndeterminedNaN(format) = &evaluated {
            if probe.expression_table.expression(materialized)
                != &ExpressionNode::UndeterminedNaN(*format)
                || declaration.evaluated_value.as_ref() != Some(&evaluated.declaration_value())
            {
                return Err(
                    "floating constant semantic result, format, or substituted value drifted"
                        .into(),
                );
            }
            return Ok(());
        }
        let ScalarValue::Float { format, bits } = &evaluated else {
            return Err(
                "floating constant replay did not produce the declared floating value".into(),
            );
        };
        let ExpressionNode::Float(literal) = probe.expression_table.expression(materialized) else {
            return Err("floating constant materialization is not a floating literal".into());
        };
        if literal.landing().is_some_and(|landing| landing != *format) {
            return Err("floating constant materialized format drifted".into());
        }
        let materialized_bits = match format {
            FloatFormat::F32 => u64::from(literal.value_f32().to_bits()),
            FloatFormat::F64 => literal.value_f64().to_bits(),
        };
        if materialized_bits != *bits
            || declaration.evaluated_value.as_ref() != Some(&evaluated.declaration_value())
        {
            return Err(
                "floating constant computation, materialized bits, or result encoding drifted"
                    .into(),
            );
        }
        Ok(())
    }

    fn float_literal_bits(
        program: &TypedTrees,
        expression: ExpressionHandle,
        format: numerics::literals::FloatFormat,
    ) -> Result<u64, String> {
        use numerics::literals::{FloatFormat, FloatLiteral};
        let literal = match program.expression_table.expression(expression) {
            ExpressionNode::Float(literal)
                if literal.landing().is_none_or(|landing| landing == format) =>
            {
                literal.clone()
            }
            ExpressionNode::Integer(literal) if literal.landing().is_none() => {
                let value = literal
                    .value_bignum()
                    .ok_or("floating constant has invalid anonymous integer value")?;
                FloatLiteral::parse(&value.to_string())
                    .ok_or("floating constant integer value cannot select format bits")?
            }
            _ => {
                return Err(
                    "floating constant literal root lost its carrier or retained computation"
                        .into(),
                );
            }
        };
        Ok(match format {
            FloatFormat::F32 => {
                let value = literal.value_f32();
                if value.is_nan() {
                    return Err(
                        "floating declaration identity requires explicit NaN representation bits"
                            .into(),
                    );
                }
                u64::from(value.to_bits())
            }
            FloatFormat::F64 => {
                let value = literal.value_f64();
                if value.is_nan() {
                    return Err(
                        "floating declaration identity requires explicit NaN representation bits"
                            .into(),
                    );
                }
                value.to_bits()
            }
        })
    }

    fn validate_dependency_values(
        program: &TypedTrees,
        dependencies: &[crate::const_evaluation::const_generic_expressions::DependencyValue],
        mut evaluate: impl FnMut(
            ExpressionHandle,
            PrimitiveType,
        ) -> Result<
            crate::const_evaluation::const_generic_expressions::value::ScalarValue,
            String,
        >,
    ) -> Result<Vec<ExpressionHandle>, String> {
        let mut authored_roots = Vec::new();
        for dependency in dependencies {
            let mut declarations = program
                .const_declarations()
                .iter()
                .filter(|declaration| declaration.symbol == dependency.declaration);
            let declaration = declarations
                .next()
                .ok_or("constant dependency lost its exact declaration")?;
            if declarations.next().is_some() {
                return Err("constant dependency declaration is ambiguous".into());
            }
            if let Some(destination @ (PrimitiveType::F32 | PrimitiveType::F64)) =
                scalar_probe_destination(program, declaration.declared_type)
            {
                if !expression_table(program).expression_is_valid(declaration.authored_initializer)
                    || expression_table(program).source_span(declaration.authored_initializer)
                        != declaration.initializer_source_span
                {
                    return Err("constant dependency lost its exact authored initializer".into());
                }
                let expected = declaration
                    .evaluated_value
                    .as_ref()
                    .ok_or("floating constant dependency lost its evaluated value")?;
                for expression in [declaration.authored_initializer, dependency.expression] {
                    let value = evaluate(expression, destination)?;
                    if !matches!(
                    value,
                    crate::const_evaluation::const_generic_expressions::value::ScalarValue::Float { .. }
                    | crate::const_evaluation::const_generic_expressions::value::ScalarValue::UndeterminedNaN(_)
                ) || &value.declaration_value() != expected
                {
                    return Err(
                        "floating constant dependency or its substituted use drifted".into(),
                    );
                }
                }
                push_unique(&mut authored_roots, declaration.authored_initializer);
                continue;
            }
            let expected = CanonicalConstIdentity {
                type_name: String::new(),
                encoding: declaration
                    .evaluated_value
                    .as_ref()
                    .and_then(|value| value.encoding())
                    .map(str::to_owned)
                    .ok_or("constant dependency lost its canonical value")?,
            }
            .decode_declaration_encoding()
            .ok_or("constant dependency has an invalid canonical value")?;
            if let Some(destination) = scalar_probe_destination(program, declaration.declared_type)
            {
                if !expression_table(program).expression_is_valid(declaration.authored_initializer)
                    || expression_table(program).source_span(declaration.authored_initializer)
                        != declaration.initializer_source_span
                {
                    return Err("constant dependency lost its exact authored initializer".into());
                }
                if !authored_roots.contains(&declaration.authored_initializer) {
                    let authored =
                        evaluate(declaration.authored_initializer, destination)?.into_index()?;
                    if authored.decode_encoding().as_ref() != Some(&expected) {
                        return Err(
                        "constant dependency authored computation drifted from its canonical value"
                            .into(),
                    );
                    }
                }
            }
            let mut leaves = Vec::new();
            pair(
                program,
                Leaf {
                    owner: declaration.symbol,
                    original: dependency.expression,
                    materialized: dependency.expression,
                    destination: declaration.declared_type,
                    structured: false,
                    expected,
                },
                &mut Vec::new(),
                &mut leaves,
            )?;
            for leaf in leaves {
                let actual = if leaf.structured {
                    dependency_leaf_value(program, leaf.materialized)?
                } else {
                    let destination = scalar_probe_destination(program, leaf.destination)
                        .ok_or("constant dependency lost its exact scalar destination")?;
                    decode_scalar(evaluate(leaf.materialized, destination)?)?
                };
                if actual != leaf.expected {
                    return Err(
                    "constant dependency or its substituted use drifted from its canonical value"
                        .into(),
                );
                }
            }
            if declaration.authored_initializer.is_valid()
                && !authored_roots.contains(&declaration.authored_initializer)
            {
                authored_roots.push(declaration.authored_initializer);
            }
        }
        Ok(authored_roots)
    }

    fn dependency_leaf_value(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Result<DecodedCanonicalConstValue, String> {
        let Some(name) = program.expression_table.name_path_node(expression) else {
            return Err("aggregate dependency leaf is not an exact constant or case use".into());
        };
        match program.symbols.get(name.symbol).kind {
            symbols::SymbolKind::Const => {
                let mut declarations = program
                    .const_declarations()
                    .iter()
                    .filter(|declaration| declaration.symbol == name.symbol);
                let declaration = declarations
                    .next()
                    .ok_or("constant dependency lost its exact declaration")?;
                if declarations.next().is_some() {
                    return Err("constant dependency declaration is ambiguous".into());
                }
                CanonicalConstIdentity {
                    type_name: String::new(),
                    encoding: declaration
                        .evaluated_value
                        .as_ref()
                        .and_then(|value| value.encoding())
                        .map(str::to_owned)
                        .ok_or("constant dependency lost its canonical value")?,
                }
                .decode_declaration_encoding()
                .ok_or_else(|| "constant dependency has an invalid canonical value".to_owned())
            }
            symbols::SymbolKind::Variant => {
                let owner = program.symbols.get(name.symbol).parent;
                let definition = program
                    .data_definition_by_symbol(owner)
                    .ok_or("case dependency lost its exact owner declaration")?;
                Ok(DecodedCanonicalConstValue::Variant {
                    type_name: definition.name.as_str().to_owned(),
                    case_name: program.symbols.name(name.symbol).to_owned(),
                    fields: Vec::new(),
                })
            }
            _ => Err("aggregate dependency leaf is not an exact constant or case use".into()),
        }
    }

    fn pair(
        program: &TypedTrees,
        mut leaf: Leaf,
        active: &mut Vec<ExpressionHandle>,
        leaves: &mut Vec<Leaf>,
    ) -> Result<(), String> {
        let table = &program.expression_table;
        if !table.expression_is_valid(leaf.original)
            || !table.expression_is_valid(leaf.materialized)
            || active.contains(&leaf.original)
            || table.source_span(leaf.original) != table.source_span(leaf.materialized)
        {
            return Err("retained initializer lost its exact authored/materialized roots".into());
        }
        if let Some(destination) = scalar_probe_destination(program, leaf.destination) {
            match (&leaf.expected, destination) {
                (DecodedCanonicalConstValue::Boolean(_), PrimitiveType::Bool) => {}
                (
                    DecodedCanonicalConstValue::Float {
                        format: numerics::literals::FloatFormat::F32,
                        ..
                    },
                    PrimitiveType::F32,
                )
                | (
                    DecodedCanonicalConstValue::Float {
                        format: numerics::literals::FloatFormat::F64,
                        ..
                    },
                    PrimitiveType::F64,
                ) => {}
                (DecodedCanonicalConstValue::Integer { type_name, .. }, primitive)
                    if type_name == primitive.name() => {}
                _ => {
                    return Err(
                        "retained initializer canonical leaf has a different declared carrier"
                            .into(),
                    );
                }
            }
            leaves.push(leaf);
            return Ok(());
        }
        let copied_aggregate = leaf.original != leaf.materialized && matches!(
        leaf.expected,
        DecodedCanonicalConstValue::Array { .. }
            | DecodedCanonicalConstValue::Record { .. }
            | DecodedCanonicalConstValue::Variant { .. }
    ) && table.authored_selection_occurrences(leaf.original).any(|occurrence| {
        program.authored_declaration_selections().get(occurrence).is_some_and(|selection| {
            let language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target() else {
                return false;
            };
            selection.source_span() == table.source_span(leaf.original)
                && program.symbols.get(selected.selected_symbol()).kind == symbols::SymbolKind::Const
        })
    });
        if copied_aggregate {
            leaf.structured = true;
            leaves.push(leaf);
            return Ok(());
        }
        active.push(leaf.original);
        match (
            table.expression(leaf.original),
            table.expression(leaf.materialized),
            type_reference_table(program).type_reference(leaf.destination),
            &leaf.expected,
        ) {
            (
                ExpressionNode::ArrayLiteral(original),
                ExpressionNode::ArrayLiteral(materialized),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                },
                DecodedCanonicalConstValue::Array { values, type_name },
            ) => {
                if type_name != &super::materialize::type_label(program, leaf.destination)?
                    || original.len() != *length
                    || materialized.len() != *length
                {
                    return Err("retained initializer array carrier or span drifted".into());
                }
                let originals = table.expression_handles(*original);
                let materialized = table.expression_handles(*materialized);
                if originals.len() != *length
                    || materialized.len() != *length
                    || values.len() != *length
                {
                    return Err("retained initializer array shape drifted".into());
                }
                for ((original, materialized), expected) in
                    originals.iter().zip(materialized).zip(values)
                {
                    pair(
                        program,
                        Leaf {
                            owner: leaf.owner,
                            original: *original,
                            materialized: *materialized,
                            destination: *element_type,
                            structured: false,
                            expected: expected.clone(),
                        },
                        active,
                        leaves,
                    )?;
                }
            }
            (
                ExpressionNode::StructLiteral(original),
                ExpressionNode::StructLiteral(materialized),
                _,
                expected,
            ) => {
                let symbol = program.type_reference_table.type_symbol(leaf.destination);
                if !symbol.is_valid()
                    || original.type_symbol != symbol
                    || materialized.type_symbol != symbol
                    || original.case_symbol != materialized.case_symbol
                {
                    return Err("retained initializer constructor identity drifted".into());
                }
                let definition = program
                    .data_definition_by_symbol(symbol)
                    .ok_or("retained initializer constructor has no exact declaration")?;
                let type_name = match expected {
                    DecodedCanonicalConstValue::Record { type_name, .. }
                    | DecodedCanonicalConstValue::Variant { type_name, .. } => type_name,
                    _ => return Err("retained canonical value is not a nominal constructor".into()),
                };
                if type_name != definition.name.as_str()
                    || program.data_members(definition).len() != definition.members.len()
                {
                    return Err(
                        "retained initializer nominal carrier or member span drifted".into(),
                    );
                }
                let (fields, expected) = match (original.case_symbol, expected) {
                    (None, DecodedCanonicalConstValue::Record { fields, .. }) => (
                        program
                            .data_members(definition)
                            .iter()
                            .filter_map(|member| member.as_field())
                            .collect::<Vec<_>>(),
                        fields,
                    ),
                    (
                        Some(case),
                        DecodedCanonicalConstValue::Variant {
                            case_name, fields, ..
                        },
                    ) => {
                        let variant = program
                            .data_members(definition)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(variant)
                                    if variant.symbol == case
                                        && variant.name.as_str() == case_name =>
                                {
                                    Some(variant)
                                }
                                _ => None,
                            })
                            .ok_or("retained initializer case identity drifted")?;
                        if program.data_payload_fields(variant).len() != variant.payload.len() {
                            return Err("retained initializer case payload span drifted".into());
                        }
                        (
                            program.data_payload_fields(variant).iter().collect(),
                            fields,
                        )
                    }
                    _ => {
                        return Err(
                            "retained initializer canonical constructor shape drifted".into()
                        );
                    }
                };
                let originals = table.struct_fields(original.fields);
                if originals.len() != original.fields.len()
                    || table.struct_fields(materialized.fields).len() != materialized.fields.len()
                {
                    return Err("retained initializer constructor field span drifted".into());
                }
                let materialized = table.struct_fields(materialized.fields);
                if originals.len() != fields.len()
                    || materialized.len() != fields.len()
                    || expected.len() != fields.len()
                {
                    return Err("retained initializer field roster drifted".into());
                }
                for (field, (name, expected)) in fields.iter().zip(expected) {
                    if field.name.as_str() != name {
                        return Err("retained canonical field order drifted".into());
                    }
                    let matching_originals = originals
                        .iter()
                        .filter(|candidate| candidate.field_symbol == field.symbol)
                        .collect::<Vec<_>>();
                    let [original] = matching_originals.as_slice() else {
                        return Err("retained initializer lost an exact original field".into());
                    };
                    let matching_materialized = materialized
                        .iter()
                        .filter(|candidate| candidate.field_symbol == field.symbol)
                        .collect::<Vec<_>>();
                    let [materialized] = matching_materialized.as_slice() else {
                        return Err("retained initializer lost an exact materialized field".into());
                    };
                    if original.name != field.name || materialized.name != field.name {
                        return Err(
                            "retained initializer field metadata differs from its declaration"
                                .into(),
                        );
                    }
                    pair(
                        program,
                        Leaf {
                            owner: leaf.owner,
                            original: original.value,
                            materialized: materialized.value,
                            destination: field.type_reference,
                            structured: false,
                            expected: expected.clone(),
                        },
                        active,
                        leaves,
                    )?;
                }
            }
            _ => {
                if matches!(
                    leaf.expected,
                    DecodedCanonicalConstValue::Array { .. }
                        | DecodedCanonicalConstValue::Record { .. }
                        | DecodedCanonicalConstValue::Variant { .. }
                ) {
                    leaf.structured = true;
                    leaves.push(leaf);
                    active.pop();
                    return Ok(());
                }
                return Err(
                    "retained initializer has no checked aggregate leaf correspondence".into(),
                );
            }
        }
        active.pop();
        Ok(())
    }

    fn decode_scalar(
        value: crate::const_evaluation::const_generic_expressions::value::ScalarValue,
    ) -> Result<DecodedCanonicalConstValue, String> {
        CanonicalConstIdentity {
            type_name: String::new(),
            encoding: value.encoding()?,
        }
        .decode_declaration_encoding()
        .ok_or_else(|| "constant declaration has an invalid scalar value".to_owned())
    }
}
mod target_observations {
    use super::invocations::CheckedInitializers;
    use crate::build_time_evaluation::{EvaluationDomain, expression_table};
    use crate::const_evaluation::const_generic_expressions::{scalar_probe_destination, value};
    use diagnostics::Diagnostic;
    use std::sync::Arc;
    use typed_trees::TypedTrees;

    use typed_trees::constant_recipes::create_evaluation_entry;
    pub(crate) fn retain_entries(typed: &mut TypedTrees, domain: &EvaluationDomain) {
        let declarations = typed
            .roots
            .const_declarations
            .handles()
            .filter_map(|handle| {
                let declaration = typed.tables.const_declarations.get(handle);
                (domain.contains(declaration.symbol)
                    && declaration.evaluated_value.is_none()
                    && !declaration.evaluation_entry.is_valid()
                    && scalar_probe_destination(typed, declaration.declared_type).is_some())
                .then(|| (handle, declaration.clone()))
            })
            .collect::<Vec<_>>();
        for (handle, declaration) in declarations {
            let expression = typed
                .expression_table
                .insert_copy(declaration.materialized_initializer);
            let entry = create_evaluation_entry(
                typed,
                declaration.symbol,
                format!("@constant-{}", typed.symbols.name(declaration.symbol)),
                expression,
                declaration.declared_type,
            );
            let entry = typed.tables.machines.insert(entry);
            let declaration = typed.tables.const_declarations.get_mut(handle);
            declaration.evaluation_entry = entry;
            declaration.evaluation_expression = expression;
        }
    }

    pub(crate) fn settle(
        typed: &mut TypedTrees,
        authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
        domain: &EvaluationDomain,
        selected: crate::SelectedBuildTimeOperators<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut declarations = typed
            .const_declarations()
            .iter()
            .filter(|declaration| {
                domain.contains(declaration.symbol) && declaration.evaluated_value.is_none()
            })
            .filter_map(|declaration| {
                scalar_probe_destination(typed, declaration.declared_type)
                    .map(|destination| (declaration.clone(), destination))
            })
            .collect::<Vec<_>>();
        if declarations.is_empty() {
            return Ok(());
        }
        while !validation::resolve_named_result_overloads(typed)?.is_empty() {}
        typed_trees_to_typed_trees::bind_pre_specialization_authored_selections(typed)
            .map_err(|diagnostic| vec![diagnostic])?;
        while !declarations.is_empty() {
            let mut ready = None;
            for (ordinal, (declaration, _)) in declarations.iter().enumerate() {
                if !typed.tables.machines.is_valid(declaration.evaluation_entry) {
                    return Err(super::failure(
                        declaration.initializer_source_span,
                        "deferred constant lost its source-owned evaluation entry",
                    ));
                }
                let mut source_probes = typed.clone();
                let entry = source_probes
                    .tables
                    .machines
                    .get(declaration.evaluation_entry)
                    .clone();
                let owner = entry.symbol;
                source_probes.push_machine(entry);
                let execution_probes =
                    crate::machine_execution::selected_operators::prepare_selected_execution(
                        &source_probes,
                        selected.provider_bodies,
                        selected.operators,
                    )
                    .map_err(|reason| super::failure(declaration.initializer_source_span, reason))?
                    .unwrap_or_else(|| source_probes.clone());
                let mut work = domain.clone();
                work.include(owner);
                let work = work.demanded_roots(&execution_probes, &[owner]);
                let references = work.references(&execution_probes);
                let pending_dependency =
                    references
                        .authored_selection_occurrences
                        .iter()
                        .any(|occurrence| {
                            execution_probes
                                .authored_declaration_selections()
                                .get(*occurrence)
                                .and_then(|selection| {
                                    selection
                                        .target()
                                        .as_resolved()
                                        .map(|selected| selected.selected_symbol())
                                })
                                .and_then(|symbol| {
                                    execution_probes.const_declaration_by_symbol(symbol)
                                })
                                .is_some_and(|dependency| dependency.evaluated_value.is_none())
                        });
                if !pending_dependency {
                    ready = Some((ordinal, source_probes, execution_probes, owner, work));
                    break;
                }
            }
            let Some((ordinal, mut source_probes, mut execution_probes, owner, work)) = ready
            else {
                return Err(super::failure(
                    declarations[0].0.initializer_source_span,
                    "target constant dependency cycle or unavailable canonical input",
                ));
            };
            let (declaration, destination) = declarations.remove(ordinal);
            let failure =
                |reason: String| super::failure(declaration.initializer_source_span, reason);
            for root in [
                declaration.authored_initializer,
                declaration.materialized_initializer,
                declaration.evaluation_expression,
            ] {
                if root.is_valid() {
                    crate::const_evaluation::const_generic_expressions::retain_current_initializer_calls(
                    &mut source_probes, owner, root,
                ).map_err(&failure)?;
                }
            }
            for root in [
                declaration.authored_initializer,
                declaration.materialized_initializer,
                declaration.evaluation_expression,
            ] {
                if !root.is_valid() {
                    continue;
                }
                for occurrence in
                    expression_table(&source_probes).authored_selection_occurrences(root)
                {
                    let selection = source_probes
                        .authored_declaration_selections()
                        .get(occurrence)
                        .ok_or_else(|| {
                            failure("target constant lost its call roster selection".into())
                        })?;
                    if selection.kind() != language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call {
                    continue;
                }
                    if typed.authored_declaration_selections().get(occurrence) != Some(selection)
                        || execution_probes
                            .authored_declaration_selections()
                            .get(occurrence)
                            != Some(selection)
                    {
                        return Err(failure(
                        "target constant call roster contains a selection outside its source graph"
                            .into(),
                    ));
                    }
                    typed
                        .expression_table
                        .attach_authored_selection_occurrences(root, [occurrence]);
                    execution_probes
                        .expression_table
                        .attach_authored_selection_occurrences(root, [occurrence]);
                }
            }
            let work = work.demanded_roots(&execution_probes, &[owner]);
            let prepared =
                crate::PreparedBuildMachineProgram::prepare_in(&execution_probes, &work)?;
            let checked = CheckedInitializers::prepare(
                &prepared,
                authority.clone(),
                &[owner],
                &work,
                selected.operators,
            )?;
            let execution = checked.typed();
            let machine = execution.machine(owner).ok_or_else(|| {
                failure("target constant lost its private evaluation owner".into())
            })?;
            let [state] = execution.machine_states(machine) else {
                return Err(failure(
                    "target constant lost its private value exit".into(),
                ));
            };
            let calls = checked
                .calls_for_retained_initializer(owner, declaration.symbol)
                .map_err(&failure)?;
            let (value, warnings) = value::evaluate_scalar_with_policy(
                execution,
                machine,
                state,
                declaration.evaluation_expression,
                destination,
                execution.arithmetic_domain_for_type_reference(declaration.declared_type),
                Some(&calls),
            )
            .map_err(&failure)?;
            crate::report_evaluation_diagnostics(&warnings)?;
            let completed = value.declaration_value();
            let materialized = match &value {
                value::ScalarValue::UndeterminedNaN(format) => {
                    typed_trees::expression::ExpressionNode::UndeterminedNaN(*format)
                }
                _ => super::invocations::scalar_snapshot(&value, destination).map_err(&failure)?,
            };
            let uses = expression_table(typed)
                .iter_expressions()
                .filter_map(|(expression, _)| {
                    let reference = typed.expression_table.source_span(expression);
                    expression_table(typed)
                        .authored_selection_occurrences(expression)
                        .any(|occurrence| {
                            typed
                                .authored_declaration_selections()
                                .get(occurrence)
                                .is_some_and(|selection| {
                                    selection.source_span() == reference
                                        && selection.target().as_resolved().is_some_and(
                                            |selected| {
                                                selected.selected_symbol() == declaration.symbol
                                            },
                                        )
                                })
                        })
                        .then_some(expression)
                })
                .collect::<Vec<_>>();
            for expression in uses {
                *typed.expression_table.expression_mut(expression) = materialized.clone();
            }
            typed
                .tables
                .const_declarations
                .for_each_mut(|_, candidate| {
                    if candidate.symbol == declaration.symbol {
                        candidate.evaluated_value = Some(completed.clone());
                    }
                });
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;

        #[test]
        fn constant_entries_separate_initializer_reads_from_their_value_job() {
            let tokens = source_files_to_tokens::Lexer::new(
                "data Cell { code: u64; } machine bytes() -> u64 { size_of<Cell>() }",
            )
            .tokenize()
            .expect("lex");
            let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
            let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
            )
            .expect("resolve");
            let mut typed = lower_symbol_resolved_trees(&resolved).expect("type");
            let machine = typed
                .realized_machine_named("bytes")
                .expect("query source")
                .clone();
            let declared_type = typed.machine_states(&machine)[0].return_type;
            let expression = typed.expression_table.iter_expressions().find_map(|(handle, expression)| {
            matches!(expression, typed_trees::expression::ExpressionNode::Call(call) if call.layout_query.is_some()).then_some(handle)
        }).expect("layout query");
            let symbol = typed.symbols.insert_generated_root_from(
                machine.symbol,
                symbols::SymbolKind::Const,
                "BYTES",
            );
            typed.push_const_declaration(typed_trees::constant::ConstDeclaration {
                symbol,
                declared_type,
                authored_initializer: expression,
                materialized_initializer: expression,
                pending_target_observation: true,
                ..Default::default()
            });
            retain_entries(&mut typed, &EvaluationDomain::all());
            let declaration = typed
                .const_declarations()
                .iter()
                .find(|declaration| declaration.evaluation_entry.is_valid())
                .expect("retained initializer");
            assert_ne!(
                declaration.materialized_initializer,
                declaration.evaluation_expression
            );
            let references = typed_trees::references::collect_selected_program_references(
                &typed,
                &typed_trees::references::DeclarationSelection::from_symbols([declaration.symbol]),
            );
            assert!(
                references
                    .expressions
                    .contains(&declaration.evaluation_expression),
                "the declaration owns the detached entry's query occurrences"
            );
            let entry = typed
                .constant_evaluation_for_expression(
                    declaration.materialized_initializer,
                    declaration.symbol,
                )
                .expect("exact source context")
                .expect("source names the value job");
            assert_eq!(
                entry.evaluation_expression,
                declaration.evaluation_expression
            );
            assert!(
                typed
                    .constant_evaluation_for_expression(
                        declaration.evaluation_expression,
                        symbols::SymbolHandle::invalid()
                    )
                    .expect("body is a distinct occurrence")
                    .is_none(),
                "entry body must not read its own value job"
            );
        }
    }
}

pub(crate) use invocations::CheckedInitializers;
pub(crate) use replay::validate as validate_retained_invocations;
pub(crate) use target_observations::retain_entries as retain_deferred_entries;
pub(crate) use target_observations::settle as settle_target_observations;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dependency<'input> {
    Current(usize),
    Retained {
        declaration: &'input symbol_resolved_trees::constant::ConstDeclaration,
        source: SourceSpan,
    },
}

impl Dependency<'_> {
    fn current(self) -> Option<usize> {
        match self {
            Self::Current(ordinal) => Some(ordinal),
            Self::Retained { .. } => None,
        }
    }

    fn is_ready(self, declarations: &[Declaration<'_>]) -> bool {
        match self {
            Self::Current(ordinal) => {
                declarations[ordinal].leaves.is_empty()
                    && declarations[ordinal].evaluated_value.is_some()
            }
            Self::Retained { declaration, .. } => declaration.evaluated_value.is_some(),
        }
    }

    fn origin(
        self,
        reference: SourceSpan,
        syntax: &SyntaxTrees,
        declarations: &[Declaration<'_>],
    ) -> Result<ConstArgumentOrigin, Vec<Diagnostic>> {
        let (declaration, initializer, value) = match self {
            Self::Current(ordinal) => {
                let input = &declarations[ordinal];
                (
                    input.definition.name.source_span(),
                    syntax.expressions.source_span(input.definition.value),
                    &input.evaluated_value,
                )
            }
            Self::Retained {
                declaration,
                source,
            } => (
                source,
                declaration.initializer_source_span,
                &declaration.evaluated_value,
            ),
        };
        Ok(ConstArgumentOrigin {
            reference,
            declaration,
            initializer,
            evaluated_value: value
                .clone()
                .ok_or_else(|| failure(reference, "custody dependency has no completed value"))?,
        })
    }

    fn custody(
        self,
        declarations: &[Declaration<'_>],
        preparation: &ConstInitializerSelection,
        purpose: InitializerCustodyPurpose,
    ) -> Result<ConstInitializerCustody, Vec<Diagnostic>> {
        match self {
            Self::Retained { declaration, .. } => {
                preparation.retained_constant_custody(declaration.symbol, purpose)
            }
            Self::Current(ordinal) => Ok(match &declarations[ordinal].definition.normalization {
                None => ConstInitializerCustody::default(),
                Some(receipt) => match purpose {
                    InitializerCustodyPurpose::Authored => receipt.authored_custody.clone(),
                    InitializerCustodyPurpose::Declaration => receipt.declaration_custody.clone(),
                    InitializerCustodyPurpose::Value => ConstInitializerCustody {
                        selections: receipt.selections.clone(),
                        builtin_operators: receipt.builtin_operators.clone(),
                        call_selections: receipt.call_selections.clone(),
                    },
                },
            }),
        }
    }
}

fn dependency<'input>(
    declarations: &[Declaration<'input>],
    preparation: &'input ConstInitializerSelection,
    source: SourceSpan,
    reference: SourceSpan,
) -> Result<Dependency<'input>, Vec<Diagnostic>> {
    if let Some(ordinal) = declarations
        .iter()
        .position(|candidate| candidate.definition.name.source_span() == source)
    {
        return Ok(Dependency::Current(ordinal));
    }
    let selected = preparation
        .trees()
        .const_declaration_at_source_span(source)
        .ok_or_else(|| {
            failure(
                reference,
                "selected constant dependency lost its declaration",
            )
        })?;
    let declaration = preparation.retained_constant(selected.symbol)?;
    if !declaration.is_public
        && !preparation
            .trees()
            .symbols
            .same_source_package(reference, source)
    {
        return Err(failure(
            reference,
            "dependency selects a private foreign declaration",
        ));
    }
    Ok(Dependency::Retained {
        declaration,
        source,
    })
}

struct PendingLeaf<'input> {
    expression: ExpressionHandle,
    destination: syntax_trees::types::TypeReferenceHandle,
    structured: bool,
    dependencies: Vec<(SourceSpan, Dependency<'input>)>,
    calls: Vec<(SourceSpan, SourceSpan)>,
}

struct Declaration<'input> {
    item: ItemHandle,
    definition: ConstDefinition,
    evaluated_value: Option<EvaluatedConstant>,
    leaves: Vec<PendingLeaf<'input>>,
    dependencies: Vec<(SourceSpan, Dependency<'input>)>,
    calls: Vec<(SourceSpan, SourceSpan)>,
    authored_dependencies: Vec<(SourceSpan, Dependency<'input>)>,
    authored_calls: Vec<(SourceSpan, SourceSpan)>,
    authored_operators: Vec<SourceSpan>,
    readiness_operators: Vec<SourceSpan>,
    normalization_owners: Vec<SourceSpan>,
}

fn propagate_target_observations(
    declarations: &mut [Declaration<'_>],
    syntax: &mut SyntaxTrees,
    target_observations: &mut [bool],
) {
    loop {
        let mut changed = false;
        for (ordinal, declaration) in declarations.iter().enumerate() {
            if !target_observations[ordinal]
                && declaration.dependencies.iter().any(|(_, dependency)| {
                    dependency
                        .current()
                        .is_some_and(|ordinal| target_observations[ordinal])
                })
            {
                target_observations[ordinal] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for (ordinal, declaration) in declarations.iter_mut().enumerate() {
        declaration.definition.pending_target_observation = target_observations[ordinal];
        if target_observations[ordinal] {
            syntax.items.replace_item(
                declaration.item,
                Item::Const(declaration.definition.clone()),
            );
        }
    }
}

pub(crate) fn evaluate(
    mut syntax: SyntaxTrees,
    context: &crate::BuildTimeSources<'_>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let sources = context.sources.clone();
    let bindings = context.source_scoped_top_level_bindings;
    let authority = context.selection_authority.clone();
    use syntax_trees_to_symbol_resolved_trees::pre_resolution::requires_const_initializer_evaluation;
    syntax = crate::const_evaluation::const_qualifications::prove_ready(syntax, context)?;
    if !syntax.root_items().any(|item| {
        matches!(item, Item::Const(definition)
        if requires_const_initializer_evaluation(&syntax, definition))
    }) {
        return Ok(syntax);
    }
    let crate::syntax_probes::InitializerSelectionInputs {
        authored,
        probe: selection_syntax,
    } = crate::syntax_probes::initializer_selection_inputs(syntax, context)?;
    syntax = authored;
    let preparation =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::prepare_const_initializer_selection(
            context.resolution_request(selection_syntax),
        )?;
    let resolved = preparation.trees();
    let mut declarations = Vec::new();
    for item in syntax.root_item_handles() {
        let Item::Const(definition) = syntax.root_item(*item) else {
            continue;
        };
        let retained = resolved
            .const_declarations
            .iter()
            .find(|declaration| {
                resolved.symbols.symbol_source_span(declaration.symbol)
                    == Some(definition.name.source_span())
            })
            .ok_or_else(|| {
                failure(
                    definition.name.source_span(),
                    "initializer lost its exact declaration",
                )
            })?;
        let leaves = preparation
            .pending_leaves(&syntax, definition)?
            .into_iter()
            .map(|leaf| PendingLeaf {
                expression: leaf.expression,
                destination: leaf.destination,
                structured: leaf.structured,
                dependencies: Vec::new(),
                calls: Vec::new(),
            })
            .collect();
        declarations.push(Declaration {
            item: *item,
            definition: definition.clone(),
            evaluated_value: retained.evaluated_value.clone(),
            leaves,
            dependencies: Vec::new(),
            calls: Vec::new(),
            authored_dependencies: Vec::new(),
            authored_calls: Vec::new(),
            authored_operators: Vec::new(),
            readiness_operators: Vec::new(),
            normalization_owners: Vec::new(),
        });
    }
    let mut target_observations = vec![false; declarations.len()];
    for ordinal in 0..declarations.len() {
        let selected =
            preparation.initializer_dependencies(&syntax, &declarations[ordinal].definition)?;
        target_observations[ordinal] = selected.pending_target_observation;
        let authored = preparation
            .initializer_authored_dependencies(&declarations[ordinal].definition, None)?;
        declarations[ordinal].authored_calls = authored.calls;
        declarations[ordinal].authored_operators = authored.initializer_operators;
        for (reference, source) in authored.constants {
            let dependency = dependency(&declarations, &preparation, source, reference)?;
            declarations[ordinal]
                .authored_dependencies
                .push((reference, dependency));
        }
        declarations[ordinal].readiness_operators = selected.initializer_operators;
        declarations[ordinal].calls = selected.calls;
        declarations[ordinal].normalization_owners = selected.normalization_owners;
        for (reference, declaration) in selected.constants {
            let dependency = dependency(&declarations, &preparation, declaration, reference)?;
            let selected = (reference, dependency);
            push_unique(&mut declarations[ordinal].dependencies, selected);
        }
        if declarations[ordinal].leaves.is_empty() {
            continue;
        }
        let leaf_count = declarations[ordinal].leaves.len();
        for leaf_ordinal in 0..leaf_count {
            let expression = declarations[ordinal].leaves[leaf_ordinal].expression;
            let selected = preparation.pending_leaf_dependencies(
                &syntax,
                &declarations[ordinal].definition,
                expression,
                None,
            )?;
            let mut dependencies = Vec::new();
            for (reference, declaration) in selected.constants {
                let dependency = dependency(&declarations, &preparation, declaration, reference)?;
                dependencies.push((reference, dependency));
            }
            declarations[ordinal].leaves[leaf_ordinal].dependencies = dependencies;
            declarations[ordinal].leaves[leaf_ordinal].calls = selected.calls;
        }
    }
    propagate_target_observations(&mut declarations, &mut syntax, &mut target_observations);
    reject_dependency_cycles(&declarations)?;
    let mut demanded = declarations
        .iter()
        .map(|declaration| {
            !declaration.definition.pending_target_observation && !declaration.leaves.is_empty()
        })
        .collect::<Vec<_>>();
    let mut pending = demanded
        .iter()
        .enumerate()
        .filter_map(|(ordinal, demanded)| demanded.then_some(ordinal))
        .collect::<Vec<_>>();
    while let Some(ordinal) = pending.pop() {
        for (_, dependency) in &declarations[ordinal].dependencies {
            if let Some(dependency) = dependency.current()
                && !demanded[dependency]
            {
                demanded[dependency] = true;
                pending.push(dependency);
            }
        }
    }
    'evaluation: while declarations
        .iter()
        .enumerate()
        .any(|(ordinal, declaration)| {
            demanded[ordinal]
                && (!declaration.leaves.is_empty() || declaration.evaluated_value.is_none())
        })
    {
        let mut ready = declarations
            .iter()
            .enumerate()
            .filter_map(|(ordinal, declaration)| {
                (demanded[ordinal]
                    && (!declaration.leaves.is_empty() || declaration.evaluated_value.is_none())
                    && declaration
                        .dependencies
                        .iter()
                        .all(|(_, dependency)| dependency.is_ready(&declarations)))
                .then_some(ordinal)
            })
            .collect::<Vec<_>>();
        if ready.is_empty() {
            let ordinal = declarations
                .iter()
                .enumerate()
                .find_map(|(ordinal, declaration)| {
                    (demanded[ordinal]
                        && (!declaration.leaves.is_empty()
                            || declaration.evaluated_value.is_none()))
                    .then_some(ordinal)
                })
                .ok_or_else(|| {
                    failure(
                        SourceSpan::default(),
                        "initializer readiness lost its pending declaration",
                    )
                })?;
            return Err(failure(
                declarations[ordinal].definition.name.source_span(),
                "constant initializer dependency cycle",
            ));
        }
        for ordinal in &ready {
            if declarations[*ordinal].leaves.is_empty() {
                let value = preparation
                    .canonicalize_initializer_payload(&syntax, &declarations[*ordinal].definition)
                    .map_err(|reason| {
                        failure(declarations[*ordinal].definition.name.source_span(), reason)
                    })?;
                declarations[*ordinal].definition.pending_qualification = value.pending_destination;
                syntax.items.replace_item(
                    declarations[*ordinal].item,
                    Item::Const(declarations[*ordinal].definition.clone()),
                );
                declarations[*ordinal].evaluated_value = Some(value.value.encoding.into());
            }
        }
        ready.retain(|ordinal| !declarations[*ordinal].leaves.is_empty());
        if ready.is_empty() {
            continue;
        }
        let mut ready_declarations = ready
            .iter()
            .map(|ordinal| declarations[*ordinal].item)
            .collect::<Vec<_>>();
        let owners = ready
            .iter()
            .flat_map(|ordinal| declarations[*ordinal].normalization_owners.iter().copied())
            .collect::<Vec<_>>();
        extend_normalization_owners(&syntax, &mut ready_declarations, &owners)?;
        syntax = crate::const_evaluation::const_generic_calls::evaluate_declaration_calls(
            syntax,
            context.clone(),
            &ready_declarations,
        )?;
        let mut probe = syntax.clone();
        crate::const_evaluation::const_qualifications::defer_unproved(&mut probe);
        let declaration_frontier = probe.root_item_handles().len();
        let mut probe_sources = Vec::new();
        for ordinal in &ready {
            for leaf in &declarations[*ordinal].leaves {
                crate::const_evaluation::const_generic_expressions::append_probe(
                    &mut probe,
                    probe_sources.len(),
                    leaf.expression,
                    leaf.destination,
                );
                probe_sources.push(syntax.expressions.source_span(leaf.expression));
            }
        }
        let mut probe_declarations = probe.root_item_handles()[declaration_frontier..].to_vec();
        let owners = ready
            .iter()
            .flat_map(|ordinal| declarations[*ordinal].normalization_owners.iter().copied())
            .collect::<Vec<_>>();
        extend_normalization_owners(&probe, &mut probe_declarations, &owners)?;
        let probe = crate::syntax_probes::normalize_initializer_probe_data(
            probe,
            sources.clone(),
            bindings,
            context.retained_base(),
            [
                context.declaration_syntax,
                context.retained_declaration_syntax,
            ],
            &probe_declarations,
        )?;
        let frontier = resolve_deferred_initializer_probe(context.resolution_request(probe))?;
        let (typed, domain) = crate::syntax_probes::prepare_evaluation(
            frontier,
            &probe_sources,
            &[],
            &[],
            context.predecessor.clone(),
        )?;
        let prepared = crate::PreparedBuildMachineProgram::prepare_in(&typed, &domain)?;
        let execution = prepared.typed();
        let operator_facts = validation::derive_pre_flow_operator_selections(execution);
        let admission = domain.admission(execution, authority.clone());
        let mut observations = Vec::new();
        let mut pending_choices = Vec::new();
        for ordinal in &ready {
            let mut choices = Vec::new();
            for leaf in &declarations[*ordinal].leaves {
                let reference = syntax.expressions.source_span(leaf.expression);
                let mut probes = execution.machines().iter().filter(|machine| {
                    execution.symbols.symbol_source_span(machine.symbol) == Some(reference)
                });
                let (Some(probe), None) = (probes.next(), probes.next()) else {
                    return Err(failure(
                        reference,
                        "initializer probe lost its unique source owner",
                    ));
                };
                target_observations[*ordinal] |= admission.closure_needs_execution_preparation(
                    execution,
                    probe.symbol,
                    &operator_facts,
                );
                let observation =
                    crate::const_evaluation::const_generic_expressions::observe_authored_probe(
                        &typed, reference, &syntax,
                    )
                    .map_err(|reason| failure(reference, reason))?;
                for choice in observation.calls() {
                    push_unique(&mut choices, *choice);
                }
                observations.push((leaf.expression, observation));
            }
            let definition = &declarations[*ordinal].definition;
            let selected = preparation.selected_initializer_dependencies(definition, &choices)?;
            let authored =
                preparation.initializer_authored_dependencies(definition, Some(&choices))?;
            target_observations[*ordinal] |= selected.pending_target_observation;
            if !selected.pending_calls.is_empty() || !authored.pending_calls.is_empty() {
                pending_choices.push(*ordinal);
            }
            let dependencies = selected
                .constants
                .iter()
                .map(|(reference, source)| {
                    dependency(&declarations, &preparation, *source, *reference)
                        .map(|dependency| (*reference, dependency))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let authored_dependencies = authored
                .constants
                .iter()
                .map(|(reference, source)| {
                    dependency(&declarations, &preparation, *source, *reference)
                        .map(|dependency| (*reference, dependency))
                })
                .collect::<Result<Vec<_>, _>>()?;
            declarations[*ordinal].dependencies = dependencies;
            declarations[*ordinal].authored_dependencies = authored_dependencies;
            declarations[*ordinal].calls = selected.calls;
            declarations[*ordinal].readiness_operators = selected.initializer_operators;
            declarations[*ordinal].normalization_owners = selected.normalization_owners;
            declarations[*ordinal].authored_calls = authored.calls;
            declarations[*ordinal].authored_operators = authored.initializer_operators;
            for leaf_ordinal in 0..declarations[*ordinal].leaves.len() {
                let leaf = &declarations[*ordinal].leaves[leaf_ordinal];
                let selected = preparation.pending_leaf_dependencies(
                    &syntax,
                    &declarations[*ordinal].definition,
                    leaf.expression,
                    Some(&choices),
                )?;
                let dependencies = selected
                    .constants
                    .iter()
                    .map(|(reference, source)| {
                        dependency(&declarations, &preparation, *source, *reference)
                            .map(|dependency| (*reference, dependency))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let leaf = &mut declarations[*ordinal].leaves[leaf_ordinal];
                leaf.dependencies = dependencies;
                leaf.calls = selected.calls;
            }
        }
        propagate_target_observations(&mut declarations, &mut syntax, &mut target_observations);
        reject_dependency_cycles(&declarations)?;
        for (ordinal, target_observation) in target_observations.iter().enumerate() {
            if *target_observation {
                demanded[ordinal] = false;
            }
        }
        ready.retain(|ordinal| demanded[*ordinal]);
        if ready.is_empty() {
            continue 'evaluation;
        }
        let mut discovered = Vec::new();
        for ordinal in &ready {
            for (_, dependency) in &declarations[*ordinal].dependencies {
                if let Some(dependency) = dependency.current() {
                    discovered.push(dependency);
                }
            }
        }
        while let Some(ordinal) = discovered.pop() {
            if !demanded[ordinal] {
                demanded[ordinal] = true;
                discovered.extend(
                    declarations[ordinal]
                        .dependencies
                        .iter()
                        .filter_map(|(_, dependency)| dependency.current()),
                );
            }
        }
        if ready.iter().any(|ordinal| {
            declarations[*ordinal]
                .dependencies
                .iter()
                .any(|(_, dependency)| !dependency.is_ready(&declarations))
        }) {
            continue 'evaluation;
        }
        if pending_choices
            .iter()
            .any(|ordinal| ready.contains(ordinal))
        {
            return Err(failure(
                SourceSpan::default(),
                "typed initializer did not settle its exact source call choices",
            ));
        }
        let mut authored_probes = Vec::new();
        for (expression, observation) in observations {
            if !ready.iter().any(|ordinal| {
                declarations[*ordinal]
                    .leaves
                    .iter()
                    .any(|leaf| leaf.expression == expression)
            }) {
                continue;
            }
            let leaf = ready
                .iter()
                .flat_map(|ordinal| &declarations[*ordinal].leaves)
                .find(|leaf| leaf.expression == expression)
                .ok_or_else(|| {
                    failure(
                        syntax.expressions.source_span(expression),
                        "initializer lost its authored leaf",
                    )
                })?;
            let expected = dependency_origins(
                &syntax,
                &declarations,
                &preparation,
                &leaf.dependencies,
                InitializerCustodyPurpose::Authored,
            )?;
            let custody = observation
                .validate(&expected, &leaf.calls)
                .map_err(|reason| failure(syntax.expressions.source_span(expression), reason))?;
            authored_probes.push((expression, custody));
        }
        let checked = ready
            .iter()
            .any(|ordinal| {
                declarations[*ordinal]
                    .leaves
                    .iter()
                    .any(|leaf| leaf.structured || !leaf.calls.is_empty())
            })
            .then(|| {
                let mut symbols = Vec::new();
                for ordinal in &ready {
                    for leaf in &declarations[*ordinal].leaves {
                        let reference = syntax.expressions.source_span(leaf.expression);
                        let matching = typed
                            .machines()
                            .iter()
                            .filter(|machine| {
                                typed.symbols.symbol_source_span(machine.symbol) == Some(reference)
                            })
                            .collect::<Vec<_>>();
                        let [machine] = matching.as_slice() else {
                            return Err(failure(
                                reference,
                                "initializer probe lost its unique source owner",
                            ));
                        };
                        symbols.push(machine.symbol);
                    }
                }
                let independent = domain.demanded_roots(&typed, &symbols);
                let prepared =
                    crate::PreparedBuildMachineProgram::prepare_in(&typed, &independent)?;
                invocations::CheckedInitializers::prepare(
                    &prepared,
                    authority.clone(),
                    &symbols,
                    &independent,
                    &[],
                )
            })
            .transpose()?;
        let typed = checked
            .as_ref()
            .map_or(&typed, invocations::CheckedInitializers::typed);
        for ordinal in ready {
            let authored_expression = declarations[ordinal].definition.value;
            let mut replacements = HashMap::new();
            let mut selections = Vec::new();
            let mut builtin_operators = Vec::new();
            let mut call_selections = Vec::new();
            let mut scalar_value = None;
            let mut structured_value = None;
            for leaf in &declarations[ordinal].leaves {
                let reference = syntax.expressions.source_span(leaf.expression);
                let authored = &authored_probes
                    .iter()
                    .find(|(expression, _)| *expression == leaf.expression)
                    .ok_or_else(|| {
                        failure(reference, "initializer lost its checked source custody")
                    })?
                    .1;
                let expected = dependency_origins(
                    &syntax,
                    &declarations,
                    &preparation,
                    &leaf.dependencies,
                    InitializerCustodyPurpose::Value,
                )?;
                if leaf.structured {
                    let checked = checked
                        .as_ref()
                        .expect("aggregate leaf has checked preparation");
                    let calls = checked
                        .calls_for_source(reference)
                        .map_err(|reason| failure(reference, reason))?;
                    let evaluation = calls
                        .evaluate_leaf(reference, authored, &syntax)
                        .map_err(|reason| failure(reference, reason))?;
                    for selection in &evaluation.calls {
                        push_unique(&mut call_selections, *selection);
                    }
                    extend_unique(&mut selections, evaluation.origins);
                    extend_unique(&mut builtin_operators, evaluation.operators);
                    if declarations[ordinal].leaves.len() == 1
                        && leaf.expression == authored_expression
                    {
                        let value = materialize::canonical_value(
                            typed,
                            evaluation.destination,
                            &evaluation.value,
                        )
                        .map_err(|reason| failure(reference, reason))?;
                        structured_value = Some(language_semantics::const_value::CanonicalConstIdentity::from_declaration_value(&value)
                            .ok_or_else(|| failure(reference, "structured declaration value exceeds canonical encoding limits"))?.encoding.into());
                    }
                    let materialized = materialize::literal(
                        &mut syntax,
                        typed,
                        evaluation.destination,
                        &evaluation.value,
                        reference,
                    )
                    .map_err(|reason| failure(reference, reason))?;
                    replacements.insert(leaf.expression, materialized);
                    crate::report_evaluation_diagnostics(&evaluation.warnings)?;
                    continue;
                }
                let result = if leaf.calls.is_empty() {
                    crate::const_evaluation::const_generic_expressions::evaluate_probe(
                        typed,
                        reference,
                        false,
                        authority.as_deref(),
                        Some(&expected),
                        &syntax,
                    )
                } else {
                    let checked = checked
                        .as_ref()
                        .expect("call-bearing declaration has checked preparation");
                    let calls = checked
                        .calls_for_source(reference)
                        .map_err(|reason| failure(reference, reason))?;
                    crate::const_evaluation::const_generic_expressions::evaluate_probe_with_calls(
                        typed,
                        reference,
                        false,
                        authority.as_deref(),
                        authored,
                        &syntax,
                        &calls,
                    )
                }
                .map_err(|reason| failure(reference, reason))?;
                for selection in &result.call_selections {
                    push_unique(&mut call_selections, *selection);
                }
                extend_unique(&mut selections, result.origins);
                extend_unique(&mut builtin_operators, result.operators);
                scalar_value = Some(result.value.declaration_value());
                use crate::const_evaluation::const_generic_expressions::value::ScalarValue;
                let literal = match result.value {
                    ScalarValue::UndeterminedNaN(format) => ExpressionNode::UndeterminedNaN(format),
                    ScalarValue::Float { format, bits } => {
                        let literal = numerics::literals::FloatLiteral::from_bits(format, bits)
                            .ok_or_else(|| failure(reference, "invalid floating representation"))?;
                        ExpressionNode::Float(FloatSpelling::materialized(literal, reference))
                    }
                    ScalarValue::Index(value) => match value.decode_encoding() {
                        Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
                            let spelling = value.to_string();
                            ExpressionNode::Integer(
                                numerics::literals::IntegerLiteral::from_parts(
                                    spelling.starts_with('-'),
                                    numerics::literals::IntegerRadix::Decimal,
                                    spelling.strip_prefix('-').unwrap_or(&spelling),
                                )
                                .map_err(|reason| failure(reference, reason))?,
                            )
                        }
                        Some(DecodedCanonicalConstValue::Boolean(value)) => {
                            ExpressionNode::Boolean(value)
                        }
                        _ => {
                            return Err(failure(
                                reference,
                                "initializer probe did not produce a scalar value",
                            ));
                        }
                    },
                };
                let materialized = syntax.expressions.insert(literal);
                syntax.expressions.set_source_span(materialized, reference);
                replacements.insert(leaf.expression, materialized);
                crate::report_evaluation_diagnostics(&result.warnings)?;
            }
            let value = if declarations[ordinal].leaves.len() == 1
                && authored_expression == declarations[ordinal].leaves[0].expression
            {
                *replacements
                    .get(&declarations[ordinal].leaves[0].expression)
                    .expect("scalar initializer replacement")
            } else {
                materialize_value(&mut syntax, authored_expression, &replacements)?
            };
            let mut definition = declarations[ordinal].definition.clone();
            let mut pending_destination = None;
            let result_value = if declarations[ordinal].leaves.len() == 1
                && authored_expression == declarations[ordinal].leaves[0].expression
                && !declarations[ordinal].leaves[0].structured
            {
                scalar_value.ok_or_else(|| {
                    failure(
                        syntax.expressions.source_span(authored_expression),
                        "scalar initializer lost its evaluated value",
                    )
                })?
            } else if let Some(value) = structured_value {
                value
            } else {
                let mut materialized_definition = definition.clone();
                materialized_definition.value = value;
                let payload = preparation
                    .canonicalize_initializer_payload(&syntax, &materialized_definition)
                    .map_err(|reason| {
                        failure(syntax.expressions.source_span(authored_expression), reason)
                    })?;
                pending_destination = payload.pending_destination;
                payload.value.encoding.into()
            };
            for (reference, dependency) in &declarations[ordinal].authored_dependencies {
                let origin = dependency.origin(*reference, &syntax, &declarations)?;
                push_unique(&mut selections, origin);
                let custody = dependency.custody(
                    &declarations,
                    &preparation,
                    InitializerCustodyPurpose::Value,
                )?;
                extend_unique(&mut selections, custody.selections);
                extend_unique(&mut builtin_operators, custody.builtin_operators);
                extend_unique(&mut call_selections, custody.call_selections);
            }
            let authored_custody = ConstInitializerCustody {
                selections: dependency_origins(
                    &syntax,
                    &declarations,
                    &preparation,
                    &declarations[ordinal].authored_dependencies,
                    InitializerCustodyPurpose::Authored,
                )?,
                builtin_operators: declarations[ordinal].authored_operators.clone(),
                call_selections: declarations[ordinal].authored_calls.clone(),
            };
            let declaration_custody = ConstInitializerCustody {
                selections: dependency_origins(
                    &syntax,
                    &declarations,
                    &preparation,
                    &declarations[ordinal].dependencies,
                    InitializerCustodyPurpose::Declaration,
                )?,
                builtin_operators: declarations[ordinal].readiness_operators.clone(),
                call_selections: declarations[ordinal].calls.clone(),
            };
            definition.normalization = Some(ConstInitializerNormalization {
                authored_expression,
                authored_custody,
                declaration_custody,
                result_value,
                selections,
                builtin_operators,
                call_selections,
            });
            definition.value = value;
            definition.qualification = None;
            if pending_destination.is_none()
                && preparation.initializer_requires_destination_proof(&syntax, &definition)
            {
                pending_destination = Some(syntax_trees::item::ConstDestinationObligation {
                    declaration: definition.name.source_span(),
                    destination: definition.type_reference,
                    initializer: value,
                });
            }
            definition.pending_qualification = pending_destination;
            syntax
                .items
                .replace_item(declarations[ordinal].item, Item::Const(definition.clone()));
            declarations[ordinal].definition = definition;
            declarations[ordinal].evaluated_value = Some(
                declarations[ordinal]
                    .definition
                    .normalization
                    .as_ref()
                    .unwrap()
                    .result_value
                    .clone(),
            );
            declarations[ordinal].leaves.clear();
        }
        syntax = crate::const_evaluation::const_qualifications::prove_ready(syntax, context)?;
        for declaration in &mut declarations {
            if let Item::Const(definition) = syntax.root_item(declaration.item) {
                declaration.definition = definition.clone();
            }
        }
    }
    Ok(syntax)
}

fn reject_dependency_cycles(declarations: &[Declaration<'_>]) -> Result<(), Vec<Diagnostic>> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Visit {
        Unseen,
        Active,
        Complete,
    }
    let mut visits = vec![Visit::Unseen; declarations.len()];
    let mut path = Vec::new();
    for root in 0..declarations.len() {
        if visits[root] != Visit::Unseen || declarations[root].evaluated_value.is_some() {
            continue;
        }
        visits[root] = Visit::Active;
        path.push((root, 0usize));
        while let Some((owner, next_dependency)) = path.last_mut() {
            let Some((_, dependency)) = declarations[*owner].dependencies.get(*next_dependency)
            else {
                visits[*owner] = Visit::Complete;
                path.pop();
                continue;
            };
            *next_dependency += 1;
            let Some(dependency) = dependency.current() else {
                continue;
            };
            if declarations[dependency].evaluated_value.is_some() {
                continue;
            }
            match visits[dependency] {
                Visit::Complete => {}
                Visit::Unseen => {
                    visits[dependency] = Visit::Active;
                    path.push((dependency, 0));
                }
                Visit::Active => {
                    let declaration = &declarations[dependency].definition;
                    return Err(failure(
                        declaration.name.source_span(),
                        if declaration.pending_target_observation {
                            "target constant dependency cycle or unavailable canonical input"
                        } else {
                            "constant initializer dependency cycle"
                        },
                    ));
                }
            }
        }
    }
    Ok(())
}

fn dependency_origins(
    syntax: &SyntaxTrees,
    declarations: &[Declaration<'_>],
    preparation: &ConstInitializerSelection,
    dependencies: &[(SourceSpan, Dependency<'_>)],
    purpose: InitializerCustodyPurpose,
) -> Result<Vec<ConstArgumentOrigin>, Vec<Diagnostic>> {
    let mut origins = Vec::new();
    for (reference, dependency) in dependencies {
        let origin = dependency.origin(*reference, syntax, declarations)?;
        push_unique(&mut origins, origin);
        let custody = dependency.custody(declarations, preparation, purpose)?;
        extend_unique(&mut origins, custody.selections);
    }
    Ok(origins)
}

fn failure(reference: SourceSpan, reason: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("constant initializer: {reason}")).with_source_span(reference)]
}

fn materialize_value(
    syntax: &mut SyntaxTrees,
    expression: ExpressionHandle,
    replacements: &HashMap<ExpressionHandle, ExpressionHandle>,
) -> Result<ExpressionHandle, Vec<Diagnostic>> {
    if let Some(replacement) = replacements.get(&expression) {
        return Ok(*replacement);
    }
    let source_span = syntax.expressions.source_span(expression);
    let value = match syntax.expressions.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(elements) => {
            let elements = syntax.expressions.expression_handles(elements).to_vec();
            let materialized = elements
                .into_iter()
                .map(|element| materialize_value(syntax, element, replacements))
                .collect::<Result<Vec<_>, _>>()?;
            ExpressionNode::ArrayLiteral(syntax.expressions.insert_expression_handles(materialized))
        }
        ExpressionNode::StructLiteral(mut constructor) => {
            let mut fields = syntax
                .expressions
                .struct_fields(constructor.fields)
                .to_vec();
            for field in &mut fields {
                field.value = materialize_value(syntax, field.value, replacements)?;
            }
            constructor.fields = syntax.expressions.insert_struct_fields(fields);
            ExpressionNode::StructLiteral(constructor)
        }
        _ => return Ok(expression),
    };
    let handle = syntax.expressions.insert(value);
    syntax.expressions.set_source_span(handle, source_span);
    Ok(handle)
}

/// Constants named by the types a replayed initializer is checked against.
struct TypeExtentConstants<'program> {
    program: &'program typed_trees::TypedTrees,
    visited: Vec<typed_trees::types::TypeReferenceHandle>,
    constants: Vec<symbols::SymbolHandle>,
}

impl Extend<symbols::SymbolHandle> for TypeExtentConstants<'_> {
    fn extend<T: IntoIterator<Item = symbols::SymbolHandle>>(&mut self, symbols: T) {
        for symbol in symbols {
            if symbol.is_valid()
                && self.program.symbols.get(symbol).kind == symbols::SymbolKind::Const
                && !self.constants.contains(&symbol)
            {
                self.constants.push(symbol);
            }
        }
    }
}

impl typed_trees::references::TypedReferences for TypeExtentConstants<'_> {
    fn type_reference(&mut self, reference: typed_trees::types::TypeReferenceHandle) -> bool {
        if self.visited.contains(&reference) {
            return false;
        }
        self.visited.push(reference);
        true
    }
}
