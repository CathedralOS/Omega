//! Structural type, shape, claim, and return-custody helpers.

use super::*;

pub(super) fn return_unit_affine_discards(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    source_parameters: &[StateParameter],
    operations: &[CheckedUnitEffectOperationPlan],
    admitted_local_symbols: &[SymbolHandle],
) -> Option<Vec<u32>> {
    let transferred_parameters = operations
        .iter()
        .flat_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            } => structural_arguments
                .iter()
                .filter(|argument| argument.byte_sequence_literal.is_none())
                .map(|argument| argument.source_parameter_index)
                .collect::<Vec<_>>(),
            CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
            | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => Vec::new(),
        })
        .collect::<BTreeSet<_>>();
    let events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Affine
                && event.claim_identity == PermissionClaimIdentity::Unknown
                && event.provenance == psi_language_semantics::PermissionProvenance::Unknown
                && !event.obligation_live
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty()
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(events.len());
    for event in events {
        let parameter_index = structural_parameters.iter().position(|parameter| {
            source_parameters
                .get(parameter.position as usize)
                .is_some_and(|source| {
                    event.root
                        == psi_facts::PlaceRoot::Symbol(parameter_root_symbol(machine, source))
                })
        });
        if parameter_index.is_none() {
            let psi_facts::PlaceRoot::Symbol(root) = event.root else {
                return None;
            };
            if admitted_local_symbols.contains(&root) {
                continue;
            }
            return None;
        }
        let parameter_index = parameter_index?;
        let parameter = &structural_parameters[parameter_index];
        let source_parameter = source_parameters.get(parameter.position as usize)?;
        if parameter.multiplicity != Multiplicity::Affine
            || type_graph_requires_nominal_drop(program, source_parameter.type_reference)
            || output.contains(&(parameter_index as u32))
        {
            return None;
        }
        let parameter_index = u32::try_from(parameter_index).ok()?;
        if !transferred_parameters.contains(&parameter_index) {
            output.push(parameter_index);
        }
    }
    Some(output)
}

pub(super) fn checked_no_code_affine_discard_positions(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &psi_typed_trees::state::State,
) -> Option<Vec<u32>> {
    let positions = crate::flow::terminal_cleanup::checked_whole_affine_discard_parameters(
        program, facts, machine, state,
    )?
    .into_iter()
    .map(|(_, position)| position)
    .collect::<Vec<_>>();
    if positions.iter().any(|position| {
        program
            .state_parameters(state)
            .get(*position as usize)
            .is_none_or(|parameter| {
                type_graph_requires_nominal_drop(program, parameter.type_reference)
            })
    }) {
        return None;
    }
    Some(positions)
}

pub(super) fn terminal_field_identity(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<String> {
    program.data_definitions().iter().find_map(|definition| {
        program.data_members(definition).iter().find_map(|member| {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == symbol).then(|| {
                field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned())
            })
        })
    })
}

pub(super) fn checked_state_contracts_supported(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
) -> bool {
    let source_parameters = program.state_parameters(state);
    program.state_contracts(state).iter().all(|contract| {
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
                                == Some(
                                    psi_language_semantics::DomainClassification::ProgressProfile,
                                )
                    }) {
                        let psi_facts::PlaceRoot::Symbol(root) = place.root else {
                            return false;
                        };
                        let rooted_in_telescope = source_parameters.iter().any(|parameter| {
                            parameter_root_symbol(machine.symbol, parameter) == root
                                || parameter.symbol == root
                        });
                        return rooted_in_telescope
                            && place.segments.iter().all(|segment| {
                                matches!(segment, psi_facts::PlaceSegment::Field { .. })
                            });
                    }
                    if !place.segments.is_empty() {
                        return false;
                    }
                    let psi_facts::PlaceRoot::Symbol(root) = place.root else {
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
                    psi_typed_trees::expression::ExpressionNode::Boolean(true)
                ),
                _ => false,
            })
    })
}

