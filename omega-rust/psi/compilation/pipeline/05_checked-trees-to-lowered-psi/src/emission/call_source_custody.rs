//! Source custody shared by ordinary and boundary calls, independent of result.

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
        // An erased fact-only citation owes no call occurrence.
        if validation::is_fact_only_citation(&checked.typed, statement) {
            continue;
        }
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
                // An exclusive `&mut` view-call loan (`let slots: &mut [T] =
                // self.items.as_mut_slice()`) plans no operation: the
                // language operation records no flow call, the local binds a
                // loan rather than a value, and every write through it
                // rejoins the viewed place. Nothing owes its initializer a
                // call operation, so it leaves this audit like the marker
                // locals the body count already skips.
                if crate::expression_preparation::source_custody::is_exclusive_view_call_loan_marker(
                    checked, local,
                ) {
                    continue;
                }
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
                .is_some_and(|call| call.coordinate == coordinate)
                || match operation {
                    CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                        result,
                        value: checked_trees::CheckedCallScalarArgument::Computation(_),
                    } => result.statement_index == statement_index,
                    // A mutable local's storage initializer computation owns
                    // its outer call exactly as an immutable local's does.
                    CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                        statement_index: established,
                        value: checked_trees::CheckedCallScalarArgument::Computation(_),
                        ..
                    } => *established == statement_index,
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
                                !call.operation().call().is_some_and(|planned| {
                                    !planned.is_boundary()
                                        && planned.structural_result().is_some()
                                        && planned.coordinate == coordinate
                                })
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
        }
        | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
            value: checked_trees::CheckedCallScalarArgument::Computation(computation),
            ..
        } = owner
        {
            // A local initializer's computation owns its outer call as well as
            // nested operands. Requiring a second scalar call operation would
            // execute it twice; replay the existing computation's complete call
            // roster.
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
            && let Some(owner_call) = owner.call().filter(|call| call.is_boundary())
            && let CheckedUnitCallResult::Structural {
                binding: result,
                discard_result_on_return,
            } = owner_call.result()
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

/// A projected Fused binding selects an erased capability, not an implicit
/// runtime borrow. Its boundary signature has no self operand, so argument
/// access facts cannot stand in for the exact parameter and field custody.
/// The replayed parameter and path let attachment validation retain the exact
/// field requirement without mistaking an ordinary context for lexical self.
pub(crate) struct ParameterBindingReceiver<'parameters> {
    pub parameter: &'parameters checked_trees::CheckedUnitStructuralParameterPlan,
    pub path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
}

