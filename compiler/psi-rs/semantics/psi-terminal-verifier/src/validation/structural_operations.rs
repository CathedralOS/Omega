//! Validates structural, boundary, and effect operation custody.

use super::*;

pub(super) fn validate_unit_operation_static(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &psi_terminal::Operation,
) -> Result<(), ModuleError> {
    match &operation.kind {
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            let callee = machines
                .get(callee)
                .copied()
                .ok_or(ModuleError::UnknownCallTarget {
                    operation: operation.id,
                    callee: *callee,
                })?;
            if callee.result != TerminalMachineResult::Unit || !callee.parameters.is_empty() {
                return Err(ModuleError::UnitCallTargetHasScalarSignature {
                    operation: operation.id,
                    callee: callee.id,
                });
            }
            if structural_arguments.iter().any(|argument| {
                !argument.path.is_empty()
                    && !matches!(
                        argument.path.as_slice(),
                        [StructuralPathSegment::FixedIndex(_)]
                    )
                    && !is_nonempty_field_path(&argument.path)
            }) {
                return Err(ModuleError::InvalidStructuralArgumentPath {
                    operation: operation.id,
                    argument_index: structural_arguments
                        .iter()
                        .position(|argument| {
                            !argument.path.is_empty()
                                && !matches!(
                                    argument.path.as_slice(),
                                    [StructuralPathSegment::FixedIndex(_)]
                                )
                                && !is_nonempty_field_path(&argument.path)
                        })
                        .unwrap_or_default() as u32,
                });
            }
            let projected = structural_arguments
                .iter()
                .any(|argument| !argument.path.is_empty());
            if projected
                && (machine.result != TerminalMachineResult::Unit
                    || !machine.parameters.is_empty()
                    || machine.structural_parameters.len() != 1
                    || structural_arguments.len() != 1
                    || callee.structural_parameters.len() != 1)
            {
                return Err(ModuleError::ProjectedUnitCallOutsideBoundedSlice {
                    operation: operation.id,
                });
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                operation.id,
                true,
            )?;
            if let Some((argument_index, _)) = structural_arguments
                .iter()
                .zip(&callee.structural_parameters)
                .enumerate()
                .find(|(_, (argument, expected))| {
                    !argument.path.is_empty()
                        && (!expected.qualifications.is_empty()
                            || machine
                                .structural_parameters
                                .iter()
                                .find(|actual| actual.place == argument.place)
                                .is_some_and(|actual| !actual.qualifications.is_empty()))
                })
            {
                return Err(ModuleError::InvalidStructuralArgumentPath {
                    operation: operation.id,
                    argument_index: argument_index as u32,
                });
            }
            validate_unit_call_contract_places(callee, operation.id)?;
            if projected {
                let projected_parameter = callee.structural_parameters[0].place;
                if unit_call_contract_propositions(callee).any(|proposition| {
                    propositions::proposition_content_roots(proposition)
                        .contains(&projected_parameter)
                }) {
                    return Err(
                        ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
                            operation: operation.id,
                            callee: callee.id,
                            place: projected_parameter,
                        },
                    );
                }
            }
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &callee.published_service_ceiling,
            )?;
            if requirement_obligations.len() != callee.contract.requires.len() {
                return Err(ModuleError::CallRequirementArityMismatch {
                    operation: operation.id,
                    expected: callee.contract.requires.len(),
                    actual: requirement_obligations.len(),
                });
            }
            validate_unit_call_claim_transfers(
                machine,
                callee,
                structural_arguments,
                claim_transfers,
                operation.id,
            )?;
            validate_unit_call_crash_continuations(
                module,
                machine,
                callee,
                structural_arguments,
                crash_continuations,
                operation.id,
            )?;
        }
        OperationKind::CallStructuralScalar {
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            let callee = machines
                .get(callee)
                .copied()
                .ok_or(ModuleError::UnknownCallTarget {
                    operation: operation.id,
                    callee: *callee,
                })?;
            let expected = callee.result.scalar().map(|result| result.scalar_type);
            let actual = operation.result.scalar().map(|result| result.scalar_type);
            if !callee.parameters.is_empty() || expected.is_none() || actual != expected {
                return Err(ModuleError::StructuralScalarCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                    expected,
                    actual,
                });
            }
            if let Some(argument_index) = structural_arguments
                .iter()
                .position(|argument| !argument.path.is_empty())
            {
                return Err(ModuleError::InvalidStructuralArgumentPath {
                    operation: operation.id,
                    argument_index: argument_index as u32,
                });
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                operation.id,
                true,
            )?;
            validate_unit_call_contract_places(callee, operation.id)?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &callee.published_service_ceiling,
            )?;
            if requirement_obligations.len() != callee.contract.requires.len() {
                return Err(ModuleError::CallRequirementArityMismatch {
                    operation: operation.id,
                    expected: callee.contract.requires.len(),
                    actual: requirement_obligations.len(),
                });
            }
            validate_unit_call_claim_transfers(
                machine,
                callee,
                structural_arguments,
                claim_transfers,
                operation.id,
            )?;
            validate_unit_call_crash_continuations(
                module,
                machine,
                callee,
                structural_arguments,
                crash_continuations,
                operation.id,
            )?;
        }
        OperationKind::BoundaryCall {
            boundary,
            arguments: _,
            structural_arguments,
            completion_receipts,
            requirement_obligations,
        } => {
            let boundary = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .ok_or(ModuleError::UnknownBoundaryCallTarget {
                    operation: operation.id,
                    boundary: *boundary,
                })?;
            if !requirement_obligations.is_empty() {
                return Err(ModuleError::BoundaryStructuralRequirementsMintObligations(
                    operation.id,
                ));
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &boundary.structural_parameters,
                operation.id,
                true,
            )?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &boundary.published_service_ceiling,
            )?;
            validate_boundary_requirements(machine, boundary, structural_arguments, operation.id)?;
            validate_boundary_completion_receipts(
                machine,
                structural_arguments,
                completion_receipts,
                operation.id,
            )?;
        }
        OperationKind::PortWrite { service, .. } => {
            if !module
                .services
                .iter()
                .any(|candidate| candidate.id == *service)
            {
                return Err(ModuleError::UnknownOperationService {
                    operation: operation.id,
                    service: *service,
                });
            }
            if !machine.published_service_ceiling.contains(service) {
                return Err(ModuleError::OperationServiceOutsidePublishedCeiling {
                    operation: operation.id,
                    service: *service,
                });
            }
        }
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            let Some(place) = machine
                .structural_places
                .iter()
                .find(|place| place.id == *destination)
            else {
                return Err(ModuleError::UnknownTrivialAffineLocal {
                    operation: operation.id,
                    place: *destination,
                });
            };
            let StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } = place.kind
            else {
                return Err(ModuleError::UnknownTrivialAffineLocal {
                    operation: operation.id,
                    place: *destination,
                });
            };
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
            else {
                return Err(ModuleError::UnknownStructuralType(structural_type));
            };
            if !matches!(declaration.shape, StructuralTypeShape::Record { ref fields } if fields.is_empty())
            {
                return Err(ModuleError::TrivialAffineLocalRequiresEmptyRecord {
                    operation: operation.id,
                    place: *destination,
                });
            }
        }
        _ => unreachable!("caller selects only structural/effect operations"),
    }
    Ok(())
}

