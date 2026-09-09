//! State-local inspection of an exact boundary result. Ordinary successor
//! planning retains borrowed carriers and non-payload arguments on each edge.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state_index: usize,
    signatures: &[Signature],
) -> Option<CheckedComposedUnitControlStatePlan> {
    let states = program.machine_states(machine);
    let state = states.get(state_index)?;
    let (structural, scalar) = &signatures[state_index];
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(result_local) = statements.first()? else {
        return None;
    };
    if result_local.is_mutable {
        return None;
    }
    let transition_start = statements
        .iter()
        .position(|statement| matches!(statement, StatementNode::Transition(_)))?;
    let mut destructures = Vec::new();
    for statement in &statements[1..transition_start] {
        let StatementNode::LocalData(local) = statement else {
            return None;
        };
        let encoded = local.name.as_str().strip_prefix("__arm_destructure#V=")?;
        let (variant, encoded) = encoded.split_once('#')?;
        let (fields, subject) = encoded.split_once("#~subject=")?;
        if local.is_mutable || variant.is_empty() || fields.is_empty() || subject.is_empty() {
            return None;
        }
        for field in fields.split('#') {
            if field.is_empty() || destructures.contains(&(variant, field)) {
                return None;
            }
            destructures.push((variant, field));
        }
    }
    let (result, result_symbol) = checked_unit_structural_result_local(
        program,
        shapes,
        std::slice::from_ref(&statements[0]),
        &[],
    )?;
    if result.statement_index != 0
        || result.binding_ordinal != 0
        || result_symbol != result_local.symbol
        || result.multiplicity != Multiplicity::Affine
    {
        return None;
    }
    // Parser-generated markers carry the whole inspected subject, not an
    // executable initializer. Check that exact root before omitting them.
    for (index, statement) in statements[1..transition_start].iter().enumerate() {
        let StatementNode::LocalData(local) = statement else {
            return None;
        };
        let subject = crate::flow::place::contextual_canonical_place_from_expression(
            program,
            state.symbol,
            index + 1,
            local.initial_value,
        )?;
        if subject.root != facts::PlaceRoot::Symbol(result_symbol) || !subject.segments.is_empty() {
            return None;
        }
    }
    let TypeReferenceNode::Named { symbol, .. } = program
        .type_reference_table
        .type_reference(result_local.type_reference)
    else {
        return None;
    };
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)?;
    if typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(data))
        != DataShapeKind::Enum
    {
        return None;
    }
    let variants = program
        .data_members(data)
        .iter()
        .map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if variants.len() != statements.len() - transition_start {
        return None;
    }
    if variants.iter().any(|variant| {
        program.data_payload_fields(variant).iter().any(|field| {
            program
                .primitive_type_reference(field.type_reference)
                .is_none()
        })
    }) {
        return None;
    }
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let source_calls = facts.flow.control.calls.span_or_empty(flow.calls);
    if source_calls
        .windows(2)
        .any(|pair| pair[0].statement_index > pair[1].statement_index)
    {
        return None;
    }
    let prefix_calls = &source_calls
        [..source_calls.partition_point(|call| call.statement_index < transition_start)];
    let [call] = prefix_calls else {
        return None;
    };
    if call.statement_index != 0 || call.call_ordinal != 0 {
        return None;
    }
    let built = build_call_operation(
        program,
        facts,
        machine,
        state,
        structural,
        &[],
        &[],
        &[],
        call,
        false,
        Some(ExpectedCallValueResult::Structural(&result)),
        &[],
    );
    let operation = match built? {
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            source_site,
            result: produced,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            target_contract_commitment,
            service_reach,
            scalar_arguments,
            structural_arguments,
            ..
        } if produced == result
            && structural_arguments.iter().all(|argument| {
                whole_shared_argument(argument)
                    || (argument.source_parameter_index().is_some()
                        && argument.path.is_empty()
                        && argument.access == CheckedStructuralAccess::MutableBorrow)
            }) =>
        {
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                source_site,
                result: produced,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                target_contract_commitment,
                service_reach,
                scalar_arguments,
                structural_arguments,
                discard_result_on_return: false,
            }
        }
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            source_site,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            scalar_arguments,
            structural_arguments,
            completion_receipts,
        } => {
            if !completion_receipts.is_empty() || !whole_view_arguments(&structural_arguments) {
                return None;
            }
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
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
            }
        }
        _ => return None,
    };
    let mut cases = Vec::new();
    let mut used_destructures = Vec::new();
    for (index, statement) in statements[transition_start..].iter().enumerate() {
        let StatementNode::Transition(transition) = statement else {
            return None;
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            return None;
        };
        let ordinal = u32::try_from(transition_start + index).ok()?;
        let (subject, case_symbol) = crate::proof::exact_outcome_case_test(program, guard)?;
        let subject = crate::flow::place::contextual_canonical_place_from_expression(
            program,
            state.symbol,
            ordinal as usize,
            subject,
        )?;
        if subject.root != facts::PlaceRoot::Symbol(result_symbol) || !subject.segments.is_empty() {
            return None;
        }
        let variant = variants
            .iter()
            .copied()
            .find(|variant| variant.symbol == case_symbol)?;
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
            if program.primitive_type_reference(field.type_reference)? != parameter.primitive_type
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
    if destructures
        .iter()
        .any(|destructure| !used_destructures.contains(destructure))
    {
        return None;
    }
    // A case edge disposes this completed result after projecting its selected
    // scalar fields. Require its actual ledger evidence; an empty or filtered
    // roster cannot establish that no other local needs cleanup.
    let mut result_cleanup_present = false;
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
            || event.provenance
                != (language_semantics::PermissionProvenance::Established {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    source: PermissionEventSource::Statement { statement_index: 0 },
                })
            || event.obligation_live
            || !facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .is_empty()
        {
            return None;
        }
        result_cleanup_present = true;
    }
    if !result_cleanup_present {
        return None;
    }
    if !return_unit_affine_discards(
        program,
        facts,
        machine.symbol,
        state.symbol,
        structural,
        program.state_parameters(state),
        &[],
        &[result_symbol],
    )?
    .is_empty()
    {
        return None;
    }
    Some(CheckedComposedUnitControlStatePlan {
        state: state.symbol,
        structural_parameters: structural.clone(),
        scalar_parameters: scalar.clone(),
        entry_claims: Vec::new(),
        bindings: Vec::new(),
        binding_initializers: Vec::new(),
        operations: vec![operation],
        terminator: CheckedComposedUnitControlTerminatorPlan::ClosedSum { result, cases },
    })
}
