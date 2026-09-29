//! Source custody shared by Unit and boundary call operands.

use super::{
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, LoweringError, terminal_scalar_type, unsupported,
};
use checked_trees::CheckedUnitCallResult;
pub(crate) mod authored;
mod boundary_buffers;
pub(crate) mod initializers;
pub(crate) mod literal_arguments;
pub(crate) mod occurrences;
pub(crate) mod projected_receivers;
#[cfg(test)]
mod tests;

/// Whether the checked flow roster records a call at this statement.
fn statement_makes_call(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    state: symbols::SymbolHandle,
    statement_index: usize,
) -> bool {
    let control = &checked.facts.flow.control;
    control
        .states
        .iter()
        .map(|(_, flow)| flow)
        .filter(|flow| flow.machine_symbol == plan.machine && flow.state_symbol == state)
        .any(|flow| {
            control
                .calls
                .span_or_empty(flow.calls)
                .iter()
                .any(|call| call.statement_index == statement_index)
        })
}

/// Preserve calls around authored stores and direct call initializers, even
/// when a result is unused. Each retained owner checks its exact operands.
pub(crate) fn validate_store_and_initializer_calls(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    use checked_trees::statement::StatementNode;
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, plan.state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let has_stores = statements
        .iter()
        .any(|statement| matches!(statement, StatementNode::Assignment(_)));
    for (statement_index, statement) in statements.iter().enumerate() {
        let initializer = match statement {
            StatementNode::LocalData(local)
                if checked
                    .expression_table
                    .expression_is_valid(local.initial_value)
                    && matches!(
                        checked.expression_table.expression(local.initial_value),
                        checked_trees::expression::ExpressionNode::Call(_)
                    ) =>
            {
                Some(local.initial_value)
            }
            StatementNode::Assignment(_) | StatementNode::LocalData(_) => continue,
            // A final value expression remains a completion value when stores
            // precede it. Only actual call syntax owes a call occurrence here.
            StatementNode::Expression(expression)
                if !matches!(
                    checked.expression_table.expression(*expression),
                    checked_trees::expression::ExpressionNode::Call(_)
                ) =>
            {
                continue;
            }
            StatementNode::Call(call) if call.discards_result => None,
            // A transition owes a call only when its guard or target makes
            // one; the checked flow roster records every call a statement
            // makes.
            StatementNode::Transition(_)
                if !statement_makes_call(checked, plan, state.symbol, statement_index) =>
            {
                continue;
            }
            _ if !has_stores => continue,
            _ => None,
        };
        let statement_index = u32::try_from(statement_index)
            .map_err(|_| LoweringError::Unsupported("call statement ordinal exceeds u32"))?;
        let coordinate = checked_trees::CheckedUnitCallCoordinate {
            statement_index,
            call_ordinal: 0,
        };
        let mut owners = plan.operations.iter().enumerate().filter(|(_, operation)| {
            operation
                .call()
                .is_some_and(|call| call.coordinate() == coordinate)
                || match operation {
                    CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                        result,
                        value: checked_trees::CheckedCallScalarArgument::Computation(_),
                    } => result.statement_index == statement_index,
                    // A structural local whose initializer vocabulary produces
                    // a value (a view, aggregate, or alias) rather than a call
                    // op is established by this operation; it owns the authored
                    // call unless one of its nested calls already does.
                    // Argument values precede a separate call operation and
                    // must not also claim that call's occurrence.
                    CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                        result,
                        calls,
                        operand_source: None,
                        ..
                    } => {
                        result.statement_index == statement_index
                            && calls.iter().all(|call| {
                                !matches!(
                                    call.operation(),
                                    CheckedUnitEffectOperationPlan::StructuralCall {
                                        coordinate: actual,
                                        ..
                                    } if *actual == coordinate
                                )
                            })
                    }
                    _ => false,
                }
        });
        let Some((operation_index, owner)) = owners.next() else {
            return unsupported("Unit body omits or duplicates an authored call");
        };
        if owners.next().is_some() {
            return unsupported("Unit body omits or duplicates an authored call");
        }
        if let CheckedUnitEffectOperationPlan::EstablishScalarLocal {
            value: checked_trees::CheckedCallScalarArgument::Computation(computation),
            ..
        } = owner
        {
            // A local initializer's computation owns its outer call as well as
            // nested operands. Requiring a second ScalarCall would execute it
            // twice; replay the existing computation's complete call roster.
            crate::expression_preparation::source_custody::validate_computation_calls(
                checked,
                plan.machine,
                plan.state,
                statement_index,
                *computation,
                initializer
                    .or(match statement {
                        StatementNode::Expression(expression) => Some(*expression),
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported("computed value lost its source"))?,
            )?;
        }
        if matches!(statement, StatementNode::Call(call) if call.discards_result)
            && let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                result,
                discard_result_on_return,
                ..
            } = owner
        {
            // The source discard independently demands its continuation. A
            // producer cannot delete cleanup and substitute return-time custody.
            initializers::validate_discarded_structural(
                checked,
                plan.machine,
                plan.state,
                coordinate,
                result,
            )?;
            if *discard_result_on_return
                || (result.multiplicity == language_semantics::Multiplicity::Affine
                    && !matches!(plan.operations.get(operation_index + 1),
                        Some(CheckedUnitEffectOperationPlan::CallContinuationCleanup { coordinate: cleanup, .. })
                            if *cleanup == coordinate))
            {
                return unsupported(
                    "authored discard requires immediate normal continuation cleanup",
                );
            }
        }
        // Selected dispatch may rewrite the source call after checked planning.
        // Its existing exact application owner validates that realization;
        // ordinary calls instead rejoin their captured flow occurrence.
        if let Some(expression) = initializer {
            // EstablishStructuralValue owners carry initializer vocabulary
            // that never produced a checked call (view/alias constructions);
            // there is no flow occurrence left to rejoin.
            if !matches!(
                owner,
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
            ) {
                occurrences::validate(checked, plan.machine, plan.state, coordinate, expression)?;
            }
        } else {
            authored::locate_source(checked, plan.state, coordinate)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_operation(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    caller_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
) -> Result<(), LoweringError> {
    // Selected operators retain their own exact provider application;
    // they are not authored calls with positional argument facts.
    let Some(planned) = operation.call() else {
        return Ok(());
    };
    let coordinate = planned.coordinate();
    if let CheckedUnitCallResult::Structural(result) = planned.result() {
        initializers::validate_structural(
            checked,
            caller_machine,
            caller_state,
            coordinate,
            result,
        )?;
    }
    if let CheckedUnitCallResult::Scalar(result) = planned.result() {
        if initializers::discards_result(checked, caller_state, coordinate)? {
            let call = authored::locate_source(checked, caller_state, coordinate)?;
            let target = authored::target_signature(checked, caller_machine, call.source_target)?;
            if result.statement_index != coordinate.statement_index
                || checked.primitive_type_reference(target.return_type)
                    != Some(result.primitive_type)
            {
                return unsupported("discarded scalar result changed its authored signature");
            }
        } else {
            let (_, state) = crate::expression_preparation::source_custody::authored_state(
                checked,
                caller_state,
            )?;
            let statements = checked
                .typed
                .statement_table
                .statements(state.statement_nodes);
            if let Some(checked_trees::statement::StatementNode::Assignment(_)) =
                statements.get(result.statement_index as usize)
            {
                // A store consuming its own statement's call product has no
                // authored local binding; the consuming store's ScalarResult
                // lane rejoins the value by dense scalar position. The authored
                // assignment's right-hand side is this call, so its exact
                // signature still pins the stored result.
                let call = authored::locate_source(checked, caller_state, coordinate)?;
                let target =
                    authored::target_signature(checked, caller_machine, call.source_target)?;
                if result.statement_index != coordinate.statement_index
                    || checked.primitive_type_reference(target.return_type)
                        != Some(result.primitive_type)
                {
                    return unsupported("stored scalar result changed its authored signature");
                }
            } else {
                // A final scalar call owns the return expression, not an invented
                // local. Completion replay separately checks its exact result binding.
                let role = if result.statement_index as usize + 1 == statements.len()
                    && matches!(
                        statements.last(),
                        Some(checked_trees::statement::StatementNode::Expression(_))
                    ) {
                    CheckedScalarExpressionRole::Return
                } else {
                    // Keyed by the local's authored ordinal, not the dense
                    // binding ordinal its plan assigns.
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal:
                            crate::expression_preparation::source_custody::source_local_ordinal(
                                checked,
                                caller_state,
                                result.statement_index,
                            )?,
                    }
                };
                let source = crate::expression_preparation::source_custody::locate(
                    checked,
                    caller_state,
                    result.statement_index,
                    role,
                )?;
                if source.machine != caller_machine
                    || source.primitive_type != result.primitive_type
                {
                    return unsupported(
                        "call result binding disagrees with its authored scalar local",
                    );
                }
            }
        }
    }
    let arguments = planned.scalar_arguments();
    // A boundary call retains no erased lane.
    let erased_arguments = planned.erased_scalar_arguments().unwrap_or_default();
    let boundary = planned.is_boundary();
    // Only scalar calls and boundary structural calls rejoin their
    // initializer computations here.
    let initializer_owner = match planned.result() {
        CheckedUnitCallResult::Unit => false,
        CheckedUnitCallResult::Scalar(_) => true,
        CheckedUnitCallResult::Structural(_) => boundary,
    };
    if initializer_owner
        && coordinate.call_ordinal == 0
        && arguments.iter().any(|argument| {
            matches!(
                argument,
                checked_trees::CheckedCallScalarArgument::Computation(_)
            )
        })
    {
        initializers::validate(checked, caller_machine, caller_state, coordinate)?;
    }
    let call = authored::locate(
        checked,
        caller_machine,
        caller_state,
        coordinate,
        planned.target_machine(),
        planned.target_state(),
    )?;
    // Unit and scalar calls retain no site to compare.
    if call.boundary != boundary
        || planned
            .source_site()
            .is_some_and(|site| site != call.source_site)
        || call.scalar_arguments.len() != arguments.len() + erased_arguments.len()
    {
        return unsupported("call operands disagree with their authored call site or signature");
    }
    literal_arguments::validate(checked, caller_machine, &call, operation)?;
    boundary_buffers::validate(
        checked,
        caller_state,
        caller_parameters,
        coordinate,
        &call,
        operation,
    )?;
    validate_owned_parameter_arguments(checked, caller_state, caller_parameters, &call, operation)?;
    // Authored scalar operands include the erased positions; the checked plan
    // splits them onto the proof-only erased lane in erased-formal order.
    let mut retained_ordinal = 0usize;
    let mut erased_ordinal = 0usize;
    for (position, (expression, primitive_type)) in call.scalar_arguments.iter().enumerate() {
        if call
            .erased_scalar_positions
            .contains(&u32::try_from(position).map_err(|_| {
                LoweringError::Unsupported("call authored operand position exceeds u32")
            })?)
        {
            let argument =
                erased_arguments
                    .get(erased_ordinal)
                    .ok_or(LoweringError::Unsupported(
                        "call erased operand has no checked actual",
                    ))?;
            let role = CheckedScalarExpressionRole::ErasedUnitCallArgument {
                call_ordinal: coordinate.call_ordinal,
                erased_ordinal: u32::try_from(erased_ordinal).map_err(|_| {
                    LoweringError::Unsupported("call erased operand ordinal exceeds u32")
                })?,
            };
            erased_ordinal += 1;
            let checked_trees::CheckedCallScalarArgument::Pure(argument) = argument else {
                return unsupported("call erased operand requires a pure checked term");
            };
            let (binding, selected) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(caller_state, coordinate.statement_index, role)
                .ok_or(LoweringError::Unsupported(
                    "call erased operand has no unique source-bound checked plan",
                ))?;
            if binding.expression != *expression
                || selected != argument
                || argument.primitive_type() != Some(*primitive_type)
            {
                return unsupported("call erased operand disagrees with its authored argument");
            }
            crate::expression_preparation::source_custody::validate_pure(
                checked,
                binding,
                terminal_scalar_type(*primitive_type)?,
            )?;
            continue;
        }
        let argument = arguments
            .get(retained_ordinal)
            .ok_or(LoweringError::Unsupported(
                "call scalar operand has no checked actual",
            ))?;
        let argument_ordinal = u32::try_from(retained_ordinal)
            .map_err(|_| LoweringError::Unsupported("call scalar operand ordinal exceeds u32"))?;
        retained_ordinal += 1;
        let role = if boundary {
            CheckedScalarExpressionRole::BoundaryCallArgument {
                call_ordinal: coordinate.call_ordinal,
                argument_ordinal,
            }
        } else {
            CheckedScalarExpressionRole::UnitCallArgument {
                call_ordinal: coordinate.call_ordinal,
                argument_ordinal,
            }
        };
        match argument {
            checked_trees::CheckedCallScalarArgument::Pure(argument) => {
                let (binding, selected) = checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(caller_state, coordinate.statement_index, role)
                    .ok_or(LoweringError::Unsupported(
                        "call scalar operand has no unique source-bound checked plan",
                    ))?;
                if binding.expression != *expression
                    || binding.destination.is_valid()
                    || selected != argument
                    || argument.primitive_type() != Some(*primitive_type)
                {
                    return unsupported("call scalar operand disagrees with its authored argument");
                }
                crate::expression_preparation::source_custody::validate_pure(
                    checked,
                    binding,
                    terminal_scalar_type(*primitive_type)?,
                )?;
            }
            checked_trees::CheckedCallScalarArgument::Computation(computation) => {
                let plans = &checked.facts.values.scalar_computations;
                let root = plans
                    .root_at(caller_state, coordinate.statement_index, role)
                    .ok_or(LoweringError::Unsupported(
                        "call scalar operand has no unique checked computation root",
                    ))?;
                if root.machine != caller_machine
                    || root.root != *computation
                    || !plans.nodes.is_valid(*computation)
                {
                    return unsupported("call scalar operand disagrees with its computation root");
                }
                let node = plans.nodes.get(*computation);
                if node.authored_root != *expression || node.primitive_type != *primitive_type {
                    return unsupported("call computation disagrees with its authored argument");
                }
                crate::expression_preparation::source_custody::validate_computation_calls(
                    checked,
                    caller_machine,
                    caller_state,
                    coordinate.statement_index,
                    *computation,
                    *expression,
                )?;
            }
        }
    }
    Ok(())
}

