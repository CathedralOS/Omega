//! Rejoin one invocation before its dense operands enter producer resolution.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{
    CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarComputationStructuralArgument, CheckedTrees, CheckedUnitCallCoordinate,
};
use symbols::SymbolHandle;

use super::{
    authored_state, borrow_rows, owned_arguments, primitive_arguments, shared_nominal_arguments,
};
use crate::{LoweringError, unsupported};

pub(super) mod access_occurrences;
mod arrays;

/// Authored formal order, preserving scalar handles and structural occurrences.
pub(crate) enum RejoinedComputationArgument {
    Scalar {
        expression: ExpressionHandle,
        computation: CheckedScalarComputationHandle,
    },
    Structural {
        expression: ExpressionHandle,
    },
    Construction {
        expression: ExpressionHandle,
        elements: Vec<(ExpressionHandle, CheckedScalarComputationHandle)>,
    },
}

/// Validates one live call's source occurrence, signature, and mixed actuals.
///
/// This does not resolve callees or discharge contracts, nor replace whole-root
/// replay. Consumers must stage the returned entries in this formal order.
pub(crate) fn rejoin_computation_call_arguments(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    computation: CheckedScalarComputationHandle,
) -> Result<Vec<RejoinedComputationArgument>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    if !plans.nodes.is_valid(computation) {
        return unsupported("computed invocation has a stale computation");
    }
    let node = plans.nodes.get(computation);
    let CheckedScalarComputationKind::Call {
        source_call,
        target_machine,
        target_state,
        call_ordinal,
        arguments,
        structural_arguments,
    } = &node.kind
    else {
        return unsupported("computed invocation is not a call");
    };
    let (caller, caller_state) = authored_state(checked, state)?;
    if caller.symbol != machine
        || checked
            .statement_table
            .statements(caller_state.statement_nodes)
            .get(statement as usize)
            .is_none()
    {
        return unsupported("computed invocation disagrees with its authored caller");
    }
    let control = &checked.facts.flow.control;
    if !control.calls.is_valid(*source_call) {
        return unsupported("computed invocation has no live checked source call");
    }
    let source = control.calls.get(*source_call);
    let mut states = control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state);
    let source_state = states.next().ok_or(LoweringError::Unsupported(
        "computed invocation has no checked source state",
    ))?;
    if states.next().is_some() {
        return unsupported("computed invocation has ambiguous checked source state");
    }
    let mut calls = control
        .calls
        .span(source_state.calls)
        .ok_or(LoweringError::Unsupported(
            "computed invocation has an invalid source call span",
        ))?
        .iter()
        .filter(|candidate| {
            candidate.statement_index == statement as usize
                && candidate.call_ordinal == *call_ordinal as usize
        });
    let exact = calls.next().ok_or(LoweringError::Unsupported(
        "computed invocation lost its exact source occurrence",
    ))?;
    if calls.next().is_some()
        || !std::ptr::eq(exact, source)
        || source.target_symbol != *target_state
        || !checked
            .expression_table
            .expression_is_valid(source.authored_expression)
    {
        return unsupported("computed invocation substituted its source occurrence");
    }
    let ExpressionNode::Call(call) = checked
        .expression_table
        .expression(source.authored_expression)
    else {
        return unsupported("computed invocation source is not an authored call");
    };
    let (owner, target) = authored_state(checked, *target_state)?;
    let parameters = checked.state_parameters(target);
    let authored = checked.expression_table.expression_handles(call.arguments);
    let has_receiver = !checked.call_has_no_runtime_receiver(call, owner, target);
    let explicit_parameters = if has_receiver {
        let Some((receiver, explicit)) = parameters.split_first() else {
            return unsupported("computed receiver has no formal");
        };
        if !call.receiver.is_valid()
            || !receiver.is_self
            || explicit.iter().any(|parameter| parameter.is_self)
        {
            return unsupported("computed receiver has an invalid formal position");
        }
        explicit
    } else {
        parameters
    };
    if owner.symbol != *target_machine
        || call.target_symbol != *target_state
        || source.has_receiver != call.receiver.is_valid()
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
        || checked.primitive_type_reference(target.return_type) != Some(node.primitive_type)
        || authored.len() != call.arguments.count() as usize
        || authored.len() != explicit_parameters.len()
    {
        return unsupported("computed invocation disagrees with its authored signature");
    }
    crate::call_source_custody::occurrences::validate(
        checked,
        machine,
        state,
        CheckedUnitCallCoordinate {
            statement_index: statement,
            call_ordinal: *call_ordinal,
        },
        source.authored_expression,
    )?;
    let scalars = plans
        .operands
        .span(*arguments)
        .ok_or(LoweringError::Unsupported(
            "computed invocation has an invalid argument span",
        ))?;
    let structural = plans
        .structural_arguments
        .span(*structural_arguments)
        .ok_or(LoweringError::Unsupported(
            "computed invocation has an invalid structural span",
        ))?;
    let borrow_call = if structural.is_empty() {
        None
    } else {
        let borrow_call = borrow_rows::call(
            checked,
            machine,
            state,
            CheckedUnitCallCoordinate {
                statement_index: statement,
                call_ordinal: *call_ordinal,
            },
            *target_state,
        )?;
        if borrow_call.has_receiver != source.has_receiver
            || borrow_call.receiver_symbol != source.receiver_symbol
            || borrow_call.accesses != source.accesses
        {
            return unsupported("computed invocation substituted its captured borrow call rows");
        }
        Some(borrow_call)
    };
    let access_positions = if structural.iter().any(|argument| match argument {
        CheckedScalarComputationStructuralArgument::Array { .. } => true,
        CheckedScalarComputationStructuralArgument::Case(_) => false,
        CheckedScalarComputationStructuralArgument::Place(argument) => matches!(
            argument.access,
            checked_trees::CheckedStructuralAccess::SharedBorrow
                | checked_trees::CheckedStructuralAccess::Owned
        ),
    }) {
        Some(access_occurrences::rejoin(
            checked,
            borrow_call.ok_or(LoweringError::Unsupported(
                "computed shared borrow has no call roster",
            ))?,
            authored,
        )?)
    } else {
        None
    };
    let mut scalars = scalars.iter();
    let mut structural = structural.iter();
    let mut previous_access = None;
    let mut result = Vec::with_capacity(authored.len());
    // The receiver has a distinct captured occurrence, not a fabricated row in
    // the explicit-argument observation roster. Its storage still enters the
    // ordinary positional structural lane before the explicit actuals.
    if has_receiver {
        let Some(CheckedScalarComputationStructuralArgument::Place(argument)) = structural.next()
        else {
            return unsupported("computed receiver lost its structural operand");
        };
        shared_nominal_arguments::validate(
            checked,
            caller_state,
            statement,
            owner,
            &parameters[0],
            call.receiver,
            argument,
            borrow_call.ok_or(LoweringError::Unsupported(
                "computed receiver lost its call custody",
            ))?,
            None,
        )?;
        result.push(RejoinedComputationArgument::Structural {
            expression: call.receiver,
        });
    }
    for (formal_position, (parameter, expression)) in explicit_parameters
        .iter()
        .zip(authored.iter().copied())
        .enumerate()
    {
        if parameter.is_self
            || parameter.is_const
            || !checked.expression_table.expression_is_valid(expression)
            || !checked
                .type_reference_table
                .contains_type_reference(parameter.type_reference)
        {
            return unsupported("computed invocation has an unsupported formal or stale actual");
        }
        if let Some(primitive) = checked.primitive_type_reference(parameter.type_reference) {
            let computation = *scalars.next().ok_or(LoweringError::Unsupported(
                "computed invocation omits a scalar argument",
            ))?;
            if !plans.nodes.is_valid(computation)
                || plans.nodes.get(computation).authored_root != expression
                || plans.nodes.get(computation).primitive_type != primitive
                || (parameter.is_mutable && !super::super::supported_mutable_parameter(primitive))
            {
                return unsupported(
                    "computed invocation scalar disagrees with its authored formal",
                );
            }
            result.push(RejoinedComputationArgument::Scalar {
                expression,
                computation,
            });
        } else {
            let argument = structural.next().ok_or(LoweringError::Unsupported(
                "computed invocation omits a structural argument",
            ))?;
            if let CheckedScalarComputationStructuralArgument::Case(subject) = argument {
                if owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                    || parameter.is_mutable
                    || subject.expression != expression
                    || checked.normalized_type_identity(subject.type_reference)
                        != checked.normalized_type_identity(parameter.type_reference)
                {
                    return unsupported(
                        "computed case differs from its exact owned formal or actual",
                    );
                }
                result.push(RejoinedComputationArgument::Construction {
                    expression,
                    elements: crate::scalar_computations::cases::source::construction(
                        checked, subject,
                    )?,
                });
                continue;
            }
            if let CheckedScalarComputationStructuralArgument::Array { .. } = argument {
                if owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
                    return unsupported(
                        "computed array operands require an ordinary checked callee",
                    );
                }
                result.push(RejoinedComputationArgument::Construction {
                    expression,
                    elements: arrays::rejoin(checked, machine, parameter, expression, argument)?,
                });
                continue;
            }
            let CheckedScalarComputationStructuralArgument::Place(argument) = argument else {
                return unsupported("computed structural operand has no retained place");
            };
            let borrow_call = borrow_call.ok_or(LoweringError::Unsupported(
                "computed invocation has no exact borrow call",
            ))?;
            let position = if argument.access == checked_trees::CheckedStructuralAccess::Owned {
                owned_arguments::validate(
                    checked,
                    caller_state,
                    parameter.type_reference,
                    expression,
                    argument,
                    borrow_call,
                    access_positions
                        .as_ref()
                        .map(|positions| positions[formal_position])
                        .ok_or(LoweringError::Unsupported(
                            "computed owned argument has no positional source observation",
                        ))?,
                )?
            } else if matches!(checked.type_reference_table.type_reference(parameter.type_reference),
                checked_trees::types::TypeReferenceNode::Reference { referee, .. }
                    if checked.primitive_type_reference(*referee).is_none())
            {
                let position = access_positions
                    .as_ref()
                    .map(|positions| positions[formal_position])
                    .ok_or(LoweringError::Unsupported(
                        "record argument lost its observation position",
                    ))?;
                shared_nominal_arguments::validate(
                    checked,
                    caller_state,
                    statement,
                    owner,
                    parameter,
                    expression,
                    argument,
                    borrow_call,
                    Some(position),
                )?;
                position
            } else {
                primitive_arguments::validate(
                    checked,
                    caller_state,
                    statement,
                    parameter.type_reference,
                    expression,
                    argument,
                    borrow_call,
                    access_positions
                        .as_ref()
                        .map(|positions| positions[formal_position]),
                )?
            };
            if previous_access.is_some_and(|previous| previous >= position) {
                return unsupported(
                    "computed invocation duplicates or reorders structural access rows",
                );
            }
            previous_access = Some(position);
            result.push(RejoinedComputationArgument::Structural { expression });
        }
    }
    if scalars.next().is_some() || structural.next().is_some() {
        return unsupported("computed invocation has extra scalar or structural arguments");
    }
    Ok(result)
}