/// Validate the complete bounded representation for a nonempty run of
/// pairwise-disjoint field transfers, followed by disposal of every maximal
/// residual sibling subtree in recursive reverse declaration order. This
/// partition is checked independently of producer facts before the ownership
/// walk relies on the path-sensitive terminator.

fn validate_unit_call_contract_places(
    callee: &TerminalMachine,
    operation: OperationId,
) -> Result<(), ModuleError> {
    let parameters = callee
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    for proposition in unit_call_contract_propositions(callee) {
        if let Some(place) = propositions::proposition_content_roots(proposition)
            .into_iter()
            .find(|place| !parameters.contains(place))
        {
            return Err(ModuleError::UnitCallContractPlaceHasNoArgument {
                operation,
                callee: callee.id,
                place,
            });
        }
    }
    Ok(())
}

fn unit_call_contract_propositions(callee: &TerminalMachine) -> impl Iterator<Item = &Proposition> {
    callee
        .contract
        .requires
        .iter()
        .chain(
            callee
                .contract
                .ensures
                .iter()
                .map(|clause| &clause.proposition),
        )
        .chain(
            callee
                .contract
                .crash_routes
                .iter()
                .flat_map(|bucket| &bucket.alternatives)
                .filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => None,
                    CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
                }),
        )
}

