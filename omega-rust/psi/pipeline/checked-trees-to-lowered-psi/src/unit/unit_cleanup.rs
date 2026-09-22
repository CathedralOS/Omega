//! Structural Unit cleanup lowering.
//!
//! The nominal entry point retains family precedence. Ordered nominal cleanup
//! and partial-affine cleanup live in separate responsibility modules.
//! Build cleanup requirements and their obligation identities here, but leave
//! certificates to final operation proof emission. Owned-field requirements are
//! validity-scoped observations, not permanent assumption slots; their available
//! premises are known only after the complete caller and cleanup edge exist.

use super::{
    BTreeSet, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedTrees, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape, LoweredPsi, LoweringError,
    MachineId, Multiplicity, NominalAffineCleanup, PlaceId, PrimitiveType, Proposition, ScalarTerm,
    ScalarType, ServiceReachInterface, ServiceReachPlan, ServiceReachSummary, StructuralFieldType,
    StructuralMultiplicity, StructuralTypeId, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, Terminator, checked_unit_call_closure_including, dense_identity,
    lookup_machine_id, lookup_type_id, lower_unit_closure, machine_id, obligation_id, place_id,
    unique_unit_machine, unsupported,
};
use crate::unit::attached_unit::bodies::UnitPlans;
use crate::unit::attached_unit::{RuntimeRequirementOwner, UnitClosureRequest};
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralParameterPlan};
use symbols::SymbolHandle;
use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};
mod ordered;
mod partial;
use ordered::lower_ordered_nominal_affine_unit_cleanup_machine;
pub(crate) use partial::{
    checked_partial_affine_residuals, validate_anonymous_partial_permissions,
};

