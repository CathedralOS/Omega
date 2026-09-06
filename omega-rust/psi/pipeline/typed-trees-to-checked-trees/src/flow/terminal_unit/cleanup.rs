//! Nominal and partial-affine Unit cleanup planning.

use super::*;

mod residuals;

pub(super) fn build_nominal_affine_unit_cleanup_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedNominalAffineUnitCleanupMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
        || !program.machine_owned_data(machine).is_empty()
        || !program.machine_trait_conformances(machine).is_empty()
        || !machine.conformance_bounds.is_empty()
        || !program.machine_invokes(machine).is_empty()
        || machine.suspends
        || machine.blocks
        || !is_unit(program, state.return_type)
        || !program
            .statement_table
            .statements(state.statement_nodes)
            .is_empty()
    {
        return None;
    }

    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders, false)?;
    let source_parameters = program.state_parameters(state);
    if source_parameters.is_empty() || source_parameters.len() != structural_parameters.len() {
        return None;
    }
    for (position, (source_parameter, checked_parameter)) in source_parameters
        .iter()
        .zip(&structural_parameters)
        .enumerate()
    {
        let TypeReferenceNode::Named {
            symbol: parameter_data_symbol,
            ..
        } = program
            .type_reference_table
            .type_reference(source_parameter.type_reference)
        else {
            return None;
        };
        let parameter_data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *parameter_data_symbol)?;
        let parameter_shape = shapes.types.get(&checked_parameter.type_identity)?;
        if source_parameter.is_self
            || source_parameter.is_const
            || source_parameter.is_mutable
            || checked_parameter.is_self
            || usize::try_from(checked_parameter.position).ok()? != position
            || checked_parameter.multiplicity != Multiplicity::Affine
            || !checked_parameter.qualifications.is_empty()
            || !program.data_type_parameters(parameter_data).is_empty()
            || !is_bounded_nominal_cleanup_record(&parameter_shape.shape)
        {
            return None;
        }
    }
    let attachment_shape = shapes.types.get(&attachment_type_identity)?;
    if !matches!(
        &attachment_shape.shape,
        CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
    ) {
        return None;
    }

    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    if !entry_claims.is_empty()
        || facts
            .qualifications
            .for_machine(machine.symbol)
            .is_some_and(|fact| !fact.body_committed.is_empty())
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
        || !facts
            .flow
            .control
            .calls
            .span_or_empty(state_flow.calls)
            .is_empty()
        || !service_reach_is_empty(facts, state_flow.service_reach)
        || !service_reach_plan_is_empty(
            facts,
            facts.service_reaches.plan_for_machine(machine.symbol)?,
        )
        || !source_parameters
            .iter()
            .all(|parameter| has_exact_root_affine_discard(facts, machine, state, parameter))
    {
        return None;
    }
    let caller_requirements = nominal_cleanup_caller_boolean_requirements(
        program,
        facts,
        machine,
        state,
        source_parameters,
    )?;
    let mut cleanups = Vec::with_capacity(source_parameters.len());
    for (source_parameter, checked_parameter) in
        source_parameters.iter().zip(&structural_parameters).rev()
    {
        let TypeReferenceNode::Named {
            symbol: parameter_data_symbol,
            ..
        } = program
            .type_reference_table
            .type_reference(source_parameter.type_reference)
        else {
            return None;
        };
        let parameter_data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *parameter_data_symbol)?;
        let cleanup_machines = program
            .machines()
            .iter()
            .filter(|candidate| {
                candidate.supply_mode == MachineSupplyMode::CheckedBody
                    && candidate.name.as_str().ends_with("::drop")
                    && candidate.attached_data_symbol == parameter_data.symbol
            })
            .collect::<Vec<_>>();
        let [cleanup_machine] = cleanup_machines.as_slice() else {
            return None;
        };
        let [cleanup_state] = program.machine_states(cleanup_machine) else {
            return None;
        };
        let [cleanup_receiver] = program.state_parameters(cleanup_state) else {
            return None;
        };
        let TypeReferenceNode::Reference { access, .. } = program
            .type_reference_table
            .type_reference(cleanup_receiver.type_reference)
        else {
            return None;
        };
        if !access.is_readable() || !access.is_exclusive() {
            return None;
        }
        if !cleanup_receiver.is_self
            || cleanup_receiver.is_const
            || !cleanup_machine.lifetime_parameters.is_empty()
            || !program.machine_type_parameters(cleanup_machine).is_empty()
            || !program.machine_owned_data(cleanup_machine).is_empty()
            || !program
                .machine_trait_conformances(cleanup_machine)
                .is_empty()
            || !cleanup_machine.conformance_bounds.is_empty()
            || !program.machine_invokes(cleanup_machine).is_empty()
            || cleanup_machine.suspends
            || cleanup_machine.blocks
            || !is_unit(program, cleanup_state.return_type)
        {
            return None;
        }
        let cleanup_requirements = nominal_cleanup_boolean_requirements(
            program,
            facts,
            cleanup_machine,
            cleanup_state,
            cleanup_receiver,
        )?;
        if let Some(missing) = nominal_cleanup_missing_requirement(
            checked_parameter.position,
            &caller_requirements,
            &cleanup_requirements,
        ) {
            diagnostics.push(nominal_cleanup_missing_requirement_diagnostic(
                program,
                machine,
                state,
                source_parameter,
                cleanup_machine,
                missing,
            ));
            return None;
        }
        let cleanup_statements = program
            .statement_table
            .statements(cleanup_state.statement_nodes);
        if cleanup_statements
            .iter()
            .any(|statement| !matches!(statement, StatementNode::Call(_)))
        {
            return None;
        }
        let cleanup_target = unit_effects.for_machine(cleanup_machine.symbol)?;
        let (cleanup_return, cleanup_calls) = cleanup_target.operations.split_last()?;
        let CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } = cleanup_return
        else {
            return None;
        };
        if usize::try_from(*statement_index).ok()? != cleanup_calls.len()
            || cleanup_calls.len() != cleanup_statements.len()
            || !trivial_affine_local_discard_ordinals.is_empty()
            || !trivial_affine_discards.is_empty()
            || cleanup_target.attachment_type_identity.as_deref()
                != Some(checked_parameter.type_identity.as_str())
            || !cleanup_target.structural_parameters.is_empty()
            || !cleanup_target.trivial_affine_locals.is_empty()
            || !cleanup_target.entry_claims.is_empty()
            || !cleanup_target.body_qualifications.is_empty()
            || !service_reach_is_empty(facts, cleanup_target.service_reach)
            || !service_reach_plan_is_empty(facts, cleanup_target.contract_service_reach)
        {
            return None;
        }
        let mut cleanup_helpers = Vec::with_capacity(cleanup_calls.len());
        for (statement_index, operation) in cleanup_calls.iter().enumerate() {
            let CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                service_reach,
                scalar_arguments,
                structural_arguments,
                claim_transfers,
            } = operation
            else {
                return None;
            };
            if usize::try_from(coordinate.statement_index).ok()? != statement_index
                || coordinate.call_ordinal != 0
                || *target_machine == cleanup_machine.symbol
                || cleanup_helpers
                    .iter()
                    .any(|(helper, _, _)| helper == target_machine)
                || !service_reach_is_empty(facts, *service_reach)
                || !scalar_arguments.is_empty()
                || !structural_arguments.is_empty()
                || !claim_transfers.is_empty()
            {
                return None;
            }
            cleanup_helpers.push((
                *target_machine,
                *target_state,
                *target_contract_report_fingerprint,
            ));
        }
        for (helper_machine, helper_state, helper_fingerprint) in cleanup_helpers {
            let helper = unit_effects.for_machine(helper_machine)?;
            let helper_shape = shapes
                .types
                .get(helper.attachment_type_identity.as_ref()?)?;
            if helper.machine == machine.symbol
                || helper.machine == cleanup_machine.symbol
                || helper.state != helper_state
                || helper.contract_report_fingerprint != helper_fingerprint
                || !matches!(&helper_shape.shape, CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty())
                || !helper.structural_parameters.is_empty()
                || !helper.trivial_affine_locals.is_empty()
                || !helper.entry_claims.is_empty()
                || !helper.body_qualifications.is_empty()
                || !service_reach_is_empty(facts, helper.service_reach)
                || !service_reach_plan_is_empty(facts, helper.contract_service_reach)
                || !matches!(helper.operations.as_slice(), [CheckedUnitEffectOperationPlan::ReturnUnit { statement_index: 0, trivial_affine_local_discard_ordinals, trivial_affine_discards }] if trivial_affine_local_discard_ordinals.is_empty() && trivial_affine_discards.is_empty())
            {
                return None;
            }
        }
        cleanups.push(CheckedUnitNominalAffineCleanupPlan {
            source_parameter_index: checked_parameter.position,
            type_identity: checked_parameter.type_identity.clone(),
            cleanup_machine: cleanup_machine.symbol,
            cleanup_state: cleanup_state.symbol,
            cleanup_contract_report_fingerprint: cleanup_target.contract_report_fingerprint,
            requirements: cleanup_requirements,
        });
    }

    Some(CheckedNominalAffineUnitCleanupMachinePlan {
        machine: CheckedUnitEffectMachinePlan {
            machine: machine.symbol,
            state: state.symbol,
            attachment_type_identity: Some(attachment_type_identity),
            structural_parameters,
            scalar_parameters: Vec::new(),
            provider_attachment_requirements: Vec::new(),
            trivial_affine_locals: Vec::new(),
            entry_claims: Vec::new(),
            body_qualifications: Vec::new(),
            contract_report_fingerprint: contract.report_fingerprint,
            contract_commitment: contract.commitment,
            contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
            service_reach: state_flow.service_reach,
            operations: vec![CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 0,
                trivial_affine_local_discard_ordinals: Vec::new(),
                trivial_affine_discards: Vec::new(),
            }],
        },
        caller_requirements,
        cleanups,
    })
}

