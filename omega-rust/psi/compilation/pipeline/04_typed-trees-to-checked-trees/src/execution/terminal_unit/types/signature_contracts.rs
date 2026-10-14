//! Signature contracts and domain qualifications a checked structural parameter
//! or result carries, and the domain requirements boundary machines publish.

use super::*;

pub(in crate::execution::terminal_unit) fn checked_state_contracts_have_plan(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
) -> bool {
    let Some(plan) = facts.contract_plans.for_machine(machine) else {
        return false;
    };
    program.state_contracts(state).iter().all(|contract| {
        if contract.kind == SignatureContractKind::Requires {
            // The invocation collector retains the complete predicate roster,
            // including scalar membership subjects. Structural qualifications
            // stay on their parameter declarations. Lowering publishes this
            // same roster for every result kind; a second structural-only
            // admission rule here used to discard valid scalar requirements
            // whenever the body returned an aggregate.
            return contract.binding.is_none()
                && plan.crash.structural_runtime_requirements().is_some();
        }
        checked_structural_signature_has_plan(program, state, structural_parameters, contract)
    })
}

pub(in crate::execution::terminal_unit) fn checked_structural_signature_has_plan(
    program: &dyn ::validation::CheckingInput,
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
                let Some(place) = validation::canonical_place_from_expression_in_state(
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
                    let Some(root) = place.root.as_symbol() else {
                        return false;
                    };
                    let rooted_in_telescope = source_parameters
                        .iter()
                        .any(|parameter| parameter_is_root(parameter, root));
                    return rooted_in_telescope
                        && place
                            .segments
                            .iter()
                            .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. }));
                }
                if !place.segments.is_empty() {
                    return false;
                }
                let Some(root) = place.root.as_symbol() else {
                    return false;
                };
                let Some(position) = source_parameters
                    .iter()
                    .position(|parameter| parameter_is_root(parameter, root))
                else {
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

pub(in crate::execution::terminal_unit) fn boundary_domain_requirements(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<CheckedUnitStructuralDomainRequirementPlan>> {
    let source_parameters = program.state_parameters(state);
    // A satisfying machine's state also carries the rows its conformance
    // inherits from the trait requirement. Those restate a clause the
    // conformance check already matched against this signature, and their
    // membership expressions name the requirement's own formals, not this
    // state's parameters; the authored clauses alone describe the plan.
    let checked_requires = facts
        .proof
        .contract_facts
        .iter()
        .filter(|(_, fact)| {
            fact.kind == ContractProofFactKind::Requires
                && fact.inherited_scope.is_none()
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
        let Some(membership) = program.proof_facts.get(checked.fact).as_membership() else {
            // Authored and implicit entry predicates use the scalar lane.
            // Its complete clause projection refuses unsupported facts rather
            // than dropping a requirement this structural roster cannot carry.
            continue;
        };
        if validation::scalar_membership_carrier(program, membership).is_some() {
            // A scalar subject may itself be an expression. Its declaration-
            // local projection, not a structural root-only place recognizer,
            // establishes the subject in boundary_scalar_requires.
            continue;
        }
        let place = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            0,
            membership.value,
        )?;
        if !place.segments.is_empty() {
            return None;
        }
        let root = place.root.as_symbol()?;
        let source_position = source_parameters
            .iter()
            .position(|parameter| parameter_is_root(parameter, root))?;
        // Scalar qualifications use the complete predicate lane published by
        // boundary_scalar_requires, including implicit entry clauses. They do
        // not invent a structural argument or gain routed domain authority.
        if program
            .primitive_type_reference(source_parameters[source_position].type_reference)
            .is_some()
        {
            continue;
        }
        let argument_index = structural_parameters
            .iter()
            .position(|parameter| parameter.position as usize == source_position)?;
        let domain = type_domain_semantic_id(
            program,
            source_parameters[source_position].type_reference,
            membership.domain_symbol,
        )?;
        let definition = program.domain_definition(membership.domain_symbol)?;
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
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<SemanticDomainId>> {
    let mut output = qualification_carriers(program, shapes, type_reference, binders)?
        .into_iter()
        .map(|(domain, _)| domain)
        .collect::<Vec<_>>();
    output.sort_by_key(|domain| domain.0);
    output.dedup();
    Some(output)
}

/// Register each domain qualification on `type_reference` and pair it with
/// the type identity of the carrier it was registered against, in authored
/// order.
pub(in crate::execution::terminal_unit) fn qualification_carriers(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    mut type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<(SemanticDomainId, typed_trees::type_identity::TypeIdentityId)>> {
    // A borrowed byte view is its own storage: its identity looks through the
    // loan and qualification shells to `slice(u8)`, but only the whole view
    // registers that byte carrier. A qualification beneath the view therefore
    // names the view, so registration does not depend on some earlier
    // signature having registered the same carrier first.
    let borrowed_view = matches!(
        byte_sequence_carrier(program, type_reference, &[]),
        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
    )
    .then_some(type_reference);
    let mut output = Vec::new();
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    let domain = match constraint {
                        // Arithmetic policy remains in the exact type identity,
                        // but adds no semantic-domain obligation. This is true
                        // for aggregate contents as well as scalar parameters;
                        // any accompanying predicate keeps its own custody.
                        TypeConstraintNode::ArithmeticDomain(_) => continue,
                        TypeConstraintNode::Range { .. } => {
                            // A scalar range is a receiving restriction, not a
                            // nominal membership. Admit it only after the
                            // complete exact carrier retained that restriction;
                            // unresolved bounds cannot become an unqualified loan.
                            let identity = shapes.add_type(type_reference, binders, &[])?;
                            shapes.types.get(&identity)?.scalar_restriction.as_ref()?;
                            continue;
                        }
                        // Carry is permission on the value's existing claims,
                        // checked and retained by the carry channel. It names
                        // no value-domain membership or runtime storage theory.
                        TypeConstraintNode::Domain(domain)
                            if matches!(
                                domain.subject,
                                typed_trees::types::DomainConstraintSubject::Carry(_)
                            ) =>
                        {
                            continue;
                        }
                        TypeConstraintNode::Domain(domain) => domain,
                        _ => return None,
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
                        borrowed_view.unwrap_or(*base_type)
                    };
                    // The carrier is already peeled of this constraint, so
                    // name the declaration typing selected rather than let
                    // registration fall back to the family's first one.
                    let carrier_identity = shapes.add_selected_domain(
                        domain.semantic_id,
                        domain.symbol,
                        carrier,
                        binders,
                    )?;
                    output.push((domain.semantic_id, carrier_identity));
                }
                type_reference = *base_type;
            }
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            _ => break,
        }
    }
    Some(output)
}

/// Fold the domains an authorized `ensures result in <domain>` clause pins on
/// a boundary requirement into the result's structural qualifications. The
/// clause only counts when its fact replays the established qualification
/// authorization (a `BoundaryRequirement` establishment route or carry
/// permission naming this requirement). Source checking validates the subject
/// and carrier, while this join must refuse membership whose authorization or
/// result carrier it cannot retain. Ordinary Boolean guarantees grant no
/// qualification and do not enter this operation. A scalar or unit result
/// cannot carry a structural domain qualification, so folding onto one refuses the plan
/// rather than silently dropping the authored custody.
pub(in crate::execution::terminal_unit) fn fold_authorized_result_domains(
    program: &dyn ::validation::CheckingInput,
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
        for fact in contract.facts.handles() {
            let Some(membership) = program.proof_facts.get(fact).as_membership() else {
                continue;
            };
            if crate::facts::qualification_evidence::boundary_qualification_authorization(
                program,
                owner_symbol,
                signature,
                contract.kind.clone(),
                fact,
            )
            .is_none()
            {
                // A source membership guarantee cannot disappear from the
                // receiving signature or gain issuance authority through a
                // matching carrier. Retain its exact authorized route or refuse.
                return None;
            }
            // Authorization also admits mutable output parameters, but this
            // plan carries result qualifications only. Until that separate
            // output effect is represented, decline rather than moving its
            // membership onto the unrelated returned value or dropping it.
            if !program
                .expression_table
                .names_reserved_result(membership.value)
            {
                return None;
            }
            let Some(domain) = crate::facts::qualification_evidence::domain_definition(
                program,
                membership.domain_symbol,
            ) else {
                continue;
            };
            authorized.push((domain.semantic_id, domain.symbol));
        }
    }
    authorized
        .sort_by_key(|(domain, symbol)| (domain.0, symbol.arena_index(), symbol.generation()));
    authorized.dedup();
    if authorized.is_empty() {
        return Some(result);
    }
    let CheckedBoundaryMachineResultPlan::Structural {
        type_identity,
        multiplicity,
        mut qualifications,
        projected_qualifications,
    } = result
    else {
        return None;
    };
    for (domain, selected_symbol) in authorized {
        shapes.add_selected_domain(domain, selected_symbol, result_type, binders)?;
        if !qualifications.contains(&domain) {
            qualifications.push(domain);
        }
    }
    qualifications.sort_by_key(|domain| domain.0);
    Some(CheckedBoundaryMachineResultPlan::Structural {
        type_identity,
        multiplicity,
        qualifications,
        projected_qualifications,
    })
}

