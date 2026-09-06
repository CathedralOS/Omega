//! One boundary structural result inspected through its complete closed case roster.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let [entry, first_leaf, second_leaf] = program.machine_states(machine) else {
        return None;
    };
    let binders = machine_binders(program, machine);
    let (entry_attachment, entry_structural, entry_scalar) =
        structural_scalar_signature(program, shapes, machine, entry, &binders, false)?;
    if !entry_structural.is_empty()
        || !entry_scalar.is_empty()
        || !super::topology::only_implicit_reference_self_is_omitted(
            program,
            entry,
            &entry_structural,
            &entry_scalar,
        )
        || !program.state_contracts(entry).is_empty()
        || !is_unit(program, entry.return_type)
    {
        return None;
    }

    let statements = program.statement_table.statements(entry.statement_nodes);
    let [
        StatementNode::LocalData(result_local),
        StatementNode::LocalData(payload_local),
        StatementNode::Transition(first_transition),
        StatementNode::Transition(second_transition),
    ] = statements
    else {
        return None;
    };
    if result_local.is_mutable
        || payload_local.is_mutable
        || !payload_local
            .name
            .as_str()
            .starts_with("__arm_destructure#V=")
        || first_transition.exit != TransitionExit::Ordinary
        || second_transition.exit != TransitionExit::Ordinary
        || first_transition.continuation.is_valid()
        || second_transition.continuation.is_valid()
    {
        return None;
    }
    let (result, result_symbol) =
        checked_unit_structural_result_local(program, shapes, statements, &binders)?;
    if result.statement_index != 0
        || result.binding_ordinal != 0
        || result_symbol != result_local.symbol
        || result.multiplicity != Multiplicity::Affine
    {
        return None;
    }

    let TypeReferenceNode::Named {
        symbol: result_data_symbol,
        ..
    } = program
        .type_reference_table
        .type_reference(result_local.type_reference)
    else {
        return None;
    };
    let result_data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *result_data_symbol)?;
    if typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(result_data))
        != DataShapeKind::Enum
    {
        return None;
    }
    let variants = program
        .data_members(result_data)
        .iter()
        .map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if variants.len() != 2 {
        return None;
    }

    let (payload_variant_name, payload_field_name) =
        destructure_identity(payload_local.name.as_str())?;
    let payload_variant_definition = variants
        .iter()
        .copied()
        .find(|variant| variant.name.as_str() == payload_variant_name)?;
    let [payload_field_definition] = program.data_payload_fields(payload_variant_definition) else {
        return None;
    };
    let payload_primitive =
        program.primitive_type_reference(payload_field_definition.type_reference)?;
    if payload_field_definition.name.as_str() != payload_field_name {
        return None;
    }

    let entry_flow = state_flow(facts, machine.symbol, entry.symbol)?;
    let entry_calls = facts.flow.control.calls.span_or_empty(entry_flow.calls);
    let mut result_calls = entry_calls
        .iter()
        .filter(|call| call.statement_index == 0 && call.call_ordinal == 0);
    let result_call = result_calls.next()?;
    if result_calls.next().is_some() {
        return None;
    }
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        source_site,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        scalar_arguments,
        structural_arguments,
        completion_receipts,
    } = build_call_operation(
        program,
        facts,
        machine,
        entry,
        &[],
        &[],
        &[],
        &[],
        result_call,
        false,
        Some(ExpectedCallValueResult::Structural(&result)),
    )?
    else {
        return None;
    };
    let target_boundary = boundaries
        .iter()
        .find(|boundary| boundary.machine == target_machine)?;
    if !matches!(
        &target_boundary.result,
        CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity: Multiplicity::Affine,
            qualifications,
        } if type_identity == &result.type_identity && qualifications.is_empty()
    ) {
        return None;
    }
    let boundary_call = CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
        coordinate,
        source_site,
        result: result.clone(),
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        scalar_arguments,
        structural_arguments,
        completion_receipts,
        discard_result_on_return: false,
    };

    let mut cases = Vec::with_capacity(2);
    for (ordinal, transition) in [first_transition, second_transition]
        .into_iter()
        .enumerate()
    {
        let TransitionGuardNode::When(guard) = transition.guard else {
            return None;
        };
        let (subject, case_symbol) = crate::proof::exact_outcome_case_test(program, guard)?;
        let subject_place = crate::flow::place::contextual_canonical_place_from_expression(
            program,
            entry.symbol,
            2 + ordinal,
            subject,
        )?;
        if subject_place.root != facts::PlaceRoot::Symbol(result_symbol)
            || !subject_place.segments.is_empty()
        {
            return None;
        }
        let variant = variants
            .iter()
            .copied()
            .find(|variant| variant.symbol == case_symbol)?;
        let case_identity = variant_identity(variant);
        if cases
            .iter()
            .any(|case: &CheckedClosedSumCaseSuccessorPlan| case.case_identity == case_identity)
        {
            return None;
        }
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = program.statement_table.transition_target(transition.target)
        else {
            return None;
        };
        let target = [first_leaf, second_leaf]
            .into_iter()
            .find(|state| state.symbol == path.symbol)?;
        let target_scalar = leaf_signature(
            program,
            shapes,
            machine,
            target,
            &binders,
            &entry_attachment,
        )?;
        let arguments = program.statement_table.expression_handles(*arguments);
        let payloads = if variant.symbol == payload_variant_definition.symbol {
            let [target_parameter] = target_scalar.as_slice() else {
                return None;
            };
            let [argument] = arguments else {
                return None;
            };
            let argument_place = crate::flow::place::contextual_canonical_place_from_expression(
                program,
                entry.symbol,
                2 + ordinal,
                *argument,
            )?;
            if argument_place.root != facts::PlaceRoot::Symbol(result_symbol)
                || argument_place.segments
                    != [
                        facts::PlaceSegment::Case {
                            variant: payload_variant_definition.symbol,
                        },
                        facts::PlaceSegment::Field {
                            symbol: payload_field_definition.symbol,
                        },
                    ]
                || target_parameter.primitive_type != payload_primitive
            {
                return None;
            }
            vec![CheckedClosedSumPayloadTransferPlan {
                field_identity: field_identity(payload_field_definition),
                primitive_type: payload_primitive,
                target_scalar_parameter_index: 0,
            }]
        } else {
            if !target_scalar.is_empty()
                || !arguments.is_empty()
                || !program.data_payload_fields(variant).is_empty()
            {
                return None;
            }
            Vec::new()
        };
        if !return_unit_affine_discards(
            program,
            facts,
            machine.symbol,
            entry.symbol,
            &[],
            program.state_parameters(entry),
            &[],
            &[result_symbol],
        )?
        .is_empty()
        {
            return None;
        }
        cases.push(CheckedClosedSumCaseSuccessorPlan {
            case_identity,
            statement_ordinal: u32::try_from(2 + ordinal).ok()?,
            target_state: target.symbol,
            payloads,
        });
    }
    if variants
        .iter()
        .map(|variant| variant_identity(variant))
        .any(|identity| !cases.iter().any(|case| case.case_identity == identity))
    {
        return None;
    }

    let leaf_states = [first_leaf, second_leaf]
        .into_iter()
        .map(|state| {
            build_leaf(
                program,
                facts,
                shapes,
                boundaries,
                machine,
                state,
                &binders,
                &entry_attachment,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let mut provider_inputs = vec![(entry, entry_calls, std::slice::from_ref(&boundary_call))];
    for (state, plan) in [first_leaf, second_leaf].into_iter().zip(&leaf_states) {
        let flow = state_flow(facts, machine.symbol, state.symbol)?;
        provider_inputs.push((
            state,
            facts.flow.control.calls.span_or_empty(flow.calls),
            plan.operations.as_slice(),
        ));
    }
    let provider_requirements = checked_composed_provider_attachment_requirements(
        program,
        shapes,
        machine,
        &entry_attachment,
        &provider_inputs,
    )?;
    let mut states = vec![CheckedComposedUnitControlStatePlan {
        state: entry.symbol,
        structural_parameters: Vec::new(),
        scalar_parameters: Vec::new(),
        entry_claims: Vec::new(),
        bindings: Vec::new(),
        binding_initializers: Vec::new(),
        operations: vec![boundary_call],
        terminator: CheckedComposedUnitControlTerminatorPlan::ClosedSum { result, cases },
    }];
    states.extend(leaf_states);
    super::assembly::finish(
        facts,
        machine,
        entry_attachment,
        provider_requirements,
        states,
    )
}

fn leaf_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    expected_attachment: &str,
) -> Option<Vec<CheckedStructuralScalarParameterPlan>> {
    if !is_unit(program, state.return_type) || !program.state_contracts(state).is_empty() {
        return None;
    }
    let (attachment, structural, scalar) =
        structural_scalar_signature(program, shapes, machine, state, binders, false)?;
    (attachment == expected_attachment
        && structural.is_empty()
        && super::topology::only_implicit_reference_self_is_omitted(
            program,
            state,
            &structural,
            &scalar,
        ))
    .then_some(scalar)
}

fn build_leaf(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    expected_attachment: &str,
) -> Option<CheckedComposedUnitControlStatePlan> {
    let scalar_parameters = leaf_signature(
        program,
        shapes,
        machine,
        state,
        binders,
        expected_attachment,
    )?;
    let statements = program.statement_table.statements(state.statement_nodes);
    if statements.is_empty()
        || statements.iter().enumerate().any(|(index, statement)| {
            !matches!(statement, StatementNode::Call(_))
                && super::super::control::tail_call(program, state, index).is_none()
        })
    {
        return None;
    }
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let mut calls = super::super::control::outer_calls(
        program,
        facts,
        machine.symbol,
        state,
        facts.flow.control.calls.span(flow.calls)?,
    )?;
    if calls.len() != statements.len() {
        return None;
    }
    calls.sort_by_key(|call| call.statement_index);
    let mut operations = Vec::with_capacity(calls.len() + 1);
    for (statement_index, call) in calls.iter().enumerate() {
        if call.statement_index != statement_index || call.call_ordinal != 0 {
            return None;
        }
        let operation = build_call_operation(
            program,
            facts,
            machine,
            state,
            &[],
            &[],
            &[],
            &[],
            call,
            false,
            None,
        )?;
        match &operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. }
                if boundaries.iter().any(|boundary| {
                    boundary.machine == *target_machine && boundary.result.is_unit()
                }) => {}
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                claim_transfers,
                ..
            } if structural_arguments.is_empty() && claim_transfers.is_empty() => {}
            _ => return None,
        }
        operations.push(operation);
    }
    Some(CheckedComposedUnitControlStatePlan {
        state: state.symbol,
        structural_parameters: Vec::new(),
        scalar_parameters,
        entry_claims: Vec::new(),
        bindings: Vec::new(),
        binding_initializers: Vec::new(),
        operations,
        terminator: CheckedComposedUnitControlTerminatorPlan::ReturnUnit,
    })
}

fn variant_identity(variant: &typed_trees::data::DataVariant) -> String {
    variant
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| variant.name.as_str().to_owned())
}

fn field_identity(field: &typed_trees::data::DataField) -> String {
    field
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| field.name.as_str().to_owned())
}

fn destructure_identity(name: &str) -> Option<(&str, &str)> {
    let encoded = name.strip_prefix("__arm_destructure#V=")?;
    let (variant, encoded) = encoded.split_once('#')?;
    let (field, subject) = encoded.split_once('#')?;
    (!variant.is_empty() && !field.is_empty() && subject.starts_with("~subject="))
        .then_some((variant, field))
}