pub(super) fn nominal_cleanup_boolean_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    cleanup_machine: &typed_trees::machine::Machine,
    cleanup_state: &typed_trees::state::State,
    cleanup_receiver: &StateParameter,
) -> Option<Vec<CheckedUnitNominalAffineCleanupRequirementPlan>> {
    let checked_requires =
        checked_requires_expressions(program, facts, cleanup_machine.symbol, cleanup_state.symbol)?;
    let requirements = checked_requires
        .into_iter()
        .map(|expression| {
            direct_boolean_field_requirement(
                program,
                cleanup_state.symbol,
                cleanup_receiver,
                expression,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    Some(canonical_nominal_cleanup_requirements(requirements))
}

pub(super) fn nominal_scalar_caller_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    source_parameters: &[StateParameter],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> Option<(
    Vec<CheckedUnitNominalAffineCallerRequirementPlan>,
    Vec<CheckedStructuralScalarIntegerBoundRequirementPlan>,
)> {
    // Both accepted callers preserve checked entry requirements unchanged. The
    // Unit lane admits only direct Boolean root facts; the scalar lane also
    // retains direct fixed-width integer literal bounds and pairwise parameter
    // relations for exact arithmetic. Wider bodies must instead consult
    // path-specific exit contexts.
    let caller_requires =
        checked_requires_expressions(program, facts, caller_machine.symbol, caller_state.symbol)?;
    let mut structural_requirements = Vec::new();
    let mut scalar_requirements = Vec::new();
    for expression in caller_requires {
        if let Some(requirement) = source_parameters.iter().enumerate().find_map(
            |(source_parameter_index, source_parameter)| {
                let source_parameter_index = u32::try_from(source_parameter_index).ok()?;
                direct_boolean_field_requirement(
                    program,
                    caller_state.symbol,
                    source_parameter,
                    expression,
                )
                .map(
                    |requirement| CheckedUnitNominalAffineCallerRequirementPlan {
                        source_parameter_index,
                        field_identity: requirement.field_identity,
                        expected: requirement.expected,
                    },
                )
            },
        ) {
            structural_requirements.push(requirement);
            continue;
        }
        scalar_requirements.push(direct_integer_requirement(
            program,
            caller_machine.symbol,
            caller_state,
            source_parameters,
            scalar_parameters,
            expression,
        )?);
    }
    structural_requirements.sort_by(|left, right| {
        left.source_parameter_index
            .cmp(&right.source_parameter_index)
            .then(left.field_identity.cmp(&right.field_identity))
            .then(left.expected.cmp(&right.expected))
    });
    structural_requirements.dedup();
    Some((structural_requirements, scalar_requirements))
}

pub(super) fn nominal_cleanup_caller_boolean_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    source_parameters: &[StateParameter],
) -> Option<Vec<CheckedUnitNominalAffineCallerRequirementPlan>> {
    let (structural, scalar) = nominal_scalar_caller_requirements(
        program,
        facts,
        caller_machine,
        caller_state,
        source_parameters,
        &[],
    )?;
    scalar.is_empty().then_some(structural)
}

pub(super) fn direct_integer_requirement(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    source_parameters: &[StateParameter],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedStructuralScalarIntegerBoundRequirementPlan> {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let parameter = |parameter_expression| {
        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            0,
            parameter_expression,
        )?;
        if !place.segments.is_empty() {
            return None;
        }
        let facts::PlaceRoot::Symbol(root) = place.root else {
            return None;
        };
        let source_position = source_parameters.iter().position(|parameter| {
            parameter_root_symbol(machine, parameter) == root || parameter.symbol == root
        })?;
        let parameter_position = scalar_parameters
            .iter()
            .position(|parameter| parameter.source_position as usize == source_position)?;
        let primitive_type = scalar_parameters.get(parameter_position)?.primitive_type;
        if !matches!(
            primitive_type,
            PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
        ) {
            return None;
        }
        Some((u32::try_from(parameter_position).ok()?, primitive_type))
    };
    let (parameter_position, primitive_type, kind, bound) = match (
        binary.operator,
        program.expression_table.expression(binary.left),
        program.expression_table.expression(binary.right),
    ) {
        (BinaryOperator::LessOrEqual, _, ExpressionNode::Integer(bound)) => {
            let (position, primitive_type) = parameter(binary.left)?;
            (
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Upper,
                CheckedStructuralScalarIntegerBoundPlan::Literal(bound.clone()),
            )
        }
        (BinaryOperator::LessOrEqual, ExpressionNode::Integer(bound), _) => {
            let (position, primitive_type) = parameter(binary.right)?;
            (
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Lower,
                CheckedStructuralScalarIntegerBoundPlan::Literal(bound.clone()),
            )
        }
        (BinaryOperator::LessOrEqual, _, ExpressionNode::Binary(bound))
            if bound.operator == BinaryOperator::Subtract =>
        {
            let (position, primitive_type) = parameter(binary.left)?;
            let (subtrahend, subtrahend_type) = parameter(bound.right)?;
            let ExpressionNode::Integer(maximum) = program.expression_table.expression(bound.left)
            else {
                return None;
            };
            let maximum_matches = match primitive_type {
                PrimitiveType::U8 => maximum.value_u64() == Some(u64::from(u8::MAX)),
                PrimitiveType::U16 => maximum.value_u64() == Some(u64::from(u16::MAX)),
                PrimitiveType::U32 => maximum.value_u64() == Some(u64::from(u32::MAX)),
                PrimitiveType::U64 => maximum.value_u64() == Some(u64::MAX),
                PrimitiveType::I8 => maximum.value_i64() == Some(i64::from(i8::MAX)),
                PrimitiveType::I16 => maximum.value_i64() == Some(i64::from(i16::MAX)),
                PrimitiveType::I32 => maximum.value_i64() == Some(i64::from(i32::MAX)),
                PrimitiveType::I64 => maximum.value_i64() == Some(i64::MAX),
                _ => false,
            };
            (maximum_matches && primitive_type == subtrahend_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Upper,
                CheckedStructuralScalarIntegerBoundPlan::MaximumMinusParameter(subtrahend),
            ))?
        }
        (BinaryOperator::LessOrEqual, ExpressionNode::Binary(bound), _)
            if bound.operator == BinaryOperator::Subtract =>
        {
            let (position, primitive_type) = parameter(binary.right)?;
            let (subtrahend, subtrahend_type) = parameter(bound.right)?;
            let ExpressionNode::Integer(minimum) = program.expression_table.expression(bound.left)
            else {
                return None;
            };
            let minimum_matches = match primitive_type {
                PrimitiveType::I8 => minimum.value_i64() == Some(i64::from(i8::MIN)),
                PrimitiveType::I16 => minimum.value_i64() == Some(i64::from(i16::MIN)),
                PrimitiveType::I32 => minimum.value_i64() == Some(i64::from(i32::MIN)),
                PrimitiveType::I64 => minimum.value_i64() == Some(i64::MIN),
                _ => false,
            };
            (minimum_matches && primitive_type == subtrahend_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Lower,
                CheckedStructuralScalarIntegerBoundPlan::SignedMinimumMinusParameter(subtrahend),
            ))?
        }
        (BinaryOperator::LessOrEqual, ExpressionNode::Binary(bound), _)
            if bound.operator == BinaryOperator::Add =>
        {
            let (position, primitive_type) = parameter(binary.right)?;
            let (addend, addend_type, minimum) = match (
                program.expression_table.expression(bound.left),
                program.expression_table.expression(bound.right),
            ) {
                (ExpressionNode::Integer(minimum), _) => {
                    let (addend, addend_type) = parameter(bound.right)?;
                    (addend, addend_type, minimum)
                }
                (_, ExpressionNode::Integer(minimum)) => {
                    let (addend, addend_type) = parameter(bound.left)?;
                    (addend, addend_type, minimum)
                }
                _ => return None,
            };
            let minimum_matches = match primitive_type {
                PrimitiveType::I8 => minimum.value_i64() == Some(i64::from(i8::MIN)),
                PrimitiveType::I16 => minimum.value_i64() == Some(i64::from(i16::MIN)),
                PrimitiveType::I32 => minimum.value_i64() == Some(i64::from(i32::MIN)),
                PrimitiveType::I64 => minimum.value_i64() == Some(i64::MIN),
                _ => false,
            };
            (minimum_matches && primitive_type == addend_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Lower,
                CheckedStructuralScalarIntegerBoundPlan::SignedMinimumPlusParameter(addend),
            ))?
        }
        (BinaryOperator::LessOrEqual, _, ExpressionNode::Binary(bound))
            if bound.operator == BinaryOperator::Add =>
        {
            let (position, primitive_type) = parameter(binary.left)?;
            let (addend, addend_type, maximum) = match (
                program.expression_table.expression(bound.left),
                program.expression_table.expression(bound.right),
            ) {
                (ExpressionNode::Integer(maximum), _) => {
                    let (addend, addend_type) = parameter(bound.right)?;
                    (addend, addend_type, maximum)
                }
                (_, ExpressionNode::Integer(maximum)) => {
                    let (addend, addend_type) = parameter(bound.left)?;
                    (addend, addend_type, maximum)
                }
                _ => return None,
            };
            let maximum_matches = match primitive_type {
                PrimitiveType::I8 => maximum.value_i64() == Some(i64::from(i8::MAX)),
                PrimitiveType::I16 => maximum.value_i64() == Some(i64::from(i16::MAX)),
                PrimitiveType::I32 => maximum.value_i64() == Some(i64::from(i32::MAX)),
                PrimitiveType::I64 => maximum.value_i64() == Some(i64::MAX),
                _ => false,
            };
            (maximum_matches && primitive_type == addend_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Upper,
                CheckedStructuralScalarIntegerBoundPlan::SignedMaximumPlusParameter(addend),
            ))?
        }
        (BinaryOperator::LessOrEqual, _, ExpressionNode::Binary(bound))
            if bound.operator == BinaryOperator::Divide =>
        {
            let (position, primitive_type) = parameter(binary.left)?;
            let (divisor, divisor_type) = parameter(bound.right)?;
            let ExpressionNode::Integer(boundary) = program.expression_table.expression(bound.left)
            else {
                return None;
            };
            let bound_plan = quotient_extremum_bound_plan(primitive_type, boundary, divisor)?;
            (primitive_type == divisor_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Upper,
                bound_plan,
            ))?
        }
        (BinaryOperator::LessOrEqual, ExpressionNode::Binary(bound), _)
            if bound.operator == BinaryOperator::Divide =>
        {
            let (position, primitive_type) = parameter(binary.right)?;
            let (divisor, divisor_type) = parameter(bound.right)?;
            let ExpressionNode::Integer(boundary) = program.expression_table.expression(bound.left)
            else {
                return None;
            };
            let bound_plan = quotient_extremum_bound_plan(primitive_type, boundary, divisor)?;
            (primitive_type == divisor_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Lower,
                bound_plan,
            ))?
        }
        (BinaryOperator::LessOrEqual, _, _) => {
            let (left, primitive_type) = parameter(binary.left)?;
            let (right, right_type) = parameter(binary.right)?;
            (primitive_type == right_type).then_some((
                left,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Upper,
                CheckedStructuralScalarIntegerBoundPlan::Parameter(right),
            ))?
        }
        _ => return None,
    };
    Some(CheckedStructuralScalarIntegerBoundRequirementPlan {
        parameter_position,
        primitive_type,
        kind,
        bound,
    })
}