/// Register the carrier an authored `ensures result in <domain>` clause
/// qualifies on an ordinary or state-graph machine. The result declaration
/// and every receiving call publish that membership as a qualification of
/// the returned carrier, and the structural domain catalog can only lower a
/// domain whose declaration this collector holds. A boundary requirement
/// registers through `fold_authorized_result_domains`, which also replays
/// its authorization; a checked body proved its own clause. A membership
/// over another value names a different carrier and is skipped, and a scalar,
/// unit or reference result carries no structural qualification.
pub(in crate::execution::terminal_unit) fn register_authored_result_domains(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<()> {
    let return_type = state.return_type;
    if is_unit(program, return_type)
        || program.primitive_type_reference(return_type).is_some()
        || crate::execution::terminal_unit::is_reference(program, return_type)
    {
        return Some(());
    }
    for contract in program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
    {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(membership) = fact.as_membership() else {
                continue;
            };
            if !program
                .expression_table
                .names_reserved_result(membership.value)
            {
                continue;
            }
            let Some(definition) = program.domain_definition(membership.domain_symbol) else {
                continue;
            };
            shapes.add_selected_domain(
                definition.semantic_id,
                definition.symbol,
                return_type,
                binders,
            )?;
        }
    }
    Some(())
}

/// Body commitments whose theory belongs to structural carriers. Classify
/// normalized domain instances by their carrier, not by their predicates or
/// the arrangement of an authored cast. Scalar computations retain scalar
/// membership and its proof; a structural request would duplicate that owner.
/// This query assigns ownership only and establishes no membership evidence.
pub(in crate::execution::terminal_unit) fn body_structural_qualifications(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
) -> Vec<SemanticDomainId> {
    let mut domains = facts
        .qualifications
        .for_machine(machine)
        .map(|fact| {
            fact.body_committed
                .iter()
                .copied()
                .filter(|domain| {
                    !matches!(
                        *domain,
                        language_semantics::SemanticDomainTable::WRAPPING
                            | language_semantics::SemanticDomainTable::SATURATING
                            | language_semantics::SemanticDomainTable::TRAPPING
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if domains.is_empty() {
        return domains;
    }
    let scalar_domains = program
        .type_reference_table
        .constrained_type_references()
        .into_iter()
        .filter(|(carrier, _)| program.primitive_type_reference(*carrier).is_some())
        .flat_map(|(_, constraints)| program.type_reference_table.constraints(constraints))
        .filter_map(|constraint| match constraint {
            TypeConstraintNode::Domain(domain) => Some(domain.semantic_id.0),
            _ => None,
        })
        // A transparent alias's own declaration identity may no longer be
        // a normalized type atom. Its declared carrier still owns it; closed
        // indexed instances above retain their concrete carrier independently.
        .chain(program.domain_definitions().iter().filter_map(|domain| {
            program
                .primitive_type_reference(domain.target_type)
                .map(|_| domain.semantic_id.0)
        }))
        .collect::<BTreeSet<_>>();
    domains.retain(|domain| !scalar_domains.contains(&domain.0));
    // A vacuous cast whose structural value this body produces is owned by
    // that value: its checked membership travels with the produced place.
    // Only a commitment with no such producer remains a body request.
    let (owned, unowned): (Vec<&checked_trees::VacuousQualificationUse>, Vec<_>) = facts
        .qualifications
        .vacuous_uses
        .iter()
        .filter(|use_| use_.machine == machine)
        .partition(|use_| {
            facts
                .values
                .structural_values
                .nodes
                .iter()
                .any(|(_, node)| {
                    node.machine == machine
                        && node.state == use_.state
                        && node.statement_index == use_.statement_index
                        && node.expression == use_.expression
                })
        });
    domains.retain(|domain| {
        !owned.iter().any(|use_| use_.semantic_domain == *domain)
            || unowned.iter().any(|use_| use_.semantic_domain == *domain)
    });
    domains.sort_by_key(|domain| domain.0);
    domains.dedup();
    domains
}

/// Collect domain constraints below a structural root without treating them
/// as root qualifications. Fixed-array indexes are the canonical source path
/// for each occurrence; unsupported dynamic shapes remain absent from plans.
pub(in crate::execution::terminal_unit) fn projected_parameter_qualifications(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<CheckedStructuralPathQualification>> {
    let (output, carriers) =
        validation::structural_qualification_contract(program, type_reference).ok()?;
    for (domain, carrier) in carriers {
        shapes.add_domain(domain, carrier, binders)?;
    }
    Some(output)
}

pub(super) fn type_domain_semantic_id(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    domain: SymbolHandle,
) -> Option<SemanticDomainId> {
    fn collect(
        program: &dyn ::validation::CheckingInput,
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
            .domain_definition(domain)
            .filter(|definition| definition.index_arguments.is_empty())
            .map(|definition| definition.semantic_id)
            .filter(|semantic_id| semantic_id.is_valid()),
        _ => None,
    }
}

pub(super) fn type_domain_carrier(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    domain: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    fn collect(
        program: &dyn ::validation::CheckingInput,
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
