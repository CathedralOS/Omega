//! Nominal-cleanup specialization for structural scalar returns.
//!
//! Emit the scalar caller and its cleanup dependencies into one shared namespace.
//! The caller retains its authored result; normal cleanup is an operation on
//! its original parameter places, independent of the result's representation.
use super::super::{
    BTreeMap, CheckedStructuralScalarIntegerBoundKind, CheckedStructuralScalarIntegerBoundPlan,
    IntegerSign, Proposition, ScalarTerm, integer_value, obligation_id,
};
use super::{
    BTreeSet, Block, CheckedScalarBindingValue, CheckedScalarExpressionRole,
    CheckedStructuralScalarReturnCleanupAction, CheckedStructuralScalarReturnMachinePlan,
    CheckedTrees, LoweredPsi, LoweringError, Multiplicity, PrimitiveType, ScalarType,
    StructuralPlaceDeclaration, StructuralPlaceKind, TerminalAffineCleanupAction,
    TerminalMachineResult, Terminator, ValueDeclaration, allocate_dense, block_id,
    boolean_decision_block_count, edge_id, emit_direct_expression,
    emit_inlined_boolean_value_blocks, finalize_operation_proofs, lower_boolean_value_decision,
    lower_checked_scalar_expression_at, lower_unit_parameters, terminal_scalar_type, unsupported,
    validate_boolean_parameter_types, validate_direct_parameter_types, value_id,
};
use crate::emission::boolean_control::LoweredBooleanDecisionExit;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::returns::structural_scalar_return::expressions::{
    boolean_local_reference_count, is_branch_free_structural_boolean_expression,
    is_branch_free_structural_scalar_expression, is_structural_short_circuit_boolean_decision,
    source_distribute_boolean_local, validate_boolean_decision_parameter_types,
};
use crate::returns::structural_scalar_return::shared_convergence::{
    normalize_shared_boolean_comparison_leaves, resolve_shared_boolean_member_fields,
    shared_boolean_runtime_parameters, valid_shared_boolean_runtime_inputs,
};

