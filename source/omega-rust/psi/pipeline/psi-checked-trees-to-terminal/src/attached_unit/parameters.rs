//! Parameter, transfer, and structural-argument lowering for attached Unit
//! closures.

use super::*;

/// Rejoin direct Unit scalar parameters and routed-Service receipts to the
/// exact typed state signature before raw checked-to-Terminal lowering erases
/// their authored source partition. Owner-selected Fused provenance is
/// validated by the compiler's later custody gate; this layer independently
/// rejects missing, fabricated, or source-substituted checked custody.
pub(crate) fn validate_direct_unit_parameter_custody(
    checked: &CheckedTrees,
) -> Result<(), LoweringError> {
    let has_receipt = |parameters: &[psi_checked_trees::CheckedUnitStructuralParameterPlan]| {
        parameters
            .iter()
            .any(|parameter| parameter.fused_service_erasure.is_some())
    };
    let flow = &checked.facts.flow;
    let unsupported_receipt = flow
        .terminal_unit_effects
        .boundary_machines
        .iter()
        .any(|plan| has_receipt(&plan.structural_parameters))
        || flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .flat_map(|machine| &machine.states)
            .any(|state| has_receipt(&state.structural_parameters))
        || flow
            .terminal_partial_affine_unit_cleanups
            .machines
            .iter()
            .any(|plan| has_receipt(&plan.machine.structural_parameters))
        || flow
            .terminal_nominal_affine_unit_cleanups
            .machines
            .iter()
            .any(|plan| has_receipt(&plan.machine.structural_parameters))
        || flow
            .terminal_structural_unit_controls
            .machines
            .iter()
            .flat_map(|machine| &machine.states)
            .any(|state| has_receipt(&state.structural_parameters))
        || flow
            .terminal_structural_scalar_returns
            .machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters))
        || flow
            .terminal_structural_scalar_returns
            .selected_operator_machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters))
        || flow
            .terminal_structural_scalar_returns
            .trait_operator_machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters))
        || flow
            .terminal_boundary_scalar_returns
            .boundary_machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters))
        || flow
            .terminal_boundary_scalar_returns
            .machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters))
        || flow
            .terminal_structural_returns
            .machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters))
        || flow
            .terminal_structural_call_returns
            .machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters));
    if unsupported_receipt {
        return unsupported(
            "fused Service parameter receipt appears outside the first direct Unit-machine rung",
        );
    }

    for plan in &checked.facts.flow.terminal_unit_effects.machines {
        let carries_receipt = has_receipt(&plan.structural_parameters);
        let carries_parameter_custody = carries_receipt || !plan.scalar_parameters.is_empty();
        let Some(machine) = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == plan.machine)
        else {
            if carries_parameter_custody {
                return unsupported("direct Unit parameter plan has no exact typed machine");
            }
            continue;
        };
        let Some(state) = checked
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == plan.state)
        else {
            if carries_parameter_custody {
                return unsupported("direct Unit parameter plan has no exact typed state");
            }
            continue;
        };
        let source_parameters = checked.state_parameters(state);
        let expected_scalar_parameters = source_parameters
            .iter()
            .enumerate()
            .filter_map(|(position, source)| {
                checked
                    .primitive_type_reference(source.type_reference)
                    .map(|primitive_type| (position, source, primitive_type))
            })
            .map(|(position, source, primitive_type)| {
                if source.is_self || source.is_const || source.is_mutable {
                    return Err(LoweringError::Unsupported(
                        "direct Unit scalar parameter is not an immutable direct value",
                    ));
                }
                Ok(psi_checked_trees::CheckedStructuralScalarParameterPlan {
                    source_position: u32::try_from(position).map_err(|_| {
                        LoweringError::Unsupported("direct Unit scalar source position exceeds u32")
                    })?,
                    primitive_type,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        if plan.scalar_parameters != expected_scalar_parameters {
            return unsupported(
                "direct Unit scalar parameters do not rejoin the exact typed source partition",
            );
        }
        for (position, source) in source_parameters.iter().enumerate() {
            let carrier = checked
                .bound_service_parameter_carrier(source.type_reference)
                .map_err(|_| {
                    LoweringError::Unsupported(
                        "typed Unit parameter has an invalid routed Service carrier",
                    )
                })?;
            let matches = plan
                .structural_parameters
                .iter()
                .filter(|parameter| usize::try_from(parameter.position).ok() == Some(position))
                .collect::<Vec<_>>();
            let checked_parameter = match matches.as_slice() {
                [parameter] => Some(*parameter),
                [] => None,
                _ => {
                    return unsupported(
                        "Unit structural parameters duplicate an authored source position",
                    );
                }
            };
            let Some(carrier) = carrier else {
                if checked_parameter
                    .is_some_and(|parameter| parameter.fused_service_erasure.is_some())
                {
                    return unsupported(
                        "ordinary Unit parameter fabricates a fused Service erasure receipt",
                    );
                }
                continue;
            };
            let parameter = checked_parameter.ok_or(LoweringError::Unsupported(
                "typed routed Service parameter has no checked structural parameter",
            ))?;
            let receipt =
                parameter
                    .fused_service_erasure
                    .as_ref()
                    .ok_or(LoweringError::Unsupported(
                        "typed routed Service parameter lost its Fused erasure receipt",
                    ))?;
            if source.is_self
                || source.is_const
                || source.is_mutable
                || parameter.is_self
                || parameter.multiplicity != Multiplicity::Affine
                || parameter.access != psi_checked_trees::CheckedStructuralAccess::Owned
                || receipt.source_parameter != source.symbol
                || receipt.requirement != carrier.requirement
                || receipt.carrier_type_identity != carrier.carrier_type_identity
            {
                return unsupported(
                    "fused Service parameter receipt does not rejoin its exact owned affine typed source",
                );
            }
            let Some(authorization) = checked.fused_service_erasure(carrier.requirement) else {
                return unsupported(
                    "fused Service parameter lacks compiler-owned erasure authorization",
                );
            };
            if authorization.provider_plan_digest != receipt.provider_plan_digest {
                return unsupported("fused Service parameter substituted its provider-plan digest");
            }
            let base_identity = carrier.base_type_identity;
            let mut qualifications = carrier.qualifications;
            qualifications.sort_by_key(|domain| domain.0);
            qualifications.dedup();
            if parameter.type_identity != base_identity
                || parameter.qualifications != qualifications
                || qualifications.len() != 1
            {
                return unsupported(
                    "fused Service parameter substituted its base type or Bound qualification",
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn lower_unit_scalar_parameter_types(
    parameters: &[psi_checked_trees::CheckedStructuralScalarParameterPlan],
) -> Result<Vec<ScalarType>, LoweringError> {
    if parameters
        .windows(2)
        .any(|pair| pair[0].source_position >= pair[1].source_position)
    {
        return unsupported("Unit scalar parameters are not in strict source order");
    }
    parameters
        .iter()
        .map(|parameter| terminal_scalar_type(parameter.primitive_type))
        .collect()
}
pub(crate) fn lower_unit_parameters(
    parameters: &[psi_checked_trees::CheckedUnitStructuralParameterPlan],
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[(psi_language_semantics::SemanticDomainId, StructuralDomainId)],
    next_place: &mut u64,
) -> Result<Vec<StructuralParameterDeclaration>, LoweringError> {
    let mut positions = BTreeSet::new();
    parameters
        .iter()
        .enumerate()
        .map(|(dense_position, parameter)| {
            if !positions.insert(parameter.position) {
                return Err(LoweringError::Unsupported(
                    "Unit structural parameters contain duplicate source positions",
                ));
            }
            let mut qualifications = parameter
                .qualifications
                .iter()
                .map(|domain| lookup_domain_id(domain_ids, *domain))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            qualifications.sort();
            qualifications.dedup();
            if qualifications.len() != parameter.qualifications.len() {
                return Err(LoweringError::Unsupported(
                    "Unit structural parameter repeats a qualification",
                ));
            }
            if parameter.fused_service_erasure.is_some()
                && (parameter.is_self
                    || parameter.multiplicity != Multiplicity::Affine
                    || parameter.access != psi_checked_trees::CheckedStructuralAccess::Owned
                    || parameter.qualifications.len() != 1)
            {
                return Err(LoweringError::Unsupported(
                    "fused Service parameter has an invalid checked structural shape",
                ));
            }
            Ok(StructuralParameterDeclaration {
                place: place_id(allocate_dense(next_place)?),
                position: u32::try_from(dense_position).map_err(|_| {
                    LoweringError::Unsupported("Unit structural parameter count exceeds u32")
                })?,
                is_self: parameter.is_self,
                structural_type: lookup_type_id(type_ids, &parameter.type_identity)?,
                multiplicity: match parameter.multiplicity {
                    Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                    Multiplicity::Affine => StructuralMultiplicity::Affine,
                    Multiplicity::Linear => StructuralMultiplicity::Linear,
                },
                access: match parameter.access {
                    psi_checked_trees::CheckedStructuralAccess::Owned => StructuralAccess::Owned,
                    psi_checked_trees::CheckedStructuralAccess::SharedBorrow => {
                        StructuralAccess::SharedBorrow
                    }
                    psi_checked_trees::CheckedStructuralAccess::MutableBorrow => {
                        StructuralAccess::MutableBorrow
                    }
                    psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => {
                        StructuralAccess::WriteOnlyBorrow
                    }
                },
                qualifications,
                projected_qualifications: Vec::new(),
            })
        })
        .collect()
}

pub(crate) fn lower_contract_service_ceiling(
    rows: &psi_language_semantics::ServiceReachRowTable,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    if contract.checked_inferred != summary.transitive {
        return unsupported("Unit contract reach does not match checked transitive reach");
    }
    let source = match contract.interface {
        ServiceReachInterface::PublishedCeiling(row) => {
            require_valid_service_row(row)?;
            let ceiling = rows.services(row);
            if rows
                .services(summary.transitive)
                .iter()
                .any(|service| !ceiling.contains(service))
            {
                return unsupported("checked Unit service reach exceeds its published ceiling");
            }
            ceiling
        }
        ServiceReachInterface::InternalInferred => rows.services(summary.transitive),
    };
    let mut lowered = source
        .iter()
        .map(|service| lookup_service_id(service_ids, *service))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lowered.sort();
    lowered.dedup();
    if lowered.len() != source.len() {
        return unsupported("Unit published service ceiling contains duplicates");
    }
    Ok(lowered)
}

pub(crate) fn lower_published_service_ceiling(
    rows: &psi_language_semantics::ServiceReachRowTable,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    if matches!(contract.interface, ServiceReachInterface::InternalInferred) {
        return unsupported("public Unit contract has no published service ceiling");
    }
    lower_contract_service_ceiling(rows, contract, summary, service_ids)
}

pub(crate) fn lower_installation_machine_service_ceiling(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    if let Some(reach) = checked.facts.service_reaches.for_machine(machine)
        && matches!(contract.interface, ServiceReachInterface::InternalInferred)
        && !reach.unresolved_installation_reaches.is_empty()
    {
        if contract.checked_inferred != summary.transitive
            || reach.inferred_transitive != summary.transitive
        {
            return unsupported(
                "installation-bound machine reach disagrees with its checked transitive row",
            );
        }
        let source = checked
            .facts
            .service_reaches
            .rows
            .services(summary.transitive);
        let mut lowered = source
            .iter()
            .map(|service| lookup_service_id(service_ids, *service))
            .collect::<Result<Vec<_>, LoweringError>>()?;
        lowered.sort();
        lowered.dedup();
        if lowered.len() != source.len() {
            return unsupported("installation-bound service ceiling contains duplicates");
        }
        return Ok(lowered);
    }
    lower_contract_service_ceiling(
        &checked.facts.service_reaches.rows,
        contract,
        summary,
        service_ids,
    )
}

pub(crate) fn validate_transfer_shape(
    arguments: &[psi_checked_trees::CheckedUnitStructuralArgumentPlan],
    transfers: &[psi_checked_trees::CheckedUnitClaimTransferPlan],
    caller_parameters: &[StructuralParameterDeclaration],
    target_parameters: &[psi_checked_trees::CheckedUnitStructuralParameterPlan],
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
    expected_claim_arguments: &[u32],
) -> Result<(), LoweringError> {
    if arguments.len() != target_parameters.len() {
        return unsupported(
            "Unit call structural argument arity does not match its checked target",
        );
    }
    for (argument, target) in arguments.iter().zip(target_parameters) {
        if argument.byte_sequence_literal.is_some() {
            if argument.source_parameter_index != u32::MAX
                || !argument.path.is_empty()
                || argument.type_identity != target.type_identity
                || argument.access != target.access
                || target.multiplicity != Multiplicity::Unrestricted
                || !target.qualifications.is_empty()
            {
                return unsupported("byte-sequence literal argument has invalid checked custody");
            }
            let structural_type = lookup_type_id(type_ids, &argument.type_identity)?;
            if !structural_types.iter().any(|declaration| {
                declaration.id == structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
                    )
            }) {
                return unsupported("byte-sequence literal argument requires a borrowed-view type");
            }
            continue;
        }
        let source = caller_parameters
            .get(
                usize::try_from(argument.source_parameter_index).map_err(|_| {
                    LoweringError::Unsupported("Unit structural argument index exceeds usize")
                })?,
            )
            .ok_or(LoweringError::Unsupported(
                "Unit structural argument has an invalid caller parameter index",
            ))?;
        if argument.type_identity != target.type_identity
            || (argument.path.is_empty()
                && source.structural_type != lookup_type_id(type_ids, &argument.type_identity)?)
        {
            return unsupported("Unit structural argument type identity is inconsistent");
        }
        let source_access = match source.access {
            StructuralAccess::Owned => psi_checked_trees::CheckedStructuralAccess::Owned,
            StructuralAccess::SharedBorrow => {
                psi_checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            StructuralAccess::MutableBorrow => {
                psi_checked_trees::CheckedStructuralAccess::MutableBorrow
            }
            StructuralAccess::WriteOnlyBorrow => {
                psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
            }
        };
        if argument.access != target.access
            || !checked_access_can_supply(source_access, argument.access)
        {
            return unsupported("Unit structural argument access is inconsistent");
        }
    }
    let actual = transfers
        .iter()
        .map(|transfer| transfer.argument_index)
        .collect::<Vec<_>>();
    if actual != expected_claim_arguments
        || actual.iter().any(|index| {
            usize::try_from(*index)
                .ok()
                .map_or(true, |index| index >= arguments.len())
        })
    {
        return unsupported("Unit claim transfer does not exactly match target entry custody");
    }
    Ok(())
}

fn checked_access_can_supply(
    source: psi_checked_trees::CheckedStructuralAccess,
    presented: psi_checked_trees::CheckedStructuralAccess,
) -> bool {
    use psi_checked_trees::CheckedStructuralAccess;
    match source {
        CheckedStructuralAccess::Owned => true,
        CheckedStructuralAccess::SharedBorrow => presented == CheckedStructuralAccess::SharedBorrow,
        CheckedStructuralAccess::MutableBorrow => matches!(
            presented,
            CheckedStructuralAccess::SharedBorrow
                | CheckedStructuralAccess::MutableBorrow
                | CheckedStructuralAccess::WriteOnlyBorrow
        ),
        CheckedStructuralAccess::WriteOnlyBorrow => {
            presented == CheckedStructuralAccess::WriteOnlyBorrow
        }
    }
}

pub(crate) fn lower_structural_arguments(
    arguments: &[psi_checked_trees::CheckedUnitStructuralArgumentPlan],
    parameters: &[StructuralParameterDeclaration],
    literal_places: &[PlaceId],
) -> Result<Vec<StructuralArgument>, LoweringError> {
    let mut next_literal = 0usize;
    arguments
        .iter()
        .map(|argument| {
            if argument.byte_sequence_literal.is_some() {
                let place = *literal_places
                    .get(next_literal)
                    .ok_or(LoweringError::Unsupported(
                        "byte-sequence literal place is absent",
                    ))?;
                next_literal += 1;
                return Ok(StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: match argument.access {
                        psi_checked_trees::CheckedStructuralAccess::Owned => {
                            StructuralAccess::Owned
                        }
                        psi_checked_trees::CheckedStructuralAccess::SharedBorrow => {
                            StructuralAccess::SharedBorrow
                        }
                        psi_checked_trees::CheckedStructuralAccess::MutableBorrow => {
                            StructuralAccess::MutableBorrow
                        }
                        psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => {
                            StructuralAccess::WriteOnlyBorrow
                        }
                    },
                });
            }
            let parameter = parameters
                .get(
                    usize::try_from(argument.source_parameter_index).map_err(|_| {
                        LoweringError::Unsupported("Unit structural argument index exceeds usize")
                    })?,
                )
                .ok_or(LoweringError::Unsupported(
                    "Unit structural argument has an invalid caller parameter index",
                ))?;
            Ok(StructuralArgument {
                place: parameter.place,
                path: lower_structural_path(&argument.path),
                access: match argument.access {
                    psi_checked_trees::CheckedStructuralAccess::Owned => StructuralAccess::Owned,
                    psi_checked_trees::CheckedStructuralAccess::SharedBorrow => {
                        StructuralAccess::SharedBorrow
                    }
                    psi_checked_trees::CheckedStructuralAccess::MutableBorrow => {
                        StructuralAccess::MutableBorrow
                    }
                    psi_checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => {
                        StructuralAccess::WriteOnlyBorrow
                    }
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(|lowered| {
            if next_literal == literal_places.len() {
                Ok(lowered)
            } else {
                unsupported("byte-sequence literal place count disagrees with arguments")
            }
        })
}

pub(crate) fn lower_structural_path(
    path: &[CheckedUnitStructuralPathSegment],
) -> Vec<StructuralPathSegment> {
    path.iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::Field(identity) => {
                StructuralPathSegment::Field(identity.clone())
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                StructuralPathSegment::FixedIndex(*index)
            }
        })
        .collect()
}