pub(crate) fn validate_parameter_binding_receiver<'parameters>(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    caller_parameters: &'parameters [checked_trees::CheckedUnitStructuralParameterPlan],
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    call: &authored::AuthoredCall,
) -> Result<Option<ParameterBindingReceiver<'parameters>>, LoweringError> {
    use checked_trees::CheckedUnitStructuralTypeShape;
    use checked_trees::expression::ExpressionNode;
    use checked_trees::statement::StatementNode;
    use checked_trees::{CheckedUnitStructuralFieldType, CheckedUnitStructuralPathSegment};

    let Some(owner) = checked.traits().iter().find(|definition| {
        definition.is_boundary
            && checked
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == call.source_target)
    }) else {
        // Boundary machines and operators have no Binding requirement receiver.
        return Ok(None);
    };
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    if machine.symbol != caller_machine {
        return unsupported("binding receiver belongs to another authored machine");
    }
    let (root, path, carrier) = match call.source_site {
        Some(checked_trees::NominalMachineUseSite::Expression(expression)) => {
            let ExpressionNode::Call(source) = checked.expression_table.expression(expression)
            else {
                return unsupported("binding receiver lost its authored call");
            };
            if !source.receiver.is_valid()
                || !matches!(
                    checked.expression_table.expression(source.receiver),
                    ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
                )
            {
                return Ok(None);
            }
            let projected = projected_receivers::source(
                checked,
                caller_machine,
                caller_state,
                coordinate.statement_index as usize,
                source.receiver,
            )?;
            let carrier = validation::declared_place_type_raw(
                &checked.typed,
                machine,
                Some(state),
                source.receiver,
            )
            .ok_or(LoweringError::Unsupported(
                "binding receiver has no exact declared type",
            ))?;
            (projected.root, projected.path, carrier)
        }
        Some(checked_trees::NominalMachineUseSite::Statement(_)) => {
            let Some(statement @ StatementNode::Call(source)) = checked
                .statement_table
                .statements(state.statement_nodes)
                .get(coordinate.statement_index as usize)
            else {
                return unsupported("binding receiver lost its authored statement");
            };
            let members = checked.statement_table.name_path_members(source.receiver);
            let Some(parameter) = checked.state_parameters(state).iter().find(|parameter| {
                !parameter.is_self
                    && (parameter.symbol == source.receiver_root_symbol
                        || members.first() == Some(&parameter.name))
            }) else {
                return Ok(None);
            };
            if members.len() < 2 {
                return Ok(None);
            }
            if members.first() != Some(&parameter.name)
                || parameter.symbol != source.receiver_root_symbol
                || !source.receiver_symbol.is_valid()
            {
                return unsupported("binding receiver changed its parameter or field identity");
            }
            let (root, path) = crate::unit::attached_unit::statement_receiver_path(
                checked, machine, state, statement,
            )?;
            // The shared path validator has checked the endpoint against its
            // exact declared owner. Recover that field's type, not its spelling.
            let mut fields = checked
                .data_definitions()
                .iter()
                .flat_map(|definition| checked.data_members(definition))
                .filter_map(|member| match member {
                    checked_trees::data::DataMember::Field(field)
                        if field.symbol == source.receiver_symbol =>
                    {
                        Some(field)
                    }
                    _ => None,
                });
            let field = fields.next().ok_or(LoweringError::Unsupported(
                "binding receiver endpoint has no declared field",
            ))?;
            if fields.next().is_some() {
                return unsupported("binding receiver endpoint has multiple declarations");
            }
            (root, path, field.type_reference)
        }
        _ => return Ok(None),
    };
    let Some((position, source_parameter)) = checked
        .state_parameters(state)
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == root && !parameter.is_self)
    else {
        // Attached self and local receiver routes retain their existing owners.
        return Ok(None);
    };
    if path.is_empty() {
        return Ok(None);
    }
    let mut parameters = caller_parameters
        .iter()
        .filter(|parameter| parameter.position as usize == position);
    let parameter = parameters.next().ok_or(LoweringError::Unsupported(
        "binding receiver lost its retained structural parameter",
    ))?;
    if parameters.next().is_some()
        || parameter.is_self
        || source_parameter.relevance.is_erased()
        || parameter.fused_service_erasure.is_some()
    {
        return unsupported("binding receiver has inconsistent ordinary parameter custody");
    }
    let source_type =
        validation::unwrapped_type_reference(&checked.typed, source_parameter.type_reference)
            .ok_or(LoweringError::Unsupported(
                "binding receiver parameter has no declared carrier",
            ))?;
    if parameter.type_identity != checked.normalized_type_identity(source_type).as_str() {
        return unsupported("binding receiver parameter changed its retained carrier identity");
    }
    let requirement =
        typed_trees::service::exact_bound_service_requirement(&checked.typed, carrier).ok_or(
            LoweringError::Unsupported("parameter receiver is not an exact Binding carrier"),
        )?;
    if owner.symbol != requirement || checked.fused_service_erasure(requirement).is_none() {
        return unsupported("binding receiver lacks its exact Fused requirement authorization");
    }
    let mut identity = parameter.type_identity.as_str();
    let mut endpoint = None;
    for (position, segment) in path.iter().enumerate() {
        let shape = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .find(|shape| shape.identity == identity)
            .ok_or(LoweringError::Unsupported(
                "binding receiver lost its retained carrier shape",
            ))?;
        match (segment, &shape.shape) {
            (
                CheckedUnitStructuralPathSegment::Field(name),
                CheckedUnitStructuralTypeShape::Record { fields },
            ) => {
                let mut fields = fields.iter().filter(|field| field.identity == *name);
                let field = fields.next().ok_or(LoweringError::Unsupported(
                    "binding receiver lost its retained field",
                ))?;
                if fields.next().is_some() || field.relevance.is_erased() {
                    return unsupported("binding receiver has inconsistent retained field custody");
                }
                endpoint = Some(&field.field_type);
                if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                    &field.field_type
                {
                    identity = type_identity;
                } else if position + 1 != path.len() {
                    return unsupported("binding receiver projects through a nonstructural field");
                }
            }
            (
                CheckedUnitStructuralPathSegment::FixedIndex(index),
                CheckedUnitStructuralTypeShape::FixedArray {
                    element_type_identity,
                    length,
                },
            ) if *index < *length => {
                identity = element_type_identity;
                endpoint = None;
            }
            _ => {
                return unsupported(
                    "binding receiver projection disagrees with its retained carrier",
                );
            }
        }
    }
    if !matches!(endpoint, Some(CheckedUnitStructuralFieldType::FusedServiceBacked {
        provider_type_identity, erasure,
    }) if erasure.requirement == requirement
        && provider_type_identity == checked.normalized_type_identity(carrier).as_str())
    {
        return unsupported("binding receiver lost its exact Fused field erasure receipt");
    }
    Ok(Some(ParameterBindingReceiver { parameter, path }))
}