/// Retain ordinary hook bodies through the shared closure, then emit the real
/// scalar caller and its return actions before final proof reconstruction.
pub(super) fn lower_nominal_structural_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedStructuralScalarReturnMachinePlan,
) -> Result<LoweredPsi, LoweringError> {
    if plan.structural_parameters.is_empty()
        || plan.cleanup_actions.len() != plan.structural_parameters.len()
    {
        return unsupported("nominal scalar return exceeds its first bounded slice");
    }
    let expected_return_ordinal = u32::try_from(plan.bindings.len()).map_err(|_| {
        LoweringError::Unsupported("nominal scalar return binding count exceeds u32")
    })?;
    if plan.return_statement_ordinal != expected_return_ordinal {
        return unsupported("nominal scalar return coordinates are not a contiguous prefix");
    }
    let mut positions = BTreeSet::new();
    for parameter in &plan.structural_parameters {
        if parameter.is_self
            || parameter.multiplicity != Multiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !positions.insert(parameter.position)
        {
            return unsupported("nominal scalar return cleanup frontier drifted");
        }
    }
    if plan
        .structural_parameters
        .windows(2)
        .any(|pair| pair[0].position >= pair[1].position)
    {
        return unsupported("nominal scalar return structural parameters are not in source order");
    }
    for parameter in &plan.scalar_parameters {
        if !positions.insert(parameter.source_position) {
            return unsupported(
                "nominal scalar return parameter maps overlap or repeat a source position",
            );
        }
        terminal_scalar_type(parameter.primitive_type)?;
    }
    if plan
        .scalar_parameters
        .windows(2)
        .any(|pair| pair[0].source_position >= pair[1].source_position)
    {
        return unsupported("nominal scalar return scalar parameters are not in source order");
    }
    let parameter_count = plan
        .structural_parameters
        .len()
        .checked_add(plan.scalar_parameters.len())
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return parameter count exceeds usize",
        ))?;
    if positions.len() != parameter_count
        || positions
            .iter()
            .copied()
            .enumerate()
            .any(|(index, position)| u32::try_from(index).ok() != Some(position))
    {
        return unsupported(
            "nominal scalar return parameter maps do not partition source positions",
        );
    }
    for (parameter, cleanup) in plan
        .structural_parameters
        .iter()
        .zip(plan.cleanup_actions.iter().rev())
    {
        match cleanup {
            CheckedStructuralScalarReturnCleanupAction::DiscardRoot(cleanup_position)
                if *cleanup_position == parameter.position => {}
            CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)
                if cleanup.source_parameter_index == parameter.position
                    && cleanup.type_identity == parameter.type_identity => {}
            _ => return unsupported("nominal scalar return cleanup frontier drifted"),
        }
    }
    let (source, state) =
        crate::expression_preparation::source_custody::authored_state(checked, plan.state)?;
    if source.symbol != plan.machine
        || checked.primitive_type_reference(state.return_type) != Some(plan.result_type)
    {
        return unsupported("nominal scalar completion differs from its authored result type");
    }
    let flow = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == plan.machine && state.state_symbol == plan.state)
                .then_some(state)
        })
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return is missing its checked flow state",
        ))?;
    let reach = checked
        .facts
        .service_reaches
        .plan_for_machine(plan.machine)
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return is missing its checked service-reach plan",
        ))?;
    let mut hooks = Vec::new();
    for action in &plan.cleanup_actions {
        if let CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup) = action
            && !hooks.contains(&cleanup.cleanup_machine)
        {
            hooks.push(cleanup.cleanup_machine);
        }
    }
    let shapes = &checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .structural_types;
    let published = &checked.facts.flow.terminal_unit_effects;
    for shape in shapes {
        if published
            .structural_types
            .iter()
            .any(|other| other.identity == shape.identity && other != shape)
        {
            return unsupported(
                "nominal scalar return structural type conflicts with its cleanup closure",
            );
        }
    }
    let type_roots = plan
        .structural_parameters
        .iter()
        .map(|parameter| parameter.type_identity.clone())
        .chain(plan.attachment_type_identity.iter().cloned())
        .collect::<Vec<_>>();
    let mut services = Vec::new();
    crate::unit::attached_unit::collect_service_summary(
        &checked.facts.service_reaches.rows,
        flow.service_reach,
        &mut services,
    )?;
    crate::unit::attached_unit::collect_installation_machine_contract_services(
        checked,
        plan.machine,
        reach,
        flow.service_reach,
        &mut services,
    )?;
    let mut closure = crate::unit::attached_unit::lower_unit_closure(
        checked,
        &crate::unit::attached_unit::UnitClosureRequest {
            plans: crate::unit::attached_unit::bodies::UnitPlans::with_structural_types(
                published, shapes,
            ),
            entry: plan.machine,
            unit_roots: &hooks,
            external: Some(
                crate::unit::attached_unit::shared_closure::ExternalUnitRoots {
                    boundary_roots: &[],
                    structural_type_roots: &type_roots,
                    service_roots: &services,
                    scalar_roots: &[],
                },
            ),
            scalar_entry: false,
        },
    )?;
    let receivers = closure.cleanup_receivers();
    let structural_parameters = lower_unit_parameters(
        &plan.structural_parameters,
        &closure.type_ids,
        &closure.lowered.semantic_module.structural_types,
        &[],
        &mut closure.next_place,
    )?;
    // Terminal formals use dense structural positions. Source predicates keep
    // authored positions across primitive parameters; only their coordinate
    // changes here, never the allocated runtime place or structural type.
    let predicate_parameters = plan
        .structural_parameters
        .iter()
        .zip(&structural_parameters)
        .map(
            |(source, terminal)| terminal_psi::StructuralParameterDeclaration {
                position: source.position,
                ..terminal.clone()
            },
        )
        .collect::<Vec<_>>();
    let structural_parameter_indexes = plan
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.position, index))
        .collect::<BTreeMap<_, _>>();
    let cleanup_actions = plan
        .cleanup_actions
        .iter()
        .map(|action| {
            let position = match action {
                CheckedStructuralScalarReturnCleanupAction::DiscardRoot(position) => *position,
                CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup) => {
                    cleanup.source_parameter_index
                }
            };
            let parameter = structural_parameter_indexes
                .get(&position)
                .and_then(|index| structural_parameters.get(*index))
                .ok_or(LoweringError::Unsupported(
                    "nominal scalar return cleanup terminal root is absent",
                ))?;
            match action {
                CheckedStructuralScalarReturnCleanupAction::DiscardRoot(_) => {
                    Ok(TerminalAffineCleanupAction::DiscardRoot(parameter.place))
                }
                CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup) => {
                    Ok(TerminalAffineCleanupAction::InvokeNominal(
                        crate::unit::unit_cleanup::lower_nominal_cleanup_action(
                            checked,
                            cleanup,
                            parameter,
                            &closure.type_ids,
                            &closure.machine_ids,
                            &receivers,
                            &mut closure.next_call_obligation,
                        )?,
                    ))
                }
            }
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if plan
        .caller_requirements
        .iter()
        .enumerate()
        .any(|(index, requirement)| {
            plan.caller_requirements[..index].iter().any(|earlier| {
                earlier.source_parameter_index == requirement.source_parameter_index
                    && earlier.field_identity == requirement.field_identity
                    && earlier.expected == requirement.expected
            })
        })
    {
        return unsupported("contextual nominal scalar full caller requirements are duplicated");
    }
    let mut full_caller_requires = plan
        .caller_requirements
        .iter()
        .map(|requirement| {
            let parameter = structural_parameter_indexes
                .get(&requirement.source_parameter_index)
                .and_then(|index| structural_parameters.get(*index))
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal scalar full caller root is absent",
                ))?;
            crate::unit::unit_cleanup::contexts::field_clause(
                &closure.lowered.semantic_module.structural_types,
                parameter,
                &requirement.field_identity,
                requirement.expected,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    crate::scalar_graph::scalar_contracts::canonicalize_requires(&mut full_caller_requires)?;
    let caller_machine =
        crate::terminal_identities::lookup_machine_id(&closure.machine_ids, plan.machine)?;
    let declared_service_reach = crate::unit::attached_unit::lower_declared_service_reach(
        checked,
        plan.machine,
        &closure.service_ids,
    )?;
    let published_service_ceiling =
        crate::unit::attached_unit::lower_installation_machine_service_ceiling(
            checked,
            plan.machine,
            reach,
            flow.service_reach,
            &closure.service_ids,
        )?;
    let attachment = plan
        .attachment_type_identity
        .as_deref()
        .map(|identity| crate::terminal_identities::lookup_type_id(&closure.type_ids, identity))
        .transpose()?;
    let mut next_value = closure.next_value;
    let operation_identity_base = closure
        .next_operation
        .max(closure.next_call_obligation)
        .checked_sub(1)
        .ok_or(LoweringError::Unsupported(
            "nominal scalar closure contains a null identity counter",
        ))?;
    let result_type = terminal_scalar_type(plan.result_type)?;
    let entry_block = block_id(allocate_dense(&mut closure.next_block)?);
    let edge = edge_id(allocate_dense(&mut closure.next_edge)?);
    let first_unused_edge = closure.next_edge;
    let first_unused_block = closure.next_block;
    let mut lowered = closure.lowered;
    let scalar_parameters = plan
        .scalar_parameters
        .iter()
        .map(|parameter| {
            Ok(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(allocate_dense(&mut next_value)?),
                scalar_type: terminal_scalar_type(parameter.primitive_type)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let scalar_parameter_count = scalar_parameters.len();
    let mut scalar_requirements = plan
        .scalar_requirements
        .iter()
        .map(|requirement| {
            let parameter = scalar_parameters
                .get(
                    usize::try_from(requirement.parameter_position).map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar requirement parameter position exceeds usize",
                        )
                    })?,
                )
                .ok_or(LoweringError::Unsupported(
                    "nominal scalar requirement parameter is absent",
                ))?;
            let scalar_type = terminal_scalar_type(requirement.primitive_type)?;
            if parameter.scalar_type != scalar_type {
                return unsupported("nominal scalar requirement parameter type drifted");
            }
            let ScalarType::Integer(integer_type) = scalar_type else {
                return unsupported("nominal scalar requirement is not an integer bound");
            };
            let bound = match &requirement.bound {
                CheckedStructuralScalarIntegerBoundPlan::Literal(bound) => {
                    ScalarTerm::integer(integer_type, integer_value(bound, scalar_type)?).map_err(
                        |_| LoweringError::Unsupported("nominal scalar bound is invalid"),
                    )?
                }
                CheckedStructuralScalarIntegerBoundPlan::Parameter(position) => {
                    let bound_parameter = scalar_parameters
                        .get(usize::try_from(*position).map_err(|_| {
                            LoweringError::Unsupported(
                                "nominal scalar bound parameter position exceeds usize",
                            )
                        })?)
                        .ok_or(LoweringError::Unsupported(
                            "nominal scalar bound parameter is absent",
                        ))?;
                    if bound_parameter.scalar_type != scalar_type {
                        return unsupported("nominal scalar bound parameter type drifted");
                    }
                    ScalarTerm::value(bound_parameter.id, scalar_type)
                }
                CheckedStructuralScalarIntegerBoundPlan::MaximumMinusParameter(position) => {
                    let bound_parameter = scalar_parameters
                        .get(usize::try_from(*position).map_err(|_| {
                            LoweringError::Unsupported(
                                "nominal scalar computed-bound parameter position exceeds usize",
                            )
                        })?)
                        .ok_or(LoweringError::Unsupported(
                            "nominal scalar computed-bound parameter is absent",
                        ))?;
                    if bound_parameter.scalar_type != scalar_type {
                        return unsupported("nominal scalar computed-bound parameter type drifted");
                    }
                    let maximum = ScalarTerm::integer(integer_type, integer_type.maximum_value())
                        .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound maximum is invalid",
                        )
                    })?;
                    ScalarTerm::exact_integer_subtract(
                        integer_type,
                        maximum,
                        ScalarTerm::value(bound_parameter.id, scalar_type),
                    )
                    .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound subtraction is invalid",
                        )
                    })?
                }
                CheckedStructuralScalarIntegerBoundPlan::SignedMinimumMinusParameter(position) => {
                    if integer_type.sign() != IntegerSign::Signed {
                        return unsupported(
                            "nominal scalar minimum-minus bound requires a signed carrier",
                        );
                    }
                    let bound_parameter = scalar_parameters
                        .get(usize::try_from(*position).map_err(|_| {
                            LoweringError::Unsupported(
                                "nominal scalar computed-bound parameter position exceeds usize",
                            )
                        })?)
                        .ok_or(LoweringError::Unsupported(
                            "nominal scalar computed-bound parameter is absent",
                        ))?;
                    if bound_parameter.scalar_type != scalar_type {
                        return unsupported("nominal scalar computed-bound parameter type drifted");
                    }
                    let minimum = ScalarTerm::integer(integer_type, integer_type.minimum_value())
                        .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound minimum is invalid",
                        )
                    })?;
                    ScalarTerm::exact_integer_subtract(
                        integer_type,
                        minimum,
                        ScalarTerm::value(bound_parameter.id, scalar_type),
                    )
                    .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound subtraction is invalid",
                        )
                    })?
                }
                CheckedStructuralScalarIntegerBoundPlan::SignedMinimumPlusParameter(position) => {
                    if integer_type.sign() != IntegerSign::Signed {
                        return unsupported(
                            "nominal scalar minimum-plus bound requires a signed carrier",
                        );
                    }
                    let bound_parameter = scalar_parameters
                        .get(usize::try_from(*position).map_err(|_| {
                            LoweringError::Unsupported(
                                "nominal scalar computed-bound parameter position exceeds usize",
                            )
                        })?)
                        .ok_or(LoweringError::Unsupported(
                            "nominal scalar computed-bound parameter is absent",
                        ))?;
                    if bound_parameter.scalar_type != scalar_type {
                        return unsupported("nominal scalar computed-bound parameter type drifted");
                    }
                    let minimum = ScalarTerm::integer(integer_type, integer_type.minimum_value())
                        .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound minimum is invalid",
                        )
                    })?;
                    ScalarTerm::exact_integer_add(
                        integer_type,
                        minimum,
                        ScalarTerm::value(bound_parameter.id, scalar_type),
                    )
                    .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound addition is invalid",
                        )
                    })?
                }
                CheckedStructuralScalarIntegerBoundPlan::SignedMaximumPlusParameter(position) => {
                    if integer_type.sign() != IntegerSign::Signed {
                        return unsupported(
                            "nominal scalar maximum-plus bound requires a signed carrier",
                        );
                    }
                    let bound_parameter = scalar_parameters
                        .get(usize::try_from(*position).map_err(|_| {
                            LoweringError::Unsupported(
                                "nominal scalar computed-bound parameter position exceeds usize",
                            )
                        })?)
                        .ok_or(LoweringError::Unsupported(
                            "nominal scalar computed-bound parameter is absent",
                        ))?;
                    if bound_parameter.scalar_type != scalar_type {
                        return unsupported("nominal scalar computed-bound parameter type drifted");
                    }
                    let maximum = ScalarTerm::integer(integer_type, integer_type.maximum_value())
                        .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound maximum is invalid",
                        )
                    })?;
                    ScalarTerm::exact_integer_add(
                        integer_type,
                        maximum,
                        ScalarTerm::value(bound_parameter.id, scalar_type),
                    )
                    .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound addition is invalid",
                        )
                    })?
                }
                CheckedStructuralScalarIntegerBoundPlan::MaximumDivideParameter(position) => {
                    let bound_parameter = scalar_parameters
                        .get(usize::try_from(*position).map_err(|_| {
                            LoweringError::Unsupported(
                                "nominal scalar computed-bound parameter position exceeds usize",
                            )
                        })?)
                        .ok_or(LoweringError::Unsupported(
                            "nominal scalar computed-bound parameter is absent",
                        ))?;
                    if bound_parameter.scalar_type != scalar_type {
                        return unsupported("nominal scalar computed-bound parameter type drifted");
                    }
                    let maximum = ScalarTerm::integer(integer_type, integer_type.maximum_value())
                        .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound maximum is invalid",
                        )
                    })?;
                    ScalarTerm::exact_integer_divide(
                        integer_type,
                        maximum,
                        ScalarTerm::value(bound_parameter.id, scalar_type),
                    )
                    .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound division is invalid",
                        )
                    })?
                }
                CheckedStructuralScalarIntegerBoundPlan::SignedMinimumDivideParameter(position) => {
                    if integer_type.sign() != IntegerSign::Signed {
                        return unsupported(
                            "nominal scalar minimum-divide bound requires a signed carrier",
                        );
                    }
                    let bound_parameter = scalar_parameters
                        .get(usize::try_from(*position).map_err(|_| {
                            LoweringError::Unsupported(
                                "nominal scalar computed-bound parameter position exceeds usize",
                            )
                        })?)
                        .ok_or(LoweringError::Unsupported(
                            "nominal scalar computed-bound parameter is absent",
                        ))?;
                    if bound_parameter.scalar_type != scalar_type {
                        return unsupported("nominal scalar computed-bound parameter type drifted");
                    }
                    let minimum = ScalarTerm::integer(integer_type, integer_type.minimum_value())
                        .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound minimum is invalid",
                        )
                    })?;
                    ScalarTerm::exact_integer_divide(
                        integer_type,
                        minimum,
                        ScalarTerm::value(bound_parameter.id, scalar_type),
                    )
                    .map_err(|_| {
                        LoweringError::Unsupported(
                            "nominal scalar computed-bound division is invalid",
                        )
                    })?
                }
            };
            let parameter = ScalarTerm::value(parameter.id, scalar_type);
            Ok(match requirement.kind {
                CheckedStructuralScalarIntegerBoundKind::Lower => {
                    Proposition::LessOrEqual(bound, parameter)
                }
                CheckedStructuralScalarIntegerBoundKind::Upper => {
                    Proposition::LessOrEqual(parameter, bound)
                }
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    scalar_requirements.sort_by_cached_key(|requirement| {
        terminal_codec::canonical_proposition_order_key(requirement)
            .expect("validated scalar requirements have canonical encodings")
    });
    scalar_requirements.dedup();
    let mut operations = OperationBuffer::new(operation_identity_base);
    let mut scalar_values = Vec::with_capacity(
        scalar_parameter_count
            .checked_add(plan.bindings.len())
            .ok_or(LoweringError::Unsupported(
                "nominal scalar value namespace exceeds usize",
            ))?,
    );
    scalar_values.extend_from_slice(&scalar_parameters);
    let authored_return_expression = lower_checked_scalar_expression_at(
        checked,
        plan.state,
        plan.return_statement_ordinal,
        CheckedScalarExpressionRole::Return,
    )?;
    let candidate_source_distributed_short_circuit_bindings = plan
        .bindings
        .len()
        .checked_sub(1)
        .and_then(|binding_index| {
            let binding = &plan.bindings[binding_index];
            let statement_ordinal = u32::try_from(binding_index).ok()?;
            if binding.statement_ordinal != statement_ordinal
                || binding.destination != checked_trees::CheckedScalarBindingDestination::Immutable
                || binding.value != CheckedScalarBindingValue::Expression
                || binding.primitive_type != PrimitiveType::Bool
            {
                return None;
            }
            if !(0..binding_index).all(|prior_index| {
                let Ok(prior_ordinal) = u32::try_from(prior_index) else {
                    return false;
                };
                lower_checked_scalar_expression_at(
                    checked,
                    plan.state,
                    prior_ordinal,
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: prior_ordinal,
                    },
                )
                .is_ok_and(|expression| {
                    is_branch_free_structural_scalar_expression(
                        &expression,
                        scalar_parameter_count,
                        prior_index,
                    )
                })
            }) {
                return None;
            }
            let LoweredDirectExpression::Boolean {
                expression: return_expression,
            } = &authored_return_expression
            else {
                return None;
            };
            let local_position = scalar_parameter_count + binding_index;
            if !is_branch_free_structural_boolean_expression(
                return_expression,
                scalar_parameter_count,
                binding_index + 1,
            ) || boolean_local_reference_count(return_expression, local_position) == 0
            {
                return None;
            }
            let expression = lower_checked_scalar_expression_at(
                checked,
                plan.state,
                statement_ordinal,
                CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: statement_ordinal,
                },
            )
            .ok()?;
            let LoweredDirectExpression::Boolean { expression } = expression else {
                return None;
            };
            if !is_structural_short_circuit_boolean_decision(
                &expression,
                scalar_parameter_count,
                binding_index,
            ) {
                return None;
            }
            let decision = source_distribute_boolean_local(
                lower_boolean_value_decision(&expression),
                return_expression,
                local_position,
            );
            Some((binding_index, decision))
        })
        .or_else(|| {
            let final_binding_index = plan.bindings.len().checked_sub(1)?;
            let LoweredDirectExpression::Boolean {
                expression: return_expression,
            } = &authored_return_expression
            else {
                return None;
            };
            let final_binding_position = scalar_parameter_count + final_binding_index;
            if !matches!(return_expression.as_ref(),
                    LoweredBooleanReturnExpression::Local { position }
                        if *position == final_binding_position)
            {
                return None;
            }
            (0..final_binding_index).find_map(|short_circuit_index| {
                if !plan.bindings[short_circuit_index..].iter().enumerate().all(
                    |(offset, binding)| {
                        let index = short_circuit_index + offset;
                        u32::try_from(index)
                            .is_ok_and(|ordinal| binding.statement_ordinal == ordinal)
                            && binding.destination
                                == checked_trees::CheckedScalarBindingDestination::Immutable
                            && binding.value == CheckedScalarBindingValue::Expression
                            && binding.primitive_type == PrimitiveType::Bool
                    },
                ) {
                    return None;
                }
                let short_circuit_ordinal = u32::try_from(short_circuit_index).ok()?;
                let short_circuit_expression = lower_checked_scalar_expression_at(
                    checked,
                    plan.state,
                    short_circuit_ordinal,
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: short_circuit_ordinal,
                    },
                )
                .ok()?;
                let LoweredDirectExpression::Boolean {
                    expression: short_circuit_expression,
                } = short_circuit_expression
                else {
                    return None;
                };
                if !is_structural_short_circuit_boolean_decision(
                    &short_circuit_expression,
                    scalar_parameter_count,
                    short_circuit_index,
                ) {
                    return None;
                }
                let mut decision = lower_boolean_value_decision(&short_circuit_expression);
                for continuation_index in short_circuit_index + 1..=final_binding_index {
                    let continuation_ordinal = u32::try_from(continuation_index).ok()?;
                    let continuation_expression = lower_checked_scalar_expression_at(
                        checked,
                        plan.state,
                        continuation_ordinal,
                        CheckedScalarExpressionRole::LocalInitializer {
                            binding_ordinal: continuation_ordinal,
                        },
                    )
                    .ok()?;
                    let LoweredDirectExpression::Boolean {
                        expression: continuation_expression,
                    } = continuation_expression
                    else {
                        return None;
                    };
                    let prior_position = scalar_parameter_count + continuation_index - 1;
                    if !(is_branch_free_structural_boolean_expression(
                        &continuation_expression,
                        scalar_parameter_count,
                        continuation_index,
                    ) || is_structural_short_circuit_boolean_decision(
                        &continuation_expression,
                        scalar_parameter_count,
                        continuation_index,
                    )) || boolean_local_reference_count(&continuation_expression, prior_position)
                        == 0
                    {
                        return None;
                    }
                    decision = source_distribute_boolean_local(
                        decision,
                        &continuation_expression,
                        prior_position,
                    );
                }
                Some((short_circuit_index, decision))
            })
        });
    let source_distributed_short_circuit_bindings = plan
        .shared_boolean_convergence
        .is_none()
        .then_some(candidate_source_distributed_short_circuit_bindings)
        .flatten();
    for (binding_index, binding) in plan.bindings.iter().enumerate() {
        let statement_ordinal = u32::try_from(binding_index).map_err(|_| {
            LoweringError::Unsupported("nominal scalar return binding index exceeds u32")
        })?;
        if binding.statement_ordinal != statement_ordinal
            || binding.destination != checked_trees::CheckedScalarBindingDestination::Immutable
            || binding.value != CheckedScalarBindingValue::Expression
        {
            return unsupported(
                "nominal scalar return bindings are not a direct expression prefix",
            );
        }
        if source_distributed_short_circuit_bindings
            .as_ref()
            .is_some_and(|(first_distributed, _)| binding_index >= *first_distributed)
            || plan.shared_boolean_convergence.is_some_and(|convergence| {
                usize::try_from(convergence.binding_ordinal).ok() == Some(binding_index)
            })
        {
            continue;
        }
        let scalar_type = terminal_scalar_type(binding.primitive_type)?;
        let expression = lower_checked_scalar_expression_at(
            checked,
            plan.state,
            statement_ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: statement_ordinal,
            },
        )?;
        if !is_branch_free_structural_scalar_expression(
            &expression,
            scalar_parameter_count,
            binding_index,
        ) {
            return unsupported("nominal scalar binding is not one branch-free local expression");
        }
        if expression.scalar_type() != scalar_type {
            return unsupported(
                "nominal scalar binding value does not match its checked local type",
            );
        }
        validate_direct_parameter_types(
            &expression,
            &scalar_values
                .iter()
                .map(|value: &ValueDeclaration| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let id = emit_direct_expression(
            &expression,
            &scalar_values,
            &mut next_value,
            &mut operations,
        );
        scalar_values.push(ValueDeclaration {
            qualifications: Default::default(),
            id,
            scalar_type,
        });
    }
    let expression = authored_return_expression;
    let expression_available_locals = source_distributed_short_circuit_bindings
        .as_ref()
        .map_or(plan.bindings.len(), |(first_distributed, _)| {
            *first_distributed
        });
    let authored_short_circuit_return = matches!(
        &expression,
        LoweredDirectExpression::Boolean { expression }
            if is_structural_short_circuit_boolean_decision(
                expression,
                scalar_parameter_count,
                expression_available_locals,
            )
    );
    let nominal_short_circuit_return =
        source_distributed_short_circuit_bindings.is_some() || authored_short_circuit_return;
    if !is_branch_free_structural_scalar_expression(
        &expression,
        scalar_parameter_count,
        plan.bindings.len(),
    ) && !nominal_short_circuit_return
    {
        return unsupported(
            "nominal scalar return expression is not branch-free or one top-level Boolean decision",
        );
    }
    if expression.scalar_type() != result_type {
        return unsupported("nominal scalar return value does not match its checked result type");
    }
    let parameter_types = scalar_values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    let prepared_convergence = if let Some(convergence) = plan.shared_boolean_convergence {
        let binding_index = usize::try_from(convergence.binding_ordinal).map_err(|_| {
            LoweringError::Unsupported("shared Boolean convergence binding exceeds usize")
        })?;
        let decision = lower_checked_scalar_expression_at(
            checked,
            plan.state,
            convergence.binding_ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: convergence.binding_ordinal,
            },
        )?;
        let LoweredDirectExpression::Boolean {
            expression: decision,
        } = decision
        else {
            return unsupported("shared Boolean convergence decision is not Boolean");
        };
        let decision = resolve_shared_boolean_member_fields(
            *decision,
            &predicate_parameters,
            &lowered.semantic_module.structural_types,
        )?;
        let decision = normalize_shared_boolean_comparison_leaves(&decision).ok_or(
            LoweringError::Unsupported(
                "shared Boolean convergence contains a non-normalizable comparison leaf",
            ),
        )?;
        if binding_index >= plan.bindings.len()
            || shared_boolean_runtime_parameters(&decision)
                .is_none_or(|inputs| !valid_shared_boolean_runtime_inputs(&inputs))
        {
            return unsupported("shared Boolean convergence has no normalized runtime input");
        }
        validate_boolean_parameter_types(&decision, &parameter_types)?;
        Some(decision)
    } else if let Some((_, decision)) = &source_distributed_short_circuit_bindings {
        validate_boolean_decision_parameter_types(decision, &parameter_types)?;
        None
    } else {
        validate_direct_parameter_types(&expression, &parameter_types)?;
        None
    };
    // Reuse the exact admitted decision: replay and input normalization above
    // completed before emission, and none of their inputs have changed.
    let blocks = if let Some(decision) = prepared_convergence {
        let decision = lower_boolean_value_decision(&decision);
        let decision_block_count = boolean_decision_block_count(&decision);
        let continuation_block = block_id(
            first_unused_block
                .checked_add(
                    u64::try_from(decision_block_count.saturating_sub(1)).map_err(|_| {
                        LoweringError::Unsupported(
                            "shared Boolean convergence block count exceeds u64",
                        )
                    })?,
                )
                .ok_or(LoweringError::Unsupported(
                    "shared Boolean convergence block identity overflows",
                ))?,
        );
        let entry_operation_count = operations.operations.len();
        let mut next_edge = first_unused_edge;
        let (mut root, mut children) = emit_inlined_boolean_value_blocks(
            &decision,
            &scalar_values,
            Vec::new(),
            LoweredBooleanDecisionExit::Jump {
                target: continuation_block,
            },
            entry_block,
            block_id(first_unused_block),
            &mut next_value,
            &mut next_edge,
            &mut operations,
        );
        let mut entry_operations = operations.operations[..entry_operation_count].to_vec();
        entry_operations.extend(root.operations);
        root.operations = entry_operations;
        let convergence_value = ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(allocate_dense(&mut next_value)?),
            scalar_type: ScalarType::Boolean,
        };
        scalar_values.push(convergence_value);
        validate_direct_parameter_types(
            &expression,
            &scalar_values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let continuation_operation_start = operations.operations.len();
        let value = emit_direct_expression(
            &expression,
            &scalar_values,
            &mut next_value,
            &mut operations,
        );
        let return_edge = edge_id(next_edge);
        let mut blocks = Vec::with_capacity(2_usize.checked_add(children.len()).ok_or(
            LoweringError::Unsupported("shared Boolean convergence block count exceeds usize"),
        )?);
        blocks.push(root);
        blocks.append(&mut children);
        blocks.push(Block {
            structural_parameters: Vec::new(),
            id: continuation_block,
            parameters: vec![convergence_value],
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            operations: operations.operations[continuation_operation_start..].to_vec(),
            terminator: Terminator::Return {
                edge: return_edge,
                value,
                cleanup_actions: cleanup_actions.clone(),
            },
        });
        attach_edge_local_cleanup_obligations(
            &mut blocks,
            &cleanup_actions,
            operations.next_identity,
        )?;
        blocks
    } else if nominal_short_circuit_return {
        let entry_operation_count = operations.operations.len();
        let decision = if let Some((_, decision)) = source_distributed_short_circuit_bindings {
            decision
        } else {
            let LoweredDirectExpression::Boolean { expression } = &expression else {
                unreachable!("the bounded nominal decision is Boolean")
            };
            lower_boolean_value_decision(expression)
        };
        let mut next_edge = first_unused_edge;
        let (mut root, mut children) = emit_inlined_boolean_value_blocks(
            &decision,
            &scalar_values,
            Vec::new(),
            LoweredBooleanDecisionExit::Return,
            entry_block,
            block_id(first_unused_block),
            &mut next_value,
            &mut next_edge,
            &mut operations,
        );
        let mut entry_operations = operations.operations[..entry_operation_count].to_vec();
        entry_operations.extend(root.operations);
        root.operations = entry_operations;
        let mut blocks = Vec::with_capacity(1_usize.checked_add(children.len()).ok_or(
            LoweringError::Unsupported("nominal scalar return block count exceeds usize"),
        )?);
        blocks.push(root);
        blocks.append(&mut children);
        attach_edge_local_cleanup_obligations(
            &mut blocks,
            &cleanup_actions,
            operations.next_identity,
        )?;
        blocks
    } else {
        let value = emit_direct_expression(
            &expression,
            &scalar_values,
            &mut next_value,
            &mut operations,
        );
        vec![Block {
            structural_parameters: Vec::new(),
            id: entry_block,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            operations: operations.operations,
            terminator: Terminator::Return {
                edge,
                value,
                cleanup_actions,
            },
        }]
    };
    full_caller_requires.extend(scalar_requirements);
    crate::scalar_graph::scalar_contracts::canonicalize_requires(&mut full_caller_requires)?;
    let erased_scalar_formals = crate::scalar_graph::scalar_contracts::erased_formal_declarations(
        &plan.erased_scalar_parameters,
        &mut next_value,
    )?;
    let view_roster = predicate_parameters
        .iter()
        .map(|parameter| (parameter.position, parameter.clone()))
        .collect::<Vec<_>>();
    let element_views = crate::expression_preparation::bindings::element_views(
        &view_roster,
        &lowered.semantic_module.structural_types,
    );
    let views = crate::scalar_graph::scalar_contracts::ContractViewNamespace {
        parameters: &view_roster,
        structural_types: &lowered.semantic_module.structural_types,
        element_views: &element_views,
    };
    crate::unit::runtime_requirements::validate_scalar_source(checked, source, state)?;
    let authored_requires = crate::unit::attached_unit::lower_entry_requirements(
        checked,
        plan.machine,
        &scalar_parameters,
        &erased_scalar_formals,
        &predicate_parameters,
        &lowered.semantic_module.structural_types,
    )?
    .requires;
    if authored_requires != full_caller_requires {
        return unsupported(
            "nominal scalar cleanup requirements differ from the authored callable contract",
        );
    }
    let result = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(allocate_dense(&mut next_value)?),
        scalar_type: result_type,
    };
    let mut next_guarantee = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .chain(blocks.iter())
        .flat_map(|block| &block.operations)
        .map(|operation| operation.id.get())
        .max()
        .unwrap_or(0)
        .max(operation_identity_base)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "nominal scalar guarantee identity space is exhausted",
        ))?;
    // Return-leaf cleanup obligations may use identities beyond operation IDs.
    for block in &blocks {
        if let Terminator::Return {
            cleanup_actions, ..
        } = &block.terminator
        {
            for action in cleanup_actions {
                if let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action {
                    for obligation in &cleanup.requirement_obligations {
                        next_guarantee =
                            next_guarantee.max(obligation.get().checked_add(1).ok_or(
                                LoweringError::Unsupported(
                                    "nominal scalar cleanup identity space is exhausted",
                                ),
                            )?);
                    }
                }
            }
        }
    }
    let ensures = crate::unit::attached_unit::lower_normal_guarantees(
        checked,
        plan.state,
        &scalar_parameters,
        Some(result),
        &erased_scalar_formals,
        &views,
        &mut next_guarantee,
    )?;
    lowered.semantic_module.machines.insert(
        0,
        super::TerminalMachine {
            closed_reach_application: None,
            declared_service_reach,
            id: caller_machine,
            attachment,
            parameters: scalar_parameters,
            structural_parameters: structural_parameters.clone(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(result),
            structural_places: structural_parameters
                .iter()
                .map(|parameter| StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: parameter.is_self,
                    },
                })
                .collect(),
            entry_claims: Vec::new(),
            published_service_ceiling,
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: entry_block,
            blocks,
            contract: super::MachineContract {
                id: crate::terminal_identities::contract_id(caller_machine.get()),
                crash_routes: Vec::new(),
                erased_scalar_formals,
                erased_proof_formals:
                    crate::scalar_graph::scalar_contracts::erased_proof_formal_declarations(
                        &plan.erased_proof_parameters,
                    ),
                requires: full_caller_requires,
                ensures,
                outcome_specific_ensures: Vec::new(),
            },
        },
    );
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

