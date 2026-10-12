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
use checked_trees::{
    CheckedTrees, CheckedUnitCallCoordinate, FlowCallResult, NominalMachineUseSite,
};
use symbols::SymbolHandle;

pub(crate) struct AuthoredCall {
    pub source_target: SymbolHandle,
    pub application: typed_trees::static_applications::StaticMachineApplicationHandle,
    pub source_site: Option<NominalMachineUseSite>,
    pub scalar_arguments: Vec<(ExpressionHandle, PrimitiveType)>,
    pub structural_arguments: Vec<(u32, ExpressionHandle)>,
}

impl AuthoredCall {}

/// The invocation stage 04 bound at `coordinate`. Its row names the callee as
/// spelled, the explicit arguments and the syntax that spells the call, so
/// lowering never walks the statement again to find the occurrence.
fn planned_call<'checked>(
    checked: &'checked CheckedTrees,
    state: &checked_trees::state::State,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
) -> Result<PlannedCall<'checked>, LoweringError> {
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
    let site = match call.authored_site {
        checked_trees::FlowCallSite::Expression => {
            Some(NominalMachineUseSite::Expression(call.authored_expression))
        }
        checked_trees::FlowCallSite::Statement => {
            let index = state
                .statement_nodes
                .start()
                .arena_index()
                .checked_add(coordinate.statement_index)
                .ok_or(LoweringError::Unsupported(
                    "call source statement identity overflows",
                ))?;
            Some(NominalMachineUseSite::Statement(arena::Handle::from_parts(
                index,
                state.statement_nodes.start().generation(),
            )))
        }
        checked_trees::FlowCallSite::NamedTransition => None,
        checked_trees::FlowCallSite::Unbound => {
            return unsupported("call coordinate has no bound authored site");
        }
    };
    Ok(PlannedCall {
        target: call.authored_target,
        application: call.authored_application,
        arguments: control
            .call_arguments
            .span_or_empty(call.authored_arguments),
        site,
    })
}

struct PlannedCall<'checked> {
    target: SymbolHandle,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    arguments: &'checked [ExpressionHandle],
    site: Option<NominalMachineUseSite>,
}

pub(crate) fn locate(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
) -> Result<AuthoredCall, LoweringError> {
    let program = &checked.typed;
    let (machine, state) = crate::declarations::state_declaration(checked, caller_state)?;
    let PlannedCall {
        target: source_target,
        application,
        arguments,
        site: source_site,
    } = planned_call(checked, state, caller_state, coordinate)?;
    let target =
        crate::declarations::target_signature(checked, machine.symbol, source_target, application)?;
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
        application,
        source_site,
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
