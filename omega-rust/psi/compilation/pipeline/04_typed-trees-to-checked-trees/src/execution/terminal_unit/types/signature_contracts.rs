//! Signature contracts and domain qualifications a checked structural parameter
//! or result carries, and the domain requirements boundary machines publish.

use super::*;

pub(in crate::execution::terminal_unit) fn checked_state_contracts_supported(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
) -> bool {
    program.state_contracts(state).iter().all(|contract| {
        checked_structural_signature_contract_supported(
            program,
            machine,
            state,
            structural_parameters,
            contract,
        )
    })
}

pub(in crate::execution::terminal_unit) fn checked_structural_signature_contract_supported(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    contract: &typed_trees::signature::SignatureContract,
) -> bool {
    let source_parameters = program.state_parameters(state);
    program
        .proof_facts
        .span_or_empty(contract.facts)
        .iter()
        .all(|fact| match (&contract.kind, fact) {
            (SignatureContractKind::Requires, ProofFact::Membership(membership)) => {
                let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    0,
                    membership.value,
                ) else {
                    return false;
                };
                if program.domain_definitions().iter().any(|domain| {
                    domain.symbol == membership.domain_symbol
                        && domain.classification
                            == Some(language_semantics::DomainClassification::ProgressProfile)
                }) {
                    let facts::PlaceRoot::Symbol(root) = place.root else {
                        return false;
                    };
                    let rooted_in_telescope = source_parameters.iter().any(|parameter| {
                        parameter_root_symbol(machine.symbol, parameter) == root
                            || parameter.symbol == root
                    });
                    return rooted_in_telescope
                        && place
                            .segments
                            .iter()
                            .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. }));
                }
                if !place.segments.is_empty() {
                    return false;
                }
                let facts::PlaceRoot::Symbol(root) = place.root else {
                    return false;
                };
                let Some(position) = source_parameters.iter().position(|parameter| {
                    parameter_root_symbol(machine.symbol, parameter) == root
                        || parameter.symbol == root
                }) else {
                    return false;
                };
                let Some(domain) = type_domain_semantic_id(
                    program,
                    source_parameters[position].type_reference,
                    membership.domain_symbol,
                ) else {
                    return false;
                };
                structural_parameters.iter().any(|parameter| {
                    parameter.position as usize == position
                        && parameter.qualifications.contains(&domain)
                })
            }
            (SignatureContractKind::Ensures, ProofFact::Expression(expression)) => matches!(
                program.expression_table.expression(*expression),
                typed_trees::expression::ExpressionNode::Boolean(true)
            ),
            _ => false,
        })
}