fn attach_edge_local_cleanup_obligations(
    blocks: &mut [Block],
    cleanup_actions: &[TerminalAffineCleanupAction],
    next_operation_identity: u64,
) -> Result<(), LoweringError> {
    let mut first_return = true;
    // Cleanup obligations are edge-local semantic events. Keep the first
    // leaf's identities and allocate fresh ones beyond all operation goals for
    // later leaves. Finalization proves each site's actual live observations;
    // cloning another leaf's certificate would also copy its premise indexes.
    let mut next_cleanup_obligation =
        next_operation_identity
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "nominal scalar Boolean cleanup obligation identity space is exhausted",
            ))?;
    for block in blocks {
        let Terminator::Return {
            cleanup_actions: leaf_cleanup,
            ..
        } = &mut block.terminator
        else {
            continue;
        };
        *leaf_cleanup = cleanup_actions.to_vec();
        if first_return {
            first_return = false;
            continue;
        }
        for action in leaf_cleanup {
            let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
                continue;
            };
            for obligation in &mut cleanup.requirement_obligations {
                let identity = next_cleanup_obligation;
                next_cleanup_obligation =
                    next_cleanup_obligation
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "nominal scalar Boolean cleanup obligation identity space is exhausted",
                        ))?;
                *obligation = obligation_id(identity);
            }
        }
    }
    Ok(())
}
