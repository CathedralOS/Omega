//! Invoke exact selected endpoint calls without preexecuting nested expressions.

use super::integer_type::ScalarPosition;
use crate::const_evaluation::const_generic_expressions::value::{self, ConstantCalls};
use crate::{BuildTimeAdmissionPlan, BuildTimeSelectionAuthority, BuildTimeValue};
use diagnostics::Diagnostic;
use language_semantics::const_value::{
    CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue,
};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    types::{PrimitiveType, TypeReferenceHandle},
};

pub(crate) struct Invocation<'program> {
    pub working: &'program TypedTrees,
    pub execution: &'program TypedTrees,
    pub admission: &'program BuildTimeAdmissionPlan,
    pub authority: Option<&'program dyn BuildTimeSelectionAuthority>,
}

impl Invocation<'_> {
    fn callee(&self, expression: ExpressionHandle) -> Result<super::EndpointCallee, String> {
        let (machine, static_application) =
            super::selected_endpoint_machine(self.working, expression)
                .ok_or("range endpoint call needs an exact closed selected machine")?;
        let endpoint = super::PendingEndpoint {
            expression,
            root: expression,
            machine: machine.symbol,
            static_application,
            contract_expression: false,
        };
        super::resolve_endpoint_callee(self.execution, &endpoint)
    }

    fn position(
        &self,
        callee: super::EndpointCallee,
        reference: TypeReferenceHandle,
    ) -> Result<ScalarPosition, String> {
        let types = if callee.static_application {
            self.execution
        } else {
            self.working
        };
        ScalarPosition::prepare(types, self.execution, reference, self.authority)
    }

    fn result_position(&self, callee: super::EndpointCallee) -> Result<ScalarPosition, String> {
        let machine = self
            .execution
            .machine(callee.instance)
            .ok_or("range endpoint lost its selected machine")?;
        let entry = self
            .execution
            .machine_states(machine)
            .first()
            .ok_or("range endpoint lost entry")?;
        self.position(callee, entry.return_type)
    }

    /// Evaluate the call's arguments and snapshot each to its literal
    /// spelling. The literal vector stays aligned with the call's argument
    /// span so the shared snapshot probe can substitute evaluated operands
    /// without re-evaluating them.
    fn arguments(
        &self,
        expression: ExpressionHandle,
        callee: super::EndpointCallee,
    ) -> Result<(Vec<BuildTimeValue>, Vec<ExpressionNode>, Vec<Diagnostic>), String> {
        let Some(call) = self.execution.expression_table.call_node(expression) else {
            return Err("range endpoint lost its call".into());
        };
        let machine = self
            .execution
            .machine(callee.instance)
            .ok_or("range endpoint lost its selected machine")?;
        let entry = self
            .execution
            .machine_states(machine)
            .first()
            .ok_or("range endpoint lost entry")?;
        let arguments = self
            .execution
            .expression_table
            .expression_handles(call.arguments);
        let parameters = self.execution.state_parameters(entry);
        if arguments.len() != parameters.len() {
            return Err(
                "range endpoint call argument count does not match its selected entry".into(),
            );
        }
        let mut values = Vec::new();
        let mut literals = Vec::new();
        let mut warnings = Vec::new();
        for (argument, parameter) in arguments.iter().zip(parameters) {
            let position = self.position(callee, parameter.type_reference)?;
            let (value, new_warnings) = value::evaluate_closed_scalar_with_policy(
                self.execution,
                *argument,
                primitive(&position),
                policy(&position),
                self,
            )?;
            append_warnings(&mut warnings, new_warnings);
            match (position, value.decode_encoding()) {
                (ScalarPosition::Boolean, Some(DecodedCanonicalConstValue::Boolean(value))) => {
                    literals.push(ExpressionNode::Boolean(value));
                    values.push(BuildTimeValue::Bool(value))
                }
                (
                    ScalarPosition::Integer(position),
                    Some(DecodedCanonicalConstValue::Integer { value, .. }),
                ) => {
                    position.require_value(
                        self.execution,
                        self.admission,
                        &numerics::bignum::BigInt::from_i128(value),
                    )?;
                    literals.push(ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_parts(
                            value < 0,
                            numerics::literals::IntegerRadix::Decimal,
                            &value.unsigned_abs().to_string(),
                        )
                        .map_err(|_| "range endpoint argument lost its literal spelling")?
                        .with_landing(numerics::literals::IntegerLanding {
                            landed_type: super::integer_landing(position.primitive)?,
                            domain: numerics::arithmetic::ArithmeticDomain::Exact,
                        }),
                    ));
                    let bits = if position.primitive.is_signed_integer() {
                        i64::try_from(value).map_err(
                            |_| "range endpoint argument exceeds signed interpreter storage",
                        )?
                    } else {
                        u64::try_from(value).map_err(
                            |_| "range endpoint argument exceeds unsigned interpreter storage",
                        )? as i64
                    };
                    values.push(BuildTimeValue::Int(bits));
                }
                _ => return Err("range endpoint argument differs from its selected carrier".into()),
            }
        }
        Ok((values, literals, warnings))
    }

    /// Discharge this endpoint call's authored non-domain `requires`
    /// premises at its concrete arguments: an authored premise has meaning
    /// only inside an invocation, so the shared snapshot probe rebuilds the
    /// call with the evaluated literal arguments and re-runs ordinary
    /// contract checking on the exact invocation — its diagnostics are the
    /// honest rejection when a premise does not hold at these operands.
    fn snapshot_requires_discharge(
        &self,
        expression: ExpressionHandle,
        machine: &typed_trees::machine::Machine,
        literals: &[ExpressionNode],
    ) -> Result<bool, String> {
        let Some(call) = self.execution.expression_table.call_node(expression) else {
            return Err("range endpoint lost its call".into());
        };
        let argument_handles = self
            .execution
            .expression_table
            .expression_handles(call.arguments);
        if argument_handles.len() != literals.len() {
            return Err(
                "range endpoint call argument count does not match its evaluated snapshots".into(),
            );
        }
        let entry = self
            .execution
            .machine_states(machine)
            .first()
            .ok_or("range endpoint lost entry")?;
        let snapshots = argument_handles
            .iter()
            .zip(literals)
            .map(
                |(argument, literal)| crate::machine_execution::admission::SnapshotArgument {
                    receiver: false,
                    original: *argument,
                    literal: Some(literal.clone()),
                },
            )
            .collect();
        // The probe keeps the whole program: library satisfaction
        // references and declaration reach read machines far outside the
        // callee's call closure. The signature bound this call still spells
        // is rewritten to a closed stand-in inside the probe program, so
        // the caller's own unfinished endpoint obligations stay out of this
        // call's admission evidence.
        let retained: Vec<symbols::SymbolHandle> = self
            .execution
            .machines()
            .iter()
            .map(|machine| machine.symbol)
            .collect();
        crate::machine_execution::admission::snapshot_arguments_discharge(
            self.execution,
            &retained,
            machine.symbol,
            "@const-endpoint",
            &typed_trees_to_typed_trees::PreparationRequest::preliminary(),
            &typed_trees_to_checked_trees::CheckingRequest::new(),
            expression,
            machine.name.as_str(),
            entry.return_type,
            snapshots,
            &crate::const_evaluation::range_endpoints::deferred_endpoint_stand_ins(self.execution),
        )?;
        Ok(true)
    }
}