/// A static boundary requirement may carry the implicit membership contracts
/// induced by qualified parameter types, plus authored `ensures result in
/// <domain>` clauses whose every fact carries the established qualification
/// authorization (`boundary_qualification_authorization` — a domain
/// BoundaryRequirement route or carry permission pinned to this signature),
/// but no other independently authored proof contract in this Terminal slice.
/// Reconstruct the exact pair set from both surfaces so an omitted or extra
/// membership cannot be hidden by lowering.
pub(in crate::execution::terminal_unit) fn signature_contracts_are_exact_parameter_qualifications(
    program: &TypedTrees,
    owner_symbol: SymbolHandle,
    signature: &typed_trees::signature::StateSignature,
) -> bool {
    let parameters = program.state_signature_parameters(signature);
    let mut expected = Vec::<(usize, SemanticDomainId)>::new();
    for (position, parameter) in parameters.iter().enumerate() {
        let mut type_reference = parameter.type_reference;
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        let TypeConstraintNode::Domain(domain) = constraint else {
                            return false;
                        };
                        if !domain.semantic_id.is_valid() {
                            return false;
                        }
                        if parameter.is_self
                            && program.domain_definitions().iter().any(|definition| {
                                definition.symbol == domain.symbol
                                    && definition.classification
                                        == Some(
                                            language_semantics::DomainClassification::ProgressProfile,
                                        )
                            })
                        {
                            continue;
                        }
                        expected.push((position, domain.semantic_id));
                    }
                    type_reference = *base_type;
                }
                TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                _ => break,
            }
        }
    }

    let mut actual = Vec::<(usize, SemanticDomainId)>::new();
    for contract in program.state_signature_contracts(signature) {
        // Crash ceilings have their own checked capsule and Terminal lane.
        if matches!(contract.kind, SignatureContractKind::Crashes { .. })
            && contract.binding.is_none()
        {
            continue;
        }
        // `ensures true` is the no-op postcondition the structural signature
        // replay already treats as admissible; it asserts no outbound proof
        // obligation this call edge could fail to carry.
        if contract.kind == SignatureContractKind::Ensures
            && contract.binding.is_none()
            && program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .all(|fact| {
                    matches!(
                        fact,
                        ProofFact::Expression(expression)
                            if matches!(
                                program.expression_table.expression(*expression),
                                ExpressionNode::Boolean(true)
                            )
                    )
                })
        {
            continue;
        }
        if contract.kind != SignatureContractKind::Requires || contract.binding.is_some() {
            // The one independently authored clause this slice admits is
            // `ensures result in <domain>`: the domain must already
            // authorize this signature through a BoundaryRequirement
            // establishment route or a carry permission, which is exactly
            // what the qualification-authorization evidence on the checked
            // fact replays.
            let authorized_ensures = contract.kind == SignatureContractKind::Ensures
                && contract.facts.count() > 0
                && (0..contract.facts.count()).all(|offset| {
                    let fact = arena::Handle::from_parts(
                        contract
                            .facts
                            .start()
                            .arena_index()
                            .checked_add(offset)
                            .expect("proof fact handle index overflow"),
                        contract.facts.start().generation(),
                    );
                    crate::facts::qualification_evidence::boundary_qualification_authorization(
                        program,
                        owner_symbol,
                        signature,
                        contract.kind.clone(),
                        fact,
                    )
                    .is_some()
                });
            if authorized_ensures {
                continue;
            }
            return false;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Membership(membership) = fact else {
                return false;
            };
            let ExpressionNode::Name(path) = program.expression_table.expression(membership.value)
            else {
                return false;
            };
            if program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
            {
                return false;
            }
            let [name] = program.expression_table.name_path_members(path.members) else {
                return false;
            };
            let Some(position) = parameters.iter().position(|parameter| {
                parameter.symbol == path.symbol
                    || parameter.name == *name
                    || (parameter.is_self && name.is_self_receiver())
            }) else {
                return false;
            };
            // A provider receiver's progress profile is an installation-time
            // premise, not an outbound structural ABI qualification.
            if parameters[position].is_self
                && program.domain_definitions().iter().any(|domain| {
                    domain.symbol == membership.domain_symbol
                        && domain.classification
                            == Some(language_semantics::DomainClassification::ProgressProfile)
                })
            {
                continue;
            }
            if contract.token_count != 0 {
                return false;
            }
            let Some(domain) = type_domain_semantic_id(
                program,
                parameters[position].type_reference,
                membership.domain_symbol,
            ) else {
                return false;
            };
            actual.push((position, domain));
        }
    }
    expected.sort_by_key(|(position, domain)| (*position, domain.0));
    actual.sort_by_key(|(position, domain)| (*position, domain.0));
    expected == actual
}