pub(super) fn quotient_extremum_bound_plan(
    primitive_type: PrimitiveType,
    boundary: &numerics::literals::IntegerLiteral,
    divisor: u32,
) -> Option<CheckedStructuralScalarIntegerBoundPlan> {
    let maximum_matches = match primitive_type {
        PrimitiveType::U8 => boundary.value_u64() == Some(u64::from(u8::MAX)),
        PrimitiveType::U16 => boundary.value_u64() == Some(u64::from(u16::MAX)),
        PrimitiveType::U32 => boundary.value_u64() == Some(u64::from(u32::MAX)),
        PrimitiveType::U64 => boundary.value_u64() == Some(u64::MAX),
        PrimitiveType::I8 => boundary.value_i64() == Some(i64::from(i8::MAX)),
        PrimitiveType::I16 => boundary.value_i64() == Some(i64::from(i16::MAX)),
        PrimitiveType::I32 => boundary.value_i64() == Some(i64::from(i32::MAX)),
        PrimitiveType::I64 => boundary.value_i64() == Some(i64::MAX),
        _ => false,
    };
    if maximum_matches {
        return Some(CheckedStructuralScalarIntegerBoundPlan::MaximumDivideParameter(divisor));
    }
    let minimum_matches = match primitive_type {
        PrimitiveType::I8 => boundary.value_i64() == Some(i64::from(i8::MIN)),
        PrimitiveType::I16 => boundary.value_i64() == Some(i64::from(i16::MIN)),
        PrimitiveType::I32 => boundary.value_i64() == Some(i64::from(i32::MIN)),
        PrimitiveType::I64 => boundary.value_i64() == Some(i64::MIN),
        _ => false,
    };
    minimum_matches
        .then_some(CheckedStructuralScalarIntegerBoundPlan::SignedMinimumDivideParameter(divisor))
}

