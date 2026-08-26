//! Structural call closure, argument custody, and claim transfer.

use super::*;

pub(super) fn build_unit_trivial_affine_locals(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    statements: &[StatementNode],
) -> Option<Vec<(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)>> {
    statements
        .iter()
        .enumerate()
        .map(|(declaration_ordinal, statement)| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            let TypeReferenceNode::Named { .. } = program
                .type_reference_table
                .type_reference(local.type_reference)
            else {
                return None;
            };
            if local.is_mutable
                || !local.initial_value.is_valid()
                || crate::checks::type_multiplicity(program, local.type_reference)
                    != Multiplicity::Affine
                || !parameter_qualifications(program, shapes, local.type_reference, binders)?
                    .is_empty()
                || type_graph_requires_nominal_drop(program, local.type_reference)
            {
                return None;
            }
            let ExpressionNode::StructLiteral(literal) =
                program.expression_table.expression(local.initial_value)
            else {
                return None;
            };
            if literal.case_name.is_some()
                || !program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty()
            {
                return None;
            }
            let local_events = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.machine_symbol == machine.symbol
                        && event.state_symbol == state.symbol
                        && event.root == psi_facts::PlaceRoot::Symbol(local.symbol)
                })
                .map(|(_, event)| event)
                .collect::<Vec<_>>();
            let [event] = local_events.as_slice() else {
                return None;
            };
            if event.source != PermissionEventSource::StateExit
                || event.kind != PermissionEventKind::AffineDrop
                || event.multiplicity != Multiplicity::Affine
                || event.access != PermissionAccess::Owned
                || event.claim_identity != PermissionClaimIdentity::Unknown
                || event.provenance != psi_language_semantics::PermissionProvenance::Unknown
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
            let type_identity = shapes.add_type(local.type_reference, binders, &[])?;
            let shape = shapes.types.get(&type_identity)?;
            if !matches!(
                &shape.shape,
                CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
            ) {
                return None;
            }
            Some((
                CheckedTrivialAffineStructuralLocalPlan {
                    declaration_ordinal: u32::try_from(declaration_ordinal).ok()?,
                    type_identity,
                },
                local.symbol,
            ))
        })
        .collect()
}

