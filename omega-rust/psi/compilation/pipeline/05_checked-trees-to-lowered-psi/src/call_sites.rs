//! Calls as stage 04 recorded them. The source-custody binding pass joins
//! every flow call row (`FlowCallFact`) to its authored call site and records
//! the site kind, the callee as spelled, the explicit arguments,
//! what the statement does with the result, and whether a named transition is
//! the state's closing invocation. Lowering reads those rows here, with the
//! callee's declared signature, instead of walking the authored statement
//! again.

use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::expression::ExpressionHandle;
use checked_trees::types::PrimitiveType;
use checked_trees::{CheckedTrees, CheckedUnitCallCoordinate, FlowCallResult};
use symbols::SymbolHandle;

pub(crate) struct AuthoredCall {
    pub source_target: SymbolHandle,
    pub scalar_arguments: Vec<(ExpressionHandle, PrimitiveType)>,
    pub structural_arguments: Vec<(u32, ExpressionHandle)>,
}

pub(crate) fn locate(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
) -> Result<AuthoredCall, LoweringError> {
    let program = &checked.typed;
    let (machine, _) = crate::declarations::state_declaration(checked, caller_state)?;
    let control = &checked.facts.flow.control;
    let call = control
        .call_at(
            caller_state,
            coordinate.statement_index as usize,
            coordinate.call_ordinal as usize,
        )
        .ok_or(LoweringError::Unsupported(
            "call coordinate has no checked flow occurrence",
        ))?;
    if call.authored_site == checked_trees::FlowCallSite::Unbound {
        return unsupported("call coordinate has no bound authored site");
    }
    let source_target = call.authored_target;
    let arguments = control
        .call_arguments
        .span_or_empty(call.authored_arguments);
    let target = crate::declarations::target_signature(checked, machine.symbol, source_target)?;
    let parameters = target.parameters;
    if arguments.len() != parameters.len() {
        return unsupported("call row disagrees with its callee's argument count");
    }
    let mut scalar_arguments = Vec::new();
    let mut structural_arguments = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        let argument = arguments[position];
        let Some(primitive) = program.primitive_type_reference(parameter.type_reference) else {
            structural_arguments.push((
                u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported("call structural position exceeds u32")
                })?,
                argument,
            ));
            continue;
        };
        if parameter.is_const
            || (parameter.is_mutable
                && !(matches!(primitive, PrimitiveType::Bool) || primitive.is_fixed_integer()))
        {
            return unsupported("call passes a primitive the callee cannot receive by value");
        }
        scalar_arguments.push((argument, primitive));
    }
    Ok(AuthoredCall {
        source_target,
        scalar_arguments,
        structural_arguments,
    })
}

fn planned_result(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> FlowCallResult {
    checked
        .facts
        .flow
        .control
        .call_at(
            state,
            coordinate.statement_index as usize,
            coordinate.call_ordinal as usize,
        )
        .map_or_else(Default::default, |call| call.authored_result)
}

/// A discarded invocation produces a value but no source-local binding.
/// A discarded invocation produces a value but no source-local binding.
pub(crate) fn discards_result(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> bool {
    planned_result(checked, state, coordinate) == FlowCallResult::Discarded
}

/// Whether the call at `coordinate` is the whole initializer of a declared
/// immutable local, so its retained result is a source-named value. A result
/// an assignment stores has no source name of its own.
pub(crate) fn initializes_immutable_local(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> bool {
    planned_result(checked, state, coordinate) == FlowCallResult::ImmutableLocal
}