/// A static boundary requirement may carry the implicit membership contracts
/// induced by qualified parameter types, but no independently authored proof
/// contract in this Terminal slice. Reconstruct the exact pair set from both
/// surfaces so an omitted or extra membership cannot be hidden by lowering.
pub(super) fn signature_contracts_are_exact_parameter_qualifications(
    program: &TypedTrees,
    signature: &psi_typed_trees::signature::StateSignature,
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
                                            psi_language_semantics::DomainClassification::ProgressProfile,
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
        if contract.kind != SignatureContractKind::Requires || contract.binding.is_some() {
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
                    || (parameter.is_self && name.as_str() == "self")
            }) else {
                return false;
            };
            // A provider receiver's progress profile is an installation-time
            // premise, not an outbound structural ABI qualification.
            if parameters[position].is_self
                && program.domain_definitions().iter().any(|domain| {
                    domain.symbol == membership.domain_symbol
                        && domain.classification
                            == Some(psi_language_semantics::DomainClassification::ProgressProfile)
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

pub(super) fn boundary_domain_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
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
        let psi_facts::PlaceRoot::Symbol(root) = place.root else {
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

pub(super) fn parameter_qualifications(
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
                for constraint in program.type_reference_table.constraints(*constraints) {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        return None;
                    };
                    if !domain.semantic_id.is_valid() {
                        return None;
                    }
                    shapes.add_domain(domain.semantic_id, *base_type, binders)?;
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

fn type_domain_semantic_id(
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

fn type_domain_carrier(
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

pub(super) fn state_flow<'a>(
    facts: &'a CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<&'a psi_checked_trees::FlowStateFact> {
    facts.flow.control.states.iter().find_map(|(_, candidate)| {
        (candidate.machine_symbol == machine && candidate.state_symbol == state)
            .then_some(candidate)
    })
}

pub(super) fn machine_binders(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Vec<(SymbolHandle, String)> {
    program
        .machine_type_parameters(machine)
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.symbol, format!("$T{index}")))
        .collect()
}

pub(super) fn parameter_root_symbol(
    machine: SymbolHandle,
    parameter: &StateParameter,
) -> SymbolHandle {
    if parameter.is_self {
        machine
    } else {
        parameter.symbol
    }
}

pub(super) fn is_reference(program: &TypedTrees, mut type_reference: TypeReferenceHandle) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { .. } => return true,
            _ => return false,
        }
    }
}

pub(super) fn structural_access_for_type_reference(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<CheckedStructuralAccess> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference {
                access, referee, ..
            } => {
                // A structural carrier represents one referent occurrence.
                // Nested reference values need a separate pointer-as-data
                // shape and therefore remain outside this rung.
                if is_reference(program, *referee) {
                    return None;
                }
                return Some(match access {
                    psi_language_semantics::ReferenceAccess::Shared => {
                        CheckedStructuralAccess::SharedBorrow
                    }
                    psi_language_semantics::ReferenceAccess::Mutable => {
                        CheckedStructuralAccess::MutableBorrow
                    }
                    psi_language_semantics::ReferenceAccess::WriteOnly => {
                        CheckedStructuralAccess::WriteOnlyBorrow
                    }
                });
            }
            _ => return Some(CheckedStructuralAccess::Owned),
        }
    }
}