pub(super) fn build_call_operation(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    call: &psi_checked_trees::FlowCallFact,
    allow_field_path_projection: bool,
    expected_boundary_result: Option<PrimitiveType>,
) -> Option<CheckedUnitEffectOperationPlan> {
    let coordinate = CheckedUnitCallCoordinate {
        statement_index: u32::try_from(call.statement_index).ok()?,
        call_ordinal: u32::try_from(call.call_ordinal).ok()?,
    };
    let call_site = crate::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;

    if program
        .symbols
        .builtin_function_symbol(BuiltinFunction::AsmPortOut)
        == Some(call.target_symbol)
    {
        let arguments = crate::call_site_argument_expressions(program, &call_site);
        let [port, value] = arguments else {
            return None;
        };
        return Some(CheckedUnitEffectOperationPlan::PortWrite {
            coordinate,
            port: exact_integer_at(
                facts,
                machine.symbol,
                state.symbol,
                call.statement_index,
                *port,
                PrimitiveType::U16,
            )?
            .try_into()
            .ok()?,
            value: exact_integer_at(
                facts,
                machine.symbol,
                state.symbol,
                call.statement_index,
                *value,
                PrimitiveType::U8,
            )?
            .try_into()
            .ok()?,
            service_reach: call.service_reach.clone(),
        });
    }

    let mut static_boundaries = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == call.target_symbol)
                .map(move |signature| (definition, signature, false))
        })
        .collect::<Vec<_>>();
    if let Some((_, requirement)) = program.machine_parameter_signature(call.target_symbol) {
        static_boundaries.extend(
            program
                .traits()
                .iter()
                .filter(|definition| definition.is_boundary)
                .flat_map(|definition| {
                    program
                        .trait_machine_signatures(definition)
                        .iter()
                        .filter(move |signature| signature.symbol == requirement.symbol)
                        .map(move |signature| (definition, signature, true))
                }),
        );
    }
    if let [(definition, signature, selected_parameter)] = static_boundaries.as_slice() {
        let arguments = crate::call_site_argument_expressions(program, &call_site);
        let source_parameters = program.state_signature_parameters(signature);
        let abi_parameters = source_parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| !parameter.is_self)
            .collect::<Vec<_>>();
        let caller_source_parameters = program.state_parameters(state);
        let mut scalar_parameters = Vec::new();
        let mut structural_arguments = Vec::new();
        for (abi_position, ((_, parameter), argument)) in
            abi_parameters.iter().zip(arguments.iter()).enumerate()
        {
            let source_position = u32::try_from(abi_position).ok()?;
            if let Some(primitive_type) = program.primitive_type_reference(parameter.type_reference)
            {
                scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                    source_position,
                    primitive_type,
                });
                continue;
            }
            let byte_sequence = byte_sequence_carrier(program, parameter.type_reference, &[]);
            let target_identity = if byte_sequence.is_some() {
                byte_sequence_type_identity(program, parameter.type_reference, &[], &[])?
            } else {
                base_type_identity(program, parameter.type_reference, &[])?
            };
            if byte_sequence == Some(psi_checked_trees::CheckedByteSequenceCarrier::BorrowedView)
                && let ExpressionNode::String(bytes) =
                    program.expression_table.expression(*argument)
            {
                structural_arguments.push(CheckedUnitStructuralArgumentPlan {
                    source_parameter_index: u32::MAX,
                    path: Vec::new(),
                    type_identity: target_identity,
                    access: structural_access_for_type_reference(
                        program,
                        parameter.type_reference,
                    )?,
                    byte_sequence_literal: Some(bytes.to_vec()),
                });
                continue;
            }

            let place = crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                call.statement_index,
                *argument,
            )?;
            let psi_facts::PlaceRoot::Symbol(source_symbol) = place.root else {
                return None;
            };
            if !place.segments.is_empty() {
                return None;
            }
            let source_parameter = caller_source_parameters.iter().find(|candidate| {
                parameter_root_symbol(machine.symbol, candidate) == source_symbol
            })?;
            let source_position = caller_source_parameters
                .iter()
                .position(|candidate| candidate.symbol == source_parameter.symbol)?;
            let source_parameter_index = caller_parameters.iter().position(|candidate| {
                candidate.position == u32::try_from(source_position).unwrap_or(u32::MAX)
            })?;
            if caller_parameters.get(source_parameter_index)?.type_identity != target_identity {
                return None;
            }
            structural_arguments.push(CheckedUnitStructuralArgumentPlan {
                source_parameter_index: u32::try_from(source_parameter_index).ok()?,
                path: Vec::new(),
                type_identity: target_identity,
                access: exact_structural_argument_access(
                    facts,
                    machine.symbol,
                    state.symbol,
                    call,
                    &place,
                    structural_access_for_type_reference(program, parameter.type_reference)?,
                )?,
                byte_sequence_literal: None,
            });
        }
        if !program.trait_type_parameters(definition).is_empty()
            || !program
                .state_signature_type_parameters(signature)
                .is_empty()
            || program
                .state_signature_parameters(signature)
                .iter()
                .any(|parameter| !parameter.is_self && (parameter.is_const || parameter.is_mutable))
            || arguments.len() != abi_parameters.len()
            || if *selected_parameter {
                call.has_receiver
            } else {
                !call.has_receiver
                    || (call.receiver_symbol != definition.symbol
                        && !provider_attachment_receiver_matches(
                            program,
                            machine,
                            &call_site,
                            definition.symbol,
                        ))
            }
            || match expected_boundary_result {
                None => !is_unit(program, signature.return_type),
                Some(expected) => {
                    program.primitive_type_reference(signature.return_type) != Some(expected)
                }
            }
            || !signature_contracts_are_exact_parameter_qualifications(program, signature)
            || signature.suspends
            || signature.blocks
        {
            return None;
        }
        let capsule = facts
            .contract_plans
            .crash_capsule(definition.symbol, signature.symbol)?;
        let completion_receipts = if *selected_parameter {
            call_claim_transfers(
                facts,
                machine.symbol,
                state.symbol,
                call,
                caller_parameters,
                entry_claims,
                &structural_arguments,
                PermissionEventKind::Transfer,
            )?
        } else {
            Vec::new()
        };
        return Some(CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            target_machine: signature.symbol,
            target_state: signature.symbol,
            target_contract_fingerprint: capsule.target_contract_fingerprint(),
            service_reach: call.service_reach,
            scalar_arguments: checked_boundary_scalar_arguments(
                facts,
                state.symbol,
                coordinate,
                &scalar_parameters,
            )?,
            structural_arguments,
            completion_receipts,
        });
    }
    if !static_boundaries.is_empty() {
        return None;
    }

    let target_state = crate::find_state(program, call.target_symbol)?;
    let target_machine = program.machines().iter().find(|candidate| {
        program
            .machine_states(candidate)
            .iter()
            .any(|candidate_state| candidate_state.symbol == target_state.symbol)
    })?;
    let target_contract = facts.contract_plans.for_machine(target_machine.symbol)?;
    let boundary = target_machine.supply_mode.is_boundary_declaration();
    if if boundary {
        match expected_boundary_result {
            None => !is_unit(program, target_state.return_type),
            Some(expected) => {
                program.primitive_type_reference(target_state.return_type) != Some(expected)
            }
        }
    } else {
        expected_boundary_result.is_some() || !is_unit(program, target_state.return_type)
    } {
        return None;
    }
    if !boundary && target_machine.supply_mode != MachineSupplyMode::CheckedBody {
        return None;
    }
    let structural_arguments = structural_call_arguments(
        program,
        facts,
        call,
        machine,
        state,
        caller_parameters,
        target_machine,
        target_state,
        &call_site,
        call.receiver_symbol,
        call.statement_index,
        true,
        allow_field_path_projection,
    )?;
    let scalar_parameters = program
        .state_parameters(target_state)
        .iter()
        .enumerate()
        .filter_map(|(position, parameter)| {
            program
                .primitive_type_reference(parameter.type_reference)
                .map(|primitive_type| (position, parameter, primitive_type))
        })
        .map(|(position, parameter, primitive_type)| {
            if parameter.is_self || parameter.is_const || parameter.is_mutable {
                return None;
            }
            Some(CheckedStructuralScalarParameterPlan {
                source_position: u32::try_from(position).ok()?,
                primitive_type,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let scalar_arguments = if boundary {
        checked_boundary_scalar_arguments(facts, state.symbol, coordinate, &scalar_parameters)?
    } else {
        Vec::new()
    };
    if !boundary
        && !ordinary_projected_call_is_supported(
            program,
            facts,
            machine,
            state,
            caller_parameters,
            target_machine,
            target_state,
            &structural_arguments,
            allow_field_path_projection,
        )
    {
        return None;
    }
    let transfers = call_claim_transfers(
        facts,
        machine.symbol,
        state.symbol,
        call,
        caller_parameters,
        entry_claims,
        &structural_arguments,
        if boundary {
            PermissionEventKind::Consume
        } else {
            PermissionEventKind::Transfer
        },
    )?;

    if boundary {
        Some(CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            target_machine: target_machine.symbol,
            target_state: target_state.symbol,
            target_contract_fingerprint: target_contract.fingerprint,
            service_reach: call.service_reach.clone(),
            scalar_arguments,
            structural_arguments,
            completion_receipts: transfers,
        })
    } else {
        Some(CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine: target_machine.symbol,
            target_state: target_state.symbol,
            target_contract_fingerprint: target_contract.fingerprint,
            service_reach: call.service_reach.clone(),
            structural_arguments,
            claim_transfers: transfers,
        })
    }
}

fn provider_attachment_receiver_matches(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    call_site: &crate::CallSite<'_>,
    provider_symbol: SymbolHandle,
) -> bool {
    let crate::CallSite::Statement(call) = call_site else {
        return false;
    };
    let [self_name, field_name] = program.statement_table.name_path_members(call.receiver) else {
        return false;
    };
    if self_name.as_str() != "self" {
        return false;
    }
    let Some(attached_name) = machine.attached_data.as_ref() else {
        return false;
    };
    let Some(attached) = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_name)
    else {
        return false;
    };
    program.data_members(attached).iter().any(|member| {
        let DataMember::Field(field) = member else {
            return false;
        };
        if field.name != *field_name || field.relevance.is_erased() {
            return false;
        }
        matches!(
            program
                .type_reference_table
                .type_reference(field.type_reference),
            TypeReferenceNode::Named { symbol, .. }
                | TypeReferenceNode::DynamicTrait { symbol, .. }
                if *symbol == provider_symbol
        )
    })
}

