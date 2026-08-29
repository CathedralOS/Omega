//! Scalar, structural-call, access, and claim operation contracts.

use super::*;

pub(crate) fn validate_values_and_bindings(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
    structural_types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    structural_domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut definitions = BTreeMap::new();
    for definition in function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
    {
        if definitions.insert(definition.value, *definition).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateValue(
                definition.value,
            ));
        }
    }

    let dominators = dominators(function.entry, blocks.keys().copied(), predecessors);
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            for use_site in &node.uses {
                let Some(definition) = definitions.get(&use_site.value) else {
                    return Err(OptimizationUnitValidationError::UndefinedValue {
                        machine: function.machine,
                        block: block.id,
                        value: use_site.value,
                    });
                };
                match definition.site {
                    ValueDefinitionSite::FunctionParameter(_) => {}
                    ValueDefinitionSite::BlockParameter {
                        block: defining, ..
                    } => {
                        if !dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(&defining))
                        {
                            return Err(OptimizationUnitValidationError::NondominatingValue {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                    ValueDefinitionSite::Node {
                        block: defining,
                        node,
                    } if defining == block.id => {
                        if usize::try_from(node).expect("u32 fits usize") >= node_index {
                            return Err(OptimizationUnitValidationError::UseBeforeDefinition {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                    ValueDefinitionSite::Node {
                        block: defining, ..
                    } => {
                        if !dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(&defining))
                        {
                            return Err(OptimizationUnitValidationError::NondominatingValue {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                }
            }
            if !operation_scalar_types_match(
                function,
                &node.operation,
                &definitions,
                functions,
                boundary_machines,
            ) {
                return Err(
                    OptimizationUnitValidationError::ScalarOperationContractMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("unit node index fits u32"),
                    },
                );
            }
            if !operation_structural_call_contract_matches(
                function,
                &node.operation,
                functions,
                boundary_machines,
                structural_types,
                structural_domains,
            ) {
                return Err(
                    OptimizationUnitValidationError::StructuralCallContractMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("unit node index fits u32"),
                    },
                );
            }
            if !operation_service_contract_matches(
                function,
                &node.operation,
                functions,
                boundary_machines,
                services,
            ) {
                return Err(
                    OptimizationUnitValidationError::OperationServiceContractMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("unit node index fits u32"),
                    },
                );
            }
            for edge in &node.successors {
                let target = blocks.get(&edge.target).expect("successor validated");
                if edge.bindings.len() != target.parameters.len() {
                    return Err(OptimizationUnitValidationError::BindingArityMismatch {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    });
                }
                for (binding, parameter) in edge.bindings.iter().zip(&target.parameters) {
                    let source_type = definitions
                        .get(&binding.argument)
                        .map(|row| row.scalar_type);
                    if binding.parameter != parameter.value
                        || binding.scalar_type != parameter.scalar_type
                        || source_type != Some(parameter.scalar_type)
                    {
                        return Err(OptimizationUnitValidationError::BindingTypeMismatch {
                            machine: function.machine,
                            edge: edge.psi_edge,
                            value: binding.argument,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn operation_service_contract_matches(
    caller: &PsiOptimizationFunction,
    operation: &O,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundaries: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
) -> bool {
    let reached_is_published = |reached: &[ServiceId]| {
        reached
            .iter()
            .all(|service| caller.published_service_ceiling.contains(service))
    };
    match operation {
        O::Call { callee, .. }
        | O::CallUnit { callee, .. }
        | O::CallStructuralScalar { callee, .. }
        | O::CallStructural { callee, .. } => functions
            .get(callee)
            .is_some_and(|callee| reached_is_published(&callee.published_service_ceiling)),
        O::BoundaryCall { boundary, .. } => boundaries
            .get(boundary)
            .is_some_and(|boundary| reached_is_published(&boundary.published_service_ceiling)),
        O::PortWrite { service, .. } => {
            services.contains_key(service) && caller.published_service_ceiling.contains(service)
        }
        _ => true,
    }
}

/// Independently reconstruct the structural half of every call contract from
/// verifier-owned module/function catalogs. Call-local source/receipt rows are
/// evidence to compare, never the authority from which the expected contract
/// is inferred.
pub(crate) fn operation_structural_call_contract_matches(
    caller: &PsiOptimizationFunction,
    operation: &O,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    match operation {
        O::EstablishPayloadlessCase { .. } => {
            payloadless_establishment_matches(caller, operation, types)
        }
        O::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                StructuralProjectionPolicy::Unit,
                false,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            )
        }),
        O::CallStructuralScalar {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                StructuralProjectionPolicy::EmptyOnly,
                false,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            )
        }),
        O::CallStructural {
            result,
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                StructuralProjectionPolicy::EmptyOnly,
                false,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            ) && validate_structural_call_result(
                result,
                callee,
                exact_payloadless_structural_call(operation, callee, types),
                claim_transfers,
                returned_claim_transfers,
                types,
            ) && payloadless_selected_evidence_surface_matches(operation, callee, types)
        }),
        O::BoundaryCall {
            boundary,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
            ..
        } => boundary_machines.get(boundary).is_some_and(|boundary| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &boundary.structural_parameters,
                types,
                StructuralProjectionPolicy::Boundary,
                true,
            ) && boundary_requirements_match(caller, structural_arguments, boundary, domains)
                && boundary_completion_matches(
                    caller,
                    structural_arguments,
                    completion_claim_sources,
                    completion_receipts,
                )
        }),
        _ => true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructuralProjectionPolicy {
    Unit,
    EmptyOnly,
    Boundary,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StructuralSourceContract<'a> {
    structural_type: StructuralTypeId,
    multiplicity: psi_terminal::StructuralMultiplicity,
    access: psi_terminal::StructuralAccess,
    qualifications: &'a [StructuralDomainId],
}

pub(crate) fn structural_arguments_match(
    caller: &PsiOptimizationFunction,
    arguments: &[psi_terminal::StructuralArgument],
    parameters: &[psi_terminal::StructuralParameterDeclaration],
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    projection: StructuralProjectionPolicy,
    allow_byte_literal: bool,
) -> bool {
    if arguments.len() != parameters.len() {
        return false;
    }
    for (argument, parameter) in arguments.iter().zip(parameters) {
        let Some(source) = structural_source_contract(caller, argument.place, allow_byte_literal)
        else {
            return false;
        };
        let path_shape_matches = match projection {
            StructuralProjectionPolicy::Unit => {
                argument.path.is_empty()
                    || matches!(
                        argument.path.as_slice(),
                        [psi_terminal::StructuralPathSegment::FixedIndex(_)]
                    )
                    || is_nonempty_field_path(&argument.path)
            }
            StructuralProjectionPolicy::EmptyOnly => argument.path.is_empty(),
            StructuralProjectionPolicy::Boundary => true,
        };
        let Some(actual_type) =
            resolve_structural_path(types, source.structural_type, &argument.path)
        else {
            return false;
        };
        if !path_shape_matches
            || actual_type != parameter.structural_type
            || argument.access != parameter.access
            || !structural_access_can_supply(source.access, argument.access)
        {
            return false;
        }
        let unrestricted_write_only_field = is_nonempty_field_path(&argument.path)
            && argument.access == psi_terminal::StructuralAccess::WriteOnlyBorrow
            && parameter.access == psi_terminal::StructuralAccess::WriteOnlyBorrow
            && source.access == psi_terminal::StructuralAccess::WriteOnlyBorrow
            && parameter.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
            && source.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted;
        let actual_multiplicity = if argument.path.is_empty() {
            source.multiplicity
        } else if unrestricted_write_only_field {
            psi_terminal::StructuralMultiplicity::Unrestricted
        } else if parameter.multiplicity == psi_terminal::StructuralMultiplicity::Affine
            && source.multiplicity == psi_terminal::StructuralMultiplicity::Affine
            && is_bounded_partial_affine_path(types, source.structural_type, &argument.path)
        {
            psi_terminal::StructuralMultiplicity::Affine
        } else {
            psi_terminal::StructuralMultiplicity::Linear
        };
        if actual_multiplicity != parameter.multiplicity
            || parameter.qualifications.iter().any(|qualification| {
                !argument.path.is_empty() || !source.qualifications.contains(qualification)
            })
            || (projection == StructuralProjectionPolicy::Unit
                && !argument.path.is_empty()
                && !source.qualifications.is_empty())
        {
            return false;
        }
    }
    for first in 0..arguments.len() {
        for second in first + 1..arguments.len() {
            let left = &arguments[first];
            let right = &arguments[second];
            if left.place == right.place
                && structural_paths_may_overlap(&left.path, &right.path)
                && (structural_access_is_exclusive(left.access)
                    || structural_access_is_exclusive(right.access))
            {
                return false;
            }
        }
    }
    true
}

pub(crate) fn structural_source_contract(
    caller: &PsiOptimizationFunction,
    place: PlaceId,
    allow_byte_literal: bool,
) -> Option<StructuralSourceContract<'_>> {
    caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
        .map(|parameter| StructuralSourceContract {
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            qualifications: &parameter.qualifications,
        })
        .or_else(|| {
            allow_byte_literal.then_some(())?;
            caller
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .find_map(|node| {
                    let O::EstablishByteSequenceLiteral {
                        place: declaration,
                        structural_type,
                        ..
                    } = &node.operation
                    else {
                        return None;
                    };
                    (declaration.id == place).then_some(StructuralSourceContract {
                        structural_type: structural_type.id,
                        multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
                        access: psi_terminal::StructuralAccess::Owned,
                        qualifications: &[],
                    })
                })
        })
}

pub(crate) fn structural_access_can_supply(
    source: psi_terminal::StructuralAccess,
    presented: psi_terminal::StructuralAccess,
) -> bool {
    match source {
        psi_terminal::StructuralAccess::Owned => true,
        psi_terminal::StructuralAccess::SharedBorrow => {
            presented == psi_terminal::StructuralAccess::SharedBorrow
        }
        psi_terminal::StructuralAccess::MutableBorrow => matches!(
            presented,
            psi_terminal::StructuralAccess::SharedBorrow
                | psi_terminal::StructuralAccess::MutableBorrow
                | psi_terminal::StructuralAccess::WriteOnlyBorrow
        ),
        psi_terminal::StructuralAccess::WriteOnlyBorrow => {
            presented == psi_terminal::StructuralAccess::WriteOnlyBorrow
        }
    }
}

pub(crate) fn structural_access_is_exclusive(access: psi_terminal::StructuralAccess) -> bool {
    matches!(
        access,
        psi_terminal::StructuralAccess::MutableBorrow
            | psi_terminal::StructuralAccess::WriteOnlyBorrow
    )
}

pub(crate) fn structural_paths_may_overlap(
    left: &[psi_terminal::StructuralPathSegment],
    right: &[psi_terminal::StructuralPathSegment],
) -> bool {
    left.iter().zip(right).all(|(left, right)| left == right)
}

pub(crate) fn is_nonempty_field_path(path: &[psi_terminal::StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path
            .iter()
            .all(|segment| matches!(segment, psi_terminal::StructuralPathSegment::Field(_)))
}

pub(crate) fn is_bounded_partial_affine_path(
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[psi_terminal::StructuralPathSegment],
) -> bool {
    is_nonempty_field_path(path)
        || (matches!(path, [psi_terminal::StructuralPathSegment::FixedIndex(_)])
            && types.get(&root).is_some_and(|declaration| {
                matches!(
                    (&declaration.shape, path),
                    (
                        psi_terminal::StructuralTypeShape::FixedArray { length: 2, .. },
                        [psi_terminal::StructuralPathSegment::FixedIndex(0 | 1)]
                    ) | (
                        psi_terminal::StructuralTypeShape::FixedArray { length: 3, .. },
                        [psi_terminal::StructuralPathSegment::FixedIndex(0 | 1 | 2)]
                    )
                )
            }))
}

pub(crate) fn validate_internal_claim_transfers(
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    arguments: &[psi_terminal::StructuralArgument],
    transfers: &[psi_terminal::ClaimTransfer],
) -> bool {
    for (argument, parameter) in arguments.iter().zip(&callee.structural_parameters) {
        let mut caller_paths = caller
            .entry_claim_declarations
            .iter()
            .filter(|claim| claim.input == argument.place && claim.path.starts_with(&argument.path))
            .map(|claim| &claim.path[argument.path.len()..])
            .collect::<Vec<_>>();
        let mut callee_paths = callee
            .entry_claim_declarations
            .iter()
            .filter(|claim| claim.input == parameter.place)
            .map(|claim| claim.path.as_slice())
            .collect::<Vec<_>>();
        caller_paths.sort();
        callee_paths.sort();
        if caller_paths != callee_paths {
            return false;
        }
        if !argument.path.is_empty()
            && (caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
                || callee
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place))
        {
            return false;
        }
        let mut caller_content = caller
            .content_entry_claims
            .iter()
            .filter(|claim| claim.input.root == argument.place)
            .map(|claim| (&claim.input.segments, &claim.projections))
            .collect::<Vec<_>>();
        let mut callee_content = callee
            .content_entry_claims
            .iter()
            .filter(|claim| claim.input.root == parameter.place)
            .map(|claim| (&claim.input.segments, &claim.projections))
            .collect::<Vec<_>>();
        caller_content.sort();
        callee_content.sort();
        if caller_content != callee_content {
            return false;
        }
    }
    let callee_claims = callee
        .entry_claim_declarations
        .iter()
        .map(|claim| (claim.claim, claim.input))
        .chain(
            callee
                .content_entry_claims
                .iter()
                .map(|claim| (claim.claim, claim.input.root)),
        )
        .collect::<BTreeMap<_, _>>();
    if transfers.len() != callee_claims.len()
        || transfers.windows(2).any(|pair| pair[0] >= pair[1])
        || transfers
            .iter()
            .map(|transfer| transfer.claim)
            .collect::<BTreeSet<_>>()
            .len()
            != transfers.len()
    {
        return false;
    }
    for transfer in transfers {
        let Some(argument) = arguments.get(transfer.argument_index as usize) else {
            return false;
        };
        let Some((claim_input, claim_path)) = function_claim_input(caller, transfer.claim) else {
            return false;
        };
        let target_place = callee
            .structural_parameters
            .get(transfer.argument_index as usize)
            .map(|parameter| parameter.place);
        let structural_match = claim_path.starts_with(&argument.path)
            && callee.entry_claim_declarations.iter().any(|claim| {
                Some(claim.input) == target_place && claim.path == claim_path[argument.path.len()..]
            });
        let content_match = argument.path.is_empty()
            && caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.claim == transfer.claim && claim.input.root == argument.place)
            && callee
                .content_entry_claims
                .iter()
                .any(|claim| Some(claim.input.root) == target_place);
        if claim_input != argument.place || (!structural_match && !content_match) {
            return false;
        }
    }
    callee_claims.into_values().all(|input| {
        callee
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == input)
            .is_some_and(|index| {
                transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
            })
    })
}

pub(crate) fn function_claim_input(
    function: &PsiOptimizationFunction,
    claim: ClaimId,
) -> Option<(PlaceId, &[psi_terminal::StructuralPathSegment])> {
    function
        .entry_claim_declarations
        .iter()
        .find_map(|candidate| {
            (candidate.claim == claim).then_some((candidate.input, candidate.path.as_slice()))
        })
        .or_else(|| {
            function.content_entry_claims.iter().find_map(|candidate| {
                (candidate.claim == claim).then_some((
                    candidate.input.root,
                    &[] as &[psi_terminal::StructuralPathSegment],
                ))
            })
        })
}

pub(crate) fn proposition_structural_roots(proposition: &Proposition) -> BTreeSet<PlaceId> {
    fn scalar_term_roots(term: &ScalarTerm, roots: &mut BTreeSet<PlaceId>) {
        match term {
            ScalarTerm::BooleanField { root, .. } | ScalarTerm::IntegerField { root, .. } => {
                roots.insert(*root);
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_roots(operand, roots),
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                scalar_term_roots(left, roots);
                scalar_term_roots(right, roots);
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                scalar_term_roots(value, roots);
                scalar_term_roots(count, roots);
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
    }

    fn content_term_roots(term: &ContentTerm, roots: &mut BTreeSet<PlaceId>) {
        match term {
            ContentTerm::Projection { subject, .. } => {
                roots.insert(subject.root);
            }
            ContentTerm::Separate(terms) => {
                for term in terms {
                    content_term_roots(term, roots);
                }
            }
        }
    }

    fn collect(proposition: &Proposition, roots: &mut BTreeSet<PlaceId>) {
        match proposition {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                scalar_term_roots(left, roots);
                scalar_term_roots(right, roots);
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                roots.insert(left.root());
                roots.insert(right.root());
            }
            Proposition::ByteSequenceEqual { left, right } => {
                roots.insert(left.root());
                roots.insert(right.root());
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                roots.insert(subject.root());
            }
            Proposition::ContentConservation(conservation) => {
                content_term_roots(conservation.left(), roots);
                content_term_roots(conservation.right(), roots);
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions {
                    collect(proposition, roots);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect(premise, roots);
                collect(conclusion, roots);
            }
            Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
        }
    }

    let mut roots = BTreeSet::new();
    collect(proposition, &mut roots);
    roots
}

pub(crate) fn payloadless_establishment_matches(
    function: &PsiOptimizationFunction,
    operation: &O,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let O::EstablishPayloadlessCase {
        psi_operation,
        result,
        result_case,
    } = operation
    else {
        return false;
    };
    function.structural_places.iter().any(|place| {
        place.id == result.place
            && matches!(
                place.kind,
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } if producer == *psi_operation && structural_type == result.structural_type
            )
    }) && result.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.claims.is_empty()
        && types.get(&result.structural_type).is_some_and(|declaration| {
            matches!(
                &declaration.shape,
                psi_terminal::StructuralTypeShape::Sum { cases }
                    if cases.iter().any(|case| case.id == *result_case && case.fields.is_empty())
            )
        })
}

pub(crate) fn exact_payloadless_case_return_exits(
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let Some(signature) = callee.result.structural() else {
        return false;
    };
    if !signature.qualifications.is_empty()
        || signature.multiplicity != psi_terminal::StructuralMultiplicity::Unrestricted
        || callee
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                matches!(
                    node.operation,
                    O::Call { .. }
                        | O::CallUnit { .. }
                        | O::CallStructuralScalar { .. }
                        | O::CallStructural { .. }
                        | O::BoundaryCall { .. }
                )
            })
    {
        return false;
    }
    let mut exits = 0_usize;
    for block in &callee.blocks {
        let Some(node) = block.nodes.last() else {
            return false;
        };
        let O::ReturnStructural {
            source,
            returned_claims,
            ..
        } = &node.operation
        else {
            continue;
        };
        if !returned_claims.is_empty() {
            return false;
        }
        let Some(producer) = callee.structural_places.iter().find_map(|place| {
            (place.id == *source)
                .then_some(place.kind)
                .and_then(|kind| match kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } if structural_type == signature.structural_type => Some(producer),
                    _ => None,
                })
        }) else {
            return false;
        };
        let Some(producer) = callee
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .map(|node| &node.operation)
            .find(|operation| {
                matches!(
                    operation,
                    O::EstablishPayloadlessCase { psi_operation, .. }
                        if *psi_operation == producer
                )
            })
        else {
            return false;
        };
        let O::EstablishPayloadlessCase { result, .. } = producer else {
            return false;
        };
        if result.place != *source
            || result.structural_type != signature.structural_type
            || !payloadless_establishment_matches(callee, producer, types)
        {
            return false;
        }
        exits += 1;
    }
    exits != 0
}