pub(super) fn byte_sequence_type_identity(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<String> {
    let carrier = byte_sequence_carrier(program, type_reference, substitutions)?;
    let mut identity_type = type_reference;
    if carrier == psi_checked_trees::CheckedByteSequenceCarrier::BorrowedView {
        loop {
            match program.type_reference_table.type_reference(identity_type) {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => identity_type = *referee,
                _ => break,
            }
        }
    }
    Some(
        program
            .normalized_type_identity_with_binders_and_substitutions(
                identity_type,
                binders,
                substitutions,
            )
            .into_string(),
    )
}

/// True when automatic disposal would have to run a reachable nominal
/// `::drop`. The currently accepted Terminal Psi cleanup carrier represents
/// only checked no-code affine disposal, so producers must fail closed here.
pub(in crate::flow) fn type_graph_requires_nominal_drop(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    type_graph_requires_nominal_drop_with_substitutions(
        program,
        type_reference,
        &[],
        &mut BTreeSet::new(),
    )
}

pub(super) fn type_graph_requires_nominal_drop_with_substitutions(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visited: &mut BTreeSet<String>,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => return false,
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    break;
                };
                if *replacement == type_reference {
                    return false;
                }
                type_reference = *replacement;
            }
            _ => break,
        }
    }

    let identity = program
        .normalized_type_identity_with_binders_and_substitutions(type_reference, &[], substitutions)
        .into_string();
    if !visited.insert(identity) {
        return false;
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => false,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_graph_requires_nominal_drop_with_substitutions(
                program,
                *base_type,
                substitutions,
                visited,
            )
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_graph_requires_nominal_drop_with_substitutions(
                program,
                *element_type,
                substitutions,
                visited,
            )
        }
        TypeReferenceNode::Named { symbol, name } => program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol || data.name.as_str() == name.as_str())
            .is_some_and(|data| {
                data_graph_requires_nominal_drop_with_substitutions(
                    program,
                    data,
                    substitutions,
                    visited,
                )
            }),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let Some(data) = program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == *base_symbol)
            else {
                return false;
            };
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let parameters = program.data_type_parameters(data);
            if arguments.len() != parameters.len() {
                return false;
            }
            let mut nested_substitutions = substitutions.to_vec();
            nested_substitutions.extend(
                parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
            data_graph_requires_nominal_drop_with_substitutions(
                program,
                data,
                &nested_substitutions,
                visited,
            )
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

pub(super) fn data_graph_requires_nominal_drop_with_substitutions(
    program: &TypedTrees,
    data: &psi_typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visited: &mut BTreeSet<String>,
) -> bool {
    if program.machines().iter().any(|machine| {
        machine.name.as_str().ends_with("::drop")
            && machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| attached == &data.name)
    }) {
        return true;
    }
    program
        .data_members(data)
        .iter()
        .any(|member| match member {
            DataMember::Field(field) => type_graph_requires_nominal_drop_with_substitutions(
                program,
                field.type_reference,
                substitutions,
                visited,
            ),
            DataMember::Variant(variant) => {
                program.data_payload_fields(variant).iter().any(|field| {
                    type_graph_requires_nominal_drop_with_substitutions(
                        program,
                        field.type_reference,
                        substitutions,
                        visited,
                    )
                })
            }
        })
}

pub(super) fn is_unit(program: &TypedTrees, mut type_reference: TypeReferenceHandle) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Unit => return true,
            _ => return false,
        }
    }
}

pub(super) fn base_type_identity(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<String> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => type_reference = *referee,
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::FixedArray {
                length: psi_typed_trees::types::FixedArrayLength::Literal(_),
                ..
            } => {
                return Some(
                    program
                        .normalized_type_identity_with_binders(type_reference, binders)
                        .into_string(),
                );
            }
            _ => return None,
        }
    }
}

pub(super) fn attached_data_identity(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<String> {
    let name = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *name)?;
    if !program.data_type_parameters(data).is_empty() {
        return None;
    }
    let path = program.symbols.display_path(data.symbol, "::");
    Some(format!("named({})", normalized_atom("name", &path)))
}

pub(super) struct ShapeCollector<'program> {
    pub(super) program: &'program TypedTrees,
    pub(super) types: BTreeMap<String, CheckedUnitStructuralTypePlan>,
    pub(super) domains: Vec<CheckedUnitStructuralDomainPlan>,
    in_progress: BTreeSet<String>,
}

impl<'program> ShapeCollector<'program> {
    pub(super) fn new(program: &'program TypedTrees) -> Self {
        Self {
            program,
            types: BTreeMap::new(),
            domains: Vec::new(),
            in_progress: BTreeSet::new(),
        }
    }

