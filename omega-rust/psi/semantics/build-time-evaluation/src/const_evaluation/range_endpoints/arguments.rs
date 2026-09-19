//! Invoke exact selected endpoint calls without preexecuting nested expressions.

use super::integer_type::{PositionRole, ScalarPosition};
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

pub(super) struct Invocation<'program> {
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
        };
        super::resolve_endpoint_callee(self.execution, &endpoint)
    }

    fn position(
        &self,
        callee: super::EndpointCallee,
        reference: TypeReferenceHandle,
        role: PositionRole,
    ) -> Result<ScalarPosition, String> {
        let types = if callee.static_application {
            self.execution
        } else {
            self.working
        };
        ScalarPosition::prepare(types, self.execution, reference, self.authority, role)
    }

    fn result_position(&self, callee: super::EndpointCallee) -> Result<ScalarPosition, String> {
        let machine = self
            .execution
            .machines()
            .iter()
            .find(|machine| machine.symbol == callee.instance)
            .ok_or("range endpoint lost its selected machine")?;
        let entry = self
            .execution
            .machine_states(machine)
            .first()
            .ok_or("range endpoint lost entry")?;
        self.position(callee, entry.return_type, PositionRole::Result)
    }

    fn arguments(
        &self,
        expression: ExpressionHandle,
        callee: super::EndpointCallee,
    ) -> Result<(Vec<BuildTimeValue>, Vec<Diagnostic>), String> {
        let ExpressionNode::Call(call) = self.execution.expression_table.expression(expression)
        else {
            return Err("range endpoint lost its call".into());
        };
        let machine = self
            .execution
            .machines()
            .iter()
            .find(|machine| machine.symbol == callee.instance)
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
        let mut warnings = Vec::new();
        for (argument, parameter) in arguments.iter().zip(parameters) {
            let position =
                self.position(callee, parameter.type_reference, PositionRole::Parameter)?;
            let (value, new_warnings) = value::evaluate_closed_scalar(
                self.execution,
                *argument,
                primitive(&position),
                self,
            )?;
            append_warnings(&mut warnings, new_warnings);
            match (position, value.decode_encoding()) {
                (ScalarPosition::Boolean, Some(DecodedCanonicalConstValue::Boolean(value))) => {
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
        Ok((values, warnings))
    }
}

impl ConstantCalls for Invocation<'_> {
    fn result_type(&self, expression: ExpressionHandle) -> TypeReferenceHandle {
        self.callee(expression)
            .ok()
            .and_then(|callee| {
                self.execution
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == callee.instance)
            })
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
            .machines()
            .iter()
            .find(|machine| machine.symbol == callee.instance)
            .ok_or("range endpoint lost its selected machine")?;
        let entry = self
            .execution
            .machine_states(machine)
            .first()
            .ok_or("range endpoint lost entry")?;
        let custody = crate::BuildTimeInvocationCustody::Source(
            self.execution.expression_table.source_span(expression),
        );
        if self
            .admission
            .closure_includes_authored_requires(self.execution, machine)
            && self
                .admission
                .closure_requires_are_entry_parameter_domains(self.execution, machine)
        {
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
        let ExpressionNode::Call(call) = self.execution.expression_table.expression(expression)
        else {
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
            let position =
                self.position(callee, parameter.type_reference, PositionRole::Parameter)?;
            append_warnings(
                &mut warnings,
                match &position {
                    ScalarPosition::Integer(position)
                        if position.policy != numerics::arithmetic::ArithmeticDomain::Exact =>
                    {
                        value::validate_closed_anonymous_scalar(
                            self.execution,
                            *argument,
                            position.primitive,
                            self,
                        )?
                    }
                    _ => value::validate_closed_scalar(
                        self.execution,
                        *argument,
                        primitive(&position),
                        self,
                    )?,
                },
            );
        }
        Ok((primitive(&self.result_position(callee)?), warnings))
    }

    fn evaluate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
        let callee = self.callee(expression)?;
        let (arguments, warnings) = self.arguments(expression, callee)?;
        let machine = self
            .execution
            .machines()
            .iter()
            .find(|machine| machine.symbol == callee.instance)
            .ok_or("range endpoint lost its selected machine")?;
        let custody = crate::BuildTimeInvocationCustody::Source(
            self.execution.expression_table.source_span(expression),
        );
        let result = if self
            .admission
            .closure_includes_authored_requires(self.execution, machine)
            && self
                .admission
                .closure_requires_are_entry_parameter_domains(self.execution, machine)
        {
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

fn append_warnings(warnings: &mut Vec<Diagnostic>, additions: Vec<Diagnostic>) {
    for warning in additions {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
}

#[cfg(test)]
pub(super) fn evaluate(
    _program: &TypedTrees,
    original: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    expression: ExpressionHandle,
    callee: super::EndpointCallee,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Result<(Vec<BuildTimeValue>, Vec<Diagnostic>), String> {
    crate::machine_execution::admission::require_closed_expression_custody(
        original, expression, authority,
    )?;
    let calls = Invocation {
        working: original,
        execution: original,
        admission,
        authority,
    };
    calls.validate_call(expression)?;
    calls.arguments(expression, callee)
}