fn checked_boundary_scalar_arguments(
    facts: &CheckFacts,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    parameters: &[CheckedStructuralScalarParameterPlan],
) -> Option<Vec<CheckedScalarExpression>> {
    parameters
        .iter()
        .map(|parameter| {
            let expression = facts.values.scalar_expressions.expression_at(
                caller_state,
                coordinate.statement_index,
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: coordinate.call_ordinal,
                    argument_ordinal: parameter.source_position,
                },
            )?;
            (crate::values::scalar_expression_type(expression)? == parameter.primitive_type)
                .then(|| expression.clone())
        })
        .collect()
}

pub(super) fn ordinary_projected_call_is_supported(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: &psi_typed_trees::machine::Machine,
    caller_state: &psi_typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    target_machine: &psi_typed_trees::machine::Machine,
    target_state: &psi_typed_trees::state::State,
    arguments: &[CheckedUnitStructuralArgumentPlan],
    allow_field_path_projection: bool,
) -> bool {
    if arguments
        .iter()
        .any(|argument| argument.byte_sequence_literal.is_some())
    {
        return false;
    }
    if arguments.iter().all(|argument| argument.path.is_empty()) {
        return true;
    }

    let caller_source_parameters = program.state_parameters(caller_state);
    let target_source_parameters = program.state_parameters(target_state);
    if caller_source_parameters.len() != 1
        || caller_parameters.len() != 1
        || target_source_parameters.len() != 1
        || arguments.len() != 1
        || arguments[0].source_parameter_index != 0
    {
        return false;
    }

    let field_path = !arguments[0].path.is_empty()
        && arguments[0]
            .path
            .iter()
            .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)));
    if field_path && !allow_field_path_projection {
        return false;
    }
    if !field_path
        && !matches!(
            arguments[0].path.as_slice(),
            [CheckedUnitStructuralPathSegment::FixedIndex(_)]
        )
    {
        return false;
    }

    let has_content_evidence = |machine, state| {
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
    };
    if has_content_evidence(caller_machine.symbol, caller_state.symbol)
        || has_content_evidence(target_machine.symbol, target_state.symbol)
    {
        return false;
    }

    let target_parameters = target_source_parameters
        .iter()
        .filter(|parameter| !(parameter.is_self && is_reference(program, parameter.type_reference)))
        .collect::<Vec<_>>();
    if target_parameters.len() != arguments.len() {
        return false;
    }

    if target_contract_mentions_projected_parameter(
        program,
        facts,
        target_machine,
        target_state,
        &target_source_parameters[0],
    ) {
        return false;
    }

    if field_path {
        let [caller_parameter] = caller_parameters else {
            return false;
        };
        let [target_parameter] = target_parameters.as_slice() else {
            return false;
        };
        return program.machine_states(caller_machine).len() == 1
            && program.machine_states(target_machine).len() == 1
            && caller_parameter.multiplicity == Multiplicity::Affine
            && caller_parameter.qualifications.is_empty()
            && !target_parameter.is_self
            && crate::checks::type_multiplicity(program, target_parameter.type_reference)
                == Multiplicity::Affine
            && !type_graph_requires_nominal_drop(program, target_parameter.type_reference)
            && facts
                .contract_plans
                .for_machine(caller_machine.symbol)
                .is_some()
            && facts
                .contract_plans
                .for_machine(target_machine.symbol)
                .is_some();
    }

    arguments
        .iter()
        .zip(target_parameters)
        .filter(|(argument, _)| !argument.path.is_empty())
        .all(|(_, parameter)| {
            let mut type_reference = parameter.type_reference;
            loop {
                match program.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        if !program
                            .type_reference_table
                            .constraints(*constraints)
                            .is_empty()
                        {
                            return false;
                        }
                        type_reference = *base_type;
                    }
                    TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                    _ => break,
                }
            }

            let expected_root = psi_facts::PlaceRoot::Symbol(parameter_root_symbol(
                target_machine.symbol,
                parameter,
            ));
            let matching = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.machine_symbol == target_machine.symbol
                        && event.state_symbol == target_state.symbol
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                        && event.multiplicity == Multiplicity::Linear
                        && event.obligation_live
                        && event.root == expected_root
                })
                .map(|(_, event)| event)
                .collect::<Vec<_>>();
            let [claim] = matching.as_slice() else {
                return false;
            };
            claim.claim_identity != PermissionClaimIdentity::Unknown
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(claim.segments)
                    .is_empty()
        })
}