pub(crate) fn exact_payloadless_structural_call(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        result,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence: _,
        ..
    } = operation
    else {
        return false;
    };
    let Some(callee_result) = callee.result.structural() else {
        return false;
    };
    let Some(contract) = callee.verified_contract.as_ref() else {
        return false;
    };
    callee.parameters.is_empty()
        && callee.structural_parameters.is_empty()
        && callee.entry_claim_declarations.is_empty()
        && callee.content_entry_claims.is_empty()
        && contract.requires.is_empty()
        && contract.ensures.is_empty()
        && contract.crash_routes.is_empty()
        && callee.evidence_contract_lanes.is_empty()
        && structural_arguments.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && result.structural_type == callee_result.structural_type
        && result.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
        && result.multiplicity == callee_result.multiplicity
        && result.qualifications.is_empty()
        && result.qualifications == callee_result.qualifications
        && result.claims.is_empty()
        && contract.outcome_specific_ensures.iter().all(|row| {
            proposition_structural_roots(&row.proposition)
                .into_iter()
                .all(|root| root == callee_result.place)
        })
        && exact_payloadless_case_return_exits(callee, types)
}

pub(crate) fn payloadless_selected_evidence_surface_matches(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        selected_evidence, ..
    } = operation
    else {
        return true;
    };
    selected_evidence.is_none()
        || (exact_payloadless_structural_call(operation, callee, types)
            && callee
                .verified_contract
                .as_ref()
                .is_some_and(|contract| !contract.outcome_specific_ensures.is_empty()))
}

