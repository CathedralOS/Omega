//! Case terminators consume exact owned subjects after ordinary ordered effects.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state_index: usize,
    signatures: &[Signature],
    operations: &[CheckedUnitEffectOperationPlan],
    transition_start: usize,
) -> Option<CheckedComposedUnitControlTerminatorPlan> {
    let states = program.machine_states(machine);
    let state = states.get(state_index)?;
    let (structural, _) = &signatures[state_index];
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::Transition(first) = statements.get(transition_start)? else {
        return None;
    };
    let TransitionGuardNode::When(guard) = first.guard else {
        return None;
    };
    let (expression, _) = crate::proof::exact_outcome_case_test(program, guard)?;
    let place = crate::flow::place::contextual_canonical_place_from_expression(
        program,
        state.symbol,
        transition_start,
        expression,
    )?;
    let facts::PlaceRoot::Symbol(result_symbol) = place.root else {
        return None;
    };
    if !place.segments.is_empty() {
        return None;
    }
    let (subject, type_reference, expected_provenance) = if let Some((index, parameter)) = program
        .state_parameters(state)
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == result_symbol)
    {
        let (position, retained) = structural
            .iter()
            .enumerate()
            .find(|(_, retained)| retained.position as usize == index)?;
        if retained.access != CheckedStructuralAccess::Owned
            || retained.multiplicity != Multiplicity::Affine
            || !retained.qualifications.is_empty()
        {
            return None;
        }
        (
            CheckedUnitStructuralArgumentPlan {
                source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(position).ok()?,
                },
                path: Vec::new(),
                type_identity: retained.type_identity.clone(),
                access: CheckedStructuralAccess::Owned,
            },
            parameter.type_reference,
            language_semantics::PermissionProvenance::Unknown,
        )
    } else {
        let mut matching = operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralCall {
                    result,
                    discard_result_on_return: false,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return: false,
                    ..
                } => Some(result),
                _ => None,
            })
            .filter_map(|result| {
                let StatementNode::LocalData(local) =
                    statements.get(result.statement_index as usize)?
                else {
                    return None;
                };
                (local.symbol == result_symbol
                    && !local.is_mutable
                    && result.statement_index < transition_start as u32)
                    .then_some((result, local))
            });
        let (result, local) = matching.next()?;
        if matching.next().is_some() || result.multiplicity != Multiplicity::Affine {
            return None;
        }
        (
            CheckedUnitStructuralArgumentPlan {
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                },
                path: Vec::new(),
                type_identity: result.type_identity.clone(),
                access: CheckedStructuralAccess::Owned,
            },
            local.type_reference,
            language_semantics::PermissionProvenance::Established {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                source: PermissionEventSource::Statement {
                    statement_index: result.statement_index as usize,
                },
            },
        )
    };
    if !validation::has_plain_owned_contents_with_numeric_constraints(program, type_reference) {
        return None;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)?;
    let variants = program
        .data_members(data)
        .iter()
        .map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if variants.is_empty()
        || statements.len() - transition_start > variants.len()
        || variants.iter().any(|variant| {
            program.data_payload_fields(variant).iter().any(|field| {
                program
                    .primitive_type_reference(field.type_reference)
                    .is_none()
            })
        })
    {
        return None;
    }
    let marker_start = statements[..transition_start].iter().rposition(|statement| !matches!(statement, StatementNode::LocalData(local) if local.name.as_str().starts_with("__arm_destructure#V="))).map_or(0, |index| index + 1);
    let mut destructures = Vec::new();
    for (index, statement) in statements[marker_start..transition_start]
        .iter()
        .enumerate()
    {
        let StatementNode::LocalData(local) = statement else {
            return None;
        };
        let encoded = local.name.as_str().strip_prefix("__arm_destructure#V=")?;
        let (variant, encoded) = encoded.split_once('#')?;
        let (fields, _) = encoded.split_once("#~subject=")?;
        let place = crate::flow::place::contextual_canonical_place_from_expression(
            program,
            state.symbol,
            marker_start + index,
            local.initial_value,
        )?;
        if local.is_mutable
            || place.root != facts::PlaceRoot::Symbol(result_symbol)
            || !place.segments.is_empty()
        {
            return None;
        }
        for field in fields.split('#') {
            if field.is_empty() || destructures.contains(&(variant, field)) {
                return None;
            }
            destructures.push((variant, field));
        }
    }
    let mut cases = Vec::new();
    let mut used_destructures = Vec::new();
    for (index, statement) in statements[transition_start..].iter().enumerate() {
        let StatementNode::Transition(transition) = statement else {
            return None;
        };
        let ordinal = u32::try_from(transition_start + index).ok()?;
        let selected_variants = match transition.guard {
            TransitionGuardNode::When(guard) => {
                let (expression, case_symbol) =
                    crate::proof::exact_outcome_case_test(program, guard)?;
                let place = crate::flow::place::contextual_canonical_place_from_expression(
                    program,
                    state.symbol,
                    ordinal as usize,
                    expression,
                )?;
                if place.root != facts::PlaceRoot::Symbol(result_symbol)
                    || !place.segments.is_empty()
                {
                    return None;
                }
                vec![
                    variants
                        .iter()
                        .copied()
                        .find(|variant| variant.symbol == case_symbol)?,
                ]
            }
            TransitionGuardNode::Always if transition_start + index + 1 == statements.len() => {
                variants
                    .iter()
                    .copied()
                    .filter(|variant| {
                        let identity = variant
                            .identity
                            .map(|identity| format!("#{identity}"))
                            .unwrap_or_else(|| variant.name.as_str().to_owned());
                        !cases
                            .iter()
                            .any(|case: &CheckedClosedSumCaseSuccessorPlan| {
                                case.case_identity == identity
                            })
                    })
                    .collect()
            }
            _ => return None,
        };
        if selected_variants.is_empty() {
            return None;
        }
        for variant in selected_variants {
            let identity = variant
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| variant.name.as_str().to_owned());
            if cases
                .iter()
                .any(|case: &CheckedClosedSumCaseSuccessorPlan| case.case_identity == identity)
            {
                return None;
            }
            let TransitionTargetNode::Named {
                path, arguments, ..
            } = program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            let target_index = crate::checks::termination::named_transition_target_state_index(
                program,
                machine,
                path.symbol,
            )?;
            let target = &states[target_index];
            let arguments = program.statement_table.expression_handles(*arguments);
            let target_parameters = program.state_parameters(target);
            let mut payloads = Vec::new();
            for (parameter_index, parameter) in signatures[target_index].1.iter().enumerate() {
                let argument_index = target_parameters
                    .iter()
                    .take(parameter.source_position as usize)
                    .filter(|parameter| !parameter.is_self)
                    .count();
                let argument = *arguments.get(argument_index)?;
                let Some(place) = crate::flow::place::contextual_canonical_place_from_expression(
                    program,
                    state.symbol,
                    ordinal as usize,
                    argument,
                ) else {
                    continue;
                };
                if place.root != facts::PlaceRoot::Symbol(result_symbol) {
                    continue;
                }
                let [
                    facts::PlaceSegment::Case { variant: selected },
                    facts::PlaceSegment::Field { symbol },
                ] = place.segments.as_slice()
                else {
                    return None;
                };
                if *selected != variant.symbol {
                    return None;
                }
                let field = program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.symbol == *symbol)?;
                if program.primitive_type_reference(field.type_reference)?
                    != parameter.primitive_type
                    || !destructures.contains(&(variant.name.as_str(), field.name.as_str()))
                {
                    return None;
                }
                used_destructures.push((variant.name.as_str(), field.name.as_str()));
                payloads.push(CheckedClosedSumPayloadTransferPlan {
                    field_identity: field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                    primitive_type: parameter.primitive_type,
                    target_scalar_parameter_index: u32::try_from(parameter_index).ok()?,
                });
            }
            let payload_parameters = payloads
                .iter()
                .map(|payload| payload.target_scalar_parameter_index)
                .collect::<Vec<_>>();
            let successor = successor_bindings(
                program,
                facts,
                machine,
                state_index,
                signatures,
                operations,
                transition,
                ordinal,
                &payload_parameters,
            )?;
            cases.push(CheckedClosedSumCaseSuccessorPlan {
                case_identity: identity,
                successor,
                payloads,
            });
        }
    }
    if cases.len() != variants.len() {
        return None;
    }
    if destructures
        .iter()
        .any(|destructure| !used_destructures.contains(destructure))
    {
        return None;
    }
    // Consuming case dispatch requires the actual source exit disposition.
    // Parameter-origin carriers and fresh results have different provenance.
    let mut found = false;
    for (_, event) in facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
        })
    {
        if event.root != facts::PlaceRoot::Symbol(result_symbol)
            || event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != expected_provenance
            || event.obligation_live
            || !facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .is_empty()
            || found
        {
            return None;
        }
        found = true;
    }
    if !found {
        return None;
    }
    Some(CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases })
}