pub(in crate::execution::terminal_unit) fn boundary_domain_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<CheckedUnitStructuralDomainRequirementPlan>> {
    let source_parameters = program.state_parameters(state);
    let checked_requires = facts
        .proof
        .contract_facts
        .iter()
        .filter(|(_, fact)| {
            fact.kind == ContractProofFactKind::Requires
                && (matches!(
                    fact.owner,
                    ContractProofFactOwner::Machine { machine_symbol }
                        if machine_symbol == machine.symbol
                ) || matches!(
                    fact.owner,
                    ContractProofFactOwner::MachineState { machine_symbol, state_symbol }
                        if machine_symbol == machine.symbol && state_symbol == state.symbol
                ))
        })
        .map(|(_, fact)| fact)
        .collect::<Vec<_>>();
    let authored_requires = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .map(|contract| contract.facts.count() as usize)
        .sum::<usize>();
    if checked_requires.len() != authored_requires {
        return None;
    }

    let mut output = Vec::new();
    for checked in checked_requires {
        let ProofFact::Membership(membership) = program.proof_facts.get(checked.fact) else {
            // A machine-level scalar predicate is not a qualification check:
            // it travels as a checked scalar requires row
            // (`boundary_scalar_requires`), which refuses any clause the
            // closed contract did not lower. State-level clauses have no row.
            if matches!(checked.owner, ContractProofFactOwner::Machine { .. }) {
                continue;
            }
            return None;
        };
        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            0,
            membership.value,
        )?;
        if !place.segments.is_empty() {
            return None;
        }
        let facts::PlaceRoot::Symbol(root) = place.root else {
            return None;
        };
        let source_position = source_parameters.iter().position(|parameter| {
            parameter_root_symbol(machine.symbol, parameter) == root || parameter.symbol == root
        })?;
        let argument_index = structural_parameters
            .iter()
            .position(|parameter| parameter.position as usize == source_position)?;
        let domain = type_domain_semantic_id(
            program,
            source_parameters[source_position].type_reference,
            membership.domain_symbol,
        )?;
        let definition = program
            .domain_definitions()
            .iter()
            .find(|definition| definition.symbol == membership.domain_symbol)?;
        let carrier = type_domain_carrier(
            program,
            source_parameters[source_position].type_reference,
            membership.domain_symbol,
        )
        .or_else(|| {
            definition
                .index_arguments
                .is_empty()
                .then_some(definition.target_type)
        })?;
        shapes.add_domain(domain, carrier, binders)?;
        output.push(CheckedUnitStructuralDomainRequirementPlan {
            argument_index: u32::try_from(argument_index).ok()?,
            domain,
        });
    }
    output.sort_by_key(|requirement| (requirement.argument_index, requirement.domain.0));
    output.dedup();
    Some(output)
}

pub(in crate::execution::terminal_unit) fn parameter_qualifications(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    mut type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<SemanticDomainId>> {
    let mut output = Vec::new();
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                // An arithmetic-policy shell (`i32 in Wrapping`) carries no
                // semantic-domain custody for a parameter to honor: the
                // integer policy stays in the type identity while the plan
                // peels to the underlying carrier.
                if validation::is_arithmetic_policy_only_integer(program, type_reference) {
                    type_reference = *base_type;
                    continue;
                }
                for constraint in program.type_reference_table.constraints(*constraints) {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        return None;
                    };
                    if !domain.semantic_id.is_valid() {
                        return None;
                    }
                    let carrier = if matches!(
                        byte_sequence_carrier(program, type_reference, &[]),
                        Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. })
                    ) {
                        type_reference
                    } else {
                        *base_type
                    };
                    shapes.add_domain(domain.semantic_id, carrier, binders)?;
                    output.push(domain.semantic_id);
                }
                type_reference = *base_type;
            }
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            _ => break,
        }
    }
    output.sort_by_key(|domain| domain.0);
    output.dedup();
    Some(output)
}

/// Fold the domains an authorized `ensures result in <domain>` clause pins on
/// a boundary requirement into the result's structural qualifications. The
/// clause only counts when its fact replays the established qualification
/// authorization (a `BoundaryRequirement` establishment route or carry
/// permission naming this requirement); anything else was already refused by
/// the exact-qualification gate above. A scalar or unit result cannot carry a
/// structural domain qualification, so folding onto one refuses the plan
/// rather than silently dropping the authored custody.
pub(in crate::execution::terminal_unit) fn fold_authorized_result_domains(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    owner_symbol: SymbolHandle,
    signature: &typed_trees::signature::StateSignature,
    result_type: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    result: CheckedBoundaryMachineResultPlan,
) -> Option<CheckedBoundaryMachineResultPlan> {
    let mut authorized = Vec::new();
    for contract in program.state_signature_contracts(signature) {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for offset in 0..contract.facts.count() {
            let fact = arena::Handle::from_parts(
                contract
                    .facts
                    .start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("proof fact handle index overflow"),
                contract.facts.start().generation(),
            );
            if crate::facts::qualification_evidence::boundary_qualification_authorization(
                program,
                owner_symbol,
                signature,
                contract.kind.clone(),
                fact,
            )
            .is_none()
            {
                continue;
            }
            let ProofFact::Membership(membership) = program.proof_facts.get(fact) else {
                continue;
            };
            let Some(domain) = crate::facts::qualification_evidence::domain_definition(
                program,
                membership.domain_symbol,
            ) else {
                continue;
            };
            authorized.push(domain.semantic_id);
        }
    }
    authorized.sort_by_key(|domain| domain.0);
    authorized.dedup();
    if authorized.is_empty() {
        return Some(result);
    }
    let CheckedBoundaryMachineResultPlan::Structural {
        type_identity,
        multiplicity,
        mut qualifications,
    } = result
    else {
        return None;
    };
    for domain in authorized {
        shapes.add_domain(domain, result_type, binders)?;
        if !qualifications.contains(&domain) {
            qualifications.push(domain);
        }
    }
    qualifications.sort_by_key(|domain| domain.0);
    Some(CheckedBoundaryMachineResultPlan::Structural {
        type_identity,
        multiplicity,
        qualifications,
    })
}