/// Rejoin an already located call's observations without interpreting its
/// result's destination. Constructor member calls own a nested occurrence
/// whose destination is authenticated by the structural-value source owner.
pub(crate) fn validate_observations(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    call: &authored::AuthoredCall,
) -> Result<(), LoweringError> {
    // Every call family authenticates the captured ordered observations. A
    // geometrically valid sibling or equally typed local is not evidence for
    // the authored operand's loan; result shape cannot substitute this join.
    let captured = crate::expression_preparation::source_custody::borrow_occurrences::call(
        checked,
        caller_machine,
        caller_state,
        coordinate,
        call.source_target,
    )?;
    crate::expression_preparation::source_custody::computation_calls::replay_call_accesses(
        checked,
        caller_machine,
        caller_state,
        coordinate.statement_index,
        captured,
        &call.explicit_arguments,
    )?;
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
    let coordinate = planned.coordinate;
    if let CheckedUnitCallResult::Structural {
        binding: result, ..
    } = planned.result()
    {
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
            let statement = statements.get(result.statement_index as usize);
            let statement_result =
                matches!(
                    statement,
                    Some(checked_trees::statement::StatementNode::Assignment(_))
                ) || (matches!(
                    statement,
                    Some(checked_trees::statement::StatementNode::Transition(_))
                ) && authored::locate_source(checked, caller_state, coordinate)?
                    .source_site
                    .is_none());
            if statement_result {
                // Stores and named terminal invocations consume the call's
                // product directly, without an authored scalar local. Rejoin
                // its exact signature here; the store or completion owner
                // independently checks which result it consumes. A nested
                // guard/operand call still has its own expression use-site.
                let call = authored::locate_source(checked, caller_state, coordinate)?;
                let target =
                    authored::target_signature(checked, caller_machine, call.source_target)?;
                if result.statement_index != coordinate.statement_index
                    || checked.primitive_type_reference(target.return_type)
                        != Some(result.primitive_type)
                {
                    return unsupported("statement scalar result changed its authored signature");
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
    let arguments = &planned.scalar_arguments;
    // A boundary call retains no erased lane.
    let erased_arguments = planned.erased_scalar_arguments().unwrap_or_default();
    let boundary = planned.is_boundary();
    let call = authored::locate(
        checked,
        caller_machine,
        caller_state,
        coordinate,
        planned.target.machine,
        planned.target.state,
    )?;
    validate_observations(checked, caller_machine, caller_state, coordinate, &call)?;
    // A named terminal invocation has no initializer expression. Its exact
    // statement target and signature were rejoined above; every operand and
    // nested call still rejoins its checked source below. Expression-result
    // calls additionally retain their initializer's captured outer occurrence.
    let initializer_owner = match planned.result() {
        CheckedUnitCallResult::Unit => false,
        CheckedUnitCallResult::Scalar(_) => true,
        CheckedUnitCallResult::Structural { .. } => boundary,
    };
    if initializer_owner
        && call.source_site.is_some()
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
    if boundary {
        validate_parameter_binding_receiver(
            checked,
            caller_machine,
            caller_state,
            caller_parameters,
            coordinate,
            &call,
        )?;
    }
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
    let replayed_byte_arguments = boundary_buffers::validate(
        checked,
        caller_state,
        caller_parameters,
        coordinate,
        &call,
        operation,
    )?;
    validate_parameter_arguments(
        checked,
        caller_state,
        caller_parameters,
        &call,
        operation,
        &replayed_byte_arguments,
    )?;
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
fn validate_parameter_arguments(
    checked: &CheckedTrees,
    caller_state: symbols::SymbolHandle,
    caller_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    call: &authored::AuthoredCall,
    operation: &CheckedUnitEffectOperationPlan,
    replayed_byte_arguments: &[usize],
) -> Result<(), LoweringError> {
    let Some(planned) = operation.call() else {
        return Ok(());
    };
    let structural_arguments = &planned.structural_arguments;
    if !structural_arguments
        .iter()
        .any(|argument| argument.source_parameter_index().is_some())
    {
        return Ok(());
    }
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    let target = authored::target_signature(checked, machine.symbol, call.source_target)?;
    let positions = crate::unit::attached_unit::call_parameters::structural_positions(
        checked, planned, &target,
    )?;
    for (ordinal, (argument, position)) in structural_arguments.iter().zip(positions).enumerate() {
        let Some(parameter_index) = argument.source_parameter_index() else {
            continue;
        };
        let source =
            caller_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "call argument has no source parameter",
                ))?;
        let parameter = checked
            .state_parameters(state)
            .get(source.position as usize)
            .ok_or(LoweringError::Unsupported(
                "call argument has no authored parameter",
            ))?;
        if !parameter.symbol.is_valid() || source.is_self != parameter.is_self {
            return unsupported("call argument changed its authored parameter place");
        }
        // The range owner above has already reconstructed both backing and
        // exact view geometry. Its successful replay is result-independent;
        // a view adaptation must not be read again as a plain storage path.
        if replayed_byte_arguments.contains(&ordinal) {
            continue;
        }
        let expression = call.structural_argument(
            checked,
            position,
            target.parameters[position as usize].is_self,
        );
        // Implicit receivers have no positional expression. Their dedicated
        // receiver custody rejoins the authored root and projection instead.
        if expression.is_none() && target.parameters[position as usize].is_self {
            continue;
        }
        let expression = expression.ok_or(LoweringError::Unsupported(
            "call parameter argument has no authored expression",
        ))?;
        if !checked.expression_table.expression_is_valid(expression) {
            return unsupported("call parameter argument has a stale authored expression");
        }
        let (expression, explicit_access) = match checked.expression_table.expression(expression) {
            checked_trees::expression::ExpressionNode::Borrow(borrow) => {
                let access = match borrow.access {
                    language_core::ReferenceAccess::Shared => {
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                    }
                    language_core::ReferenceAccess::Mutable => {
                        checked_trees::CheckedStructuralAccess::MutableBorrow
                    }
                    language_core::ReferenceAccess::WriteOnly => {
                        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
                    }
                };
                (borrow.target, Some(access))
            }
            _ => (expression, None),
        };
        let (root, path) = parameter_place(
            checked,
            machine,
            state,
            planned.coordinate.statement_index as usize,
            expression,
        )?;
        // Parameter identity and projection are independent of the callee's
        // result and the operand's access. Computed operands instead rejoin
        // their own producer during ordered argument staging; they are not
        // parameter aliases merely because their backing is a parameter.
        if root != parameter.symbol
            || path != argument.path
            || explicit_access.is_some_and(|access| {
                access != argument.access
                    && !(access == checked_trees::CheckedStructuralAccess::MutableBorrow
                        && argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow)
            })
        {
            return unsupported("call argument changed its authored parameter place");
        }
    }
    Ok(())
}

/// Reconstruct storage identity, not permission to use that storage. Immutable
/// reference bindings substitute their authored initializer regardless of the
/// call's result or the loan's restoration class. Borrow retention separately
/// checks the use interval, access and reactivation evidence. The receiver-only
/// alias validator cannot own this join: it also forbids explicit argument uses.
fn parameter_place(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    mut before_statement: usize,
    mut expression: checked_trees::expression::ExpressionHandle,
) -> Result<
    (
        symbols::SymbolHandle,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    ),
    LoweringError,
> {
    use checked_trees::expression::ExpressionNode;
    use checked_trees::types::TypeReferenceNode;

    let mut path = Vec::new();
    let mut visited = Vec::new();
    loop {
        if !checked.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            return unsupported("call parameter place has a stale or cyclic initializer");
        }
        visited.push(expression);
        match checked.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => {
                expression = borrow.target;
                continue;
            }
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                expression = cast.value;
                continue;
            }
            _ => {}
        }
        // Rebuilding a path must not reinterpret an authored index operator
        // as a builtin storage projection. This checks the entire place spine.
        if !validation::place_has_builtin_coordinates(
            &checked.typed,
            machine,
            Some(state),
            expression,
        ) {
            return unsupported("call parameter place has no builtin coordinate meaning");
        }
        let (mut prefix, root) =
            crate::expression_preparation::source_custody::structural_places::walk_exact_place(
                checked, machine, state, expression, true,
            )?;
        prefix.append(&mut path);
        path = prefix;
        let ExpressionNode::Name(name) = checked.expression_table.expression(root) else {
            return unsupported("call parameter place has no named storage root");
        };
        if !name.symbol.is_valid()
            || name.head_symbol != name.symbol
            || checked
                .expression_table
                .name_path_members(name.members)
                .len()
                != 1
            || checked
                .expression_table
                .name_path_member_symbols(name.member_symbols)
                .first()
                .is_some_and(|symbol| *symbol != name.symbol)
        {
            return unsupported("call parameter place changed its root identity");
        }
        if let Some((declaration, local)) = checked.state_local_declaration(state, name.symbol) {
            if declaration >= before_statement
                || local.is_mutable
                || !matches!(
                    checked
                        .type_reference_table
                        .type_reference(local.type_reference),
                    TypeReferenceNode::Reference { .. }
                )
            {
                return unsupported("call parameter alias is not an earlier immutable reference");
            }
            before_statement = declaration;
            expression = local.initial_value;
            continue;
        }
        // The existing root owner normalizes lexical self and inherited fields
        // to the exact formal. No local alias reaches its receiver-use policy.
        let source = projected_receivers::source(
            checked,
            machine.symbol,
            state.symbol,
            before_statement,
            root,
        )?;
        let mut prefix = source.path;
        prefix.append(&mut path);
        return Ok((source.root, prefix));
    }
}