pub(super) fn target_contract_mentions_projected_parameter(
    program: &TypedTrees,
    facts: &CheckFacts,
    target_machine: &psi_typed_trees::machine::Machine,
    target_state: &psi_typed_trees::state::State,
    parameter: &StateParameter,
) -> bool {
    let expected_root = parameter_root_symbol(target_machine.symbol, parameter);
    let runtime_arithmetic_requires_are_terminal = facts
        .contract_plans
        .for_machine(target_machine.symbol)
        .is_some_and(|contract| {
            contract.crash.uses_structural_proof_gated_arithmetic()
                && contract.crash.structural_runtime_requirements().is_some()
        });
    let authored_contract_mentions_parameter = program
        .state_contracts(target_state)
        .iter()
        .filter(|contract| match contract.kind {
            SignatureContractKind::Crashes { .. } => false,
            SignatureContractKind::Requires if runtime_arithmetic_requires_are_terminal => false,
            SignatureContractKind::Requires
            | SignatureContractKind::Ensures
            | SignatureContractKind::EnsuresForResultCase { .. } => true,
        })
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .any(|fact| {
            let ProofFact::Membership(membership) = fact else {
                return false;
            };
            crate::flow::canonical_place_from_expression_in_state(
                program,
                target_state.symbol,
                0,
                membership.value,
            )
            .is_some_and(|place| {
                place.root == psi_facts::PlaceRoot::Symbol(expected_root)
                    || place.root == psi_facts::PlaceRoot::Symbol(parameter.symbol)
            })
        });
    if authored_contract_mentions_parameter {
        return true;
    }

    facts
        .contract_plans
        .for_machine(target_machine.symbol)
        .is_some_and(|contract| {
            contract.crash.published().iter().any(|bucket| {
                bucket.alternative_guards().iter().any(|guard| match guard {
                    psi_checked_trees::CrashRouteGuard::Truth => false,
                    psi_checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        if matches!(
                            predicate.scalar_expression(),
                            Some(
                                psi_checked_trees::CheckedBooleanExpression::StructuralParameterField {
                                    parameter_position: 0,
                                    path,
                                }
                            ) if !path.is_empty()
                        ) {
                            return false;
                        }
                        if predicate.expression().is_some_and(|expression| {
                            crash_expression_is_nonempty_member_path_from_parameter(expression, 0)
                        }) {
                            return false;
                        }
                        predicate.expression().is_none_or(|expression| {
                            crash_expression_mentions_parameter_outside_member_path(expression, 0)
                        })
                    }
                })
            })
        })
}