fn validate_structural_arguments(
    module: &TerminalModule,
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
    operation: OperationId,
    allow_projected: bool,
) -> Result<(), ModuleError> {
    if arguments.len() != expected.len() {
        return Err(ModuleError::StructuralArgumentArityMismatch {
            operation,
            expected: expected.len(),
            actual: arguments.len(),
        });
    }
    for (index, (argument, expected)) in arguments.iter().zip(expected).enumerate() {
        let Some(actual) = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
        else {
            return Err(ModuleError::UnknownStructuralArgument {
                operation,
                argument_index: index as u32,
                place: argument.place,
            });
        };
        if !allow_projected && !argument.path.is_empty() {
            return Err(ModuleError::InvalidStructuralArgumentPath {
                operation,
                argument_index: index as u32,
            });
        }
        let Some(actual_type) =
            resolve_structural_path(module, actual.structural_type, &argument.path)
        else {
            return Err(ModuleError::InvalidStructuralArgumentPath {
                operation,
                argument_index: index as u32,
            });
        };
        if actual_type != expected.structural_type {
            return Err(ModuleError::StructuralArgumentTypeMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.structural_type,
                actual: actual_type,
            });
        }
        let actual_multiplicity = if argument.path.is_empty() {
            actual.multiplicity
        } else if expected.multiplicity == StructuralMultiplicity::Affine
            && is_nonempty_field_path(&argument.path)
            && actual.multiplicity == StructuralMultiplicity::Affine
        {
            StructuralMultiplicity::Affine
        } else {
            StructuralMultiplicity::Linear
        };
        if actual_multiplicity != expected.multiplicity {
            return Err(ModuleError::StructuralArgumentMultiplicityMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.multiplicity,
                actual: actual_multiplicity,
            });
        }
        for qualification in &expected.qualifications {
            if !argument.path.is_empty() || !actual.qualifications.contains(qualification) {
                return Err(ModuleError::StructuralArgumentMissingQualification {
                    operation,
                    argument_index: index as u32,
                    domain: *qualification,
                });
            }
        }
    }
    Ok(())
}

pub(super) fn validate_service_reach(
    operation: OperationId,
    caller: &[ServiceId],
    reached: &[ServiceId],
) -> Result<(), ModuleError> {
    if let Some(service) = reached.iter().find(|service| !caller.contains(service)) {
        return Err(ModuleError::OperationServiceOutsidePublishedCeiling {
            operation,
            service: *service,
        });
    }
    Ok(())
}

