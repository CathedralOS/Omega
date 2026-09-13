//! Land authored integer arguments before creating untyped interpreter snapshots.
//!
//! The raw interpreter entry can coerce values; it is not a source type checker.
//! Keep exact parameter identity and each argument's existing carrier here so
//! folding an ignored argument cannot hide a bad conversion. Constrained or
//! nominal destinations need their own proof/identity admission, not base-type
//! stripping. The shared context-free numeric query rejects owner-dependent
//! operations rather than evaluating them in the endpoint callee's scope.

use diagnostics::Diagnostic;
use language_semantics::const_value::DecodedCanonicalConstValue;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
};

use crate::{BuildTimeSelectionAuthority, BuildTimeValue};

pub(super) fn evaluate(
    program: &TypedTrees,
    expression: ExpressionHandle,
    machine: SymbolHandle,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Result<(Vec<BuildTimeValue>, Vec<Diagnostic>), String> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return Err("range endpoint lost its authored call".to_owned());
    };
    let machine = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
        .ok_or("range endpoint lost its selected machine")?;
    let entry = program
        .machine_states(machine)
        .first()
        .ok_or("range endpoint machine has no entry state")?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    let parameters = program.state_parameters(entry);
    if arguments.len() != parameters.len() {
        return Err(
            "range endpoint call argument count does not match its selected entry".to_owned(),
        );
    }
    let mut values = Vec::new();
    let mut warnings = Vec::new();
    for (argument, parameter) in arguments.iter().zip(parameters) {
        let destination = crate::const_generic_expressions::exact_probe_destination(
            program,
            parameter.type_reference,
        )
        .filter(|primitive| primitive.accepts_integer_literal())
        .ok_or("range endpoint arguments require unconstrained exact builtin integer parameters")?;
        crate::admission::require_closed_integer_argument(program, *argument, authority)?;
        // The context-free query above vetoes every owner-sensitive operator.
        // Only after that check may the shared scalar landing path use this
        // machine context: none of the accepted operations can vary by owner.
        // Reuse it to retain fractional warnings at nested landing boundaries.
        let (value, argument_warnings) = crate::const_generic_expressions::value::evaluate(
            program,
            machine,
            entry,
            *argument,
            destination,
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
