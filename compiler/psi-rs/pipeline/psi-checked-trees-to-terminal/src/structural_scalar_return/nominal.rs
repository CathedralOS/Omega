//! Nominal-cleanup specialization for structural scalar returns.

use super::*;

/// Reuse the already ratified bounded nominal-Unit closure construction, then
/// replace only its synthetic entry body with the checked scalar computation.
/// This keeps cleanup target/helper retention, dense identities, and ownership
/// proof validation in one implementation while making result materialization
/// precede the cleanup action on every scalar return leaf.
pub(super) fn lower_nominal_structural_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedStructuralScalarReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
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
    let mut nominal_parameters = Vec::new();
    let mut nominal_source_positions = Vec::new();
    for parameter in &plan.structural_parameters {
        if plan.cleanup_actions.iter().any(|action| {
            matches!(
                action,
                CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)
                    if cleanup.source_parameter_index == parameter.position
            )
        }) {
            let mut normalized = parameter.clone();
            normalized.position = u32::try_from(nominal_parameters.len()).map_err(|_| {
                LoweringError::Unsupported("nominal scalar return root count exceeds u32")
            })?;
            nominal_source_positions.push(parameter.position);
            nominal_parameters.push(normalized);
        }
    }
    let mut nominal_cleanups = plan
        .cleanup_actions
        .iter()
        .filter_map(|action| match action {
            CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup) => {
                Some(cleanup.clone())
            }
            CheckedStructuralScalarReturnCleanupAction::DiscardRoot(_) => None,
        })
        .collect::<Vec<_>>();
    for cleanup in &mut nominal_cleanups {
        cleanup.source_parameter_index = u32::try_from(
            nominal_source_positions
                .iter()
                .position(|position| *position == cleanup.source_parameter_index)
                .ok_or(LoweringError::Unsupported(
                    "nominal scalar return cleanup root is absent",
                ))?,
        )
        .map_err(|_| LoweringError::Unsupported("nominal scalar return root count exceeds u32"))?;
    }
    let nominal_caller_requirements = plan
        .caller_requirements
        .iter()
        .filter_map(|requirement| {
            let compact_position = nominal_source_positions
                .iter()
                .position(|position| *position == requirement.source_parameter_index)?;
            let mut normalized = requirement.clone();
            Some(
                u32::try_from(compact_position)
                    .map(|position| {
                        normalized.source_parameter_index = position;
                        normalized
                    })
                    .map_err(|_| {
                        LoweringError::Unsupported("nominal scalar return root count exceeds u32")
                    }),
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(plan.machine)
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return is missing its checked contract identity",
        ))?;
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
    let synthetic = CheckedUnitEffectMachinePlan {
        machine: plan.machine,
        state: plan.state,
        attachment_type_identity: plan.attachment_type_identity.clone(),
        structural_parameters: nominal_parameters,
        trivial_affine_locals: Vec::new(),
        entry_claims: Vec::new(),
        body_qualifications: Vec::new(),
        contract_fingerprint: contract.fingerprint,
        contract_service_reach: checked
            .facts
            .service_reaches
            .plan_for_machine(plan.machine)
            .ok_or(LoweringError::Unsupported(
                "nominal scalar return is missing its checked service-reach plan",
            ))?,
        service_reach: flow.service_reach,
        operations: vec![CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 0,
            trivial_affine_local_discard_ordinals: Vec::new(),
            trivial_affine_discards: Vec::new(),
        }],
    };
    let nominal = CheckedNominalAffineUnitCleanupMachinePlan {
        machine: synthetic,
        caller_requirements: nominal_caller_requirements,
        cleanups: nominal_cleanups,
    };
    let mut staged = checked.clone();
    for shape in &checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .structural_types
    {
        match staged
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .structural_types
            .iter()
            .find(|candidate| candidate.identity == shape.identity)
        {
            Some(existing) if existing != shape => {
                return unsupported(
                    "nominal scalar return structural type conflicts with its cleanup closure",
                );
            }
            Some(_) => {}
            None => staged
                .facts
                .flow
                .terminal_nominal_affine_unit_cleanups
                .structural_types
                .push(shape.clone()),
        }
    }
    let mut lowered = lower_nominal_affine_unit_cleanup_machine(&staged, &nominal)?;
    let result_type = terminal_scalar_type(plan.result_type)?;
    retain_additional_structural_types(
        &mut lowered.semantic_module,
        &checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .structural_types,
        plan.structural_parameters
            .iter()
            .map(|parameter| parameter.type_identity.clone()),
    )?;
    let operation_identity_base = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .map(|operation| operation.id.get())
        .max()
        .unwrap_or(0)
        .max(
            lowered
                .proof_bundle
                .evidence
                .iter()
                .map(|evidence| evidence.obligation.get())
                .max()
                .unwrap_or(0),
        );
    let type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let mut next_place = 1_u64;
    let structural_parameters =
        lower_unit_parameters(&plan.structural_parameters, &type_ids, &[], &mut next_place)?;
    let structural_parameter_indexes = plan
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.position, index))
        .collect::<BTreeMap<_, _>>();
    let entry_index = lowered
        .semantic_module
        .machines
        .iter()
        .position(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return entry machine was not retained",
        ))?;
    let compact_parameters = lowered.semantic_module.machines[entry_index]
        .structural_parameters
        .clone();
    let [compact_block] = lowered.semantic_module.machines[entry_index]
        .blocks
        .as_slice()
    else {
        return unsupported("nominal scalar return entry control is not a single block");
    };
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } = &compact_block.terminator else {
        return unsupported("nominal scalar return synthetic cleanup edge drifted");
    };
    let edge = *edge;
    let mut terminal_nominals = cleanups.clone();
    if cleanups.len()
        != plan
            .cleanup_actions
            .iter()
            .filter(|action| {
                matches!(
                    action,
                    CheckedStructuralScalarReturnCleanupAction::InvokeNominal(_)
                )
            })
            .count()
    {
        return unsupported("nominal scalar return synthetic cleanup count drifted");
    }

    let mut caller_place_rebase = BTreeMap::new();
    for compact in &compact_parameters {
        let source_position = nominal_source_positions
            .get(usize::try_from(compact.position).map_err(|_| {
                LoweringError::Unsupported(
                    "nominal scalar return compact root position exceeds usize",
                )
            })?)
            .copied()
            .ok_or(LoweringError::Unsupported(
                "nominal scalar return compact root is absent",
            ))?;
        let full = structural_parameter_indexes
            .get(&source_position)
            .and_then(|index| structural_parameters.get(*index))
            .ok_or(LoweringError::Unsupported(
                "nominal scalar return full root is absent",
            ))?;
        if compact.structural_type != full.structural_type
            || caller_place_rebase
                .insert(compact.place, full.place)
                .is_some()
        {
            return unsupported("nominal scalar return compact root mapping drifted");
        }
    }
    if caller_place_rebase.len() != nominal_source_positions.len() {
        return unsupported("nominal scalar return compact root mapping is incomplete");
    }

    let mut next_proof_root = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.structural_places)
        .map(|place| place.id.get())
        .chain(
            structural_parameters
                .iter()
                .map(|parameter| parameter.place.get()),
        )
        .max()
        .unwrap_or(0);
    let mut receiver_place_rebase = BTreeMap::new();
    for cleanup in &terminal_nominals {
        let Some(receiver) = cleanup.cleanup_receiver else {
            continue;
        };
        if receiver_place_rebase.contains_key(&receiver) {
            continue;
        }
        next_proof_root = next_proof_root
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "contextual nominal scalar proof-root identity space is exhausted",
            ))?;
        receiver_place_rebase.insert(receiver, place_id(next_proof_root));
    }

    for requirement in &mut lowered.semantic_module.machines[entry_index]
        .contract
        .requires
    {
        rebase_direct_boolean_requirement_root(
            requirement,
            &caller_place_rebase,
            "contextual nominal scalar caller requirement root drifted",
        )?;
    }
    let mut full_caller_clauses = plan
        .caller_requirements
        .iter()
        .map(|requirement| {
            let parameter = structural_parameter_indexes
                .get(&requirement.source_parameter_index)
                .and_then(|index| structural_parameters.get(*index))
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal scalar full caller root is absent",
                ))?;
            let structural_type = lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal scalar full caller type is absent",
                ))?;
            let field = match &structural_type.shape {
                StructuralTypeShape::Record { fields } => fields
                    .iter()
                    .find(|field| field.identity == requirement.field_identity)
                    .filter(|field| {
                        !field.relevance.is_erased()
                            && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
                    })
                    .map(|field| field.id),
                StructuralTypeShape::ByteSequence(_)
                | StructuralTypeShape::FixedArray { .. }
                | StructuralTypeShape::Sum { .. } => None,
            }
            .ok_or(LoweringError::Unsupported(
                "contextual nominal scalar full caller field drifted",
            ))?;
            Ok((
                (requirement.expected, parameter.place, field),
                Proposition::Equal(
                    ScalarTerm::boolean(requirement.expected),
                    ScalarTerm::boolean_field(parameter.place, field),
                ),
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    full_caller_clauses.sort_by_key(|((expected, root, field), _)| {
        (
            *expected,
            root.get().to_le_bytes(),
            field.get().to_le_bytes(),
        )
    });
    if full_caller_clauses
        .windows(2)
        .any(|pair| pair[0].0 == pair[1].0)
    {
        return unsupported("contextual nominal scalar full caller requirements are duplicated");
    }
    let full_caller_requires = full_caller_clauses
        .into_iter()
        .map(|(_, proposition)| proposition)
        .collect::<Vec<_>>();
    let compact_caller_requires = lowered.semantic_module.machines[entry_index]
        .contract
        .requires
        .clone();
    let assumption_rebase = compact_caller_requires
        .iter()
        .map(|requirement| {
            full_caller_requires
                .iter()
                .position(|full| full == requirement)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal scalar proof assumption is absent from the full caller",
                ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lowered.semantic_module.machines[entry_index]
        .contract
        .requires = full_caller_requires;
    let target_receivers = terminal_nominals
        .iter()
        .filter_map(|cleanup| {
            cleanup
                .cleanup_receiver
                .map(|receiver| (cleanup.cleanup_machine, receiver))
        })
        .collect::<BTreeMap<_, _>>();
    for (target, receiver) in target_receivers {
        if terminal_nominals.iter().any(|cleanup| {
            cleanup.cleanup_machine == target && cleanup.cleanup_receiver != Some(receiver)
        }) {
            return unsupported("shared contextual scalar cleanup receiver drifted");
        }
        let target = lowered
            .semantic_module
            .machines
            .iter_mut()
            .find(|machine| machine.id == target)
            .ok_or(LoweringError::Unsupported(
                "contextual nominal scalar cleanup target is absent",
            ))?;
        for requirement in &mut target.contract.requires {
            rebase_direct_boolean_requirement_root(
                requirement,
                &receiver_place_rebase,
                "contextual nominal scalar cleanup receiver drifted",
            )?;
        }
    }
    for evidence in &mut lowered.proof_bundle.evidence {
        let EvidenceRoute::CertificateDerived(certificate) = &mut evidence.route else {
            return unsupported("contextual nominal scalar cleanup evidence route drifted");
        };
        let ProofRule::Assumption { index } = &mut certificate.proof.rule else {
            return unsupported("contextual nominal scalar cleanup proof rule drifted");
        };
        *index = *assumption_rebase
            .get(*index)
            .ok_or(LoweringError::Unsupported(
                "contextual nominal scalar cleanup proof assumption index drifted",
            ))?;
        rebase_direct_boolean_requirement_root(
            &mut certificate.proof.conclusion,
            &caller_place_rebase,
            "contextual nominal scalar cleanup proof conclusion drifted",
        )?;
    }
    for cleanup in &mut terminal_nominals {
        if let Some(receiver) = cleanup.cleanup_receiver {
            cleanup.cleanup_receiver = Some(*receiver_place_rebase.get(&receiver).ok_or(
                LoweringError::Unsupported(
                    "contextual nominal scalar cleanup receiver mapping is absent",
                ),
            )?);
        }
    }

    let mut terminal_nominals = terminal_nominals.into_iter();
    let cleanup_actions = plan
        .cleanup_actions
        .iter()
        .map(|action| {
            let source_position = match action {
                CheckedStructuralScalarReturnCleanupAction::DiscardRoot(position) => *position,
                CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup) => {
                    cleanup.source_parameter_index
                }
            };
            let place = structural_parameter_indexes
                .get(&source_position)
                .and_then(|index| structural_parameters.get(*index))
                .map(|parameter| parameter.place)
                .ok_or(LoweringError::Unsupported(
                    "nominal scalar return cleanup terminal root is absent",
                ))?;
            match action {
                CheckedStructuralScalarReturnCleanupAction::DiscardRoot(_) => {
                    Ok(TerminalAffineCleanupAction::DiscardRoot(place))
                }
                CheckedStructuralScalarReturnCleanupAction::InvokeNominal(_) => {
                    let mut cleanup =
                        terminal_nominals.next().ok_or(LoweringError::Unsupported(
                            "nominal scalar return synthetic cleanup stream is short",
                        ))?;
                    cleanup.place = place;
                    Ok(TerminalAffineCleanupAction::InvokeNominal(cleanup))
                }
            }
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if terminal_nominals.next().is_some() {
        return unsupported("nominal scalar return synthetic cleanup stream is long");
    }
    let mut next_value = 1_u64;
    let scalar_parameters = plan
        .scalar_parameters
        .iter()
        .map(|parameter| {
            Ok(ValueDeclaration {
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
        psi_terminal_codec::canonical_proposition_order_key(requirement)
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
        scalar_values.push(ValueDeclaration { id, scalar_type });
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
    let first_unused_edge = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| block.terminator.edges())
        .map(|edge| edge.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return edge identity space is exhausted",
        ))?;
    let first_unused_block = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .map(|block| block.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return block identity space is exhausted",
        ))?;
    let entry = &mut lowered.semantic_module.machines[entry_index];
    let [synthetic_block] = entry.blocks.as_slice() else {
        return unsupported("nominal scalar return entry control is not a single block");
    };
    if synthetic_block.terminator.edge() != edge {
        return unsupported("nominal scalar return synthetic edge drifted");
    }
    let parameter_types = scalar_values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    if let Some(convergence) = plan.shared_boolean_convergence {
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
            &structural_parameters,
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
    } else if let Some((_, decision)) = &source_distributed_short_circuit_bindings {
        validate_boolean_decision_parameter_types(decision, &parameter_types)?;
    } else {
        validate_direct_parameter_types(&expression, &parameter_types)?;
    }
    let blocks = if let Some(convergence) = plan.shared_boolean_convergence {
        usize::try_from(convergence.binding_ordinal).map_err(|_| {
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
            &structural_parameters,
            &lowered.semantic_module.structural_types,
        )?;
        let decision = normalize_shared_boolean_comparison_leaves(&decision).ok_or(
            LoweringError::Unsupported(
                "shared Boolean convergence contains a non-normalizable comparison leaf",
            ),
        )?;
        if shared_boolean_runtime_parameters(&decision)
            .is_none_or(|inputs| !valid_shared_boolean_runtime_inputs(&inputs))
        {
            return unsupported("shared Boolean convergence has no normalized runtime input");
        }
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
            entry.entry,
            block_id(first_unused_block),
            &mut next_value,
            &mut next_edge,
            &mut operations,
        );
        let mut entry_operations = operations.operations[..entry_operation_count].to_vec();
        entry_operations.extend(root.operations);
        root.operations = entry_operations;
        let convergence_value = ValueDeclaration {
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
            id: continuation_block,
            parameters: vec![convergence_value],
            operations: operations.operations[continuation_operation_start..].to_vec(),
            terminator: Terminator::Return {
                edge: return_edge,
                value,
                cleanup_actions: cleanup_actions.clone(),
            },
        });
        attach_edge_local_cleanup_proofs(
            &mut blocks,
            &cleanup_actions,
            operations.next_identity,
            &mut lowered.proof_bundle,
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
            entry.entry,
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
        attach_edge_local_cleanup_proofs(
            &mut blocks,
            &cleanup_actions,
            operations.next_identity,
            &mut lowered.proof_bundle,
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
            id: entry.entry,
            parameters: Vec::new(),
            operations: operations.operations,
            terminator: Terminator::Return {
                edge,
                value,
                cleanup_actions,
            },
        }]
    };
    entry.blocks = blocks;
    entry.parameters = scalar_parameters;
    entry.contract.requires.extend(scalar_requirements);
    entry.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(next_value),
        scalar_type: result_type,
    });
    entry.structural_parameters = structural_parameters.clone();
    entry.structural_places = structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .collect();
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

fn attach_edge_local_cleanup_proofs(
    blocks: &mut [Block],
    cleanup_actions: &[TerminalAffineCleanupAction],
    next_operation_identity: u64,
    proof_bundle: &mut ProofBundle,
) -> Result<(), LoweringError> {
    let mut first_return = true;
    // Cleanup obligations are edge-local semantic events. Keep the first
    // leaf's already-verified stream, then clone its proof for each later leaf
    // under fresh identities beyond every operation-derived goal.
    let mut next_cleanup_obligation =
        next_operation_identity
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "nominal scalar Boolean cleanup obligation identity space is exhausted",
            ))?;
    let original_evidence = proof_bundle
        .evidence
        .iter()
        .map(|evidence| (evidence.obligation, evidence.clone()))
        .collect::<BTreeMap<_, _>>();
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
                let mut evidence = original_evidence.get(obligation).cloned().ok_or(
                    LoweringError::Unsupported("nominal scalar Boolean cleanup evidence is absent"),
                )?;
                let identity = next_cleanup_obligation;
                next_cleanup_obligation =
                    next_cleanup_obligation
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "nominal scalar Boolean cleanup obligation identity space is exhausted",
                        ))?;
                let leaf_obligation = obligation_id(identity);
                evidence.obligation = leaf_obligation;
                let EvidenceRoute::CertificateDerived(certificate) = &mut evidence.route else {
                    return unsupported("nominal scalar Boolean cleanup evidence route drifted");
                };
                certificate.identity =
                    EvidenceIdentity::new(identity).ok_or(LoweringError::Unsupported(
                        "nominal scalar Boolean cleanup evidence identity is invalid",
                    ))?;
                *obligation = leaf_obligation;
                proof_bundle.evidence.push(evidence);
            }
        }
    }
    Ok(())
}

fn rebase_direct_boolean_requirement_root(
    proposition: &mut Proposition,
    places: &BTreeMap<PlaceId, PlaceId>,
    error: &'static str,
) -> Result<(), LoweringError> {
    let Proposition::Equal(ScalarTerm::Boolean(_), ScalarTerm::BooleanField { root, .. }) =
        proposition
    else {
        return unsupported(error);
    };
    *root = *places.get(root).ok_or(LoweringError::Unsupported(error))?;
    Ok(())
}