pub(super) fn crash_expression_is_nonempty_member_path_from_parameter(
    expression: &psi_checked_trees::CrashPredicateExpression,
    parameter: u32,
) -> bool {
    use psi_checked_trees::CrashPredicateExpression;

    let mut expression = expression;
    let mut nonempty = false;
    while let CrashPredicateExpression::Member { receiver, .. } = expression {
        nonempty = true;
        expression = receiver;
    }
    nonempty
        && matches!(expression, CrashPredicateExpression::Parameter(index) if *index == parameter)
}

pub(super) fn crash_expression_mentions_parameter_outside_member_path(
    expression: &psi_checked_trees::CrashPredicateExpression,
    parameter: u32,
) -> bool {
    use psi_checked_trees::CrashPredicateExpression;

    match expression {
        CrashPredicateExpression::Parameter(index) => *index == parameter,
        CrashPredicateExpression::Binary { left, right, .. } => {
            crash_expression_mentions_parameter_outside_member_path(left, parameter)
                || crash_expression_mentions_parameter_outside_member_path(right, parameter)
        }
        CrashPredicateExpression::Unary { operand, .. } => {
            crash_expression_mentions_parameter_outside_member_path(operand, parameter)
        }
        CrashPredicateExpression::Member { receiver, .. } => {
            if crash_expression_is_nonempty_member_path_from_parameter(expression, parameter) {
                false
            } else {
                crash_expression_mentions_parameter_outside_member_path(receiver, parameter)
            }
        }
        CrashPredicateExpression::Call {
            receiver,
            arguments,
            ..
        } => {
            crash_expression_mentions_parameter_outside_member_path(receiver, parameter)
                || arguments.iter().any(|argument| {
                    crash_expression_mentions_parameter_outside_member_path(argument, parameter)
                })
        }
        CrashPredicateExpression::Invalid
        | CrashPredicateExpression::Opaque(_)
        | CrashPredicateExpression::ContentConservation(_) => true,
        CrashPredicateExpression::Integer(_)
        | CrashPredicateExpression::Boolean(_)
        | CrashPredicateExpression::Name(_) => false,
    }
}

