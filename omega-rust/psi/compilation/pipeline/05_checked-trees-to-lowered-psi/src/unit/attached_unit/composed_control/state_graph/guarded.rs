//! Ordered exits use the same source-coordinate coverage check as scalar returns.
//! Their values are ordinary structural producers, evaluated only after selecting
//! an exit. Dependency discovery visits every arm; execution never does so.
use super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, ClaimId, PermissionClaimIdentity,
    StructuralParameterDeclaration, SuccessorEdge, block_id,
};
use super::super::super::{
    Block, CheckedUnitEffectOperationPlan, Multiplicity, ScalarType, Terminator, ValueDeclaration,
    allocate_dense, edge_id, unsupported,
};
use super::super::{CheckedTrees, LoweringError, catalogs};
use super::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan, case_emission,
    edges,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::{CheckedScalarBranchDestination, expression::ExpressionNode};

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    start: usize,
) -> Result<(), LoweringError> {
    let (
        checked_trees::CheckedControlResultPlan::Structural(signature),
        CheckedComposedUnitControlTerminatorPlan::Guarded {
            arms,
            fallback,
            return_values,
        },
    ) = (&plan.result, &state.terminator)
    else {
        return unsupported("ordered structural exits lost their result or control");
    };
    let mut retained = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .iter()
        .filter(|tail| tail.state == state.state);
    if retained
        .next()
        .is_none_or(|tail| tail.arms != *arms || tail.fallback != *fallback)
        || retained.next().is_some()
        || crate::expression_preparation::source_custody::guarded_exits::validate(
            checked,
            state.state,
            *arms,
            fallback.as_ref(),
        )? != start
    {
        return unsupported("guarded structural tail changed its source-owned roster");
    }
    let guards = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(*arms)
        .ok_or(LoweringError::Unsupported(
            "guarded structural guards are stale",
        ))?;
    if return_values.len() != guards.len() + usize::from(fallback.is_some())
        || signature.multiplicity == Multiplicity::Linear
    {
        return unsupported("guarded structural exit coverage or result custody drifted");
    }
    let mut bindings = Vec::new();
    for operation in state
        .operation_dependencies()
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
    {
        let Some(result) = operation.structural_result_binding() else {
            continue;
        };
        if bindings.contains(&result.binding_ordinal) {
            return unsupported("ordered structural exits duplicate a result binding");
        }
        bindings.push(result.binding_ordinal);
    }
    let statements = checked.statement_table.statements(source.statement_nodes);
    let reference_result = !plan.result_reference_sources.is_empty();
    let mut returned_reference_sources = Vec::new();
    for (destination, operation) in guards
        .iter()
        .map(|guard| &guard.destination)
        .chain(fallback.iter())
        .zip(return_values)
    {
        let CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } = destination
        else {
            return unsupported("ordered structural destination is not its retained value return");
        };
        let expression = match statements.get(*statement_ordinal as usize) {
            Some(checked_trees::statement::StatementNode::Expression(expression))
                if !is_continuation =>
            {
                *expression
            }
            Some(checked_trees::statement::StatementNode::Transition(transition))
                if transition.exit == checked_trees::statement::TransitionExit::Ordinary =>
            {
                let target = if *is_continuation {
                    transition.continuation
                } else {
                    transition.target
                };
                let checked_trees::statement::TransitionTargetNode::Value(expression) =
                    checked.statement_table.transition_target(target)
                else {
                    return unsupported("ordered return substituted a named destination");
                };
                *expression
            }
            _ => return unsupported("ordered return has no exact authored expression"),
        };
        let result = match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                discard_result_on_return: false,
                ..
            }
            | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. }
                if !reference_result =>
            {
                result
            }
            CheckedUnitEffectOperationPlan::Call(call)
                if !reference_result && call.discard_result_on_return() == Some(false) =>
            {
                call.structural_result().ok_or(LoweringError::Unsupported(
                    "ordered return call has no structural result",
                ))?
            }
            CheckedUnitEffectOperationPlan::EstablishReference {
                result,
                source: reference_source,
            } if reference_result => {
                validate_reference_return(
                    checked,
                    plan,
                    source,
                    state,
                    expression,
                    *statement_ordinal,
                    operation,
                )?;
                if !returned_reference_sources.contains(reference_source) {
                    returned_reference_sources.push(reference_source.clone());
                }
                result
            }
            _ => return unsupported("ordered return lost its structural producer"),
        };
        if result.statement_index != *statement_ordinal
            || result.type_identity != signature.type_identity
            || result.multiplicity != signature.multiplicity
        {
            return unsupported("ordered return exchanged its destination or value custody");
        }
        if let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation {
            let root = checked
                .facts
                .values
                .structural_values
                .root_for_expression(state.state, *statement_ordinal, expression)
                .ok_or(LoweringError::Unsupported(
                    "ordered return lost its exact value root",
                ))?;
            if root.root != *value {
                return unsupported("ordered return exchanged its authored value");
            }
            crate::expression_preparation::source_custody::structural::validate(
                checked,
                plan.machine,
                state.state,
                operation,
            )?;
        } else if operation.call().is_some() {
            crate::expression_preparation::source_custody::structural::validate_call_value(
                checked,
                plan.machine,
                state.state,
                expression,
                operation,
                &state.structural_parameters,
            )?;
        } else if matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
        ) {
            super::super::super::byte_locals::validate(checked, state.state, operation)?;
        }
    }
    if reference_result
        && (plan.result_reference_sources.iter().any(|candidate| {
            !candidate.path.is_empty() || !returned_reference_sources.contains(&candidate.source)
        }) || returned_reference_sources.len() != plan.result_reference_sources.len())
    {
        return unsupported("ordered reference returns do not cover their candidate roster");
    }
    edges::return_discards(checked, plan.machine, source, state)?;
    Ok(())
}