fn validate_unit_call_claim_transfers(
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    transfers: &[ClaimTransfer],
    operation: OperationId,
) -> Result<(), ModuleError> {
    for (argument_index, (argument, parameter)) in arguments
        .iter()
        .zip(&callee.structural_parameters)
        .enumerate()
    {
        if !argument.path.is_empty() {
            let callee_claims = callee
                .entry_claims
                .iter()
                .filter(|claim| claim.input == parameter.place)
                .collect::<Vec<_>>();
            let claim_free_direct_affine = is_nonempty_field_path(&argument.path)
                && parameter.multiplicity == StructuralMultiplicity::Affine
                && callee_claims.is_empty()
                && caller
                    .entry_claims
                    .iter()
                    .all(|claim| claim.input != argument.place);
            if !claim_free_direct_affine
                && !matches!(callee_claims.as_slice(), [claim] if claim.path.is_empty())
            {
                return Err(ModuleError::UnitCallClaimPresenceMismatch {
                    operation,
                    argument_index: argument_index as u32,
                });
            }
            if caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
                || callee
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place)
            {
                return Err(ModuleError::UnitCallContentClaimMismatch {
                    operation,
                    argument_index: argument_index as u32,
                });
            }
        }
        let mut caller_claim_paths = caller
            .entry_claims
            .iter()
            .filter(|claim| claim.input == argument.place && claim.path.starts_with(&argument.path))
            .map(|claim| &claim.path[argument.path.len()..])
            .collect::<Vec<_>>();
        let mut callee_claim_paths = callee
            .entry_claims
            .iter()
            .filter(|claim| claim.input == parameter.place)
            .map(|claim| claim.path.as_slice())
            .collect::<Vec<_>>();
        caller_claim_paths.sort();
        callee_claim_paths.sort();
        if caller_claim_paths != callee_claim_paths {
            return Err(ModuleError::UnitCallClaimPresenceMismatch {
                operation,
                argument_index: argument_index as u32,
            });
        }
        let mut caller_content = caller
            .content_entry_claims
            .iter()
            .filter(|binding| binding.input.root == argument.place)
            .map(|binding| (&binding.input.segments, &binding.projections))
            .collect::<Vec<_>>();
        let mut callee_content = callee
            .content_entry_claims
            .iter()
            .filter(|binding| binding.input.root == parameter.place)
            .map(|binding| (&binding.input.segments, &binding.projections))
            .collect::<Vec<_>>();
        caller_content.sort();
        callee_content.sort();
        if caller_content != callee_content {
            return Err(ModuleError::UnitCallContentClaimMismatch {
                operation,
                argument_index: argument_index as u32,
            });
        }
    }
    let callee_claims = callee
        .entry_claims
        .iter()
        .map(|claim| (claim.claim, claim.input))
        .chain(
            callee
                .content_entry_claims
                .iter()
                .map(|claim| (claim.claim, claim.input.root)),
        )
        .collect::<BTreeMap<_, _>>();
    for (claim, input) in &callee_claims {
        if !callee
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == *input)
        {
            return Err(ModuleError::UnitCallClaimHasNoStructuralArgument {
                operation,
                claim: *claim,
            });
        }
    }
    if transfers.len() != callee_claims.len() {
        return Err(ModuleError::UnitCallClaimTransferCountMismatch {
            operation,
            expected: callee_claims.len(),
            actual: transfers.len(),
        });
    }
    let mut caller_claims = BTreeSet::new();
    for transfer in transfers {
        if !caller_claims.insert(transfer.claim) {
            return Err(ModuleError::DuplicateUnitCallClaimTransfer(operation));
        }
        let Some(argument) = arguments.get(transfer.argument_index as usize) else {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: transfer.argument_index,
            });
        };
        let Some((claim_input, claim_path)) = claim_input(caller, transfer.claim) else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: transfer.claim,
            });
        };
        let target_place = callee
            .structural_parameters
            .get(transfer.argument_index as usize)
            .map(|parameter| parameter.place);
        let structural_path_matches = claim_path.starts_with(&argument.path)
            && callee.entry_claims.iter().any(|claim| {
                Some(claim.input) == target_place && claim.path == claim_path[argument.path.len()..]
            });
        let content_matches = argument.path.is_empty()
            && caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.claim == transfer.claim && claim.input.root == argument.place)
            && callee
                .content_entry_claims
                .iter()
                .any(|claim| Some(claim.input.root) == target_place);
        if claim_input != argument.place || (!structural_path_matches && !content_matches) {
            return Err(ModuleError::ClaimActionPlaceMismatch {
                operation,
                claim: transfer.claim,
                argument_index: transfer.argument_index,
            });
        }
    }
    for input in callee_claims.into_values() {
        let argument_index = callee
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == input)
            .expect("callee entry claims were validated against its signature")
            as u32;
        if !transfers
            .iter()
            .any(|transfer| transfer.argument_index == argument_index)
        {
            return Err(ModuleError::MissingUnitCallClaimTransfer {
                operation,
                argument_index,
            });
        }
    }
    if transfers.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalUnitCallClaimTransfers(operation));
    }
    Ok(())
}

fn claim_input(
    machine: &TerminalMachine,
    claim: ClaimId,
) -> Option<(PlaceId, &[StructuralPathSegment])> {
    machine
        .entry_claims
        .iter()
        .find_map(|candidate| {
            (candidate.claim == claim).then_some((candidate.input, candidate.path.as_slice()))
        })
        .or_else(|| {
            machine.content_entry_claims.iter().find_map(|candidate| {
                (candidate.claim == claim)
                    .then_some((candidate.input.root, &[] as &[StructuralPathSegment]))
            })
        })
}

