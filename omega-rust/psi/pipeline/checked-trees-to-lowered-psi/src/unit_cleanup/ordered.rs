//! Ordered multi-root nominal cleanup lowering.

use super::*;

pub(super) fn lower_ordered_nominal_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
) -> Result<LoweredPsi, LoweringError> {
    let plan = &nominal.machine;
    let service_summary_is_empty = |summary: ServiceReachSummary| {
        checked
            .facts
            .service_reaches
            .rows
            .services(summary.direct)
            .is_empty()
            && checked
                .facts
                .service_reaches
                .rows
                .services(summary.transitive)
                .is_empty()
    };
    let service_plan_is_empty = |plan: ServiceReachPlan| {
        let published_is_empty = match plan.interface {
            ServiceReachInterface::InternalInferred => true,
            ServiceReachInterface::PublishedCeiling(row) => {
                checked.facts.service_reaches.rows.services(row).is_empty()
            }
        };
        published_is_empty
            && checked
                .facts
                .service_reaches
                .rows
                .services(plan.checked_inferred)
                .is_empty()
    };
    let parameter_count = plan.structural_parameters.len();
    if parameter_count < 2 || nominal.cleanups.len() != parameter_count {
        return unsupported("ordered nominal cleanup requires matched actions");
    }
    let [
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 0,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        },
    ] = plan.operations.as_slice()
    else {
        return unsupported("ordered nominal cleanup caller operation sequence drifted");
    };
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .any(|candidate| candidate.machine == plan.machine)
        || !plan.trivial_affine_locals.is_empty()
        || !plan.entry_claims.is_empty()
        || !plan.body_qualifications.is_empty()
        || !service_summary_is_empty(plan.service_reach)
        || !service_plan_is_empty(plan.contract_service_reach)
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return unsupported("ordered nominal cleanup caller signature drifted");
    }
    for (position, parameter) in plan.structural_parameters.iter().enumerate() {
        let cleanup = &nominal.cleanups[parameter_count - position - 1];
        if usize::try_from(parameter.position).ok() != Some(position)
            || usize::try_from(cleanup.source_parameter_index).ok() != Some(position)
            || parameter.is_self
            || parameter.multiplicity != Multiplicity::Affine
            || !parameter.qualifications.is_empty()
            || cleanup.type_identity != parameter.type_identity
            || cleanup.cleanup_machine == plan.machine
            || cleanup.cleanup_contract_report_fingerprint == 0
        {
            return unsupported("ordered nominal cleanup parameter join drifted");
        }
    }

    let nominal_types = &checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .structural_types;
    if nominal_types
        .iter()
        .any(|candidate| candidate.identity.is_empty())
        || nominal_types.iter().enumerate().any(|(index, candidate)| {
            nominal_types[..index]
                .iter()
                .any(|earlier| earlier.identity == candidate.identity)
        })
    {
        return unsupported("ordered nominal cleanup structural types are empty or duplicated");
    }
    let attachment_shape = nominal_types
        .iter()
        .find(|candidate| {
            plan.attachment_type_identity.as_deref() == Some(candidate.identity.as_str())
        })
        .ok_or(LoweringError::Unsupported(
            "ordered nominal cleanup attachment shape is absent",
        ))?;
    if !matches!(&attachment_shape.shape, CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty())
    {
        return unsupported("ordered nominal cleanup attachment is not an empty record");
    }
    for parameter in &plan.structural_parameters {
        let shape = nominal_types
            .iter()
            .find(|candidate| candidate.identity == parameter.type_identity)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup parameter shape is absent",
            ))?;
        if !is_bounded_nominal_cleanup_record(&shape.shape) {
            return unsupported("ordered nominal cleanup parameter shape is outside the bound");
        }
    }

    let checked_contextual_field =
        |source_parameter_index: u32, field_identity: &str, expected: bool| {
            let parameter = plan
                .structural_parameters
                .get(usize::try_from(source_parameter_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "contextual nominal cleanup caller requirement root is out of range",
                    )
                })?)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup caller requirement root is absent",
                ))?;
            let shape = nominal_types
                .iter()
                .find(|candidate| candidate.identity == parameter.type_identity)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup receiver shape is absent",
                ))?;
            let CheckedUnitStructuralTypeShape::Record { fields } = &shape.shape else {
                unreachable!("bounded nominal cleanup receiver is a record")
            };
            fields
            .iter()
            .find(|field| field.identity == field_identity)
            .filter(|field| {
                !field.relevance.is_erased()
                    && field.field_type
                        == CheckedUnitStructuralFieldType::Scalar(PrimitiveType::Bool)
            })
            .map(|field| (field.identity.clone(), expected))
            .ok_or(LoweringError::Unsupported(
                "contextual nominal cleanup requirement field is absent, erased, or non-Boolean",
            ))
        };
    let contextual_caller_requirements = nominal
        .caller_requirements
        .iter()
        .map(|requirement| {
            checked_contextual_field(
                requirement.source_parameter_index,
                &requirement.field_identity,
                requirement.expected,
            )
            .map(|field| (requirement.source_parameter_index, field))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if contextual_caller_requirements
        .iter()
        .collect::<BTreeSet<_>>()
        .len()
        != contextual_caller_requirements.len()
    {
        return unsupported("contextual nominal cleanup caller requirements are duplicated");
    }
    let contextual_cleanup_requirements = nominal
        .cleanups
        .iter()
        .map(|cleanup| {
            let requirements = cleanup
                .requirements
                .iter()
                .map(|requirement| {
                    checked_contextual_field(
                        cleanup.source_parameter_index,
                        &requirement.field_identity,
                        requirement.expected,
                    )
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            if requirements.iter().collect::<BTreeSet<_>>().len() != requirements.len()
                || requirements.iter().any(|field| {
                    !contextual_caller_requirements.iter().any(|(root, caller_field)| {
                        *root == cleanup.source_parameter_index && caller_field == field
                    })
                })
            {
                return unsupported(
                    "contextual nominal cleanup requirements are duplicated or lack a caller premise",
                );
            }
            Ok(requirements)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    for (index, cleanup) in nominal.cleanups.iter().enumerate() {
        if let Some(earlier) = nominal.cleanups[..index]
            .iter()
            .position(|candidate| candidate.cleanup_machine == cleanup.cleanup_machine)
            && contextual_cleanup_requirements[earlier] != contextual_cleanup_requirements[index]
        {
            return unsupported("shared nominal cleanup target requirements drifted");
        }
    }

    let mut roots = Vec::new();
    let mut cleanup_helpers = Vec::new();
    for cleanup in &nominal.cleanups {
        let target = unique_unit_machine(
            &checked.facts.flow.terminal_unit_effects,
            cleanup.cleanup_machine,
        )?;
        let contract = checked
            .facts
            .contract_plans
            .for_machine(cleanup.cleanup_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target contract is absent",
            ))?;
        let (target_return, target_calls) =
            target
                .operations
                .split_last()
                .ok_or(LoweringError::Unsupported(
                    "ordered nominal cleanup target operations are empty",
                ))?;
        let CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } = target_return
        else {
            return unsupported("ordered nominal cleanup target does not end in Unit return");
        };
        if usize::try_from(*statement_index).ok() != Some(target_calls.len())
            || !trivial_affine_local_discard_ordinals.is_empty()
            || !trivial_affine_discards.is_empty()
        {
            return unsupported("ordered nominal cleanup target operation sequence drifted");
        }
        let collect_helpers = !roots.contains(&cleanup.cleanup_machine);
        for (statement_index, operation) in target_calls.iter().enumerate() {
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
                return unsupported("ordered nominal cleanup target operation sequence drifted");
            };
            if usize::try_from(coordinate.statement_index).ok() != Some(statement_index)
                || coordinate.call_ordinal != 0
                || *target_machine == plan.machine
                || *target_machine == cleanup.cleanup_machine
                || target_calls[..statement_index].iter().any(|earlier| {
                    matches!(
                        earlier,
                        CheckedUnitEffectOperationPlan::CallUnit {
                            target_machine: earlier_target,
                            ..
                        } if earlier_target == target_machine
                    )
                })
                || !service_summary_is_empty(*service_reach)
                || !scalar_arguments.is_empty()
                || !structural_arguments.is_empty()
                || !claim_transfers.is_empty()
            {
                return unsupported("ordered nominal cleanup helper call is not exact");
            }
            if collect_helpers {
                cleanup_helpers.push((
                    cleanup.cleanup_machine,
                    *target_machine,
                    *target_state,
                    *target_contract_report_fingerprint,
                ));
            }
        }
        if target.state != cleanup.cleanup_state
            || target.contract_report_fingerprint != cleanup.cleanup_contract_report_fingerprint
            || contract.report_fingerprint != cleanup.cleanup_contract_report_fingerprint
            || target.attachment_type_identity.as_deref() != Some(cleanup.type_identity.as_str())
            || !target.structural_parameters.is_empty()
            || !target.trivial_affine_locals.is_empty()
            || !target.entry_claims.is_empty()
            || !target.body_qualifications.is_empty()
            || !service_summary_is_empty(target.service_reach)
            || !service_plan_is_empty(target.contract_service_reach)
        {
            return unsupported("ordered nominal cleanup target is not exact and bounded");
        }
        if !roots.contains(&cleanup.cleanup_machine) {
            roots.push(cleanup.cleanup_machine);
        }
    }
    for &(_, helper_machine, helper_state, helper_fingerprint) in &cleanup_helpers {
        if roots.contains(&helper_machine) {
            return unsupported("ordered nominal cleanup helper overlaps a cleanup target");
        }
        let helper =
            unique_unit_machine(&checked.facts.flow.terminal_unit_effects, helper_machine)?;
        let helper_contract = checked
            .facts
            .contract_plans
            .for_machine(helper_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup helper contract is absent",
            ))?;
        let helper_shape = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .chain(nominal_types)
            .find(|candidate| {
                helper.attachment_type_identity.as_deref() == Some(candidate.identity.as_str())
            })
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup helper attachment shape is absent",
            ))?;
        if helper.state != helper_state
            || helper.contract_report_fingerprint != helper_fingerprint
            || helper_contract.report_fingerprint != helper_fingerprint
            || !matches!(
                &helper_shape.shape,
                CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
            )
            || !helper.structural_parameters.is_empty()
            || !helper.trivial_affine_locals.is_empty()
            || !helper.entry_claims.is_empty()
            || !helper.body_qualifications.is_empty()
            || !service_summary_is_empty(helper.service_reach)
            || !service_plan_is_empty(helper.contract_service_reach)
            || !matches!(
                helper.operations.as_slice(),
                [CheckedUnitEffectOperationPlan::ReturnUnit {
                    statement_index: 0,
                    trivial_affine_local_discard_ordinals,
                    trivial_affine_discards,
                }] if trivial_affine_local_discard_ordinals.is_empty()
                    && trivial_affine_discards.is_empty()
            )
        {
            return unsupported("ordered nominal cleanup helper is not exact and empty");
        }
    }

    let mut staged = checked.clone();
    for shape in nominal_types {
        match staged
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .find(|candidate| candidate.identity == shape.identity)
        {
            Some(existing) if existing != shape => {
                return unsupported("ordered nominal cleanup structural type conflicts");
            }
            Some(_) => {}
            None => staged
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .push(shape.clone()),
        }
    }
    staged
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .push(plan.clone());
    let closure = checked_unit_call_closure_including(&staged, plan.machine, &roots)?;
    let mut expected = vec![plan.machine];
    expected.extend(&roots);
    for &(_, helper, _, _) in &cleanup_helpers {
        if !expected.contains(&helper) {
            expected.push(helper);
        }
    }
    if closure != expected {
        return unsupported("ordered nominal cleanup closure is not exact");
    }
    let mut lowered = lower_nominal_cleanup_closure(&staged, plan.machine, &roots)?;
    let type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let entry_index = lowered
        .semantic_module
        .machines
        .iter()
        .position(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "ordered nominal cleanup entry is absent",
        ))?;
    let entry_parameters = lowered.semantic_module.machines[entry_index]
        .structural_parameters
        .clone();
    if !contextual_caller_requirements.is_empty()
        && (!lowered.proof_bundle.evidence.is_empty()
            || lowered.semantic_module.machines.iter().any(|machine| {
                !machine.contract.requires.is_empty() || !machine.contract.ensures.is_empty()
            }))
    {
        return unsupported("contextual nominal cleanup obligation namespace is not isolated");
    }
    let terminal_field =
        |source_parameter_index: u32,
         field_identity: &str|
         -> Result<(PlaceId, StructuralTypeId, StructuralFieldId), LoweringError> {
            let parameter = plan
                .structural_parameters
                .get(usize::try_from(source_parameter_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "contextual nominal cleanup terminal root is out of range",
                    )
                })?)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup terminal root is absent",
                ))?;
            let terminal_parameter = entry_parameters
                .iter()
                .find(|candidate| candidate.position == parameter.position)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup terminal parameter is absent",
                ))?;
            let structural_type = lookup_type_id(&type_ids, &parameter.type_identity)?;
            let field = lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .and_then(|declaration| match &declaration.shape {
                    StructuralTypeShape::Record { fields } => {
                        fields.iter().find(|field| field.identity == field_identity)
                    }
                    StructuralTypeShape::PrimitiveScalar(_)
                    | StructuralTypeShape::ByteSequence(_)
                    | StructuralTypeShape::FixedArray { .. }
                    | StructuralTypeShape::Sum { .. }
                    | StructuralTypeShape::Mixed { .. } => None,
                })
                .filter(|field| {
                    !field.relevance.is_erased()
                        && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
                })
                .map(|field| field.id)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup terminal field identity drifted",
                ))?;
            Ok((terminal_parameter.place, structural_type, field))
        };
    let mut caller_clauses = contextual_caller_requirements
        .iter()
        .map(|(root, (field_identity, expected))| {
            let (place, _, field) = terminal_field(*root, field_identity)?;
            Ok((
                (*expected, place, field),
                Proposition::Equal(
                    ScalarTerm::boolean(*expected),
                    ScalarTerm::boolean_field(place, field),
                ),
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    caller_clauses.sort_by_key(|((expected, root, field), _)| {
        (
            *expected,
            root.get().to_le_bytes(),
            field.get().to_le_bytes(),
        )
    });
    let caller_requires = caller_clauses
        .iter()
        .map(|(_, proposition)| proposition.clone())
        .collect::<Vec<_>>();

    let mut next_proof_root = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| machine.structural_places.iter())
        .map(|place| place.id.get())
        .max()
        .unwrap_or(0);
    let mut target_contexts = Vec::<(
        symbols::SymbolHandle,
        Option<PlaceId>,
        Vec<(bool, StructuralFieldId, Proposition)>,
    )>::new();
    for (cleanup, requirements) in nominal
        .cleanups
        .iter()
        .zip(&contextual_cleanup_requirements)
    {
        if target_contexts
            .iter()
            .any(|(target, _, _)| *target == cleanup.cleanup_machine)
        {
            continue;
        }
        let receiver = if requirements.is_empty() {
            None
        } else {
            next_proof_root = next_proof_root
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup proof-root identity space is exhausted",
                ))?;
            Some(place_id(next_proof_root))
        };
        let mut clauses = requirements
            .iter()
            .map(|(field_identity, expected)| {
                let (_, _, field) = terminal_field(cleanup.source_parameter_index, field_identity)?;
                let receiver = receiver.expect(
                    "a nonempty contextual cleanup requirement set has a proof-only receiver",
                );
                Ok((
                    *expected,
                    field,
                    Proposition::Equal(
                        ScalarTerm::boolean(*expected),
                        ScalarTerm::boolean_field(receiver, field),
                    ),
                ))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        clauses.sort_by_key(|(expected, field, _)| (*expected, field.get().to_le_bytes()));
        target_contexts.push((cleanup.cleanup_machine, receiver, clauses));
    }
    for (target_symbol, _, clauses) in &target_contexts {
        let target_index = closure
            .iter()
            .position(|candidate| candidate == target_symbol)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target was not retained",
            ))?;
        let target_id = machine_id(dense_identity(target_index)?);
        let target = lowered
            .semantic_module
            .machines
            .iter_mut()
            .find(|machine| machine.id == target_id)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target was not retained",
            ))?;
        target.contract.requires = clauses
            .iter()
            .map(|(_, _, proposition)| proposition.clone())
            .collect();
    }

    let mut next_obligation_identity = 0_u64;
    let mut evidence = Vec::new();
    let mut terminal_cleanups = Vec::with_capacity(nominal.cleanups.len());
    for cleanup in &nominal.cleanups {
        let parameter = plan
            .structural_parameters
            .get(
                usize::try_from(cleanup.source_parameter_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "ordered nominal cleanup source root is out of range",
                    )
                })?,
            )
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup source root is absent",
            ))?;
        let terminal_parameter = entry_parameters
            .iter()
            .find(|candidate| candidate.position == parameter.position)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup terminal parameter is absent",
            ))?;
        let machine_index = closure
            .iter()
            .position(|candidate| *candidate == cleanup.cleanup_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target is absent",
            ))?;
        let (_, receiver, target_clauses) = target_contexts
            .iter()
            .find(|(target, _, _)| *target == cleanup.cleanup_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target context is absent",
            ))?;
        let mut requirement_obligations = Vec::with_capacity(target_clauses.len());
        for (expected, field, _) in target_clauses {
            let assumption_index = caller_clauses
                .iter()
                .position(|((caller_expected, root, caller_field), _)| {
                    caller_expected == expected
                        && *root == terminal_parameter.place
                        && caller_field == field
                })
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup caller requirement is absent",
                ))?;
            next_obligation_identity =
                next_obligation_identity
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "contextual nominal cleanup obligation identity space is exhausted",
                    ))?;
            let obligation = obligation_id(next_obligation_identity);
            requirement_obligations.push(obligation);
            evidence.push(ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(next_obligation_identity)
                        .expect("terminal obligation identity is nonzero"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: caller_requires[assumption_index].clone(),
                        rule: ProofRule::Assumption {
                            index: assumption_index,
                        },
                    },
                }),
            });
        }
        terminal_cleanups.push(NominalAffineCleanup {
            place: terminal_parameter.place,
            structural_type: lookup_type_id(&type_ids, &cleanup.type_identity)?,
            cleanup_machine: machine_id(dense_identity(machine_index)?),
            cleanup_receiver: *receiver,
            requirement_obligations,
        });
    }
    for (cleanup, checked_cleanup) in terminal_cleanups.iter().zip(&nominal.cleanups) {
        let target = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == cleanup.cleanup_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target was not retained",
            ))?;
        let [target_block] = target.blocks.as_slice() else {
            return unsupported("ordered nominal cleanup target terminal control drifted");
        };
        let expected_helpers = cleanup_helpers
            .iter()
            .filter(|(owner, _, _, _)| *owner == checked_cleanup.cleanup_machine)
            .map(|(_, helper, _, _)| {
                closure
                    .iter()
                    .position(|candidate| candidate == helper)
                    .ok_or(LoweringError::Unsupported(
                        "ordered nominal cleanup helper was not retained",
                    ))
                    .and_then(dense_identity)
                    .map(machine_id)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let target_operations_are_exact =
            target_block.operations.len() == expected_helpers.len()
                && target_block.operations.iter().zip(&expected_helpers).all(
                    |(operation, helper)| {
                        operation.result == terminal_psi::OperationResult::Unit
                            && matches!(
                                &operation.kind,
                                OperationKind::CallUnit {
                                    callee,
                                    arguments,
                                    structural_arguments,
                                    claim_transfers,
                                    requirement_obligations,
                                    crash_continuations,
                                } if callee == helper
                                    && arguments.is_empty()
                                    && structural_arguments.is_empty()
                                    && claim_transfers.is_empty()
                                    && requirement_obligations.is_empty()
                                    && crash_continuations.is_empty()
                            )
                    },
                );
        let expected_target_requires = target_contexts
            .iter()
            .find(|(target_symbol, _, _)| *target_symbol == checked_cleanup.cleanup_machine)
            .map(|(_, _, clauses)| {
                clauses
                    .iter()
                    .map(|(_, _, proposition)| proposition.clone())
                    .collect::<Vec<_>>()
            })
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target context is absent",
            ))?;
        if target.attachment != Some(cleanup.structural_type)
            || !target.parameters.is_empty()
            || !target.structural_parameters.is_empty()
            || target.result != TerminalMachineResult::Unit
            || !target.structural_places.is_empty()
            || !target.entry_claims.is_empty()
            || !target.published_service_ceiling.is_empty()
            || !target.content_entry_claims.is_empty()
            || !target.content_identity_reshuffles.is_empty()
            || !target.content_partition_compositions.is_empty()
            || target_block.id != target.entry
            || !target_block.parameters.is_empty()
            || !target_operations_are_exact
            || !matches!(
                &target_block.terminator,
                Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } if trivial_affine_discards.is_empty()
            )
            || !target.contract.crash_routes.is_empty()
            || target.contract.requires != expected_target_requires
            || !target.contract.ensures.is_empty()
        {
            return unsupported("ordered nominal cleanup target terminal machine is not exact");
        }
    }
    for &(_, helper_symbol, _, _) in &cleanup_helpers {
        let helper_index = closure
            .iter()
            .position(|candidate| *candidate == helper_symbol)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup helper was not retained",
            ))?;
        let helper_id = machine_id(dense_identity(helper_index)?);
        let helper = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == helper_id)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup helper terminal machine is absent",
            ))?;
        let [helper_block] = helper.blocks.as_slice() else {
            return unsupported("ordered nominal cleanup helper terminal control drifted");
        };
        let helper_attachment_is_empty = helper.attachment.is_some_and(|attachment| {
            lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == attachment)
                .is_some_and(|declaration| {
                    matches!(
                        &declaration.shape,
                        StructuralTypeShape::Record { fields } if fields.is_empty()
                    )
                })
        });
        if !helper_attachment_is_empty
            || !helper.parameters.is_empty()
            || !helper.structural_parameters.is_empty()
            || helper.result != TerminalMachineResult::Unit
            || !helper.structural_places.is_empty()
            || !helper.entry_claims.is_empty()
            || !helper.published_service_ceiling.is_empty()
            || !helper.content_entry_claims.is_empty()
            || !helper.content_identity_reshuffles.is_empty()
            || !helper.content_partition_compositions.is_empty()
            || helper_block.id != helper.entry
            || !helper_block.parameters.is_empty()
            || !helper_block.operations.is_empty()
            || !matches!(
                &helper_block.terminator,
                Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } if trivial_affine_discards.is_empty()
            )
            || !helper.contract.crash_routes.is_empty()
            || !helper.contract.requires.is_empty()
            || !helper.contract.ensures.is_empty()
        {
            return unsupported("ordered nominal cleanup helper terminal machine is not exact");
        }
    }
    let entry = &mut lowered.semantic_module.machines[entry_index];
    entry.contract.requires = caller_requires.clone();
    let [block] = entry.blocks.as_mut_slice() else {
        return unsupported("ordered nominal cleanup entry control drifted");
    };
    let Terminator::ReturnUnit {
        edge,
        trivial_affine_discards,
    } = &block.terminator
    else {
        return unsupported("ordered nominal cleanup entry return drifted");
    };
    if entry.structural_parameters.len() != parameter_count
        || entry.structural_places.len() != parameter_count
        || !entry.parameters.is_empty()
        || entry.result != TerminalMachineResult::Unit
        || !entry.entry_claims.is_empty()
        || !entry.published_service_ceiling.is_empty()
        || !entry.content_entry_claims.is_empty()
        || !entry.content_identity_reshuffles.is_empty()
        || !entry.content_partition_compositions.is_empty()
        || block.id != entry.entry
        || !block.parameters.is_empty()
        || !block.operations.is_empty()
        || !trivial_affine_discards.is_empty()
        || !entry.contract.crash_routes.is_empty()
        || entry.contract.requires != caller_requires
        || !entry.contract.ensures.is_empty()
    {
        return unsupported("ordered nominal cleanup terminal caller is not exact");
    }
    block.terminator = Terminator::ReturnUnitNominalAffine {
        edge: *edge,
        cleanups: terminal_cleanups,
    };
    lowered.proof_bundle.evidence = evidence;
    Ok(lowered)
}