/// Rejoin a selected reference producer to its authored return expression.
/// Candidate coverage is machine-wide; this check independently authenticates
/// the source of this arm even when its candidate receipt is changed with it.
pub(super) fn validate_reference_return(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    authored: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    expression: checked_trees::expression::ExpressionHandle,
    statement_ordinal: u32,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let (
        checked_trees::CheckedControlResultPlan::Structural(signature),
        CheckedUnitEffectOperationPlan::EstablishReference { result, source },
    ) = (&plan.result, operation)
    else {
        return unsupported("reference return lost its structural producer or signature");
    };
    if result.statement_index != statement_ordinal
        || result.type_identity != signature.type_identity
        || result.multiplicity != signature.multiplicity
    {
        return unsupported("reference return exchanged its destination or value custody");
    }
    let expected = reference_source(checked, plan, authored, state, expression)?;
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
        &source.source
    else {
        return unsupported("reference return lost its ingress parameter");
    };
    let parameter = state
        .structural_parameters
        .get(*parameter_index as usize)
        .ok_or(LoweringError::Unsupported(
            "reference return ingress parameter is absent",
        ))?;
    if expected != *source
        || parameter.access != source.access
        || parameter.multiplicity != Multiplicity::Unrestricted
        || !parameter.qualifications.is_empty()
        || plan
            .result_reference_sources
            .iter()
            .filter(|candidate| candidate.path.is_empty() && candidate.source == *source)
            .count()
            != 1
    {
        return unsupported("reference return differs from its exact candidate ingress");
    }
    Ok(())
}

