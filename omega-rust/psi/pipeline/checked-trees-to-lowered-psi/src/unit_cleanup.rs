//! Structural Unit cleanup lowering.
//!
//! The nominal entry point retains family precedence. Ordered nominal cleanup
//! and partial-affine cleanup live in separate responsibility modules.

use super::*;

mod ordered;
mod partial;
use ordered::lower_ordered_nominal_affine_unit_cleanup_machine;

pub(super) fn lower_nominal_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
) -> Result<LoweredPsi, LoweringError> {
    if nominal.cleanups.len() >= 2 {
        return lower_ordered_nominal_affine_unit_cleanup_machine(checked, nominal);
    }
    let [cleanup] = nominal.cleanups.as_slice() else {
        return unsupported("nominal affine Unit cleanup list must be nonempty");
    };
    let plan = &nominal.machine;
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .any(|candidate| candidate.machine == plan.machine)
    {
        return unsupported("nominal affine Unit machine is also published in the trivial lane");
    }
    let [parameter] = plan.structural_parameters.as_slice() else {
        return unsupported("nominal affine Unit cleanup requires one structural parameter");
    };
    let [
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        },
    ] = plan.operations.as_slice()
    else {
        return unsupported("nominal affine Unit cleanup operation sequence drifted");
    };
    if parameter.position != 0
        || parameter.is_self
        || parameter.multiplicity != Multiplicity::Affine
        || !parameter.qualifications.is_empty()
        || !plan.trivial_affine_locals.is_empty()
        || !plan.entry_claims.is_empty()
        || !plan.body_qualifications.is_empty()
        || *statement_index != 0
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
        || cleanup.source_parameter_index != 0
        || cleanup.type_identity != parameter.type_identity
        || cleanup.cleanup_machine == plan.machine
        || cleanup.cleanup_contract_report_fingerprint == 0
    {
        return unsupported("nominal affine Unit cleanup signature or coordinates drifted");
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
        return unsupported("nominal affine Unit structural types are empty or duplicated");
    }
    let attachment_shape = nominal_types
        .iter()
        .find(|candidate| {
            plan.attachment_type_identity.as_deref() == Some(candidate.identity.as_str())
        })
        .ok_or(LoweringError::Unsupported(
            "nominal affine Unit attachment type is absent from its checked shapes",
        ))?;
    if !matches!(
        &attachment_shape.shape,
        CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
    ) {
        return unsupported("nominal affine Unit attachment is not an empty record");
    }
    let parameter_shape = nominal_types
        .iter()
        .find(|candidate| candidate.identity == parameter.type_identity)
        .ok_or(LoweringError::Unsupported(
            "nominal affine Unit parameter type is absent from its checked shapes",
        ))?;
    if !is_bounded_nominal_cleanup_record(&parameter_shape.shape) {
        return unsupported("nominal affine Unit parameter is outside the bounded record shape");
    }
    let checked_contextual_field = |field_identity: &str, expected: bool| {
        let CheckedUnitStructuralTypeShape::Record { fields } = &parameter_shape.shape else {
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
    let contextual_requirements = cleanup
        .requirements
        .iter()
        .map(|requirement| {
            checked_contextual_field(&requirement.field_identity, requirement.expected)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if contextual_requirements
        .iter()
        .collect::<BTreeSet<_>>()
        .len()
        != contextual_requirements.len()
    {
        return unsupported("contextual nominal cleanup requirements are duplicated");
    }
    let contextual_caller_requirements = nominal
        .caller_requirements
        .iter()
        .map(|requirement| {
            if requirement.source_parameter_index != cleanup.source_parameter_index {
                return Err(LoweringError::Unsupported(
                    "contextual nominal cleanup caller requirement root drifted",
                ));
            }
            checked_contextual_field(&requirement.field_identity, requirement.expected)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if contextual_caller_requirements
        .iter()
        .collect::<BTreeSet<_>>()
        .len()
        != contextual_caller_requirements.len()
        || contextual_requirements.iter().any(|required| {
            !contextual_caller_requirements
                .iter()
                .any(|caller| caller == required)
        })
    {
        return unsupported(
            "contextual nominal cleanup caller requirements are duplicated or incomplete",
        );
    }

    let cleanup_target = unique_unit_machine(
        &checked.facts.flow.terminal_unit_effects,
        cleanup.cleanup_machine,
    )?;
    let cleanup_contract = checked
        .facts
        .contract_plans
        .for_machine(cleanup.cleanup_machine)
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup target is missing its checked contract identity",
        ))?;
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
    let (cleanup_return, cleanup_calls) =
        cleanup_target
            .operations
            .split_last()
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup target operation sequence is empty",
            ))?;
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index,
        trivial_affine_local_discard_ordinals,
        trivial_affine_discards,
    } = cleanup_return
    else {
        return unsupported("nominal cleanup target operation sequence drifted");
    };
    if usize::try_from(*statement_index).ok() != Some(cleanup_calls.len())
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return unsupported("nominal cleanup target operation sequence drifted");
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
            return unsupported("nominal cleanup target operation sequence drifted");
        };
        if usize::try_from(coordinate.statement_index).ok() != Some(statement_index)
            || coordinate.call_ordinal != 0
            || *target_machine == plan.machine
            || *target_machine == cleanup.cleanup_machine
            || cleanup_helpers
                .iter()
                .any(|(helper, _, _)| helper == target_machine)
            || !service_summary_is_empty(*service_reach)
            || !scalar_arguments.is_empty()
            || !structural_arguments.is_empty()
            || !claim_transfers.is_empty()
        {
            return unsupported("nominal cleanup target operation sequence drifted");
        }
        cleanup_helpers.push((
            *target_machine,
            *target_state,
            *target_contract_report_fingerprint,
        ));
    }
    if cleanup_target.state != cleanup.cleanup_state
        || cleanup_target.contract_report_fingerprint != cleanup.cleanup_contract_report_fingerprint
        || cleanup_contract.report_fingerprint != cleanup.cleanup_contract_report_fingerprint
        || cleanup_target.attachment_type_identity.as_deref()
            != Some(cleanup.type_identity.as_str())
        || !cleanup_target.structural_parameters.is_empty()
        || !cleanup_target.trivial_affine_locals.is_empty()
        || !cleanup_target.entry_claims.is_empty()
        || !cleanup_target.body_qualifications.is_empty()
        || !service_summary_is_empty(cleanup_target.service_reach)
        || !service_plan_is_empty(cleanup_target.contract_service_reach)
    {
        return unsupported("nominal cleanup target identity or bounded signature drifted");
    }

    for &(helper_machine, helper_state, helper_fingerprint) in &cleanup_helpers {
        let helper =
            unique_unit_machine(&checked.facts.flow.terminal_unit_effects, helper_machine)?;
        let helper_contract = checked
            .facts
            .contract_plans
            .for_machine(helper_machine)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup helper is missing its checked contract identity",
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
                "nominal cleanup helper attachment is missing its checked shape",
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
            return unsupported("nominal cleanup helper is not exact and empty");
        }
    }

    // Cleanup is an explicit additional closure root because it is executable
    // edge work, not a source-authored ordinary call operation.
    let mut staged = checked.clone();
    let staged_unit = &mut staged.facts.flow.terminal_unit_effects;
    for shape in nominal_types {
        match staged_unit
            .structural_types
            .iter()
            .find(|candidate| candidate.identity == shape.identity)
        {
            Some(existing) if existing != shape => {
                return unsupported(
                    "nominal affine Unit structural type conflicts with its cleanup closure",
                );
            }
            Some(_) => {}
            None => staged_unit.structural_types.push(shape.clone()),
        }
    }
    staged_unit.machines.push(plan.clone());
    let closure =
        checked_unit_call_closure_including(&staged, plan.machine, &[cleanup.cleanup_machine])?;
    let mut expected_closure = vec![plan.machine, cleanup.cleanup_machine];
    expected_closure.extend(cleanup_helpers.iter().map(|(helper, _, _)| *helper));
    if closure != expected_closure {
        return unsupported("nominal cleanup closure is not the exact bounded machine graph");
    }
    let cleanup_machine_index = closure
        .iter()
        .position(|candidate| *candidate == cleanup.cleanup_machine)
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup target is absent from its checked closure",
        ))?;
    let cleanup_terminal_id = machine_id(dense_identity(cleanup_machine_index)?);
    let helper_terminal_ids = cleanup_helpers
        .iter()
        .map(|(helper, _, _)| {
            closure
                .iter()
                .position(|candidate| candidate == helper)
                .ok_or(LoweringError::Unsupported(
                    "nominal cleanup helper is absent from its checked closure",
                ))
                .and_then(dense_identity)
                .map(machine_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut lowered =
        lower_nominal_cleanup_closure(&staged, plan.machine, &[cleanup.cleanup_machine])?;
    let type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let cleanup_type = lookup_type_id(&type_ids, &cleanup.type_identity)?;

    let (cleanup_receiver, requirement_obligations, target_requires, caller_requires, evidence) =
        if contextual_caller_requirements.is_empty() {
            (None, Vec::new(), Vec::new(), Vec::new(), Vec::new())
        } else {
            if !lowered.proof_bundle.evidence.is_empty()
                || lowered.semantic_module.machines.iter().any(|machine| {
                    !machine.contract.requires.is_empty() || !machine.contract.ensures.is_empty()
                })
            {
                return unsupported(
                    "contextual nominal cleanup obligation namespace is not isolated",
                );
            }
            let receiver = if contextual_requirements.is_empty() {
                None
            } else {
                Some(place_id(
                    lowered
                        .semantic_module
                        .machines
                        .iter()
                        .flat_map(|machine| machine.structural_places.iter())
                        .map(|place| place.id.get())
                        .max()
                        .unwrap_or(0)
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "contextual nominal cleanup proof-root identity space is exhausted",
                        ))?,
                ))
            };
            let caller_place = lowered
                .semantic_module
                .machines
                .iter()
                .find(|machine| machine.id == lowered.semantic_module.entry)
                .and_then(|machine| machine.structural_parameters.first())
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup caller parameter is absent",
                ))?
                .place;
            let terminal_fields = lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == cleanup_type)
                .and_then(|declaration| match &declaration.shape {
                    StructuralTypeShape::Record { fields } => Some(fields),
                    StructuralTypeShape::PrimitiveScalar(_)
                    | StructuralTypeShape::ByteSequence(_)
                    | StructuralTypeShape::FixedArray { .. }
                    | StructuralTypeShape::Sum { .. }
                    | StructuralTypeShape::Mixed { .. } => None,
                })
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup terminal receiver shape drifted",
                ))?;
            let terminal_field = |field_identity: &str| {
                terminal_fields
                    .iter()
                    .find(|field| field.identity == field_identity)
                    .filter(|field| {
                        !field.relevance.is_erased()
                            && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
                    })
                    .map(|field| field.id)
                    .ok_or(LoweringError::Unsupported(
                        "contextual nominal cleanup terminal field identity drifted",
                    ))
            };

            let mut caller_clauses = contextual_caller_requirements
                .iter()
                .map(|(field_identity, expected)| {
                    let field = terminal_field(field_identity)?;
                    Ok((
                        *expected,
                        field,
                        Proposition::Equal(
                            ScalarTerm::boolean(*expected),
                            ScalarTerm::boolean_field(caller_place, field),
                        ),
                    ))
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            // Every proposition in this bounded vocabulary shares the same
            // tags and root. Its canonical codec order is Boolean polarity,
            // then the little-endian byte order of StructuralFieldId. Sort
            // after terminal identities exist rather than trusting checked
            // declaration-identity order.
            caller_clauses
                .sort_by_key(|(expected, field, _)| (*expected, field.get().to_le_bytes()));
            let caller_requires = caller_clauses
                .iter()
                .map(|(_, _, proposition)| proposition.clone())
                .collect::<Vec<_>>();

            let mut target_clauses = contextual_requirements
                .iter()
                .map(|(field_identity, expected)| {
                    let field = terminal_field(field_identity)?;
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
            target_clauses
                .sort_by_key(|(expected, field, _)| (*expected, field.get().to_le_bytes()));

            let mut requirement_obligations = Vec::with_capacity(target_clauses.len());
            let mut target_requires = Vec::with_capacity(target_clauses.len());
            let mut evidence = Vec::with_capacity(target_clauses.len());
            for (obligation_index, (expected, field, target_requirement)) in
                target_clauses.into_iter().enumerate()
            {
                let identity = u64::try_from(obligation_index)
                    .ok()
                    .and_then(|index| index.checked_add(1))
                    .ok_or(LoweringError::Unsupported(
                        "contextual nominal cleanup obligation identity space is exhausted",
                    ))?;
                let assumption_index = caller_clauses
                    .iter()
                    .position(|(caller_expected, caller_field, _)| {
                        *caller_expected == expected && *caller_field == field
                    })
                    .ok_or(LoweringError::Unsupported(
                        "contextual nominal cleanup caller requirement is absent",
                    ))?;
                let caller_requirement = caller_requires[assumption_index].clone();
                let obligation = obligation_id(identity);
                requirement_obligations.push(obligation);
                target_requires.push(target_requirement);
                evidence.push(ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                        identity: EvidenceIdentity::new(identity)
                            .expect("terminal obligation identity is nonzero"),
                        proof_system_marker: ProofSystemMarker::CURRENT,
                        proof: ProofNode {
                            conclusion: caller_requirement,
                            rule: ProofRule::Assumption {
                                index: assumption_index,
                            },
                        },
                    }),
                });
            }
            (
                receiver,
                requirement_obligations,
                target_requires,
                caller_requires,
                evidence,
            )
        };

    let cleanup_terminal = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == cleanup_terminal_id)
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup target was not retained in the terminal closure",
        ))?;
    cleanup_terminal.contract.requires = target_requires.clone();
    let [cleanup_block] = cleanup_terminal.blocks.as_slice() else {
        return unsupported("nominal cleanup target terminal control drifted");
    };
    let cleanup_operations_are_exact = cleanup_block.operations.len() == helper_terminal_ids.len()
        && cleanup_block
            .operations
            .iter()
            .zip(&helper_terminal_ids)
            .all(|(operation, helper)| {
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
            });
    if cleanup_terminal.attachment != Some(cleanup_type)
        || !cleanup_terminal.parameters.is_empty()
        || !cleanup_terminal.structural_parameters.is_empty()
        || cleanup_terminal.result != TerminalMachineResult::Unit
        || !cleanup_terminal.structural_places.is_empty()
        || !cleanup_terminal.entry_claims.is_empty()
        || !cleanup_terminal.published_service_ceiling.is_empty()
        || !cleanup_terminal.content_entry_claims.is_empty()
        || !cleanup_terminal.content_identity_reshuffles.is_empty()
        || !cleanup_terminal.content_partition_compositions.is_empty()
        || !cleanup_operations_are_exact
        || !matches!(
            &cleanup_block.terminator,
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards.is_empty()
        )
        || !cleanup_terminal.contract.crash_routes.is_empty()
        || cleanup_terminal.contract.requires != target_requires
        || !cleanup_terminal.contract.ensures.is_empty()
    {
        return unsupported("nominal cleanup target terminal machine is not exact and bounded");
    }

    for &helper_id in &helper_terminal_ids {
        let helper = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == helper_id)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup helper was not retained in the terminal closure",
            ))?;
        let [helper_block] = helper.blocks.as_slice() else {
            return unsupported("nominal cleanup helper terminal control drifted");
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
        if helper.id == cleanup_terminal_id
            || helper.id == lowered.semantic_module.entry
            || !helper_attachment_is_empty
            || !helper.parameters.is_empty()
            || !helper.structural_parameters.is_empty()
            || helper.result != TerminalMachineResult::Unit
            || !helper.structural_places.is_empty()
            || !helper.entry_claims.is_empty()
            || !helper.published_service_ceiling.is_empty()
            || !helper.content_entry_claims.is_empty()
            || !helper.content_identity_reshuffles.is_empty()
            || !helper.content_partition_compositions.is_empty()
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
            return unsupported("nominal cleanup helper terminal machine is not exact and empty");
        }
    }

    let entry = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "nominal affine Unit entry machine was not lowered",
        ))?;
    let [terminal_parameter] = entry.structural_parameters.as_slice() else {
        return unsupported("nominal affine Unit terminal parameter drifted");
    };
    entry.contract.requires = caller_requires.clone();
    if entry.attachment
        != Some(lookup_type_id(
            &type_ids,
            plan.attachment_type_identity
                .as_deref()
                .ok_or(LoweringError::Unsupported(
                    "nominal cleanup caller is not attached",
                ))?,
        )?)
        || !entry.parameters.is_empty()
        || entry.result != TerminalMachineResult::Unit
        || entry.structural_places.len() != 1
        || !entry.entry_claims.is_empty()
        || !entry.published_service_ceiling.is_empty()
        || !entry.content_entry_claims.is_empty()
        || !entry.content_identity_reshuffles.is_empty()
        || !entry.content_partition_compositions.is_empty()
        || !entry.contract.crash_routes.is_empty()
        || entry.contract.requires != caller_requires
        || !entry.contract.ensures.is_empty()
        || terminal_parameter.structural_type != cleanup_type
        || terminal_parameter.multiplicity != StructuralMultiplicity::Affine
        || !terminal_parameter.qualifications.is_empty()
    {
        return unsupported("nominal affine Unit terminal parameter identity drifted");
    }
    let [block] = entry.blocks.as_mut_slice() else {
        return unsupported("nominal affine Unit terminal control drifted");
    };
    if block.id != entry.entry || !block.parameters.is_empty() || !block.operations.is_empty() {
        return unsupported("nominal affine Unit terminal control is not exact and empty");
    }
    let Terminator::ReturnUnit {
        edge,
        trivial_affine_discards: lowered_trivial_discards,
    } = &block.terminator
    else {
        return unsupported("nominal affine Unit terminal return drifted");
    };
    if !lowered_trivial_discards.is_empty() {
        return unsupported("nominal affine Unit return acquired trivial cleanup");
    }
    block.terminator = Terminator::ReturnUnitNominalAffine {
        edge: *edge,
        cleanups: vec![NominalAffineCleanup {
            place: terminal_parameter.place,
            structural_type: cleanup_type,
            cleanup_machine: cleanup_terminal_id,
            cleanup_receiver,
            requirement_obligations,
        }],
    };
    lowered.proof_bundle.evidence = evidence;
    Ok(lowered)
}

fn is_bounded_nominal_cleanup_record(shape: &CheckedUnitStructuralTypeShape) -> bool {
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

pub(super) fn lower_partial_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    partial: &CheckedPartialAffineUnitCleanupMachinePlan,
) -> Result<LoweredPsi, LoweringError> {
    partial::lower_partial_affine_unit_cleanup_machine(checked, partial)
}
