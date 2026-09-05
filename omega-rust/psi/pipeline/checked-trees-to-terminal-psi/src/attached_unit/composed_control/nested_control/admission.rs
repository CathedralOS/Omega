//! Independent general acyclic topology and scalar-edge replay.

use super::*;

pub(super) struct AdmittedNested<'a> {
    pub(super) controls: &'a [checked_trees::CheckedComposedUnitControlStatePlan],
    pub(super) leaf_calls: super::super::admission::AdmittedComposedUnit<'a>,
}

pub(super) fn admit<'a>(
    checked: &'a CheckedTrees,
    plan: &'a checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<AdmittedNested<'a>, LoweringError> {
    if plan.states.len() < 4 {
        return unsupported("nested composed Unit control requires at least four states");
    }
    let control_count = plan
        .states
        .iter()
        .position(|state| {
            matches!(
                state.terminator,
                CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            )
        })
        .ok_or(LoweringError::Unsupported(
            "nested composed Unit control has no effect leaves",
        ))?;
    let (controls, leaves) = plan.states.split_at(control_count);
    if controls.len() < 2
        || leaves.is_empty()
        || !plan.body_qualifications.is_empty()
        || controls.iter().any(|state| {
            !state.structural_parameters.is_empty()
                || !state.entry_claims.is_empty()
                || !state.bindings.is_empty()
                || !state.binding_initializers.is_empty()
                || !exact_parameters(state)
                || !super::operations::validate(state)
                || !matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
                )
        })
    {
        return unsupported("nested composed Unit controls escaped exact scalar custody");
    }
    let mut identities = plan
        .states
        .iter()
        .map(|state| state.state)
        .collect::<Vec<_>>();
    identities.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    if identities.windows(2).any(|pair| pair[0] == pair[1]) {
        return unsupported("nested composed Unit control contains duplicate states");
    }
    for control in controls {
        let CheckedComposedUnitControlTerminatorPlan::Conditional {
            guard,
            when_true,
            when_false,
        } = &control.terminator
        else {
            unreachable!("control shape was admitted")
        };
        let transition_offset = u32::try_from(control.operations.len()).map_err(|_| {
            LoweringError::Unsupported("nested control operation count exceeds u32")
        })?;
        validate_parameter_guard(
            checked,
            control,
            transition_offset,
            guard,
            &control.scalar_parameters,
        )?;
        validate_edge(checked, plan, control, when_true, transition_offset)?;
        validate_edge(
            checked,
            plan,
            control,
            when_false,
            transition_offset
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "nested control statement ordinal is exhausted",
                ))?,
        )?;
    }
    for leaf in leaves {
        super::super::admission::validate_leaf(leaf)?;
        validate_empty_leaf(leaf)?;
    }
    validate_acyclic_reachable(plan, control_count)?;
    super::super::admission::validate_contract(checked, plan)?;
    let attachment = super::super::admission::exact_attachment(checked, plan)?;
    let leaf_refs = leaves.iter().collect::<Vec<_>>();
    let mut call_states = controls
        .iter()
        .filter(|state| !state.operations.is_empty())
        .collect::<Vec<_>>();
    call_states.extend(leaf_refs.iter().copied());
    let (boundaries, internal_targets) = super::super::admission::admit_call_targets(
        checked,
        plan.machine,
        &call_states,
        super::super::custody::ComposedCustody::Empty,
        attachment,
        &plan.provider_attachment_requirements,
    )?;
    Ok(AdmittedNested {
        controls,
        leaf_calls: super::super::admission::AdmittedComposedUnit {
            entry: &controls[0],
            leaves: leaf_refs,
            boundaries,
            internal_targets,
            custody: super::super::custody::ComposedCustody::Empty,
        },
    })
}

fn exact_parameters(state: &checked_trees::CheckedComposedUnitControlStatePlan) -> bool {
    let Some(base) = state
        .scalar_parameters
        .first()
        .map(|parameter| parameter.source_position)
    else {
        return false;
    };
    base <= 1
        && state
            .scalar_parameters
            .iter()
            .enumerate()
            .all(|(index, parameter)| {
                parameter.source_position
                    == base.saturating_add(u32::try_from(index).unwrap_or(u32::MAX))
                    && parameter.primitive_type == PrimitiveType::Bool
            })
}