pub(crate) fn validate_structural_call_result(
    result: &psi_terminal::StructuralOperationResult,
    callee: &PsiOptimizationFunction,
    exact_payloadless: bool,
    claim_transfers: &[psi_terminal::ClaimTransfer],
    returned: &[psi_terminal::StructuralResultClaimTransfer],
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let Some(signature) = callee.result.structural() else {
        return false;
    };
    if result.structural_type != signature.structural_type
        || result.multiplicity != signature.multiplicity
        || result.qualifications != signature.qualifications
        || result
            .qualifications
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || result.claims.windows(2).any(|pair| pair[0] >= pair[1])
        || result.claims.iter().any(|claim| {
            resolve_structural_path(types, result.structural_type, &claim.path).is_none()
        })
        || result.claims.iter().enumerate().any(|(index, claim)| {
            result.claims[index + 1..]
                .iter()
                .any(|other| structural_paths_may_overlap(&claim.path, &other.path))
        })
    {
        return false;
    }
    if exact_payloadless {
        return true;
    }
    if callee.entry_claim_declarations.is_empty()
        || result.claims.is_empty()
        || returned.is_empty()
        || returned.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return false;
    }
    let callee_claims = callee
        .entry_claim_declarations
        .iter()
        .map(|claim| (claim.claim, claim.path.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let result_claims = result
        .claims
        .iter()
        .map(|claim| (claim.claim, claim.path.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let transferred = claim_transfers
        .iter()
        .map(|transfer| transfer.claim)
        .collect::<BTreeSet<_>>();
    let returned_callee = returned
        .iter()
        .map(|transfer| transfer.callee_claim)
        .collect::<BTreeSet<_>>();
    let returned_caller = returned
        .iter()
        .map(|transfer| transfer.caller_claim)
        .collect::<BTreeSet<_>>();
    callee_claims.len() == callee.entry_claim_declarations.len()
        && result_claims.len() == result.claims.len()
        && returned_callee.len() == returned.len()
        && returned_caller.len() == returned.len()
        && returned_callee == callee_claims.keys().copied().collect()
        && returned_caller == result_claims.keys().copied().collect()
        && transferred == result_claims.keys().copied().collect()
        && returned.iter().all(|transfer| {
            callee_claims.get(&transfer.callee_claim) == result_claims.get(&transfer.caller_claim)
        })
}

pub(crate) fn boundary_requirements_match(
    caller: &PsiOptimizationFunction,
    arguments: &[psi_terminal::StructuralArgument],
    boundary: &psi_terminal::BoundaryMachineDeclaration,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    boundary.requires.windows(2).all(|pair| pair[0] < pair[1])
        && boundary.requires.iter().all(|requirement| {
            domains.contains_key(&requirement.domain)
                && arguments
                    .get(requirement.argument_index as usize)
                    .and_then(|argument| {
                        caller
                            .structural_parameters
                            .iter()
                            .find(|parameter| parameter.place == argument.place)
                    })
                    .is_some_and(|source| source.qualifications.contains(&requirement.domain))
        })
}

pub(crate) fn boundary_completion_matches(
    caller: &PsiOptimizationFunction,
    arguments: &[psi_terminal::StructuralArgument],
    sources: &[omega_abstract_operations::CompletionClaimSource],
    receipts: &[psi_terminal::CompletionReceipt],
) -> bool {
    let mut expected_sources = caller
        .entry_claim_declarations
        .iter()
        .cloned()
        .map(|entry| omega_abstract_operations::CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    for content in &caller.content_entry_claims {
        if let Some(source) = expected_sources
            .iter_mut()
            .find(|source| source.claim == content.claim)
        {
            source.content = Some(content.clone());
        } else {
            expected_sources.push(omega_abstract_operations::CompletionClaimSource {
                claim: content.claim,
                entry: None,
                content: Some(content.clone()),
            });
        }
    }
    expected_sources.sort();
    if sources != expected_sources
        || receipts.windows(2).any(|pair| pair[0] >= pair[1])
        || receipts
            .iter()
            .map(|receipt| receipt.claim)
            .collect::<BTreeSet<_>>()
            .len()
            != receipts.len()
    {
        return false;
    }
    let expected = arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller
                .entry_claim_declarations
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
    let actual = receipts
        .iter()
        .map(|receipt| (receipt.argument_index, receipt.claim))
        .collect::<BTreeSet<_>>();
    actual.len() == receipts.len()
        && actual == expected
        && receipts.iter().all(|receipt| {
            arguments
                .get(receipt.argument_index as usize)
                .and_then(|argument| {
                    function_claim_input(caller, receipt.claim).map(|(input, path)| {
                        input == argument.place
                            && (argument.path.is_empty() || path == argument.path.as_slice())
                    })
                })
                == Some(true)
        })
}

pub(crate) fn operation_scalar_types_match(
    function: &PsiOptimizationFunction,
    operation: &O,
    definitions: &BTreeMap<ValueId, ValueDefinition>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
) -> bool {
    let scalar = |value: ValueId| definitions.get(&value).map(|row| row.scalar_type);
    let integer = |value: ValueId, expected: IntegerType| {
        scalar(value) == Some(ScalarType::Integer(expected))
    };
    let fixed = |integer: IntegerType| integer.carrier() == IntegerCarrier::Fixed;
    let binary = |left: ValueId, right: ValueId, expected: IntegerType| {
        integer(left, expected) && integer(right, expected)
    };
    match operation {
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::PortWrite { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => true,
        O::IntegerConstant {
            scalar_type, value, ..
        } => match scalar_type {
            ScalarType::Integer(integer) => integer.admits(*value),
            ScalarType::Boolean => false,
        },
        O::BooleanConstant { .. } => true,
        O::BooleanNot { operand, .. } => scalar(*operand) == Some(ScalarType::Boolean),
        O::BooleanEqual { left, right, .. } => {
            scalar(*left) == Some(ScalarType::Boolean)
                && scalar(*right) == Some(ScalarType::Boolean)
        }
        O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. } => {
            matches!(scalar(*left), Some(ScalarType::Integer(_))) && scalar(*left) == scalar(*right)
        }
        O::IntegerBitwiseNot {
            scalar_type,
            operand,
            ..
        } => integer(*operand, *scalar_type),
        O::IntegerWiden {
            source_type,
            target_type,
            operand,
            ..
        } => integer(*operand, *source_type) && source_type.can_widen_to(*target_type),
        O::IntegerExactCast {
            source_type,
            target_type,
            operand,
            ..
        } => {
            integer(*operand, *source_type)
                && source_type.can_exact_cast_to(*target_type)
                && !source_type.can_widen_to(*target_type)
                && source_type != target_type
        }
        O::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
            ..
        }
        | O::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        } => binary(*left, *right, *scalar_type),
        O::ExactIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        } => fixed(*scalar_type) && binary(*left, *right, *scalar_type),
        O::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | O::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
            ..
        } => integer(*value, *value_type) && integer(*count, *count_type),
        O::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | O::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
            ..
        } => {
            fixed(*value_type)
                && fixed(*count_type)
                && integer(*value, *value_type)
                && integer(*count, *count_type)
        }
        O::Jump { .. } => true,
        O::Conditional { condition, .. } => scalar(*condition) == Some(ScalarType::Boolean),
        O::Return {
            result,
            value,
            scalar_type,
            ..
        } => {
            scalar(*value) == Some(*scalar_type)
                && matches!(
                    function.result,
                    omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                        if signature.value == *result && signature.scalar_type == *scalar_type
                )
        }
        O::Call {
            result: _,
            scalar_type,
            callee,
            arguments,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            callee.structural_parameters.is_empty()
                && callee.declared_places.is_empty()
                && callee.entry_claim_declarations.is_empty()
                && matches!(
                    callee.result,
                    omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                        if signature.scalar_type == *scalar_type
                )
                && arguments.len() == callee.parameters.len()
                && arguments
                    .iter()
                    .zip(&callee.parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(parameter.scalar_type))
        }),
        O::CallUnit { callee, .. } => functions.get(callee).is_some_and(|callee| {
            callee.parameters.is_empty()
                && matches!(
                    callee.result,
                    omega_abstract_operations::AbstractFunctionResult::Unit
                )
        }),
        O::CallStructuralScalar { result, callee, .. } => {
            functions.get(callee).is_some_and(|callee| {
                callee.parameters.is_empty()
                    && matches!(
                        callee.result,
                        omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                            if signature.scalar_type == result.scalar_type
                    )
            })
        }
        O::CallStructural { callee, .. } => functions.get(callee).is_some_and(|callee| {
            callee.parameters.is_empty()
                && matches!(
                    callee.result,
                    omega_abstract_operations::AbstractFunctionResult::Structural(_)
                )
        }),
        O::BoundaryCall {
            result,
            boundary,
            arguments,
            ..
        } => boundary_machines.get(boundary).is_some_and(|boundary| {
            result.as_ref().map(|result| result.scalar_type) == boundary.result
                && arguments.len() == boundary.scalar_parameters.len()
                && arguments
                    .iter()
                    .zip(&boundary.scalar_parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(*parameter))
        }),
    }
}