/// Compare the receipt against actual return producers across every state, not
/// against one dispatch's subset. Body temporaries and successor arguments do
/// not contribute candidates; repeated return arms share a single candidate.
pub(super) fn validate_reference_roster(
    plan: &CheckedComposedUnitControlMachinePlan,
) -> Result<(), LoweringError> {
    use checked_trees::{
        CheckedClosedSumCaseOutcomePlan, CheckedGuardedJumpArmPlan, CheckedGuardedJumpFallbackPlan,
    };
    let mut returned = Vec::new();
    for state in &plan.states {
        let mut operations = Vec::new();
        match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } => {
                operations.extend(return_values);
            }
            CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } => {
                operations.extend(cases.iter().filter_map(|case| match &case.outcome {
                    CheckedClosedSumCaseOutcomePlan::Return(arm) => arm.operation(),
                    CheckedClosedSumCaseOutcomePlan::Successor(_) => None,
                }));
            }
            CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { return_arm, .. } => {
                operations.extend(return_arm.operation());
            }
            CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
                when_true,
                when_false,
                ..
            } => {
                operations.extend(when_true.operation());
                operations.extend(when_false.operation());
            }
            CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } => {
                operations.extend(arms.iter().filter_map(|arm| match arm {
                    CheckedGuardedJumpArmPlan::Return(arm) => arm.return_arm.operation(),
                    CheckedGuardedJumpArmPlan::Jump(_) => None,
                }));
                if let CheckedGuardedJumpFallbackPlan::Return(arm) = fallback {
                    operations.extend(arm.operation());
                }
            }
            CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result } => {
                // Ordinary whole-value returns can forward an existing call
                // or parameter carrier. Their source validator owns that
                // receipt; do not substitute an unrelated prefix producer.
                for candidate in &result.reference_sources {
                    if !candidate.path.is_empty() {
                        return unsupported("whole reference return has a nested candidate");
                    }
                    if !returned.contains(&candidate.source) {
                        returned.push(candidate.source.clone());
                    }
                }
            }
            _ => {}
        }
        for operation in operations {
            if let CheckedUnitEffectOperationPlan::EstablishReference { source, .. } = operation
                && !returned.contains(source)
            {
                returned.push(source.clone());
            }
        }
    }
    if returned.len() != plan.result_reference_sources.len()
        || plan
            .result_reference_sources
            .iter()
            .enumerate()
            .any(|(ordinal, candidate)| {
                !candidate.path.is_empty()
                    || !returned.contains(&candidate.source)
                    || plan.result_reference_sources[..ordinal].contains(candidate)
            })
    {
        return unsupported("reference returns do not cover their candidate roster");
    }
    Ok(())
}