pub(super) fn structural_call_arguments(
    program: &TypedTrees,
    facts: &CheckFacts,
    call: &psi_checked_trees::FlowCallFact,
    caller_machine: &psi_typed_trees::machine::Machine,
    caller_state: &psi_typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    target_machine: &psi_typed_trees::machine::Machine,
    target_state: &psi_typed_trees::state::State,
    call_site: &crate::CallSite<'_>,
    receiver_symbol: SymbolHandle,
    statement_index: usize,
    allow_fixed_index_projection: bool,
    allow_field_path_projection: bool,
) -> Option<Vec<CheckedUnitStructuralArgumentPlan>> {
    let source_parameters = program.state_parameters(caller_state);
    let target_parameters = program.state_parameters(target_state);
    let explicit_arguments = crate::call_site_argument_expressions(program, call_site);
    let explicit_self = explicit_arguments.len()
        > target_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
    let mut explicit_index = 0usize;
    let mut output = Vec::new();

    for target in target_parameters {
        if program
            .primitive_type_reference(target.type_reference)
            .is_some()
        {
            if target.is_self {
                return None;
            }
            explicit_arguments.get(explicit_index)?;
            explicit_index = explicit_index.checked_add(1)?;
            continue;
        }
        let place = if target.is_self {
            if is_reference(program, target.type_reference) {
                continue;
            }
            if explicit_self {
                let expression = *explicit_arguments.get(explicit_index)?;
                explicit_index += 1;
                crate::flow::canonical_place_from_expression_in_state(
                    program,
                    caller_state.symbol,
                    statement_index,
                    expression,
                )?
            } else {
                crate::flow::owned_method_receiver_place(
                    program,
                    caller_state.symbol,
                    statement_index,
                    call_site,
                    target_parameters,
                    receiver_symbol,
                )
                .or_else(|| crate::flow::canonical_place_from_symbol(receiver_symbol))?
            }
        } else {
            let expression = *explicit_arguments.get(explicit_index)?;
            explicit_index += 1;
            crate::flow::canonical_place_from_expression_in_state(
                program,
                caller_state.symbol,
                statement_index,
                expression,
            )?
        };
        let psi_facts::PlaceRoot::Symbol(source_symbol) = place.root else {
            return None;
        };
        let source_parameter = source_parameters.iter().find(|parameter| {
            parameter_root_symbol(caller_machine.symbol, parameter) == source_symbol
        })?;
        let source_index = caller_parameters.iter().position(|candidate| {
            candidate.position
                == u32::try_from(
                    source_parameters
                        .iter()
                        .position(|parameter| parameter.symbol == source_parameter.symbol)
                        .unwrap_or(usize::MAX),
                )
                .unwrap_or(u32::MAX)
        })?;
        let source_identity = caller_parameters.get(source_index)?.type_identity.clone();
        let target_identity = if target.is_self {
            attached_data_identity(program, target_machine)?
        } else if byte_sequence_carrier(program, target.type_reference, &[]).is_some() {
            byte_sequence_type_identity(program, target.type_reference, &[], &[])?
        } else {
            base_type_identity(program, target.type_reference, &[])?
        };
        let path = match place.segments.as_slice() {
            [] => Vec::new(),
            [psi_facts::PlaceSegment::FixedIndex { index }]
                if allow_fixed_index_projection
                    && caller_parameters
                        .get(source_index)?
                        .qualifications
                        .is_empty() =>
            {
                let mut source_type = source_parameter.type_reference;
                loop {
                    match program.type_reference_table.type_reference(source_type) {
                        TypeReferenceNode::Constrained { base_type, .. }
                        | TypeReferenceNode::Reference {
                            referee: base_type, ..
                        } => source_type = *base_type,
                        _ => break,
                    }
                }
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: psi_typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(source_type)
                else {
                    return None;
                };
                if *index >= *length
                    || base_type_identity(program, *element_type, &[])? != target_identity
                {
                    return None;
                }
                vec![CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).ok()?,
                )]
            }
            segments @ [psi_facts::PlaceSegment::Field { .. }, ..]
                if allow_field_path_projection
                    && caller_parameters
                        .get(source_index)?
                        .qualifications
                        .is_empty() =>
            {
                let projected_type = crate::flow::project_type_reference_from_segments(
                    program,
                    source_parameter.type_reference,
                    place.segments.as_slice(),
                )?;
                if base_type_identity(program, projected_type, &[])? != target_identity {
                    return None;
                }
                segments
                    .iter()
                    .map(|segment| match segment {
                        psi_facts::PlaceSegment::Field { symbol } => {
                            Some(CheckedUnitStructuralPathSegment::Field(
                                terminal_field_identity(program, *symbol)?,
                            ))
                        }
                        psi_facts::PlaceSegment::FixedIndex { .. }
                        | psi_facts::PlaceSegment::FixedRange { .. }
                        | psi_facts::PlaceSegment::Index { .. }
                        | psi_facts::PlaceSegment::Case { .. } => None,
                    })
                    .collect::<Option<Vec<_>>>()?
            }
            _ => return None,
        };
        if path.is_empty() && source_identity != target_identity {
            return None;
        }
        output.push(CheckedUnitStructuralArgumentPlan {
            source_parameter_index: u32::try_from(source_index).ok()?,
            path,
            type_identity: target_identity,
            access: exact_structural_argument_access(
                facts,
                caller_machine.symbol,
                caller_state.symbol,
                call,
                &place,
                structural_access_for_type_reference(program, target.type_reference)?,
            )?,
            byte_sequence_literal: None,
        });
    }
    if explicit_index != explicit_arguments.len() {
        return None;
    }
    Some(output)
}

