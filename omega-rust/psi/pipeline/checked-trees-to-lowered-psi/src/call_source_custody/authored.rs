//! Authored roots and formal-to-explicit argument mapping for retained calls.

use crate::{CheckedTrees, LoweringError, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::signature::StateParameter;
use checked_trees::statement::StatementNode;
use checked_trees::types::{PrimitiveType, TypeReferenceHandle};
use checked_trees::{CheckedUnitCallCoordinate, NominalMachineUseSite};
use symbols::SymbolHandle;

pub(crate) mod nested;

pub(crate) struct AuthoredCall {
    pub source_target: SymbolHandle,
    pub source_site: Option<NominalMachineUseSite>,
    pub scalar_arguments: Vec<(ExpressionHandle, PrimitiveType)>,
    pub structural_arguments: Vec<(u32, ExpressionHandle)>,
    pub boundary: bool,
    pub target_machine: SymbolHandle,
    pub target_state: SymbolHandle,
}

pub(super) fn locate(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> Result<AuthoredCall, LoweringError> {
    let call = locate_source(checked, caller_state, coordinate)?;
    let (machine, _) = crate::scalar_source_custody::authored_state(checked, caller_state)?;
    if machine.symbol != caller_machine
        || call.target_machine != target_machine
        || call.target_state != target_state
    {
        return unsupported("call source custody disagrees with its authored caller or target");
    }
    Ok(call)
}

pub(crate) fn locate_source(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
) -> Result<AuthoredCall, LoweringError> {
    let program = &checked.typed;
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, caller_state)?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(coordinate.statement_index as usize)
        .ok_or(LoweringError::Unsupported(
            "call source custody has no authored statement",
        ))?;
    let (source_target, arguments, source_site) = if coordinate.call_ordinal != 0 {
        let expression =
            nested::authored_postorder(checked, caller_state, coordinate.statement_index)?
                .into_iter()
                .find_map(|(ordinal, expression)| {
                    (ordinal == coordinate.call_ordinal).then_some(expression)
                })
                .ok_or(LoweringError::Unsupported(
                    "nested call source custody has no exact authored preorder coordinate",
                ))?;
        super::occurrences::validate(
            checked,
            machine.symbol,
            caller_state,
            coordinate,
            expression,
        )?;
        expression_call(checked, expression)?
    } else {
        match statement {
            StatementNode::Call(call) if coordinate.call_ordinal == 0 => {
                if program
                    .statement_table
                    .expression_handles(call.arguments)
                    .len()
                    != call.arguments.count() as usize
                {
                    return unsupported(
                        "call source custody has an invalid authored argument span",
                    );
                }
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
            StatementNode::LocalData(local)
                if coordinate.call_ordinal == 0 && !local.is_mutable =>
            {
                expression_call(checked, local.initial_value)?
            }
            StatementNode::Expression(expression) if coordinate.call_ordinal == 0 => {
                if validation::unit_return_call_is_supported(program, machine, state, *expression) {
                    super::occurrences::validate(
                        checked,
                        machine.symbol,
                        state.symbol,
                        coordinate,
                        *expression,
                    )?;
                } else if !state.return_type.is_valid()
                    || matches!(
                        program
                            .type_reference_table
                            .type_reference(state.return_type),
                        checked_trees::types::TypeReferenceNode::Unit
                    )
                {
                    return unsupported("Unit call source custody has no exact authored Unit tail");
                }
                expression_call(checked, *expression)?
            }
            _ => return unsupported("call source custody has no supported authored call root"),
        }
    };
    let target = target_signature(checked, machine.symbol, source_target)?;
    let parameters = target.parameters;
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
    let mut structural_arguments = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        if parameter.is_self && !explicit_self {
            continue;
        }
        let argument = arguments[explicit];
        explicit += 1;
        let Some(primitive) = program.primitive_type_reference(parameter.type_reference) else {
            structural_arguments.push((
                u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported("call structural source position exceeds u32")
                })?,
                argument,
            ));
            continue;
        };
        if parameter.is_self
            || parameter.is_const
            || (parameter.is_mutable
                && !crate::scalar_source_custody::supported_mutable_parameter(primitive))
            || !program.expression_table.expression_is_valid(argument)
        {
            return unsupported("call source custody has no supported owned primitive argument");
        }
        scalar_arguments.push((argument, primitive));
    }
    Ok(AuthoredCall {
        source_target,
        source_site,
        scalar_arguments,
        structural_arguments,
        boundary: target.boundary,
        target_machine: target.machine,
        target_state: target.state,
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
    if checked
        .expression_table
        .expression_handles(call.arguments)
        .len()
        != call.arguments.count() as usize
    {
        return unsupported("call source custody has an invalid authored argument span");
    }
    Ok((
        call.target_symbol,
        checked
            .typed
            .expression_table
            .expression_handles(call.arguments),
        Some(NominalMachineUseSite::Expression(expression)),
    ))
}

/// A borrowed view of one exact authored callable.
pub(crate) struct TargetSignature<'source> {
    pub parameters: &'source [StateParameter],
    pub return_type: TypeReferenceHandle,
    pub boundary: bool,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
}

pub(crate) fn target_signature(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    source_target: SymbolHandle,
) -> Result<TargetSignature<'_>, LoweringError> {
    let program = &checked.typed;
    if !source_target.is_valid() {
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
        if states.next().is_some() || !machine.symbol.is_valid() {
            return unsupported("call source custody disagrees with its authored callee owner");
        }
        return Ok(TargetSignature {
            parameters: program.state_parameters(state),
            return_type: state.return_type,
            boundary: machine.supply_mode.is_boundary_declaration(),
            machine: machine.symbol,
            state: state.symbol,
        });
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
    if signatures.next().is_some() || !signature.symbol.is_valid() {
        return unsupported("call source custody disagrees with its authored boundary requirement");
    }
    Ok(TargetSignature {
        parameters: program.state_signature_parameters(signature),
        return_type: signature.return_type,
        boundary: true,
        machine: signature.symbol,
        state: signature.symbol,
    })
}