/// Rebuild one selected `&mut place` arm from authored syntax and the checked
/// loan ledger. The composed plan's candidate roster is only a correspondence
/// receipt: admission must not let a producer and that receipt be forged to a
/// different, same-typed ingress together.
fn reference_source(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    authored: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    expression: checked_trees::expression::ExpressionHandle,
) -> Result<checked_trees::CheckedUnitStructuralArgumentPlan, LoweringError> {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
        .ok_or(LoweringError::Unsupported(
            "ordered reference return lost its machine",
        ))?;
    let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(expression) else {
        return unsupported("ordered reference return lost its authored borrow");
    };
    if borrow.access != language_core::ReferenceAccess::Mutable {
        return unsupported("ordered reference return changed its mutable access");
    }
    let (referent, access) = validation::reference_result_custody::mutable_borrowed_parts(
        &checked.typed,
        authored.return_type,
    )
    .ok_or(LoweringError::Unsupported(
        "ordered reference return lost its mutable result type",
    ))?;
    let leaf = validation::expression_result_type_reference(
        &checked.typed,
        machine,
        authored,
        borrow.target,
    )
    .ok_or(LoweringError::Unsupported(
        "ordered reference return lost its target type",
    ))?;
    if checked.normalized_type_identity(leaf) != checked.normalized_type_identity(referent) {
        return unsupported("ordered reference return changed its referent type");
    }
    let (mut path, root) =
        crate::expression_preparation::source_custody::structural_places::walk_exact_place(
            checked,
            machine,
            authored,
            borrow.target,
            true,
        )?;
    if !path.iter().all(|segment| {
        matches!(
            segment,
            checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
        )
    }) {
        return unsupported("ordered reference return has a non-static target path");
    }
    let ExpressionNode::Name(root) = checked.expression_table.expression(root) else {
        return unsupported("ordered reference return lost its named target root");
    };
    if root.head_symbol != root.symbol
        || checked
            .expression_table
            .name_path_members(root.members)
            .len()
            != 1
    {
        return unsupported("ordered reference return target root is not exact");
    }
    let parameters = checked.state_parameters(authored);
    let ingress_position = |symbol| {
        parameters.iter().position(|parameter| {
            parameter.symbol == symbol || (parameter.is_self && symbol == machine.symbol)
        })
    };
    let mut root_symbol = root.symbol;
    let borrow_state = {
        let mut states = checked
            .facts
            .borrow
            .states
            .iter()
            .map(|(_, candidate)| candidate)
            .filter(|candidate| {
                candidate.machine_symbol == machine.symbol
                    && candidate.state_symbol == authored.symbol
            });
        let state = states.next().ok_or(LoweringError::Unsupported(
            "ordered reference return lost its borrow state",
        ))?;
        if states.next().is_some() {
            return unsupported("ordered reference return has ambiguous borrow states");
        }
        state
    };
    for _ in 0..8 {
        if ingress_position(root_symbol).is_some() {
            break;
        }
        // A view over an attached field can publish the field declaration as
        // its loan root. Rejoin that declaration to self and restore its path
        // segment before following the authored element suffix. Neither the
        // candidate result roster nor a same-typed field supplies this identity.
        let mut attached_fields = checked
            .data_definitions()
            .iter()
            .filter(|data| data.symbol == machine.attached_data_symbol)
            .flat_map(|data| checked.data_members(data))
            .filter_map(|member| match member {
                checked_trees::data::DataMember::Field(field) if field.symbol == root_symbol => {
                    Some(field)
                }
                _ => None,
            });
        if let Some(field) = attached_fields.next() {
            if attached_fields.next().is_some() {
                return unsupported("ordered reference return has ambiguous attached fields");
            }
            path.insert(
                0,
                checked_trees::CheckedUnitStructuralPathSegment::Field(field.path_identity()),
            );
            root_symbol = machine.symbol;
            break;
        }
        let mut loans = checked.facts.borrow.loans.iter().filter(|(handle, loan)| {
            checked.facts.borrow.state_owns_loan(borrow_state, *handle)
                && loan.owner_symbol == root_symbol
                && loan.kind == checked_trees::BorrowAccessKind::Mutable
                && checked.facts.borrow.loan_owner_path(loan).is_empty()
        });
        let (_, loan) = loans.next().ok_or(LoweringError::Unsupported(
            "ordered reference return target has no exact parent loan",
        ))?;
        if loans.next().is_some() {
            return unsupported("ordered reference return target has ambiguous parent loans");
        }
        let mut parent = checked
            .facts
            .borrow
            .loan_segments(loan)
            .iter()
            .map(|segment| guarded_loan_segment(checked, segment))
            .collect::<Result<Vec<_>, _>>()?;
        parent.append(&mut path);
        path = parent;
        root_symbol = loan.root_symbol;
    }
    let position = ingress_position(root_symbol).ok_or(LoweringError::Unsupported(
        "ordered reference return does not reach an ingress parameter",
    ))?;
    let parameter = &parameters[position];
    if !parameter.is_mutable
        && validation::reference_result_custody::mutable_borrowed_parts(
            &checked.typed,
            parameter.type_reference,
        )
        .is_none()
    {
        return unsupported("ordered reference return ingress is not mutable");
    }
    let parameter_index = state
        .structural_parameters
        .iter()
        .position(|candidate| candidate.position as usize == position)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or(LoweringError::Unsupported(
            "ordered reference return lost its dense ingress parameter",
        ))?;
    Ok(checked_trees::CheckedUnitStructuralArgumentPlan {
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index,
        },
        path,
        type_identity: checked.normalized_type_identity(referent).into_string(),
        access,
    })
}