    pub(super) fn add_domain(
        &mut self,
        domain: SemanticDomainId,
        carrier: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<()> {
        let carrier_type_identity = self.add_type(carrier, binders, &[])?;
        let identity = self.program.semantic_domains.name(domain)?.to_owned();
        let plan = CheckedUnitStructuralDomainPlan {
            domain,
            identity,
            carrier_type_identity,
        };
        if let Some(existing) = self
            .domains
            .iter()
            .find(|existing| existing.domain == domain)
        {
            return (existing == &plan).then_some(());
        }
        self.domains.push(plan);
        Some(())
    }

    pub(super) fn add_attached_data(
        &mut self,
        data: &psi_typed_trees::data::DataDefinition,
        binders: &[(SymbolHandle, String)],
    ) -> Option<String> {
        if !self.program.data_type_parameters(data).is_empty() {
            // A static attached machine does not carry an instantiated type
            // argument tuple. Generic attached data therefore needs a later
            // explicit checked identity fact rather than guessed binding.
            return None;
        }
        let path = self.program.symbols.display_path(data.symbol, "::");
        let identity = format!("named({})", normalized_atom("name", &path));
        self.add_data_shape(identity, data.clone(), binders, Vec::new())
    }

    pub(super) fn add_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<String> {
        if let Some(carrier) = byte_sequence_carrier(self.program, type_reference, substitutions) {
            let identity =
                byte_sequence_type_identity(self.program, type_reference, binders, substitutions)?;
            let plan = CheckedUnitStructuralTypePlan {
                identity: identity.clone(),
                shape: CheckedUnitStructuralTypeShape::ByteSequence(carrier),
            };
            if self
                .types
                .get(&identity)
                .is_some_and(|existing| existing != &plan)
            {
                return None;
            }
            self.types.insert(identity.clone(), plan);
            return Some(identity);
        }
        let mut type_reference = type_reference;
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => type_reference = *referee,
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some((_, replacement)) = substitutions
                        .iter()
                        .rev()
                        .find(|(parameter, _)| parameter == symbol)
                    {
                        type_reference = *replacement;
                        continue;
                    }
                    break;
                }
                _ => break,
            }
        }
        let identity = self
            .program
            .normalized_type_identity_with_binders(type_reference, binders)
            .into_string();
        if self.types.contains_key(&identity) {
            return Some(identity);
        }
        if let Some(primitive) = self.program.primitive_type_reference(type_reference) {
            self.types.insert(
                identity.clone(),
                CheckedUnitStructuralTypePlan {
                    identity: identity.clone(),
                    shape: CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive),
                },
            );
            return Some(identity);
        }
        if let TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
        } = self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            if *length == 0
                || !substitutions.is_empty()
                || !matches!(
                    self.program
                        .type_reference_table
                        .type_reference(*element_type),
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
                )
                || crate::checks::type_multiplicity(self.program, *element_type)
                    != Multiplicity::Linear
                || !self.in_progress.insert(identity.clone())
            {
                return None;
            }
            let Some(element_type_identity) = self.add_type(*element_type, binders, substitutions)
            else {
                self.in_progress.remove(&identity);
                return None;
            };
            let length = u64::try_from(*length).ok()?;
            self.types.insert(
                identity.clone(),
                CheckedUnitStructuralTypePlan {
                    identity: identity.clone(),
                    shape: CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        length,
                    },
                },
            );
            self.in_progress.remove(&identity);
            return Some(identity);
        }
        let (data_symbol, arguments) = match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Named { symbol, name }
                if PrimitiveType::from_name(name.as_str()).is_none() =>
            {
                (*symbol, Vec::new())
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => (
                *base_symbol,
                self.program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .to_vec(),
            ),
            _ => return None,
        };
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == data_symbol)?
            .clone();
        let members = self.program.data_members(&data);
        if data.supply_mode != psi_language_semantics::DataSupplyMode::CheckedShape
            || !matches!(
                psi_typed_trees::data::DataDefinition::shape_kind_from_members(members),
                DataShapeKind::Empty
                    | DataShapeKind::Record
                    | DataShapeKind::Enum
                    | DataShapeKind::Mixed
            )
        {
            return None;
        }
        let data_parameters = self.program.data_type_parameters(&data);
        if data_parameters.len() != arguments.len() {
            return None;
        }
        let mut local_substitutions = substitutions.to_vec();
        local_substitutions.extend(
            data_parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.symbol, argument)),
        );
        self.add_data_shape(identity, data, binders, local_substitutions)
    }

    pub(super) fn add_partial_affine_array_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<String> {
        self.add_bounded_affine_array_type(type_reference, binders, false)
    }

    pub(super) fn add_affine_array_construction_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<String> {
        self.add_bounded_affine_array_type(type_reference, binders, true)
    }

    fn add_bounded_affine_array_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        allow_construction_lengths: bool,
    ) -> Option<String> {
        let mut resolved = type_reference;
        loop {
            match self.program.type_reference_table.type_reference(resolved) {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => resolved = *referee,
                _ => break,
            }
        }
        let TypeReferenceNode::FixedArray {
            element_type,
            length:
                psi_typed_trees::types::FixedArrayLength::Literal(length @ (2 | 3 | 4 | 5 | 6 | 7)),
        } = self.program.type_reference_table.type_reference(resolved)
        else {
            return self.add_type(type_reference, binders, &[]);
        };
        if *length >= 5 && !allow_construction_lengths {
            return None;
        }
        let nested_element = match self
            .program
            .type_reference_table
            .type_reference(*element_type)
        {
            TypeReferenceNode::FixedArray {
                element_type: leaf_type,
                length: psi_typed_trees::types::FixedArrayLength::Literal(inner_length @ (3 | 4)),
            } if *length == 2 => Some((*leaf_type, *inner_length)),
            _ => None,
        };
        if let Some((leaf_type, inner_length)) = nested_element {
            let inner_length = u64::try_from(inner_length).ok()?;
            if crate::checks::type_multiplicity(self.program, leaf_type) != Multiplicity::Affine
                || !matches!(
                    self.program.type_reference_table.type_reference(leaf_type),
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
                )
            {
                return None;
            }
            let identity = self
                .program
                .normalized_type_identity_with_binders(resolved, binders)
                .into_string();
            if self.types.contains_key(&identity) {
                return Some(identity);
            }
            if !self.in_progress.insert(identity.clone()) {
                return None;
            }
            let inner_identity = self
                .program
                .normalized_type_identity_with_binders(*element_type, binders)
                .into_string();
            if !self.types.contains_key(&inner_identity) {
                if !self.in_progress.insert(inner_identity.clone()) {
                    self.in_progress.remove(&identity);
                    return None;
                }
                let Some(leaf_identity) = self.add_type(leaf_type, binders, &[]) else {
                    self.in_progress.remove(&inner_identity);
                    self.in_progress.remove(&identity);
                    return None;
                };
                if !matches!(
                    self.types.get(&leaf_identity).map(|plan| &plan.shape),
                    Some(CheckedUnitStructuralTypeShape::Record { .. })
                ) {
                    self.in_progress.remove(&inner_identity);
                    self.in_progress.remove(&identity);
                    return None;
                }
                self.types.insert(
                    inner_identity.clone(),
                    CheckedUnitStructuralTypePlan {
                        identity: inner_identity.clone(),
                        shape: CheckedUnitStructuralTypeShape::FixedArray {
                            element_type_identity: leaf_identity,
                            length: inner_length,
                        },
                    },
                );
                self.in_progress.remove(&inner_identity);
            }
            if !matches!(
                self.types.get(&inner_identity).map(|plan| &plan.shape),
                Some(CheckedUnitStructuralTypeShape::FixedArray { length, .. })
                    if *length == inner_length
            ) {
                self.in_progress.remove(&identity);
                return None;
            }
            self.types.insert(
                identity.clone(),
                CheckedUnitStructuralTypePlan {
                    identity: identity.clone(),
                    shape: CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity: inner_identity,
                        length: 2,
                    },
                },
            );
            self.in_progress.remove(&identity);
            return Some(identity);
        }
        if crate::checks::type_multiplicity(self.program, *element_type) != Multiplicity::Affine
            || !matches!(
                self.program
                    .type_reference_table
                    .type_reference(*element_type),
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
            )
        {
            return None;
        }
        let identity = self
            .program
            .normalized_type_identity_with_binders(resolved, binders)
            .into_string();
        if self.types.contains_key(&identity) {
            return Some(identity);
        }
        if !self.in_progress.insert(identity.clone()) {
            return None;
        }
        let Some(element_type_identity) = self.add_type(*element_type, binders, &[]) else {
            self.in_progress.remove(&identity);
            return None;
        };
        if !matches!(
            self.types
                .get(&element_type_identity)
                .map(|plan| &plan.shape),
            Some(CheckedUnitStructuralTypeShape::Record { .. })
        ) {
            self.in_progress.remove(&identity);
            return None;
        }
        self.types.insert(
            identity.clone(),
            CheckedUnitStructuralTypePlan {
                identity: identity.clone(),
                shape: CheckedUnitStructuralTypeShape::FixedArray {
                    element_type_identity,
                    length: u64::try_from(*length).ok()?,
                },
            },
        );
        self.in_progress.remove(&identity);
        Some(identity)
    }

    pub(super) fn add_data_shape(
        &mut self,
        identity: String,
        data: psi_typed_trees::data::DataDefinition,
        binders: &[(SymbolHandle, String)],
        substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    ) -> Option<String> {
        if self.types.contains_key(&identity) {
            return Some(identity);
        }
        if !self.in_progress.insert(identity.clone()) {
            return None;
        }
        let members = self.program.data_members(&data).to_vec();
        if data.supply_mode != psi_language_semantics::DataSupplyMode::CheckedShape
            || !matches!(
                psi_typed_trees::data::DataDefinition::shape_kind_from_members(&members),
                DataShapeKind::Empty
                    | DataShapeKind::Record
                    | DataShapeKind::Enum
                    | DataShapeKind::Mixed
            )
        {
            self.in_progress.remove(&identity);
            return None;
        }
        let shape_kind = psi_typed_trees::data::DataDefinition::shape_kind_from_members(&members);
        if matches!(shape_kind, DataShapeKind::Enum | DataShapeKind::Mixed) {
            let mut fields = Vec::new();
            let mut cases = Vec::with_capacity(members.len());
            for member in &members {
                match member {
                    DataMember::Field(field) => {
                        let Some(field) =
                            self.structural_field_plan(field, binders, &substitutions, &identity)
                        else {
                            self.in_progress.remove(&identity);
                            return None;
                        };
                        fields.push(field);
                    }
                    DataMember::Variant(variant) => {
                        let mut payload_fields = Vec::new();
                        for field in self.program.data_payload_fields(variant) {
                            let Some(field) = self.structural_field_plan(
                                field,
                                binders,
                                &substitutions,
                                &identity,
                            ) else {
                                self.in_progress.remove(&identity);
                                return None;
                            };
                            payload_fields.push(field);
                        }
                        cases.push(psi_checked_trees::CheckedUnitStructuralCasePlan {
                            identity: variant
                                .identity
                                .map(|identity| format!("#{identity}"))
                                .unwrap_or_else(|| variant.name.as_str().to_owned()),
                            fields: payload_fields,
                        });
                    }
                }
            }
            self.types.insert(
                identity.clone(),
                CheckedUnitStructuralTypePlan {
                    identity: identity.clone(),
                    shape: if matches!(shape_kind, DataShapeKind::Mixed) {
                        CheckedUnitStructuralTypeShape::Mixed { fields, cases }
                    } else {
                        CheckedUnitStructuralTypeShape::Sum { cases }
                    },
                },
            );
            self.in_progress.remove(&identity);
            return Some(identity);
        }

        let mut fields = Vec::new();
        for member in &members {
            let DataMember::Field(field) = member else {
                self.in_progress.remove(&identity);
                return None;
            };
            let Some(field) = self.structural_field_plan(field, binders, &substitutions, &identity)
            else {
                self.in_progress.remove(&identity);
                return None;
            };
            fields.push(field);
        }
        self.types.insert(
            identity.clone(),
            CheckedUnitStructuralTypePlan {
                identity: identity.clone(),
                shape: CheckedUnitStructuralTypeShape::Record { fields },
            },
        );
        self.in_progress.remove(&identity);
        Some(identity)
    }

    fn structural_field_plan(
        &mut self,
        field: &psi_typed_trees::data::DataField,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        owner_identity: &str,
    ) -> Option<CheckedUnitStructuralFieldPlan> {
        let field_type = if field.relevance.is_erased() {
            CheckedUnitStructuralFieldType::Erased {
                type_identity: self
                    .program
                    .normalized_type_identity_with_binders_and_substitutions(
                        field.type_reference,
                        binders,
                        substitutions,
                    )
                    .into_string(),
            }
        } else if let Some(provider_symbol) = match self
            .program
            .type_reference_table
            .type_reference(field.type_reference)
        {
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::DynamicTrait { symbol, .. } => Some(*symbol),
            _ => None,
        } && self
            .program
            .traits()
            .iter()
            .any(|definition| definition.symbol == provider_symbol && definition.is_boundary)
        {
            CheckedUnitStructuralFieldType::ProviderBacked {
                provider_type_identity: self
                    .program
                    .normalized_type_identity_with_binders_and_substitutions(
                        field.type_reference,
                        binders,
                        substitutions,
                    )
                    .into_string(),
            }
        } else if let Some(carrier) =
            byte_sequence_carrier(self.program, field.type_reference, substitutions)
        {
            CheckedUnitStructuralFieldType::ByteSequence(carrier)
        } else {
            match scalar_type(self.program, field.type_reference, substitutions) {
                Some(primitive) => CheckedUnitStructuralFieldType::Scalar(primitive),
                None => {
                    let nested = self.add_type(field.type_reference, binders, substitutions)?;
                    if nested == owner_identity {
                        return None;
                    }
                    CheckedUnitStructuralFieldType::Structural {
                        type_identity: nested,
                    }
                }
            }
        };
        Some(CheckedUnitStructuralFieldPlan {
            identity: field
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| field.name.as_str().to_owned()),
            relevance: field.relevance,
            field_type,
        })
    }

    pub(super) fn retain_transitive(&mut self, roots: &BTreeSet<&str>) {
        let mut retained = roots
            .iter()
            .map(|root| (*root).to_owned())
            .collect::<BTreeSet<_>>();
        loop {
            let old_len = retained.len();
            for identity in retained.clone() {
                let Some(plan) = self.types.get(&identity) else {
                    continue;
                };
                match &plan.shape {
                    CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
                    | CheckedUnitStructuralTypeShape::ByteSequence(_) => {}
                    CheckedUnitStructuralTypeShape::Record { fields } => {
                        for field in fields {
                            if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                                &field.field_type
                            {
                                retained.insert(type_identity.clone());
                            }
                        }
                    }
                    CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        ..
                    } => {
                        retained.insert(element_type_identity.clone());
                    }
                    CheckedUnitStructuralTypeShape::Sum { cases } => {
                        for field in cases.iter().flat_map(|case| &case.fields) {
                            if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                                &field.field_type
                            {
                                retained.insert(type_identity.clone());
                            }
                        }
                    }
                    CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                        for field in fields
                            .iter()
                            .chain(cases.iter().flat_map(|case| &case.fields))
                        {
                            if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                                &field.field_type
                            {
                                retained.insert(type_identity.clone());
                            }
                        }
                    }
                }
            }
            if retained.len() == old_len {
                break;
            }
        }
        self.types.retain(|identity, _| retained.contains(identity));
        self.domains
            .retain(|domain| retained.contains(&domain.carrier_type_identity));
    }
}