impl ConstantCalls for Invocation<'_> {
    fn result_type(&self, expression: ExpressionHandle) -> TypeReferenceHandle {
        self.callee(expression)
            .ok()
            .and_then(|callee| self.execution.machine(callee.instance))
            .and_then(|machine| self.execution.machine_states(machine).first())
            .map_or(TypeReferenceHandle::invalid(), |entry| entry.return_type)
    }

    fn validate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(PrimitiveType, Vec<Diagnostic>), String> {
        crate::machine_execution::admission::require_call_expression_selection(
            self.execution,
            expression,
            self.authority,
        )?;
        let callee = self.callee(expression)?;
        let machine = self
            .execution
            .machine(callee.instance)
            .ok_or("range endpoint lost its selected machine")?;
        let entry = self
            .execution
            .machine_states(machine)
            .first()
            .ok_or("range endpoint lost entry")?;
        let custody = crate::BuildTimeInvocationCustody::Source(
            self.execution.expression_table.source_span(expression),
        );
        // Validation does not evaluate arguments — a call nested in a skipped
        // match arm owes only its carrier shape here. Argument values are
        // evaluated for the snapshot probe solely when an authored non-domain
        // `requires` premise makes the concrete discharge the deciding route.
        let discharged = if !self
            .admission
            .closure_includes_authored_requires(self.execution, machine)
        {
            false
        } else if self
            .admission
            .closure_requires_are_entry_parameter_domains(self.execution, machine)
        {
            true
        } else {
            let (_, literals, _) = self.arguments(expression, callee)?;
            self.snapshot_requires_discharge(expression, machine, &literals)?
        };
        if discharged {
            self.admission
                .require_common_floor_for_concrete_premise_invocation(
                    self.execution,
                    machine,
                    custody,
                )?;
        } else {
            self.admission
                .require_common_floor_for_invocation(self.execution, machine, custody)?;
        }
        let Some(call) = self.execution.expression_table.call_node(expression) else {
            return Err("range endpoint lost its call".into());
        };
        let arguments = self
            .execution
            .expression_table
            .expression_handles(call.arguments);
        let parameters = self.execution.state_parameters(entry);
        if arguments.len() != parameters.len() {
            return Err(
                "range endpoint call argument count does not match its selected entry".into(),
            );
        }
        let mut warnings = Vec::new();
        for (argument, parameter) in arguments.iter().zip(parameters) {
            let position = self.position(callee, parameter.type_reference)?;
            append_warnings(
                &mut warnings,
                value::validate_closed_scalar_with_policy(
                    self.execution,
                    *argument,
                    primitive(&position),
                    policy(&position),
                    self,
                )?,
            );
        }
        Ok((primitive(&self.result_position(callee)?), warnings))
    }

    fn evaluate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
        let callee = self.callee(expression)?;
        let (arguments, literals, warnings) = self.arguments(expression, callee)?;
        let machine = self
            .execution
            .machine(callee.instance)
            .ok_or("range endpoint lost its selected machine")?;
        let custody = crate::BuildTimeInvocationCustody::Source(
            self.execution.expression_table.source_span(expression),
        );
        let discharged = if !self
            .admission
            .closure_includes_authored_requires(self.execution, machine)
        {
            false
        } else if self
            .admission
            .closure_requires_are_entry_parameter_domains(self.execution, machine)
        {
            true
        } else {
            self.snapshot_requires_discharge(expression, machine, &literals)?
        };
        let result = if discharged {
            self.admission
                .evaluate_const_evaluable_machine_symbol_for_concrete_premise_invocation(
                    self.execution,
                    callee.instance,
                    arguments,
                    custody,
                )?
        } else {
            self.admission
                .evaluate_const_evaluable_machine_symbol_for_invocation(
                    self.execution,
                    callee.instance,
                    arguments,
                    custody,
                )?
        };
        let value = match (self.result_position(callee)?, result) {
            (ScalarPosition::Boolean, BuildTimeValue::Bool(value)) => {
                CanonicalConstValue::boolean(value)
            }
            (ScalarPosition::Integer(position), result) => {
                let result = crate::const_evaluation::const_lengths::decode_integer_result(
                    self.execution,
                    machine,
                    result,
                )?;
                position.require_value(self.execution, self.admission, &result)?;
                let value = if position.primitive.is_signed_integer() {
                    i128::from(
                        result
                            .to_i64()
                            .ok_or("range endpoint result exceeds signed storage")?,
                    )
                } else {
                    i128::from(
                        result
                            .to_u64()
                            .ok_or("range endpoint result exceeds unsigned storage")?,
                    )
                };
                let identity = CanonicalConstIdentity::integer(position.primitive.name(), value);
                CanonicalConstValue::new(identity.type_name, identity.encoding, value.to_string())
            }
            _ => {
                return Err(
                    "range endpoint result differs from its declared scalar carrier".into(),
                );
            }
        };
        Ok((value, warnings))
    }
}

fn primitive(position: &ScalarPosition) -> PrimitiveType {
    match position {
        ScalarPosition::Boolean => PrimitiveType::Bool,
        ScalarPosition::Integer(position) => position.primitive,
    }
}

fn policy(position: &ScalarPosition) -> numerics::arithmetic::ArithmeticDomain {
    match position {
        ScalarPosition::Boolean => numerics::arithmetic::ArithmeticDomain::Exact,
        ScalarPosition::Integer(position) => position.policy,
    }
}

fn append_warnings(warnings: &mut Vec<Diagnostic>, additions: Vec<Diagnostic>) {
    for warning in additions {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
}
