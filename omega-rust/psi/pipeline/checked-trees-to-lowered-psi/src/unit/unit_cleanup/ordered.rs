//! Ordered multi-root nominal cleanup lowering.

use super::super::{PlaceId, StructuralFieldId, StructuralTypeId};
use super::{
    BTreeSet, CheckedNominalAffineUnitCleanupMachinePlan, CheckedTrees,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape,
    LoweredPsi, LoweringError, Multiplicity, NominalAffineCleanup, PrimitiveType, Proposition,
    ScalarTerm, ScalarType, ServiceReachInterface, ServiceReachPlan, ServiceReachSummary,
    StructuralFieldType, StructuralTypeShape, TerminalMachineResult, Terminator,
    checked_unit_call_closure_including, dense_identity, is_bounded_nominal_cleanup_record,
    lookup_type_id, lower_unit_closure, machine_id, obligation_id, place_id, unique_unit_machine,
    unsupported,
};
use crate::unit::attached_unit::{RuntimeRequirementOwner, UnitClosureRequest};

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
        CheckedUnitEffectOperationPlan::Complete {
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
        // The hook body is an ordinary checked Unit body; the edge invokes
        // whatever it carries. Selection stays pinned — this owner's attached
        // `T::drop` state, the recorded contract identity, no scalar
        // arguments, and at most the borrowed `self` receiver the edge lends.
        let borrowed_receiver = |parameter: &super::CheckedUnitStructuralParameterPlan| {
            parameter.is_self
                && matches!(
                    parameter.access,
                    super::CheckedStructuralAccess::MutableBorrow
                        | super::CheckedStructuralAccess::WriteOnlyBorrow
                )
                && parameter.type_identity == cleanup.type_identity
        };
        if target.state != cleanup.cleanup_state
            || target.contract_report_fingerprint != cleanup.cleanup_contract_report_fingerprint
            || contract.report_fingerprint != cleanup.cleanup_contract_report_fingerprint
            || target.attachment_type_identity.as_deref() != Some(cleanup.type_identity.as_str())
            || target
                .structural_parameters
                .iter()
                .any(|parameter| !borrowed_receiver(parameter))
            || !target.scalar_parameters.is_empty()
        {
            return unsupported("ordered nominal cleanup target is not exact and bounded");
        }
        if !roots.contains(&cleanup.cleanup_machine) {
            roots.push(cleanup.cleanup_machine);
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
    // The dispatcher plus each unique cleanup root anchors a transitive call
    // closure; ordinary machines reached by a hook body lower beside it.
    let closure = checked_unit_call_closure_including(&staged, plan.machine, &roots)?;
    let mut unit_roots = Vec::with_capacity(roots.len() + 1);
    unit_roots.push(plan.machine);
    unit_roots.extend_from_slice(&roots);
    let mut lowered = lower_unit_closure(
        &staged,
        &UnitClosureRequest {
            requirements_owner: RuntimeRequirementOwner::NominalCleanup,
            ..UnitClosureRequest::unit(plan.machine, &unit_roots)
        },
    )?
    .lowered;
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
                    StructuralTypeShape::Reference { .. }
                    | StructuralTypeShape::PrimitiveScalar(_)
                    | StructuralTypeShape::ByteSequence(_)
                    | StructuralTypeShape::ElementView { .. }
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
        // A hook that keeps a borrowed `self` receiver declares it as its
        // terminal `is_self` structural parameter; that parameter is the place
        // the edge lends and the proof root any requirement clause uses.
        let hook_receiver_place = {
            let hook_index = closure
                .iter()
                .position(|candidate| *candidate == cleanup.cleanup_machine)
                .ok_or(LoweringError::Unsupported(
                    "ordered nominal cleanup target is absent",
                ))?;
            lowered
                .semantic_module
                .machines
                .iter()
                .find(|machine| {
                    dense_identity(hook_index)
                        .map(machine_id)
                        .is_ok_and(|id| machine.id == id)
                })
                .and_then(|machine| {
                    machine
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.is_self)
                        .map(|parameter| parameter.place)
                })
        };
        let receiver = hook_receiver_place.or(if requirements.is_empty() {
            None
        } else {
            next_proof_root = next_proof_root
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup proof-root identity space is exhausted",
                ))?;
            Some(place_id(next_proof_root))
        });
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
            if !caller_clauses
                .iter()
                .any(|((caller_expected, root, caller_field), _)| {
                    caller_expected == expected
                        && *root == terminal_parameter.place
                        && caller_field == field
                })
            {
                return unsupported("contextual nominal cleanup caller requirement is absent");
            }
            next_obligation_identity =
                next_obligation_identity
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "contextual nominal cleanup obligation identity space is exhausted",
                    ))?;
            let obligation = obligation_id(next_obligation_identity);
            requirement_obligations.push(obligation);
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
        // The lowered hook body is ordinary terminal control; the signature
        // pins are what remain exact — owned by the consumed type, a `Unit`
        // result, no scalar arguments, at most the borrowed `self` receiver.
        let borrowed_receiver_declared =
            |parameter: &terminal_psi::StructuralParameterDeclaration| {
                parameter.is_self
                    && parameter.structural_type == cleanup.structural_type
                    && parameter.access != terminal_psi::StructuralAccess::Owned
            };
        if target.attachment != Some(cleanup.structural_type)
            || !target.parameters.is_empty()
            || target
                .structural_parameters
                .iter()
                .any(|parameter| !borrowed_receiver_declared(parameter))
            || target.result != TerminalMachineResult::Unit
            || !target.contract.crash_routes.is_empty()
            || target.contract.requires != expected_target_requires
        {
            return unsupported("ordered nominal cleanup target terminal machine is not exact");
        }
        // A declared borrowed receiver is the receiver the edge recorded —
        // contextual-only receivers stay proof-local.
        let declared_receiver = target
            .structural_parameters
            .first()
            .map(|parameter| parameter.place);
        let expected_receiver = target_contexts
            .iter()
            .find(|(target_symbol, _, _)| *target_symbol == checked_cleanup.cleanup_machine)
            .and_then(|(_, receiver, _)| *receiver);
        if declared_receiver.is_some() && declared_receiver != expected_receiver {
            return unsupported("ordered nominal cleanup receiver identity drifted");
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
    Ok(lowered)
}