fn guarded_loan_segment(
    checked: &CheckedTrees,
    segment: &facts::PlaceSegment,
) -> Result<checked_trees::CheckedUnitStructuralPathSegment, LoweringError> {
    match segment {
        facts::PlaceSegment::Field { symbol } => {
            let mut fields = checked
                .data_definitions()
                .iter()
                .flat_map(|data| checked.data_members(data))
                .filter_map(|member| match member {
                    checked_trees::data::DataMember::Field(field) if field.symbol == *symbol => {
                        Some(field)
                    }
                    _ => None,
                });
            let field = fields.next().ok_or(LoweringError::Unsupported(
                "ordered reference return loan lost its field declaration",
            ))?;
            if fields.next().is_some() {
                return unsupported("ordered reference return loan field is ambiguous");
            }
            Ok(checked_trees::CheckedUnitStructuralPathSegment::Field(
                field.path_identity(),
            ))
        }
        facts::PlaceSegment::FixedIndex { index } => {
            Ok(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                u64::try_from(*index).map_err(|_| {
                    LoweringError::Unsupported("ordered reference return index is too large")
                })?,
            ))
        }
        facts::PlaceSegment::Case { .. }
        | facts::PlaceSegment::FixedRange { .. }
        | facts::PlaceSegment::Index { .. } => {
            unsupported("ordered reference return loan path is not static")
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claims: &[(PermissionClaimIdentity, ClaimId)],
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    erased_parameters: &[ValueDeclaration],
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Option<Terminator>, LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::Guarded {
        arms,
        fallback,
        return_values,
    } = &state.terminator
    else {
        return Ok(None);
    };
    let guards = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(*arms)
        .ok_or(LoweringError::Unsupported(
            "guarded structural guards are stale",
        ))?;
    for (position, guard) in guards.iter().enumerate() {
        let mut calls = catalogs.scalar_calls.emission_context();
        let condition = evaluation.guard_value(
            checked,
            plan.machine,
            state.state,
            guard.guard_statement_ordinal,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            &mut calls,
        )?;
        catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
        if condition.scalar_type != ScalarType::Boolean {
            return unsupported("ordered return guard is not Boolean");
        }
        let selected = block_id(allocate_dense(next_block)?);
        let exhausted = position + 1 == guards.len() && fallback.is_none();
        // Coverage was independently replayed during admission. Even the final
        // exhaustive guard is observed; either outcome then selects its value.
        let remaining = if exhausted {
            selected
        } else {
            block_id(allocate_dense(next_block)?)
        };
        let edge = |target, counter: &mut u64| -> Result<SuccessorEdge, LoweringError> {
            Ok(SuccessorEdge {
                edge: edge_id(allocate_dense(counter)?),
                target,
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            })
        };
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: std::mem::take(&mut evaluation.parameters),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: std::mem::take(&mut evaluation.block_structural_parameters),
            operations: operations[evaluation.operation_start..].to_vec(),
            terminator: Terminator::Conditional {
                condition: condition.id,
                when_true: edge(selected, next_edge)?,
                when_false: edge(remaining, next_edge)?,
            },
        });
        let retained_bindings = operations.structural_values.len();
        let mut selected_evaluation = evaluation.branch(selected, operations.len());
        let mut selected_values = values.clone();
        operations.byte_lengths.clear();
        operations.field_byte_lengths.clear();
        let terminator = emit_return(
            checked,
            plan,
            state,
            &return_values[position],
            catalogs,
            parameters,
            claims,
            &mut selected_evaluation,
            &mut selected_values,
            erased_parameters,
            next_value,
            next_block,
            next_edge,
            operations,
        )?;
        if exhausted {
            evaluation.blocks.append(&mut selected_evaluation.blocks);
            std::mem::swap(&mut evaluation.blocks, &mut selected_evaluation.blocks);
            *evaluation = selected_evaluation;
            *values = selected_values;
            return Ok(Some(terminator));
        }
        selected_evaluation.remap_transported_call_operands(operations);
        selected_evaluation.blocks.push(Block {
            id: selected_evaluation.current,
            parameters: selected_evaluation.parameters,
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: selected_evaluation.block_structural_parameters,
            operations: operations[selected_evaluation.operation_start..].to_vec(),
            terminator,
        });
        evaluation.blocks.extend(selected_evaluation.blocks);
        evaluation.current = remaining;
        evaluation.operation_start = operations.len();
        operations.structural_values.truncate(retained_bindings);
        operations.byte_lengths.clear();
        operations.field_byte_lengths.clear();
    }
    let fallback =
        return_values
            .last()
            .filter(|_| fallback.is_some())
            .ok_or(LoweringError::Unsupported(
                "ordered return lost its exhaustive final destination",
            ))?;
    emit_return(
        checked,
        plan,
        state,
        fallback,
        catalogs,
        parameters,
        claims,
        evaluation,
        values,
        erased_parameters,
        next_value,
        next_block,
        next_edge,
        operations,
    )
    .map(Some)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_return(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claims: &[(PermissionClaimIdentity, ClaimId)],
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    erased_parameters: &[ValueDeclaration],
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Terminator, LoweringError> {
    super::super::emission::emit_call_operations(
        checked,
        plan.machine,
        state,
        std::slice::from_ref(operation),
        catalogs,
        parameters,
        claims,
        evaluation,
        values,
        erased_parameters,
        next_value,
        next_block,
        next_edge,
        operations,
    )?;
    complete_structural_return(
        checked,
        plan,
        state,
        operation,
        &[],
        parameters,
        evaluation,
        next_edge,
        operations,
    )
}

/// Complete an already emitted structural producer. Operand preparations are
/// part of this selected exit's lifetime and precede the state's earlier
/// results in reverse-establishment cleanup order. The returned value is
/// transferred, so it participates in correspondence without being discarded.
#[allow(clippy::too_many_arguments)]
pub(super) fn complete_structural_return(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    preparations: &[CheckedUnitEffectOperationPlan],
    parameters: &[StructuralParameterDeclaration],
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    next_edge: &mut u64,
    operations: &OperationBuffer,
) -> Result<Terminator, LoweringError> {
    let result = operation
        .structural_result_binding()
        .ok_or(LoweringError::Unsupported(
            "ordered return has no structural producer",
        ))?;
    let source = operations
        .structural_values
        .iter()
        .find(|(ordinal, _)| *ordinal == result.binding_ordinal)
        .map(|(_, value)| evaluation.current_structural_place(value.place()))
        .ok_or(LoweringError::Unsupported(
            "ordered return value was not established",
        ))?;
    let (_, source_state) =
        crate::expression_preparation::source_custody::authored_state(checked, state.state)?;
    // A selected result participates in the cleanup correspondence even though
    // the return transfers it rather than discarding it.
    let mut discards = vec![(source, false)];
    for operation in preparations
        .iter()
        .rev()
        .chain(state.operations.iter().rev())
    {
        if let Some((result, discard_result_on_return)) = operation.discardable_structural_result()
        {
            let place = case_emission::result(state, result.binding_ordinal, operations)?.place();
            discards.push((place, discard_result_on_return));
        }
    }
    // Parameter selection sources join the reverse-establishment roster after
    // every result row: the receipt sorts them last by descending authored
    // position, and `selection_return_discards` substitutes each residual
    // join parameter at its own slot.
    let mut parameter_sources = Vec::new();
    for cleanup in &evaluation.selection_cleanups {
        for source in &cleanup.sources {
            if let Some((position, _)) = evaluation
                .structural_parameters
                .iter()
                .find(|(_, declaration)| declaration.place == *source)
            {
                parameter_sources.push((*position, *source));
            }
        }
    }
    parameter_sources.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
    parameter_sources.dedup_by_key(|(_, place)| *place);
    discards.extend(
        parameter_sources
            .into_iter()
            .map(|(_, place)| (place, false)),
    );
    discards.extend(
        edges::return_discards(checked, plan.machine, source_state, state)?
            .into_iter()
            .map(|position| (parameters[position].place, true)),
    );
    let discards = evaluation.selection_return_discards(discards)?;
    Ok(Terminator::ReturnStructural {
        edge: edge_id(allocate_dense(next_edge)?),
        source,
        returned_claims: Vec::new(),
        trivial_affine_discards: discards,
    })
}
