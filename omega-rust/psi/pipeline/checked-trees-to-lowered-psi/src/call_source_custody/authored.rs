//! Authored roots and formal-to-explicit argument mapping for retained calls.

use crate::{CheckedTrees, LoweringError, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::signature::StateParameter;
use checked_trees::statement::StatementNode;
use checked_trees::types::PrimitiveType;
use checked_trees::{CheckedUnitCallCoordinate, NominalMachineUseSite};
use symbols::SymbolHandle;

pub(super) struct AuthoredCall {
    pub source_site: Option<NominalMachineUseSite>,
    pub scalar_arguments: Vec<(ExpressionHandle, PrimitiveType)>,
    pub boundary: bool,
}

pub(super) fn locate(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> Result<AuthoredCall, LoweringError> {
    let program = &checked.typed;
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, caller_state)?;
    if machine.symbol != caller_machine {
        return unsupported("call source custody disagrees with its authored caller");
    }
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(coordinate.statement_index as usize)
        .ok_or(LoweringError::Unsupported(
            "call source custody has no authored statement",
        ))?;
    let (source_target, arguments, source_site) = match statement {
        StatementNode::Call(call) if coordinate.call_ordinal == 0 => {
            let index = state
                .statement_nodes
                .start()
                .arena_index()
                .checked_add(coordinate.statement_index)
                .ok_or(LoweringError::Unsupported(
                    "call source statement identity overflows",
                ))?;
            (
                call.target_symbol,
                program.statement_table.expression_handles(call.arguments),
                Some(NominalMachineUseSite::Statement(arena::Handle::from_parts(
                    index,
                    state.statement_nodes.start().generation(),
                ))),
            )
        }
        StatementNode::LocalData(local) if coordinate.call_ordinal == 0 && !local.is_mutable => {
            expression_call(checked, local.initial_value)?
        }
        StatementNode::Expression(expression) if coordinate.call_ordinal == 0 => {
            expression_call(checked, *expression)?
        }
        _ => return unsupported("call source custody has no supported authored call root"),
    };
    let (parameters, boundary) = target_parameters(
        checked,
        caller_machine,
        source_target,
        target_machine,
        target_state,
    )?;
    let nonself_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let explicit_self = arguments.len() > nonself_count;
    if arguments.len()
        != if explicit_self {
            parameters.len()
        } else {
            nonself_count
        }
    {
        return unsupported("call source custody disagrees with its authored argument count");
    }
    let mut explicit = 0usize;
    let mut scalar_arguments = Vec::new();
    for parameter in parameters {
        if parameter.is_self && !explicit_self {
            continue;
        }
        let argument = arguments[explicit];
        explicit += 1;
        let Some(primitive) = program.primitive_type_reference(parameter.type_reference) else {
            continue;
        };
        if parameter.is_self
            || parameter.is_const
            || parameter.is_mutable
            || !program.expression_table.expression_is_valid(argument)
        {
            return unsupported("call source custody has no immutable primitive argument");
        }
        scalar_arguments.push((argument, primitive));
    }
    Ok(AuthoredCall {
        source_site,
        scalar_arguments,
        boundary,
    })
}

fn expression_call(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Result<
    (
        SymbolHandle,
        &[ExpressionHandle],
        Option<NominalMachineUseSite>,
    ),
    LoweringError,
> {
    if !checked
        .typed
        .expression_table
        .expression_is_valid(expression)
    {
        return unsupported("call source custody has no live authored expression");
    }
    let ExpressionNode::Call(call) = checked.typed.expression_table.expression(expression) else {
        return unsupported("call source custody has no bare authored call");
    };
    Ok((
        call.target_symbol,
        checked
            .typed
            .expression_table
            .expression_handles(call.arguments),
        Some(NominalMachineUseSite::Expression(expression)),
    ))
}

fn target_parameters(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    source_target: SymbolHandle,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> Result<(&[StateParameter], bool), LoweringError> {
    let program = &checked.typed;
    if !source_target.is_valid() || !target_machine.is_valid() || !target_state.is_valid() {
        return unsupported("call source custody has no live target identity");
    }
    let intrinsic =
        validation::exact_compiler_intrinsic_boundary_requirement(program, source_target);
    let mut states = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter_map(move |state| (state.symbol == source_target).then_some((machine, state)))
    });
    if intrinsic.is_none()
        && let Some((machine, state)) = states.next()
    {
        if states.next().is_some()
            || machine.symbol != target_machine
            || state.symbol != target_state
        {
            return unsupported("call source custody disagrees with its authored callee owner");
        }
        return Ok((
            program.state_parameters(state),
            machine.supply_mode.is_boundary_declaration(),
        ));
    }
    let requirement = if let Some((requirement, _)) = intrinsic {
        requirement
    } else if let Some((owner, signature)) = program.machine_parameter_signature(source_target) {
        if owner.symbol != caller_machine {
            return unsupported("call source custody uses another machine's callable parameter");
        }
        signature.symbol
    } else {
        source_target
    };
    let mut signatures = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| program.trait_machine_signatures(definition).iter())
        .filter(|signature| signature.symbol == requirement);
    let signature = signatures.next().ok_or(LoweringError::Unsupported(
        "call source custody has no authored boundary requirement",
    ))?;
    if signatures.next().is_some()
        || target_machine != signature.symbol
        || target_state != signature.symbol
    {
        return unsupported("call source custody disagrees with its authored boundary requirement");
    }
    Ok((program.state_signature_parameters(signature), true))
}