/// Graph and straight-line calls share this source join. A compatible type and
/// a valid final ownership frontier do not identify which same-typed parameter
/// the author supplied; even an unrestricted value retains that distinction.
fn validate_owned_parameter_arguments(
    checked: &CheckedTrees,
    caller_state: symbols::SymbolHandle,
    caller_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    call: &authored::AuthoredCall,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let Some(planned) = operation.call() else {
        return Ok(());
    };
    let structural_arguments = planned.structural_arguments();
    if !structural_arguments.iter().any(|argument| {
        argument.source_parameter_index().is_some()
            && argument.access == checked_trees::CheckedStructuralAccess::Owned
            && argument.path.is_empty()
    }) {
        return Ok(());
    }
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    let target = authored::target_signature(checked, machine.symbol, call.source_target)?;
    let positions =
        literal_arguments::structural_positions(checked, &target, structural_arguments.len())?;
    for (argument, position) in structural_arguments.iter().zip(positions) {
        let Some(parameter_index) = argument.source_parameter_index() else {
            continue;
        };
        if argument.access != checked_trees::CheckedStructuralAccess::Owned
            || !argument.path.is_empty()
        {
            continue;
        }
        let source =
            caller_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "owned call argument has no source parameter",
                ))?;
        let parameter = checked
            .state_parameters(state)
            .get(source.position as usize)
            .ok_or(LoweringError::Unsupported(
                "owned call argument has no authored parameter",
            ))?;
        let expression = call
            .structural_arguments
            .iter()
            .find_map(|(formal, expression)| (*formal == position).then_some(*expression));
        // Implicit receivers have no positional expression. Their dedicated
        // receiver custody rejoins the authored root and projection instead.
        if expression.is_none() && target.parameters[position as usize].is_self {
            continue;
        }
        // Explicit `self` is rooted at the enclosing machine, while the
        // signature keeps a distinct parameter symbol. Rejoin only that
        // receiver alias, as for projected receivers; another same-typed
        // parameter still cannot stand in for the authored argument.
        if !expression.is_some_and(|expression| {
            matches!(checked.expression_table.expression(expression),
            checked_trees::expression::ExpressionNode::Name(name)
                if (name.symbol == parameter.symbol
                    || (parameter.is_self && name.symbol == machine.symbol))
                    && name.head_symbol == name.symbol
                    && checked.expression_table.name_path_members(name.members).len() == 1)
        }) {
            return unsupported("owned call argument changed its authored parameter");
        }
    }
    Ok(())
}