fn exact_structural_argument_access(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &psi_checked_trees::FlowCallFact,
    place: &crate::flow::CanonicalPlace,
    target_access: CheckedStructuralAccess,
) -> Option<CheckedStructuralAccess> {
    if target_access == CheckedStructuralAccess::Owned {
        return Some(CheckedStructuralAccess::Owned);
    }
    let psi_facts::PlaceRoot::Symbol(root_symbol) = place.root else {
        return None;
    };
    let borrow_state = facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state)?;
    let matching_calls = facts
        .borrow
        .calls
        .span_or_empty(borrow_state.calls)
        .iter()
        .filter(|candidate| {
            candidate.statement_index == call.statement_index
                && candidate.call_ordinal == call.call_ordinal
                && candidate.target_symbol == call.target_symbol
        })
        .collect::<Vec<_>>();
    let [borrow_call] = matching_calls.as_slice() else {
        return None;
    };
    let kinds = facts
        .borrow
        .argument_accesses
        .span_or_empty(borrow_call.accesses)
        .iter()
        .filter(|access| {
            access.root_symbol == root_symbol
                && facts.borrow.access_segments(access) == place.segments.as_slice()
        })
        .map(|access| &access.kind)
        .collect::<Vec<_>>();
    let first = kinds.first()?;
    if kinds.iter().any(|candidate| *candidate != *first) {
        return None;
    }
    Some(match first {
        psi_checked_trees::BorrowAccessKind::Read => CheckedStructuralAccess::SharedBorrow,
        psi_checked_trees::BorrowAccessKind::Mutable => CheckedStructuralAccess::MutableBorrow,
        psi_checked_trees::BorrowAccessKind::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
    })
}

pub(super) fn call_claim_transfers(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &psi_checked_trees::FlowCallFact,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    arguments: &[CheckedUnitStructuralArgumentPlan],
    kind: PermissionEventKind,
) -> Option<Vec<CheckedUnitClaimTransferPlan>> {
    let events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source
                    == PermissionEventSource::Call {
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_symbol: call.target_symbol,
                    }
                && event.kind == kind
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    for (argument_index, argument) in arguments.iter().enumerate() {
        if argument.byte_sequence_literal.is_some() {
            if argument.source_parameter_index != u32::MAX || !argument.path.is_empty() {
                return None;
            }
            continue;
        }
        let entries = entry_claims
            .iter()
            .filter(|entry| {
                entry.parameter_index == argument.source_parameter_index
                    && (argument.path.is_empty() || entry.path == argument.path)
            })
            .collect::<Vec<_>>();
        if entries.is_empty() {
            if caller_parameters
                .get(argument.source_parameter_index as usize)?
                .multiplicity
                == Multiplicity::Linear
            {
                return None;
            }
            continue;
        }
        for entry in entries {
            let matching = events
                .iter()
                .filter(|event| event.claim_identity == entry.claim_identity)
                .collect::<Vec<_>>();
            if matching.len() != 1 || entry.claim_identity == PermissionClaimIdentity::Unknown {
                return None;
            }
            output.push(CheckedUnitClaimTransferPlan {
                claim_identity: entry.claim_identity,
                argument_index: u32::try_from(argument_index).ok()?,
            });
        }
    }
    if output.len() != events.len() {
        return None;
    }
    Some(output)
}

pub(super) fn exact_integer_at(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    expression: psi_typed_trees::expression::ExpressionHandle,
    expected_type: PrimitiveType,
) -> Option<u64> {
    let matches = facts
        .values
        .expression_values(expression)
        .filter(|(_, value)| {
            value.origin
                == psi_checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol: machine,
                    state_symbol: state,
                    statement_index,
                    role: psi_checked_trees::CheckedValueStatementRole::CallArgument,
                }
        })
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    let [value] = matches.as_slice() else {
        return None;
    };
    if value.primitive_type != Some(expected_type) {
        return None;
    }
    let range = value.integer_range.as_ref()?;
    (range.minimum == range.maximum)
        .then(|| range.minimum.to_u64())
        .flatten()
}

pub(super) fn structural_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(String, Vec<CheckedUnitStructuralParameterPlan>)> {
    let parameters = program.state_parameters(state);
    let attached_name = machine.attached_data.as_ref()?;
    let attached = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_name)?;
    let attachment_type_identity = shapes.add_attached_data(attached, binders)?;
    let attachment_multiplicity = attached.properties.multiplicity;
    let mut structural_parameters = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        if parameter.is_const {
            return None;
        }
        if parameter.is_self && is_reference(program, parameter.type_reference) {
            continue;
        }
        // Typed attached `self` intentionally carries the machine/Self symbol,
        // not the data-definition symbol. Its carrier is the independently
        // resolved attachment above.
        let type_identity = if parameter.is_self {
            attachment_type_identity.clone()
        } else {
            shapes.add_type(parameter.type_reference, binders, &[])?
        };
        let qualifications =
            parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: u32::try_from(position).ok()?,
            is_self: parameter.is_self,
            type_identity,
            multiplicity: if parameter.is_self {
                attachment_multiplicity
            } else {
                crate::checks::type_multiplicity(program, parameter.type_reference)
            },
            access: structural_access_for_type_reference(program, parameter.type_reference)?,
            qualifications,
        });
    }
    Some((attachment_type_identity, structural_parameters))
}

