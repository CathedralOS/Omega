//! Calls as stage 04 recorded them. Statement custody joins
//! every flow call row (`FlowCallFact`) to its authored call site and records
//! the site kind, the callee as spelled, the explicit arguments,
//! what the statement does with the result, and whether a named transition is
//! the state's closing invocation. Lowering reads those rows here, with the
//! callee's declared signature, instead of walking the authored statement
//! again. Declaration applications carry their own admitted receipts into
//! `checked_invocation`; neither source owner can impersonate the other.

use crate::checked_facts::flow_facts;
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
    let call = checked_call(
        checked,
        caller_state,
        coordinate.statement_index as usize,
        coordinate.call_ordinal as usize,
    )?;
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

/// One exact checked occurrence owns its selected module realization. Authored
/// machine/public requirement identity stays in the same flow row for signature
/// and closed-application checking. All ordinary direct-call result roles record
/// this selected state, independently of their Terminal operation spelling.
/// This is a borrowed producer view, not another durable call representation.
pub(crate) struct ModuleCallTarget<'source> {
    pub machine: &'source checked_trees::machine::Machine,
    pub state: &'source checked_trees::state::State,
    pub static_requirement_dispatch: Option<checked_trees::StaticRequirementDispatchFact>,
}

pub(crate) fn selected_module_target<'source>(
    checked: &'source CheckedTrees,
    occurrence: &lowered_psi::LoweredSourceCallOccurrence,
) -> Result<ModuleCallTarget<'source>, LoweringError> {
    let (target, static_requirement_dispatch, _) = checked_invocation(checked, occurrence.origin)?;
    if target != occurrence.source_target {
        return unsupported("emitted call selected target disagrees with its checked occurrence");
    }
    let (machine, state) = crate::declarations::state_declaration(checked, target)?;
    Ok(ModuleCallTarget {
        machine,
        state,
        static_requirement_dispatch,
    })
}

/// Rejoin either admitted source owner without inventing a statement call.
pub(crate) fn checked_invocation(
    checked: &CheckedTrees,
    origin: lowered_psi::LoweredSourceCallOrigin,
) -> Result<
    (
        symbols::SymbolHandle,
        Option<checked_trees::StaticRequirementDispatchFact>,
        Option<checked_trees::NominalMachineUseSite>,
    ),
    LoweringError,
> {
    let selected = match origin {
        lowered_psi::LoweredSourceCallOrigin::Statement(coordinate) => {
            let call = checked_source_call(checked, coordinate)?;
            (
                call.target_symbol,
                call.static_requirement_dispatch,
                source_site(checked, coordinate, call)?,
            )
        }
        lowered_psi::LoweredSourceCallOrigin::Declaration {
            state,
            application,
            call,
            ..
        } => {
            let owner = checked
                .machine_holding_state(state)
                .ok_or(LoweringError::Unsupported(
                    "declaration invocation lost its source owner",
                ))?;
            let (_, retained) = checked_trees::CheckedCallOrigin::Declaration { application, call }
                .declaration(
                    &checked.facts.termination.rank_arrivals,
                    owner.symbol,
                    state,
                )
                .map_err(LoweringError::Unsupported)?;
            let contract = checked
                .facts
                .contract_plans
                .for_machine(retained.target_machine)
                .ok_or(LoweringError::Unsupported(
                    "declaration invocation lost its target contract",
                ))?;
            if retained.target_contract
                != Some(checked_trees::MachineContractIdentity {
                    report_fingerprint: contract.report_fingerprint,
                    commitment: contract.commitment,
                })
            {
                return unsupported("declaration invocation changed its bound target contract");
            }
            (
                retained.target_state,
                None,
                Some(checked_trees::NominalMachineUseSite::Expression(
                    retained.expression,
                )),
            )
        }
    };
    Ok(selected)
}

/// Authenticate one direct-call source coordinate through its unique bound row.
/// The producer retains that row's selected state; declaration views are resolved
/// only by consumers which need them. No route-local target symbol is accepted.
pub(crate) fn checked_source_call(
    checked: &CheckedTrees,
    coordinate: crate::emission::operation_emission::buffer::SourceCallCoordinate,
) -> Result<&checked_trees::FlowCallFact, LoweringError> {
    checked_call(
        checked,
        coordinate.state,
        coordinate.statement_index,
        coordinate.call_ordinal,
    )
}

/// Recover the authored occurrence from its already-bound flow row. Every
/// direct-call emitter uses this join, independent of its result carrier.
pub(crate) fn source_site(
    checked: &CheckedTrees,
    coordinate: crate::emission::operation_emission::buffer::SourceCallCoordinate,
    call: &checked_trees::FlowCallFact,
) -> Result<Option<checked_trees::NominalMachineUseSite>, LoweringError> {
    use checked_trees::{FlowCallSite, NominalMachineUseSite};
    match call.authored_site {
        FlowCallSite::Expression => Ok(Some(NominalMachineUseSite::Expression(
            call.authored_expression,
        ))),
        FlowCallSite::Statement => {
            let (_, state) = crate::declarations::state_declaration(checked, coordinate.state)?;
            let offset = u32::try_from(coordinate.statement_index)
                .map_err(|_| LoweringError::Unsupported("call statement ordinal exceeds u32"))?;
            let start = state.statement_nodes.start();
            let arena_index =
                start
                    .arena_index()
                    .checked_add(offset)
                    .ok_or(LoweringError::Unsupported(
                        "call statement coordinate overflows",
                    ))?;
            Ok(Some(NominalMachineUseSite::Statement(
                arena::Handle::from_parts(arena_index, start.generation()),
            )))
        }
        FlowCallSite::NamedTransition => Ok(None),
        FlowCallSite::Unbound => unsupported("call occurrence has no bound authored site"),
    }
}

fn checked_call(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Result<&checked_trees::FlowCallFact, LoweringError> {
    let control = &checked.facts.flow.control;
    let state = control
        .state_by_symbol(caller_state)
        .ok_or(LoweringError::Unsupported(
            "call coordinate has no checked flow state",
        ))?;
    let calls = control
        .calls
        .span(state.calls)
        .ok_or(LoweringError::Unsupported(
            "call coordinate has no complete checked flow roster",
        ))?;
    let mut matching = calls.iter().filter(|call| {
        call.statement_index == statement_index && call.call_ordinal == call_ordinal
    });
    let call = matching.next().ok_or(LoweringError::Unsupported(
        "call coordinate has no checked flow occurrence",
    ))?;
    if matching.next().is_some() {
        return unsupported("call coordinate has more than one checked flow occurrence");
    }
    if call.authored_site == checked_trees::FlowCallSite::Unbound {
        return unsupported("call coordinate has no bound authored site");
    }
    Ok(call)
}

fn planned_result(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> FlowCallResult {
    flow_facts(checked)
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