/// Collect domain constraints below a structural root without treating them
/// as root qualifications. Fixed-array indexes are the canonical source path
/// for each occurrence; unsupported dynamic shapes remain absent from plans.
pub(in crate::execution::terminal_unit) fn projected_parameter_qualifications(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<CheckedStructuralPathQualification>> {
    fn collect(
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
        shapes: &mut ShapeCollector<'_>,
        binders: &[(SymbolHandle, String)],
    ) -> Option<()> {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                collect(program, *referee, shapes, binders)
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        continue;
                    };
                    if !domain.semantic_id.is_valid() {
                        return None;
                    }
                    shapes.add_domain(domain.semantic_id, *base_type, binders)?;
                }
                collect(program, *base_type, shapes, binders)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let typed_trees::types::FixedArrayLength::Literal(length) = length else {
                    return None;
                };
                if *length != 0 {
                    collect(program, *element_type, shapes, binders)?;
                }
                Some(())
            }
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Unit
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. } => Some(()),
        }
    }
    let output =
        validation::structural_result_projected_qualifications(program, type_reference).ok()?;
    collect(program, type_reference, shapes, binders)?;
    Some(output)
}

pub(super) fn type_domain_semantic_id(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    domain: SymbolHandle,
) -> Option<SemanticDomainId> {
    fn collect(
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
        domain: SymbolHandle,
        matches: &mut Vec<SemanticDomainId>,
    ) {
        if !type_reference.is_valid() {
            return;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                collect(program, *referee, domain, matches);
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                collect(program, *base_type, domain, matches);
                for constraint in program.type_reference_table.constraints(*constraints) {
                    let TypeConstraintNode::Domain(candidate) = constraint else {
                        continue;
                    };
                    if candidate.symbol == domain
                        && candidate.semantic_id.is_valid()
                        && !matches.contains(&candidate.semantic_id)
                    {
                        matches.push(candidate.semantic_id);
                    }
                }
            }
            _ => {}
        }
    }

    let mut matches = Vec::new();
    collect(program, type_reference, domain, &mut matches);
    match matches.as_slice() {
        [semantic_id] => Some(*semantic_id),
        [] => program
            .domain_definitions()
            .iter()
            .find(|definition| definition.symbol == domain)
            .filter(|definition| definition.index_arguments.is_empty())
            .map(|definition| definition.semantic_id)
            .filter(|semantic_id| semantic_id.is_valid()),
        _ => None,
    }
}

pub(super) fn type_domain_carrier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    domain: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    fn collect(
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
        domain: SymbolHandle,
        matches: &mut Vec<TypeReferenceHandle>,
    ) {
        if !type_reference.is_valid() {
            return;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                collect(program, *referee, domain, matches);
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                collect(program, *base_type, domain, matches);
                if program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(candidate) if candidate.symbol == domain))
                    && !matches.contains(base_type)
                {
                    matches.push(*base_type);
                }
            }
            _ => {}
        }
    }

    let mut matches = Vec::new();
    collect(program, type_reference, domain, &mut matches);
    let [carrier] = matches.as_slice() else {
        return None;
    };
    Some(*carrier)
}