pub(super) fn nominal_cleanup_missing_requirement(
    source_parameter_index: u32,
    caller_requirements: &[CheckedUnitNominalAffineCallerRequirementPlan],
    required: &[CheckedUnitNominalAffineCleanupRequirementPlan],
) -> Option<CheckedUnitNominalAffineCleanupRequirementPlan> {
    required
        .iter()
        .find(|requirement| {
            !caller_requirements.iter().any(|caller| {
                caller.source_parameter_index == source_parameter_index
                    && caller.field_identity == requirement.field_identity
                    && caller.expected == requirement.expected
            })
        })
        .cloned()
}

pub(super) fn canonical_nominal_cleanup_requirements(
    mut requirements: Vec<CheckedUnitNominalAffineCleanupRequirementPlan>,
) -> Vec<CheckedUnitNominalAffineCleanupRequirementPlan> {
    requirements.sort_by(|left, right| {
        left.field_identity
            .cmp(&right.field_identity)
            .then(left.expected.cmp(&right.expected))
    });
    requirements.dedup();
    requirements
}

pub(super) fn nominal_cleanup_missing_requirement_diagnostic(
    program: &TypedTrees,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    source_parameter: &StateParameter,
    cleanup_machine: &typed_trees::machine::Machine,
    missing: CheckedUnitNominalAffineCleanupRequirementPlan,
) -> Diagnostic {
    let edge = format!(
        "automatic cleanup requires at Unit return edge from {} state {} after statement 0",
        crate::labels::machine_name(program, caller_machine.symbol),
        crate::labels::symbol_name(program, caller_state.symbol),
    );
    Diagnostic::error(format!(
        "cannot prove {edge}: missing {}.{} == {} required by {}",
        source_parameter.name.as_str(),
        missing.field_identity,
        missing.expected,
        crate::labels::machine_name(program, cleanup_machine.symbol),
    ))
}