pub(super) fn structural_scalar_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(
    String,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    let parameters = program.state_parameters(state);
    let attached_name = machine.attached_data.as_ref()?;
    let attached = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_name)?;
    let attachment_type_identity = shapes.add_attached_data(attached, binders)?;
    let attachment_multiplicity = attached.properties.multiplicity;
    let mut structural_parameters = Vec::new();
    let mut scalar_parameters = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        let source_position = u32::try_from(position).ok()?;
        if let Some(primitive_type) = program.primitive_type_reference(parameter.type_reference) {
            if parameter.is_self || parameter.is_const || parameter.is_mutable {
                return None;
            }
            scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                source_position,
                primitive_type,
            });
            continue;
        }
        if parameter.is_const {
            return None;
        }
        if parameter.is_self && is_reference(program, parameter.type_reference) {
            continue;
        }
        let type_identity = if parameter.is_self {
            attachment_type_identity.clone()
        } else {
            shapes.add_type(parameter.type_reference, binders, &[])?
        };
        let qualifications =
            parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: source_position,
            is_self: parameter.is_self,
            type_identity,
            multiplicity: if parameter.is_self {
                attachment_multiplicity
            } else {
                crate::checks::type_multiplicity(program, parameter.type_reference)
            },
            access: structural_access_for_type_reference(program, parameter.type_reference)?,
            qualifications,
        });
    }
    Some((
        attachment_type_identity,
        structural_parameters,
        scalar_parameters,
    ))
}

pub(super) fn entry_claims(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    source_parameters: &[StateParameter],
) -> Option<Vec<CheckedUnitEntryClaimPlan>> {
    let events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    for (parameter_index, parameter) in structural_parameters.iter().enumerate() {
        if parameter.multiplicity == Multiplicity::Unrestricted {
            continue;
        }
        let source = source_parameters.get(parameter.position as usize)?;
        let expected_root = psi_facts::PlaceRoot::Symbol(parameter_root_symbol(machine, source));
        let matching = events
            .iter()
            .filter(|event| event.root == expected_root)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            if parameter.multiplicity == Multiplicity::Affine {
                continue;
            }
            return None;
        }
        let mut source_type = source.type_reference;
        loop {
            match program.type_reference_table.type_reference(source_type) {
                TypeReferenceNode::Constrained { base_type, .. }
                | TypeReferenceNode::Reference {
                    referee: base_type, ..
                } => source_type = *base_type,
                _ => break,
            }
        }
        if let TypeReferenceNode::FixedArray {
            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
            ..
        } = program.type_reference_table.type_reference(source_type)
        {
            let indices = matching
                .iter()
                .map(|event| {
                    let [psi_facts::PlaceSegment::FixedIndex { index }] =
                        facts.flow.ownership.segments.span_or_empty(event.segments)
                    else {
                        return None;
                    };
                    Some(*index)
                })
                .collect::<Option<BTreeSet<_>>>()?;
            if matching.len() != *length
                || indices != (0..*length).collect::<BTreeSet<_>>()
                || !parameter.qualifications.is_empty()
            {
                return None;
            }
        }
        for event in matching {
            if event.claim_identity == PermissionClaimIdentity::Unknown {
                return None;
            }
            let policies = facts
                .carry
                .claim_policies
                .iter()
                .filter(|policy| policy.claim_identity == event.claim_identity)
                .collect::<Vec<_>>();
            let carry = match policies.as_slice() {
                [] => CarryPolicy::STRICT,
                [policy] => policy.effective,
                _ => return None,
            };
            let path = facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .iter()
                .map(|segment| match segment {
                    psi_facts::PlaceSegment::Field { symbol } => {
                        terminal_field_identity(program, *symbol)
                            .map(CheckedUnitStructuralPathSegment::Field)
                    }
                    psi_facts::PlaceSegment::FixedIndex { index } => u64::try_from(*index)
                        .ok()
                        .map(CheckedUnitStructuralPathSegment::FixedIndex),
                    psi_facts::PlaceSegment::Case { .. }
                    | psi_facts::PlaceSegment::FixedRange { .. }
                    | psi_facts::PlaceSegment::Index { .. } => None,
                })
                .collect::<Option<Vec<_>>>()?;
            output.push(CheckedUnitEntryClaimPlan {
                claim_identity: event.claim_identity,
                parameter_index: u32::try_from(parameter_index).ok()?,
                path,
                carry,
            });
        }
    }
    output.sort_by(|left, right| {
        (left.parameter_index, &left.path).cmp(&(right.parameter_index, &right.path))
    });
    (output.len() == events.len()).then_some(output)
}
