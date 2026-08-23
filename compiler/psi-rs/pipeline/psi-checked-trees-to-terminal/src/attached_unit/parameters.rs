//! Parameter, transfer, and structural-argument lowering for attached Unit
//! closures.

use super::*;

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
                qualifications,
            })
        })
        .collect()
}

pub(crate) fn lower_published_service_ceiling(
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
            rows.services(row)
        }
        ServiceReachInterface::InternalInferred if rows.services(summary.transitive).is_empty() => {
            &[]
        }
        ServiceReachInterface::InternalInferred => {
            return unsupported("effectful Unit machine has no published service ceiling");
        }
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
    lower_published_service_ceiling(
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