pub(super) fn scalar_nominal_cleanup_missing_requirement_diagnostic(
    program: &TypedTrees,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    return_statement_ordinal: u32,
    source_parameter: &StateParameter,
    cleanup_machine: &typed_trees::machine::Machine,
    missing: CheckedUnitNominalAffineCleanupRequirementPlan,
) -> Diagnostic {
    let edge = format!(
        "automatic cleanup requires at scalar return edge from {} state {} after statement {}",
        crate::labels::machine_name(program, caller_machine.symbol),
        crate::labels::symbol_name(program, caller_state.symbol),
        return_statement_ordinal,
    );
    Diagnostic::error(format!(
        "cannot prove {edge}: missing {}.{} == {} required by {}",
        source_parameter.name.as_str(),
        missing.field_identity,
        missing.expected,
        crate::labels::machine_name(program, cleanup_machine.symbol),
    ))
}

pub(super) fn checked_requires_expressions(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<Vec<typed_trees::expression::ExpressionHandle>> {
    let mut expressions = Vec::new();
    for (_, checked) in facts.proof.contract_facts.iter().filter(|(_, checked)| {
        matches!(checked.owner, ContractProofFactOwner::Machine { machine_symbol } if machine_symbol == machine)
            || matches!(checked.owner, ContractProofFactOwner::MachineState { machine_symbol, state_symbol } if machine_symbol == machine && state_symbol == state)
    }) {
        if checked.kind != ContractProofFactKind::Requires {
            return None;
        }
        let ProofFact::Expression(expression) = program.proof_facts.get(checked.fact) else {
            return None;
        };
        expressions.push(*expression);
    }
    Some(expressions)
}

pub(super) fn direct_boolean_field_requirement(
    program: &TypedTrees,
    state: SymbolHandle,
    root_parameter: &StateParameter,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitNominalAffineCleanupRequirementPlan> {
    use typed_trees::expression::{BinaryOperator, UnaryOperator};

    let (field_expression, expected) = match program.expression_table.expression(expression) {
        ExpressionNode::Member(_) => (expression, true),
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            if !matches!(
                program.expression_table.expression(unary.operand),
                ExpressionNode::Member(_)
            ) {
                return None;
            }
            (unary.operand, false)
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            let (field, literal) = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(literal), ExpressionNode::Member(_)) => {
                    (binary.right, *literal)
                }
                (ExpressionNode::Member(_), ExpressionNode::Boolean(literal)) => {
                    (binary.left, *literal)
                }
                _ => return None,
            };
            (
                field,
                if binary.operator == BinaryOperator::Equal {
                    literal
                } else {
                    !literal
                },
            )
        }
        _ => return None,
    };
    let place =
        crate::flow::canonical_place_from_expression_in_state(program, state, 0, field_expression)?;
    let [facts::PlaceSegment::Field { symbol }] = place.segments.as_slice() else {
        return None;
    };
    if place.root != facts::PlaceRoot::Symbol(root_parameter.symbol)
        || !program.data_definitions().iter().any(|data| {
            program.data_members(data).iter().any(|member| {
                matches!(member, DataMember::Field(field)
                    if field.symbol == *symbol
                        && program.primitive_type_reference(field.type_reference)
                            == Some(PrimitiveType::Bool))
            })
        })
    {
        return None;
    }
    Some(CheckedUnitNominalAffineCleanupRequirementPlan {
        field_identity: terminal_field_identity(program, *symbol)?,
        expected,
    })
}