fn validate_parameter_guard(
    checked: &CheckedTrees,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    statement_ordinal: u32,
    guard: &CheckedScalarExpression,
    parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
) -> Result<(), LoweringError> {
    let CheckedScalarExpression::Boolean(boolean) = guard else {
        return unsupported("nested composed Unit guard is not Boolean");
    };
    let CheckedBooleanExpression::Parameter { position } = boolean.as_ref() else {
        return unsupported("nested composed Unit guard is not parameter-backed");
    };
    if parameters
        .get(*position)
        .is_none_or(|parameter| parameter.primitive_type != PrimitiveType::Bool)
        || checked.facts.values.scalar_expressions.expression_at(
            state.state,
            statement_ordinal,
            CheckedScalarExpressionRole::Guard,
        ) != Some(guard)
    {
        return unsupported("nested composed Unit guard drifted from checked scalar facts");
    }
    Ok(())
}

fn validate_edge(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::CheckedComposedUnitControlStatePlan,
    edge: &checked_trees::CheckedStructuralControlSuccessorPlan,
    ordinal: u32,
) -> Result<(), LoweringError> {
    let target = plan
        .states
        .iter()
        .find(|state| state.state == edge.target_state)
        .ok_or(LoweringError::Unsupported(
            "nested composed Unit edge targets an unknown state",
        ))?;
    let scalar_matches = edge.scalar_arguments.len() == target.scalar_parameters.len()
        && edge
            .scalar_arguments
            .iter()
            .zip(&target.scalar_parameters)
            .enumerate()
            .all(|(target_index, (argument, target_parameter))| {
                let source_index = usize::try_from(argument.source_scalar_parameter_index).ok();
                let expression = checked.facts.values.scalar_expressions.expression_at(
                    source.state,
                    ordinal,
                    CheckedScalarExpressionRole::TransitionArgument {
                        argument_ordinal: argument.argument_ordinal,
                    },
                );
                argument.argument_ordinal == target_parameter.source_position
                    && argument.target_scalar_parameter_index
                        == u32::try_from(target_index).unwrap_or(u32::MAX)
                    && argument.primitive_type == target_parameter.primitive_type
                    && source_index
                        .and_then(|index| source.scalar_parameters.get(index))
                        .is_some_and(|source_parameter| {
                            source_parameter.primitive_type == target_parameter.primitive_type
                        })
                    && matches!(expression,
                        Some(CheckedScalarExpression::Boolean(boolean))
                            if matches!(boolean.as_ref(), CheckedBooleanExpression::Parameter { position }
                                if *position == source_index.unwrap_or(usize::MAX)))
            });
    let cleanup = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_edge(plan.machine, source.state, ordinal)
        .ok_or(LoweringError::Unsupported(
            "nested composed Unit edge lost its checked cleanup",
        ))?;
    if edge.statement_ordinal != ordinal
        || cleanup.target_state != edge.target_state
        || !cleanup
            .trivial_affine_discard_parameter_positions
            .is_empty()
        || !edge.transfers.is_empty()
        || !edge.trivial_affine_discard_parameter_positions.is_empty()
        || !scalar_matches
    {
        return unsupported("nested composed Unit edge map drifted");
    }
    Ok(())
}

fn validate_acyclic_reachable(
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
    control_count: usize,
) -> Result<(), LoweringError> {
    fn visit(
        plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
        control_count: usize,
        index: usize,
        active: &mut Vec<usize>,
        complete: &mut Vec<usize>,
    ) -> Result<(), LoweringError> {
        if active.contains(&index) {
            return unsupported("nested composed Unit graph contains a cycle");
        }
        if complete.contains(&index) {
            return Ok(());
        }
        active.push(index);
        if index < control_count {
            let CheckedComposedUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } = &plan.states[index].terminator
            else {
                return unsupported("nested composed Unit control topology drifted");
            };
            for edge in [when_true, when_false] {
                let target = plan
                    .states
                    .iter()
                    .position(|state| state.state == edge.target_state)
                    .ok_or(LoweringError::Unsupported(
                        "nested composed Unit edge target disappeared",
                    ))?;
                visit(plan, control_count, target, active, complete)?;
            }
        }
        active.pop();
        complete.push(index);
        Ok(())
    }
    let mut active = Vec::new();
    let mut complete = Vec::new();
    visit(plan, control_count, 0, &mut active, &mut complete)?;
    if complete.len() != plan.states.len() {
        return unsupported("nested composed Unit graph contains unreachable states");
    }
    Ok(())
}

fn validate_empty_leaf(
    leaf: &checked_trees::CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    let empty = match &leaf.operations[0] {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } => structural_arguments.is_empty() && completion_receipts.is_empty(),
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } => structural_arguments.is_empty() && claim_transfers.is_empty(),
        _ => false,
    };
    if !leaf.structural_parameters.is_empty() || !leaf.entry_claims.is_empty() || !empty {
        return unsupported("nested composed Unit leaf escaped empty custody");
    }
    Ok(())
}