pub(super) fn scalar_type(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<PrimitiveType> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, name } => {
                if let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                {
                    type_reference = *replacement;
                    continue;
                }
                return PrimitiveType::from_name(name.as_str());
            }
            _ => return None,
        }
    }
}

pub(crate) fn byte_sequence_carrier(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<psi_checked_trees::CheckedByteSequenceCarrier> {
    let mut borrowed = false;
    let mut has_domain = false;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    break;
                };
                type_reference = *replacement;
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                has_domain |= program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)));
                type_reference = *base_type;
            }
            TypeReferenceNode::Reference { referee, .. } if !borrowed => {
                borrowed = true;
                type_reference = *referee;
            }
            _ => break,
        }
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Slice { element_type }
            if borrowed
                && program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8) =>
        {
            Some(psi_checked_trees::CheckedByteSequenceCarrier::BorrowedView)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(capacity),
        } if !borrowed
            && has_domain
            && program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8) =>
        {
            Some(
                psi_checked_trees::CheckedByteSequenceCarrier::BoundedOwned {
                    capacity: u64::try_from(*capacity).ok()?,
                },
            )
        }
        _ => None,
    }
}

pub(super) fn normalized_atom(tag: &str, value: &str) -> String {
    let mut output = String::with_capacity(tag.len() + value.len() + 2);
    output.push_str(tag);
    output.push('(');
    for character in value.chars() {
        if matches!(character, '\\' | '(' | ')' | ',') {
            output.push('\\');
        }
        output.push(character);
    }
    output.push(')');
    output
}