pub(super) fn is_bounded_nominal_cleanup_record(shape: &CheckedUnitStructuralTypeShape) -> bool {
    match shape {
        CheckedUnitStructuralTypeShape::Record { fields } => fields.iter().all(|field| {
            !field.relevance.is_erased()
                && matches!(
                    &field.field_type,
                    CheckedUnitStructuralFieldType::Scalar(
                        PrimitiveType::Bool
                            | PrimitiveType::I8
                            | PrimitiveType::I16
                            | PrimitiveType::I32
                            | PrimitiveType::I64
                            | PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                            | PrimitiveType::Addr
                    )
                )
        }),
        CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
        | CheckedUnitStructuralTypeShape::ByteSequence(_)
        | CheckedUnitStructuralTypeShape::FixedArray { .. }
        | CheckedUnitStructuralTypeShape::Sum { .. }
        | CheckedUnitStructuralTypeShape::Mixed { .. } => false,
    }
}

pub(super) fn build_partial_affine_unit_cleanup_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedPartialAffineUnitCleanupMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    let result_local = matches!(statements.first(), Some(StatementNode::LocalData(_)));
    if statements.is_empty()
        || statements
            .iter()
            .skip(usize::from(result_local))
            .any(|statement| !matches!(statement, StatementNode::Call(_)))
    {
        return None;
    }
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let runtime_arithmetic_requires_are_terminal =
        contract.crash.uses_structural_proof_gated_arithmetic()
            && contract.crash.structural_runtime_requirements().is_some();
    if !is_unit(program, state.return_type)
        || program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(state))
            .any(|contract| match contract.kind {
                SignatureContractKind::Crashes { .. } => false,
                SignatureContractKind::Requires if runtime_arithmetic_requires_are_terminal => {
                    false
                }
                SignatureContractKind::Requires
                | SignatureContractKind::Ensures
                | SignatureContractKind::EnsuresForResultCase { .. } => true,
            })
    {
        return None;
    }

    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        partial_affine_structural_signature(program, shapes, machine, state, &binders)?;
    if structural_parameters.len() > 1
        || program.state_parameters(state).len() != structural_parameters.len()
        || (!result_local && structural_parameters.len() != 1)
        || program
            .state_parameters(state)
            .iter()
            .zip(&structural_parameters)
            .any(|(source, checked)| {
                source.is_self
                    || checked.is_self
                    || checked.position != 0
                    || checked.multiplicity != Multiplicity::Affine
                    || checked.access != CheckedStructuralAccess::Owned
                    || !checked.qualifications.is_empty()
                    || type_graph_requires_nominal_drop(program, source.type_reference)
            })
    {
        return None;
    }
    let result_binding = if result_local {
        let (result, symbol) =
            checked_unit_structural_result_local(program, shapes, statements, &binders)?;
        let StatementNode::LocalData(local) = &statements[0] else {
            unreachable!()
        };
        if result.multiplicity != Multiplicity::Affine
            || shapes.add_partial_affine_type(local.type_reference, &binders)?
                != result.type_identity
            || !symbol.is_valid()
        {
            return None;
        }
        Some((result, facts::PlaceRoot::Symbol(symbol)))
    } else {
        None
    };
    let (root_source, root_symbol, root_type) =
        if let Some((result, facts::PlaceRoot::Symbol(symbol))) = &result_binding {
            (
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                },
                *symbol,
                result.type_identity.clone(),
            )
        } else {
            (
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 },
                parameter_root_symbol(machine.symbol, &program.state_parameters(state)[0]),
                structural_parameters[0].type_identity.clone(),
            )
        };
    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    if !entry_claims.is_empty() {
        return None;
    }
    if facts
        .qualifications
        .for_machine(machine.symbol)
        .is_some_and(|fact| !fact.body_committed.is_empty())
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
    {
        return None;
    }

    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span_or_empty(state_flow.calls);
    if calls.len() != statements.len()
        || calls.iter().enumerate().any(|(statement_index, call)| {
            call.statement_index != statement_index || call.call_ordinal != 0
        })
    {
        return None;
    }
    if !result_local && !service_reach_is_empty(facts, state_flow.service_reach) {
        return None;
    }
    let mut operations = Vec::with_capacity(calls.len().saturating_add(1));
    let mut moved_paths =
        Vec::<(Vec<CheckedUnitStructuralPathSegment>, String)>::with_capacity(calls.len());
    for call in calls {
        if result_local && call.statement_index == 0 {
            let (result, _) = result_binding.as_ref()?;
            let StatementNode::LocalData(local) = &statements[0] else {
                unreachable!()
            };
            if call.authored_expression != local.initial_value {
                return None;
            }
            let operation = build_call_operation(
                program,
                facts,
                machine,
                state,
                &structural_parameters,
                &[],
                &[],
                &entry_claims,
                call,
                false,
                Some(ExpectedCallValueResult::Structural(result)),
                &[],
            )?;
            let mut operation = control::bind_structural_call_result(operation, result.clone())?;
            match &mut operation {
                CheckedUnitEffectOperationPlan::StructuralCall {
                    structural_arguments,
                    scalar_arguments,
                    discard_result_on_return,
                    ..
                } => {
                    if !scalar_arguments.is_empty()
                        || structural_arguments.len() != structural_parameters.len()
                        || structural_arguments.iter().any(|argument| {
                            argument.source_parameter_index() != Some(0)
                                || !argument.path.is_empty()
                                || argument.access != CheckedStructuralAccess::Owned
                        })
                    {
                        return None;
                    }
                    *discard_result_on_return = false;
                }
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    structural_arguments,
                    scalar_arguments,
                    discard_result_on_return,
                    ..
                } => {
                    if !structural_parameters.is_empty()
                        || !structural_arguments.is_empty()
                        || !scalar_arguments.is_empty()
                    {
                        return None;
                    }
                    *discard_result_on_return = false;
                }
                _ => return None,
            }
            operations.push(operation);
            continue;
        }
        if !service_reach_is_empty(facts, call.service_reach) {
            return None;
        }
        let operation = build_call_operation(
            program,
            facts,
            machine,
            state,
            &structural_parameters,
            &[],
            &[],
            &entry_claims,
            call,
            true,
            None,
            result_binding.as_slice(),
        )?;
        let CheckedUnitEffectOperationPlan::CallUnit {
            target_machine,
            structural_arguments,
            claim_transfers,
            ..
        } = &operation
        else {
            return None;
        };
        let [argument] = structural_arguments.as_slice() else {
            return None;
        };
        if !is_partial_affine_path(&argument.path) {
            return None;
        }
        if argument.source != root_source
            || argument.access != CheckedStructuralAccess::Owned
            || !claim_transfers.is_empty()
            || moved_paths.iter().any(|(earlier, _)| {
                earlier.starts_with(&argument.path) || argument.path.starts_with(earlier)
            })
        {
            return None;
        }

        let target = unit_effects.for_machine(*target_machine)?;
        let [target_parameter] = target.structural_parameters.as_slice() else {
            return None;
        };
        if target_parameter.type_identity != argument.type_identity
            || target_parameter.access != CheckedStructuralAccess::Owned
            || !target.scalar_parameters.is_empty()
            || target_parameter.is_self
            || target_parameter.multiplicity != Multiplicity::Affine
            || !target_parameter.qualifications.is_empty()
            || !target.entry_claims.is_empty()
            || !target.trivial_affine_locals.is_empty()
            || !target.body_qualifications.is_empty()
            || !service_reach_is_empty(facts, target.service_reach)
            || !service_reach_plan_is_empty(facts, target.contract_service_reach)
            || !matches!(
                target.operations.as_slice(),
                [CheckedUnitEffectOperationPlan::ReturnUnit {
                    trivial_affine_local_discard_ordinals,
                    trivial_affine_discards,
                    ..
                }] if trivial_affine_local_discard_ordinals.is_empty()
                    && trivial_affine_discards.as_slice() == [0]
            )
        {
            return None;
        }
        moved_paths.push((argument.path.clone(), argument.type_identity.clone()));
        operations.push(operation);
    }

    // Indexed cleanup keeps the existing contract-free source boundary,
    // including arrays reached through record fields.
    if moved_paths.iter().any(|(path, _)| {
        path.iter()
            .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(_)))
    }) && (!program.machine_contracts(machine).is_empty()
        || !program.state_contracts(state).is_empty()
        || operations.iter().any(|operation| {
            let CheckedUnitEffectOperationPlan::CallUnit {
                target_machine,
                target_state,
                ..
            } = operation
            else {
                return !matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                );
            };
            !program
                .machines()
                .iter()
                .find(|candidate| candidate.symbol == *target_machine)
                .is_some_and(|target| program.machine_contracts(target).is_empty())
                || !crate::find_state(program, *target_state)
                    .is_some_and(|target| program.state_contracts(target).is_empty())
        }))
    {
        return None;
    }
    let residual_affine_discards =
        partial_affine_residuals(&shapes.types, &root_source, &root_type, &moved_paths)?;
    let provenance = if result_local {
        let provenance = language_semantics::PermissionProvenance::Established {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            source: PermissionEventSource::Statement { statement_index: 0 },
        };
        let mut establishments = facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.source == PermissionEventSource::Statement { statement_index: 0 }
                    && event.kind == PermissionEventKind::Establish
                    && event.root == facts::PlaceRoot::Symbol(root_symbol)
            });
        let establishment = establishments.next()?;
        if establishments.next().is_some()
            || establishment.provenance != provenance
            || establishment.access != PermissionAccess::Owned
            || establishment.multiplicity != Multiplicity::Affine
            || establishment.claim_identity != PermissionClaimIdentity::Unknown
            || establishment.obligation_live
            || !facts
                .flow
                .ownership
                .segments
                .span_or_empty(establishment.segments)
                .is_empty()
        {
            return None;
        }
        provenance
    } else {
        language_semantics::PermissionProvenance::Unknown
    };
    if !has_exact_symbol_affine_discard(facts, machine, state, root_symbol, provenance) {
        return None;
    }
    if !result_local
        && !service_reach_plan_is_empty(
            facts,
            facts.service_reaches.plan_for_machine(machine.symbol)?,
        )
    {
        return None;
    }
    operations.push(CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index: u32::try_from(statements.len()).ok()?,
        trivial_affine_local_discard_ordinals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    });
    Some(CheckedPartialAffineUnitCleanupMachinePlan {
        machine: CheckedUnitEffectMachinePlan {
            machine: machine.symbol,
            state: state.symbol,
            attachment_type_identity: Some(attachment_type_identity),
            structural_parameters,
            scalar_parameters: Vec::new(),
            provider_attachment_requirements: Vec::new(),
            trivial_affine_locals: Vec::new(),
            entry_claims,
            body_qualifications: Vec::new(),
            contract_report_fingerprint: contract.report_fingerprint,
            contract_commitment: contract.commitment,
            contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
            service_reach: state_flow.service_reach,
            operations,
        },
        residual_affine_discards,
    })
}

