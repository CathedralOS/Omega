//! Land authored integer arguments before creating untyped interpreter snapshots.
//!
//! The raw interpreter entry can coerce values; it is not a source type checker.
//! Keep exact parameter identity and each argument's existing carrier here so
//! folding an ignored argument cannot hide a bad conversion. Closed range
//! refinements and declared integer domains check the concrete value before
//! invocation through the admission plan's fact evaluator; policy
//! qualifications cannot enter by base-type stripping. A bare `bool`
//! parameter lands through the same shared scalar evaluator with the Boolean
//! destination, after its own custody walk.
//! The shared context-free numeric query rejects owner-dependent
//! operations rather than evaluating them in the endpoint callee's scope.

use diagnostics::Diagnostic;
use language_semantics::const_value::DecodedCanonicalConstValue;
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
};

use crate::{BuildTimeAdmissionPlan, BuildTimeSelectionAuthority, BuildTimeValue};

pub(super) fn evaluate(
    program: &TypedTrees,
    original: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    expression: ExpressionHandle,
    callee: super::EndpointCallee,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Result<(Vec<BuildTimeValue>, Vec<Diagnostic>), String> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return Err("range endpoint lost its authored call".to_owned());
    };
    // Argument expressions and their evaluation context live in the working
    // tree, where the template (or the plain callee) exists. Parameter types
    // come from the executable callee: a specialized instance's substituted
    // signature exists only in the prepared tree.
    let machine = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == callee.template)
        .ok_or("range endpoint lost its selected machine")?;
    let entry = program
        .machine_states(machine)
        .first()
        .ok_or("range endpoint machine has no entry state")?;
    let types = if callee.static_application {
        original
    } else {
        program
    };
    let instance = types
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == callee.instance)
        .ok_or("range endpoint lost its executable instance")?;
    let instance_entry = types
        .machine_states(instance)
        .first()
        .ok_or("range endpoint instance has no entry state")?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    let parameters = types.state_parameters(instance_entry);
    if arguments.len() != parameters.len() {
        return Err(
            "range endpoint call argument count does not match its selected entry".to_owned(),
        );
    }
    let mut values = Vec::new();
    let mut warnings = Vec::new();
    for (argument, parameter) in arguments.iter().zip(parameters) {
        let position = super::integer_type::ScalarPosition::prepare(
            types,
            original,
            parameter.type_reference,
            authority,
        )?;
        let position = match position {
            super::integer_type::ScalarPosition::Integer(position) => position,
            super::integer_type::ScalarPosition::Boolean => {
                crate::machine_execution::admission::require_closed_boolean_argument(
                    original, *argument, authority,
                )?;
                let (value, argument_warnings) =
                    crate::const_evaluation::const_generic_expressions::value::evaluate(
                        program,
                        machine,
                        entry,
                        *argument,
                        typed_trees::types::PrimitiveType::Bool,
                        None,
                    )?;
                let Some(DecodedCanonicalConstValue::Boolean(value)) = value.decode_encoding()
                else {
                    return Err("range endpoint argument did not produce a Boolean".to_owned());
                };
                values.push(BuildTimeValue::Bool(value));
                for warning in argument_warnings {
                    if !warnings.contains(&warning) {
                        warnings.push(warning);
                    }
                }
                continue;
            }
        };
        let destination = position.primitive;
        crate::machine_execution::admission::require_closed_integer_argument(
            original, program, *argument, authority,
        )?;
        // The context-free query above vetoes every owner-sensitive operator.
        // Only after that check may the shared scalar landing path use this
        // machine context: none of the accepted operations can vary by owner.
        // Reuse it to retain fractional warnings at nested landing boundaries.
        let (value, argument_warnings) =
            crate::const_evaluation::const_generic_expressions::value::evaluate(
                program,
                machine,
                entry,
                *argument,
                destination,
                None,
            )?;
        if value.type_name != destination.name() {
            return Err(format!(
                "landed range endpoint argument cannot initialize `{}`",
                destination.name()
            ));
        }
        let Some(DecodedCanonicalConstValue::Integer { value, .. }) = value.decode_encoding()
        else {
            return Err("range endpoint argument did not produce an integer".to_owned());
        };
        position.require_value(
            original,
            admission,
            &numerics::bignum::BigInt::from_i128(value),
        )?;
        let bits = if destination.is_signed_integer() {
            i64::try_from(value)
                .map_err(|_| "range endpoint argument exceeds signed interpreter storage")?
        } else {
            u64::try_from(value)
                .map_err(|_| "range endpoint argument exceeds unsigned interpreter storage")?
                as i64
        };
        values.push(BuildTimeValue::Int(bits));
        for warning in argument_warnings {
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }
    }
    Ok((values, warnings))
}