pub(crate) fn lower_nominal_affine_unit_cleanup_machine(
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
    // A free consuming machine (a `drop<T>` specialization) is seeded into the
    // ordinary roster so callers can resolve it; only an attached entry doubles
    // as a trivial-lane body.
    if plan.attachment_type_identity.is_some()
        && checked
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
        CheckedUnitEffectOperationPlan::Complete {
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
    if let Some(attachment_type_identity) = plan.attachment_type_identity.as_deref() {
        let attachment_shape = nominal_types
            .iter()
            .find(|candidate| candidate.identity == attachment_type_identity)
            .ok_or(LoweringError::Unsupported(
                "nominal affine Unit attachment type is absent from its checked shapes",
            ))?;
        if !matches!(
            &attachment_shape.shape,
            CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
        ) {
            return unsupported("nominal affine Unit attachment is not an empty record");
        }
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

    let published = UnitPlans::published(&checked.facts.flow.terminal_unit_effects);
    let cleanup_target = unique_unit_machine(published, cleanup.cleanup_machine)?;
    let cleanup_contract = checked
        .facts
        .contract_plans
        .for_machine(cleanup.cleanup_machine)
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup target is missing its checked contract identity",
        ))?;
    // The hook body is an ordinary checked Unit body: the edge invokes whatever
    // its plan carries. Only the selection stays pinned — the plan must be this
    // owner's attached `T::drop` state, its contract identity must be the one
    // the edge recorded, its scalar signature must be empty, and its structural
    // signature carries at most the borrowed `self` receiver the edge lends.
    let borrowed_receiver = |parameter: &CheckedUnitStructuralParameterPlan| {
        parameter.is_self
            && matches!(
                parameter.access,
                CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
            )
            && parameter.type_identity == cleanup.type_identity
    };
    if cleanup_target.state != cleanup.cleanup_state
        || cleanup_target.contract_report_fingerprint != cleanup.cleanup_contract_report_fingerprint
        || cleanup_contract.report_fingerprint != cleanup.cleanup_contract_report_fingerprint
        || cleanup_target.attachment_type_identity.as_deref()
            != Some(cleanup.type_identity.as_str())
        || cleanup_target
            .structural_parameters
            .iter()
            .any(|parameter| !borrowed_receiver(parameter))
        || !cleanup_target.scalar_parameters.is_empty()
    {
        return unsupported("nominal cleanup target identity or bounded signature drifted");
    }

    // The nominal lane's shapes join the ordinary roster for this closure;
    // a shape both lanes publish must agree.
    for shape in nominal_types {
        if published
            .structural_type(&shape.identity)
            .is_some_and(|existing| existing != shape)
        {
            return unsupported(
                "nominal affine Unit structural type conflicts with its cleanup closure",
            );
        }
    }
    // A free consuming machine is already seeded into the ordinary roster;
    // an attached entry is read from the nominal lane as the closure's entry.
    let plans = if published.for_machine(plan.machine).is_some() {
        UnitPlans::with_staged_structural_types(
            &checked.facts.flow.terminal_unit_effects,
            nominal_types,
        )
    } else {
        UnitPlans::with_staged(
            &checked.facts.flow.terminal_unit_effects,
            plan,
            nominal_types,
        )
    };
    // Cleanup is an explicit additional closure root because it is executable
    // edge work, not a source-authored ordinary call operation. The closure
    // root pair anchors a transitive call closure: whatever ordinary machines
    // the hook body reaches are lowered beside it.
    let closure = checked_unit_call_closure_including(
        checked,
        plans,
        plan.machine,
        &[cleanup.cleanup_machine],
    )?;
    let cleanup_machine_index = closure
        .iter()
        .position(|candidate| *candidate == cleanup.cleanup_machine)
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup target is absent from its checked closure",
        ))?;
    let cleanup_terminal_id = machine_id(dense_identity(cleanup_machine_index)?);
    let mut lowered = lower_unit_closure(
        checked,
        &UnitClosureRequest {
            plans,
            entry: plan.machine,
            unit_roots: &[plan.machine, cleanup.cleanup_machine],
            external: None,
            requirements_owner: RuntimeRequirementOwner::NominalCleanup,
            scalar_entry: false,
        },
    )?
    .lowered;
    let type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let cleanup_type = lookup_type_id(&type_ids, &cleanup.type_identity)?;

    // A hook that touches `self` keeps its borrowed `&mut self` structural
    // parameter in the terminal signature; the edge lends the consumed place
    // to that parameter, and it doubles as the proof root any contextual
    // requirement clause roots at.
    let hook_receiver_place = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanup_terminal_id)
        .and_then(|machine| {
            machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.is_self)
                .map(|parameter| parameter.place)
        });

    let (cleanup_receiver, requirement_obligations, target_requires, caller_requires) =
        if contextual_caller_requirements.is_empty() {
            (hook_receiver_place, Vec::new(), Vec::new(), Vec::new())
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
            let receiver = hook_receiver_place.or(if contextual_requirements.is_empty() {
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
            });
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
                    StructuralTypeShape::Reference { .. }
                    | StructuralTypeShape::PrimitiveScalar(_)
                    | StructuralTypeShape::ByteSequence(_)
                    | StructuralTypeShape::ElementView { .. }
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
            for (obligation_index, (expected, field, target_requirement)) in
                target_clauses.into_iter().enumerate()
            {
                let identity = u64::try_from(obligation_index)
                    .ok()
                    .and_then(|index| index.checked_add(1))
                    .ok_or(LoweringError::Unsupported(
                        "contextual nominal cleanup obligation identity space is exhausted",
                    ))?;
                if !caller_clauses
                    .iter()
                    .any(|(caller_expected, caller_field, _)| {
                        *caller_expected == expected && *caller_field == field
                    })
                {
                    return unsupported("contextual nominal cleanup caller requirement is absent");
                }
                let obligation = obligation_id(identity);
                requirement_obligations.push(obligation);
                target_requires.push(target_requirement);
            }
            (
                receiver,
                requirement_obligations,
                target_requires,
                caller_requires,
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
    // The lowered hook body is ordinary terminal control: whatever statements
    // it lowered to is what the edge executes. The signature keeps its exact
    // pins — owned by the consumed type, a `Unit` result, no scalar arguments,
    // and at most the borrowed `self` receiver the edge lends.
    let borrowed_receiver_declared = |parameter: &StructuralParameterDeclaration| {
        parameter.is_self
            && parameter.structural_type == cleanup_type
            && parameter.access != StructuralAccess::Owned
    };
    if cleanup_terminal.attachment != Some(cleanup_type)
        || !cleanup_terminal.parameters.is_empty()
        || cleanup_terminal
            .structural_parameters
            .iter()
            .any(|parameter| !borrowed_receiver_declared(parameter))
        || cleanup_terminal.result != TerminalMachineResult::Unit
        || !cleanup_terminal.contract.crash_routes.is_empty()
        || cleanup_terminal.contract.requires != target_requires
    {
        return unsupported("nominal cleanup target terminal machine is not exact and bounded");
    }
    if cleanup_terminal
        .structural_parameters
        .first()
        .map(|parameter| parameter.place)
        != hook_receiver_place
    {
        return unsupported("nominal cleanup receiver parameter identity drifted");
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
    let expected_attachment = plan
        .attachment_type_identity
        .as_deref()
        .map(|identity| lookup_type_id(&type_ids, identity))
        .transpose()?;
    if entry.attachment != expected_attachment
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
    Ok(lowered)
}

/// Rewrite an emitted nominal consuming member inside an ordinary closure.
/// The seeded `drop<T>` specialization carries the ordinary complete-only
/// body, and its return edge installs the exact owner-attached `::drop` hook
/// here, matching the entry path's `ReturnUnitNominalAffine` patch. Hooks
/// carrying contextual requirements stay on the isolated entry lane, where
/// caller obligations own a dedicated contract namespace.
pub(super) fn patch_nominal_cleanup_member(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
    machine: &mut TerminalMachine,
    type_ids: &[(String, StructuralTypeId)],
    machine_ids: &[(SymbolHandle, MachineId)],
    cleanup_receivers: &std::collections::BTreeMap<MachineId, PlaceId>,
) -> Result<(), LoweringError> {
    let plan = &nominal.machine;
    if plan.attachment_type_identity.is_some()
        || !nominal.caller_requirements.is_empty()
        || nominal
            .cleanups
            .iter()
            .any(|cleanup| !cleanup.requirements.is_empty())
    {
        return unsupported("member nominal affine Unit cleanup requires the isolated entry lane");
    }
    if machine.attachment.is_some() {
        return unsupported("nominal affine Unit member is unexpectedly attached");
    }
    let mut cleanups = Vec::with_capacity(nominal.cleanups.len());
    for cleanup in &nominal.cleanups {
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.position == cleanup.source_parameter_index)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup member parameter is absent from its terminal signature",
            ))?;
        let cleanup_target = unique_unit_machine(
            UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
            cleanup.cleanup_machine,
        )?;
        if cleanup_target.state != cleanup.cleanup_state
            || cleanup_target.contract_report_fingerprint
                != cleanup.cleanup_contract_report_fingerprint
            || cleanup_target.attachment_type_identity.as_deref()
                != Some(cleanup.type_identity.as_str())
            || lookup_type_id(type_ids, &cleanup.type_identity)? != parameter.structural_type
        {
            return unsupported("nominal cleanup member target identity drifted");
        }
        let hook_machine = lookup_machine_id(machine_ids, cleanup.cleanup_machine)?;
        cleanups.push(NominalAffineCleanup {
            place: parameter.place,
            structural_type: parameter.structural_type,
            cleanup_machine: hook_machine,
            cleanup_receiver: cleanup_receivers.get(&hook_machine).copied(),
            requirement_obligations: Vec::new(),
        });
    }
    let entry_block = machine.entry;
    let [block] = machine.blocks.as_mut_slice() else {
        return unsupported("nominal affine Unit member terminal control drifted");
    };
    if !matches!(
        &block.terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ) || block.id != entry_block
        || !block.parameters.is_empty()
        || !block.operations.is_empty()
    {
        return unsupported("nominal affine Unit member body or return drifted");
    }
    let edge = block.terminator.edge();
    block.terminator = Terminator::ReturnUnitNominalAffine { edge, cleanups };
    Ok(())
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
        CheckedUnitStructuralTypeShape::Reference { .. }
        | CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
        | CheckedUnitStructuralTypeShape::ByteSequence(_)
        | CheckedUnitStructuralTypeShape::FixedArray { .. }
        | CheckedUnitStructuralTypeShape::BorrowedSliceView { .. }
        | CheckedUnitStructuralTypeShape::Sum { .. }
        | CheckedUnitStructuralTypeShape::Mixed { .. } => false,
    }
}

pub(crate) fn lower_partial_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    partial: &CheckedPartialAffineUnitCleanupMachinePlan,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    partial::lower_partial_affine_unit_cleanup_machine(checked, partial)
}