fn validate_unit_call_crash_continuations(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    continuations: &[CrashRouteBucket],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let boolean_roots = callee
        .contract
        .crash_routes
        .iter()
        .flat_map(|bucket| &bucket.alternatives)
        .filter_map(|guard| match guard {
            CrashRouteGuard::Truth => None,
            CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
        })
        .flat_map(propositions::proposition_boolean_field_roots)
        .collect::<BTreeSet<_>>();
    let substitutions = callee
        .structural_parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            let prefix = structural_argument_canonical_prefix(module, caller, argument);
            if prefix.is_none() && boolean_roots.contains(&parameter.place) {
                return Err(
                    ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
                        operation,
                        callee: callee.id,
                        place: parameter.place,
                    },
                );
            }
            Ok((
                parameter.place,
                (argument.place, prefix.unwrap_or_default()),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let expected = substitute_crash_route_places(&callee.contract.crash_routes, &substitutions);
    if continuations != expected {
        return Err(ModuleError::CallCrashContinuationsMismatch {
            operation,
            callee: callee.id,
        });
    }
    for continuation in continuations {
        let covered = caller.contract.crash_routes.iter().any(|published| {
            published.cause == continuation.cause
                && (published.alternatives == [CrashRouteGuard::Truth]
                    || continuation
                        .alternatives
                        .iter()
                        .all(|route| published.alternatives.contains(route)))
        });
        if !covered {
            return Err(ModuleError::CallCrashContinuationUncovered {
                operation,
                cause: continuation.cause,
            });
        }
    }
    Ok(())
}

fn substitute_crash_route_places(
    routes: &[CrashRouteBucket],
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .map(|guard| match guard {
                    CrashRouteGuard::Truth => CrashRouteGuard::Truth,
                    CrashRouteGuard::Predicate(predicate) => CrashRouteGuard::Predicate(
                        CrashPredicateTerm::new(substitute_proposition_structural_places(
                            predicate.proposition(),
                            substitutions,
                        )),
                    ),
                })
                .collect::<Vec<_>>();
            alternatives.sort();
            alternatives.dedup();
            if alternatives.contains(&CrashRouteGuard::Truth) {
                alternatives = vec![CrashRouteGuard::Truth];
            }
            CrashRouteBucket {
                cause: bucket.cause,
                alternatives,
            }
        })
        .collect()
}

pub(crate) fn structural_argument_canonical_prefix(
    module: &TerminalModule,
    caller: &TerminalMachine,
    argument: &StructuralArgument,
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let mut structural_type = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)?
        .structural_type;
    let mut prefix = Vec::with_capacity(argument.path.len());
    for segment in &argument.path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                let field = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
                    .and_then(|declaration| match &declaration.shape {
                        StructuralTypeShape::Record { fields } => fields.iter().find(|field| {
                            field.identity == *identity && !field.relevance.is_erased()
                        }),
                        StructuralTypeShape::FixedArray { .. }
                        | StructuralTypeShape::Sum { .. } => None,
                    })?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                prefix.push(CanonicalStructuralPathSegment::Field(field.id));
                structural_type = next;
            }
            StructuralPathSegment::FixedIndex(index) => {
                let element = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::FixedArray { element, length } if *index < length => {
                            Some(element)
                        }
                        _ => None,
                    })?;
                prefix.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                structural_type = element;
            }
        }
    }
    Some(prefix)
}

fn validate_boundary_requirements(
    caller: &TerminalMachine,
    boundary: &BoundaryMachineDeclaration,
    arguments: &[StructuralArgument],
    operation: OperationId,
) -> Result<(), ModuleError> {
    for requirement in &boundary.requires {
        let argument = &arguments[requirement.argument_index as usize];
        let actual = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .expect("structural arguments were validated before requirements");
        if !actual.qualifications.contains(&requirement.domain) {
            return Err(ModuleError::BoundaryArgumentMissingQualification {
                operation,
                argument_index: requirement.argument_index,
                domain: requirement.domain,
            });
        }
    }
    Ok(())
}

fn validate_boundary_completion_receipts(
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    receipts: &[CompletionReceipt],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let expected = arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller
                .entry_claims
                .iter()
                .filter_map(move |claim| {
                    (claim.input == argument.place
                        && (argument.path.is_empty() || claim.path == argument.path))
                        .then_some((index as u32, claim.claim))
                })
                .chain(caller.content_entry_claims.iter().filter_map(move |claim| {
                    (claim.input.root == argument.place).then_some((index as u32, claim.claim))
                }))
        })
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    let mut claims = BTreeSet::new();
    for receipt in receipts {
        if !actual.insert((receipt.argument_index, receipt.claim)) || !claims.insert(receipt.claim)
        {
            return Err(ModuleError::DuplicateBoundaryCompletionReceipt(operation));
        }
        let Some(argument) = arguments.get(receipt.argument_index as usize) else {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: receipt.argument_index,
            });
        };
        let Some((claim_input, claim_path)) = claim_input(caller, receipt.claim) else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: receipt.claim,
            });
        };
        if claim_input != argument.place
            || (!argument.path.is_empty() && claim_path != argument.path.as_slice())
        {
            return Err(ModuleError::ClaimActionPlaceMismatch {
                operation,
                claim: receipt.claim,
                argument_index: receipt.argument_index,
            });
        }
    }
    if actual != expected {
        return Err(ModuleError::BoundaryCompletionReceiptMismatch(operation));
    }
    if receipts.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalBoundaryCompletionReceipts(
            operation,
        ));
    }
    Ok(())
}
