//! Ordered exits use the same source-coordinate coverage check as scalar returns.
//! Their values are ordinary structural producers, evaluated only after selecting
//! an exit. Dependency discovery visits every arm; execution never does so.
use super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, ClaimId, PermissionClaimIdentity,
    StructuralParameterDeclaration, SuccessorEdge, block_id,
};
use super::super::super::{
    Block, CheckedUnitEffectOperationPlan, ScalarType, Terminator, ValueDeclaration,
    allocate_dense, edge_id, unsupported,
};
use super::super::{CheckedTrees, LoweringError, catalogs};
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan};
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::expression::ExpressionNode;

/// One selected structural completion retains the same exact value and ingress
/// regardless of whether sibling arms invoke a machine or return other values.
/// Selection topology owns only which producer executes, not its value custody.
pub(super) fn validate_structural_return(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    expression: checked_trees::expression::ExpressionHandle,
    statement_ordinal: u32,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let checked_trees::CheckedControlResultPlan::Structural(_) = &plan.result else {
        return unsupported("structural return has no structural result signature");
    };
    let reference_result = !plan.result_reference_sources.is_empty();
    match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            discard_result_on_return: false,
            ..
        }
        | CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
            if !reference_result => {}
        CheckedUnitEffectOperationPlan::Call(call)
            if !reference_result && call.discard_result_on_return() == Some(false) =>
        {
            if call.structural_result().is_none() {
                return unsupported("ordered return call has no structural result");
            }
        }
        CheckedUnitEffectOperationPlan::EstablishReference { .. } if reference_result => {
            validate_reference_return(checked, plan, source, state, expression, operation)?;
        }
        _ => return unsupported("ordered return lost its structural producer"),
    }
    match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => {
            if checked
                .facts
                .values
                .structural_values
                .root_for_expression(state.state, statement_ordinal, expression)
                .is_none()
            {
                return unsupported("ordered return lost its exact value root");
            }
        }
        CheckedUnitEffectOperationPlan::EstablishByteSequence { .. } => {
            super::super::super::byte_locals::validate(checked, state.state, operation)?;
        }
        _ => {}
    }
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
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let (
        checked_trees::CheckedControlResultPlan::Structural(_),
        CheckedUnitEffectOperationPlan::EstablishReference { source, .. },
    ) = (&plan.result, operation)
    else {
        return unsupported("reference return lost its structural producer or signature");
    };
    reference_source(checked, plan, authored, state, expression)?;
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
        &source.source
    else {
        return unsupported("reference return lost its ingress parameter");
    };
    if state
        .structural_parameters
        .get(*parameter_index as usize)
        .is_none()
    {
        return unsupported("reference return ingress parameter is absent");
    }
    Ok(())
}

/// The machine interface's entry-relative reference origins must resolve.
pub(super) fn validate_reference_roster(
    checked: &CheckedTrees,
    entry: &checked_trees::state::State,
) -> Result<(), LoweringError> {
    if !crate::reference_custody::returns_complete_reference_origins(checked, entry) {
        return unsupported("reference result has no complete entry-relative origins");
    }
    Ok(())
}

/// Rebuild a selected reference from ordinary ingress reconstruction; mutable
/// projections additionally follow the checked loan ledger. The candidate roster is only a correspondence
/// receipt: admission must not let a producer and that receipt be forged to a
/// different, same-typed ingress together.
fn reference_source(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    authored: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    expression: checked_trees::expression::ExpressionHandle,
) -> Result<checked_trees::CheckedUnitStructuralArgumentPlan, LoweringError> {
    let carried_reference = !matches!(
        checked.expression_table.expression(expression),
        ExpressionNode::Borrow(_)
    );
    if carried_reference
        || crate::reference_custody::shared_borrow_parts(checked, authored.return_type).is_some()
    {
        return validation::reference_result_custody::initializer_source(
            &checked.typed,
            authored,
            expression,
            authored.return_type,
        )
        .ok_or(LoweringError::Unsupported(
            "reference return has no exact carried ingress source",
        ));
    }
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
    let (referent, access) =
        crate::reference_custody::mutable_borrow_parts(checked, authored.return_type).ok_or(
            LoweringError::Unsupported("ordered reference return lost its mutable result type"),
        )?;
    if validation::expression_result_type_reference(
        &checked.typed,
        machine,
        authored,
        borrow.target,
    )
    .is_none()
    {
        return unsupported("ordered reference return lost its target type");
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
        parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)
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
        && crate::reference_custody::mutable_borrow_parts(checked, parameter.type_reference)
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
                residual_affine_discards: Vec::new(),
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
            super::cleanup::destination(checked, state, &guard.destination)?,
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
    let fallback_exit = super::cleanup::destination(
        checked,
        state,
        fallback.as_ref().ok_or(LoweringError::Unsupported(
            "ordered return lost its fallback destination",
        ))?,
    )?;
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
        fallback_exit,
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
    exit: checked_trees::FlowClaimJoinExit,
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
        checked, state, exit, operation, catalogs, parameters, evaluation, next_edge, operations,
    )
}

/// Complete an emitted structural producer using the selected exit's checked
/// cleanup order and the current places established by operand preparation.
#[allow(clippy::too_many_arguments)]
pub(super) fn complete_structural_return(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    exit: checked_trees::FlowClaimJoinExit,
    operation: &CheckedUnitEffectOperationPlan,
    catalogs: &mut catalogs::ComposedCatalogs,
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
    let cleanup_actions = super::cleanup::for_exit(
        checked, state, exit, catalogs, parameters, evaluation, operations,
    )?;
    Ok(Terminator::ReturnStructural {
        edge: edge_id(allocate_dense(next_edge)?),
        source,
        returned_claims: Vec::new(),
        cleanup_actions,
    })
}