pub(super) fn partial_affine_residuals(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: &str,
    moved_paths: &[(Vec<CheckedUnitStructuralPathSegment>, String)],
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    let borrowed = moved_paths
        .iter()
        .map(|(path, moved_type)| (path.as_slice(), moved_type.as_str()))
        .collect::<Vec<_>>();
    residuals::reconstruct(types, source, root_type, &borrowed, usize::MAX)
}

fn is_partial_affine_path(path: &[CheckedUnitStructuralPathSegment]) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| {
            matches!(
                segment,
                CheckedUnitStructuralPathSegment::Field(_)
                    | CheckedUnitStructuralPathSegment::FixedIndex(_)
            )
        })
}

fn is_partial_affine_field_type(field_type: &CheckedUnitStructuralFieldType) -> bool {
    matches!(
        field_type,
        CheckedUnitStructuralFieldType::Structural { .. }
            | CheckedUnitStructuralFieldType::ByteSequence(
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
            )
            | CheckedUnitStructuralFieldType::Scalar(
                PrimitiveType::Bool
                    | PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::Addr
                    | PrimitiveType::F32
                    | PrimitiveType::F64
            )
    )
}

pub(super) fn machine_has_content_evidence(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> bool {
    facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        || facts
            .qualifications
            .content
            .partition_compositions
            .iter()
            .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
}

pub(super) fn service_reach_is_empty(
    facts: &CheckFacts,
    summary: language_semantics::ServiceReachSummary,
) -> bool {
    facts
        .service_reaches
        .rows
        .services(summary.direct)
        .is_empty()
        && facts
            .service_reaches
            .rows
            .services(summary.transitive)
            .is_empty()
}

pub(super) fn service_reach_plan_is_empty(
    facts: &CheckFacts,
    plan: language_semantics::ServiceReachPlan,
) -> bool {
    let published_is_empty = match plan.interface {
        language_semantics::ServiceReachInterface::InternalInferred => true,
        language_semantics::ServiceReachInterface::PublishedCeiling(row) => {
            facts.service_reaches.rows.services(row).is_empty()
        }
    };
    published_is_empty
        && facts
            .service_reaches
            .rows
            .services(plan.checked_inferred)
            .is_empty()
}

pub(super) fn has_exact_root_affine_discard(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameter: &StateParameter,
) -> bool {
    has_exact_symbol_affine_discard(
        facts,
        machine,
        state,
        parameter_root_symbol(machine.symbol, parameter),
        language_semantics::PermissionProvenance::Unknown,
    )
}

fn has_exact_symbol_affine_discard(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    symbol: SymbolHandle,
    provenance: language_semantics::PermissionProvenance,
) -> bool {
    let matching = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Affine
                && event.claim_identity == PermissionClaimIdentity::Unknown
                && event.provenance == provenance
                && !event.obligation_live
                && event.root == facts::PlaceRoot::Symbol(symbol)
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let [event] = matching.as_slice() else {
        return false;
    };
    facts
        .flow
        .ownership
        .segments
        .span_or_empty(event.segments)
        .is_empty()
}
