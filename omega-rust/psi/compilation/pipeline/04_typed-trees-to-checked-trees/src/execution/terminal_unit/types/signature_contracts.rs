//! Signature contracts and domain qualifications a checked structural parameter
//! or result carries, and the domain requirements boundary machines publish.

use super::*;

/// Invocation clauses retain their own represented erased lanes. Their
/// presence is independent of a scalar body's result operation. Internal
/// state hypotheses are not invocation inputs and remain under the separate
/// state/arrival owner; this ordinary body cannot promote their bindings.
pub(in crate::execution::terminal_unit) fn checked_machine_signature_has_plan(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> bool {
    if program.machine_contracts(machine).len() != machine.contracts.len()
        || program.state_contracts(state).len() != state.contracts.len()
    {
        return false;
    }
    let ordinary_kind = |contract: &typed_trees::signature::SignatureContract| {
        matches!(
            contract.kind,
            SignatureContractKind::Crashes { .. }
                | SignatureContractKind::Requires
                | SignatureContractKind::Ensures
        )
    };
    program.machine_contracts(machine).iter().all(|contract| {
        if !ordinary_kind(contract) {
            return false;
        }
        if contract.binding.is_none() {
            return true;
        }
        let declared = program.proof_facts.span_or_empty(contract.facts);
        if declared.is_empty() || declared.len() != contract.facts.len() {
            return false;
        }
        contract.facts.handles().all(|fact| {
            let recorded = match contract.kind {
                SignatureContractKind::Requires => {
                    facts
                        .proof
                        .machine_requires_fact(program.typed_program(), machine.symbol, fact)
                }
                SignatureContractKind::Ensures => {
                    facts
                        .proof
                        .machine_ensures_fact(program.typed_program(), machine.symbol, fact)
                }
                _ => None,
            };
            recorded.is_some_and(|row| row.evidence_term.is_some())
        })
    }) && program
        .state_contracts(state)
        .iter()
        .all(|contract| ordinary_kind(contract) && contract.binding.is_none())
}

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
                if let Some(valid) =
                    signature_progress_requirement(program, source_parameters, membership)
                {
                    return valid;
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

/// Progress profiles retain exact checked termination subjects and supply no
/// runtime predicates. Structural signature publication independently retains
/// an authored formal's existing membership, without introducing an issuer or
/// deriving a scalar predicate. Arbitrary projected memberships still refuse.
/// Canonical termination.md 6a14afff75d92a16d3e6a0adfeee18f0fbbe8727:
/// "Progress evidence follows exact identity-preserving lineage, explicit contract
/// substitution, or checked qualification-preserving transitions."
pub(in crate::execution::terminal_unit) fn signature_progress_requirement(
    program: &dyn ::validation::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    membership: &typed_trees::domain::ProofMembershipFact,
) -> Option<bool> {
    if !program.domain_definitions().iter().any(|domain| {
        domain.symbol == membership.domain_symbol
            && domain.classification
                == Some(language_semantics::DomainClassification::ProgressProfile)
    }) {
        return None;
    }
    Some(
        validation::canonical_place_from_expression(program, membership.value).is_some_and(
            |place| {
                place.root.as_symbol().is_some_and(|root| {
                    parameters
                        .iter()
                        .any(|parameter| parameter_is_root(parameter, root))
                }) && place
                    .segments
                    .iter()
                    .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. }))
            },
        ),
    )
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
        Some(
            checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
                | checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }
        )
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
                    let carrier =
                        if matches!(
                            byte_sequence_carrier(program, type_reference, &[]),
                            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. })
                        ) || (program.primitive_type_reference(type_reference).is_some()
                            && validation::structural_identity_type(program, type_reference, &[])
                                == type_reference)
                        {
                            // Receiving scalar restrictions retain their exact
                            // identity. The membership theory must name that same
                            // carrier at parameter and call boundaries; peeling
                            // it here would register only an unrelated plain
                            // scalar presentation. This registers no membership.
                            type_reference
                        } else {
                            borrowed_view.unwrap_or(*base_type)
                        };
                    // Name the declaration typing selected for this exact
                    // carrier, whether its receiving restriction remains or
                    // its membership shell was peeled. Registration must not
                    // fall back to the family's first declaration.
                    let carrier_identity =
                        shapes.add_domain_application(domain, carrier, binders, false)?;
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

/// Retain a boundary's exact result-membership contract on its carrier. This
/// is signature transport, not issuance: a crossing may forward existing
/// custody, derive it by conservation, or introduce it through an authorized
/// route. The call-result custody checker owns that distinction and provider
/// installation retains its separate admission obligation. Requiring an
/// issuance route here incorrectly rejected preserved input qualifications.
/// Non-result outputs and nonstructural carriers still need their own plans;
/// never move their obligations onto a different returned value.
pub(in crate::execution::terminal_unit) fn fold_boundary_result_domains(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    signature: &typed_trees::signature::StateSignature,
    result_type: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    result: CheckedBoundaryMachineResultPlan,
) -> Option<CheckedBoundaryMachineResultPlan> {
    let mut guaranteed = Vec::new();
    for contract in program.state_signature_contracts(signature) {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for fact in contract.facts.handles() {
            let Some(membership) = program.proof_facts.get(fact).as_membership() else {
                continue;
            };
            if validation::reserved_result_owner(program, membership.value)
                .is_none_or(|(owner, _)| owner != signature.symbol)
            {
                return None;
            }
            // A membership whose domain reference declares no value-domain theory
            // names a carry permission. Carry travels with claim custody, which
            // the carry channel checks and retains; it publishes no structural
            // qualification and its authorized route still reaches that
            // channel. Denying the result plan here refused every requirement
            // whose result contract established a carry permission.
            let Some(domain) = crate::facts::qualification_evidence::domain_definition(
                program,
                membership.domain_symbol,
            ) else {
                continue;
            };
            guaranteed.push((membership.semantic_domain, domain.symbol));
        }
    }
    guaranteed
        .sort_by_key(|(domain, symbol)| (domain.0, symbol.arena_index(), symbol.generation()));
    guaranteed.dedup();
    if guaranteed.is_empty() {
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
    for (domain, selected_symbol) in guaranteed {
        shapes.add_selected_domain(domain, selected_symbol, result_type, binders, false)?;
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
/// registers through `fold_boundary_result_domains`; the call and installation
/// owners retain its authority obligations. A checked body proved its own clause. A membership
/// over another value names a different carrier and is skipped, and a scalar,
/// unit result carries no structural qualification. Borrowed record guarantees
/// qualify their exact referent path; a domain on the shell retains that shell.
pub(in crate::execution::terminal_unit) fn register_authored_result_domains(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<()> {
    let return_type = state.return_type;
    if is_unit(program, return_type) || program.primitive_type_reference(return_type).is_some() {
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
            let Some((owner, carrier)) =
                validation::reserved_result_owner(program, membership.value)
            else {
                continue;
            };
            if owner != machine.symbol && owner != state.symbol {
                continue;
            }
            let (_, carrier) =
                validation::result_membership_carrier(program, carrier, membership.domain_symbol)?;
            let Some(definition) = program.domain_definition(membership.domain_symbol) else {
                continue;
            };
            shapes.add_selected_domain(
                membership.semantic_domain,
                definition.symbol,
                carrier,
                binders,
                true,
            )?;
        }
    }
    Some(())
}

/// Freeze each authored `requires <formal> in <domain>` entry membership on the
/// structural formal it names. A bare annotation does not erase a requires
/// membership: the scalar formal route publishes the same requirement on its
/// declaration, and this is that rule's structural sibling. The body assumes
/// the proposition its own entry clause states, while the caller's arrival still
/// owes that domain's establishment route, which its checked call admitted.
/// Registration describes the selected domain's theory; it establishes no
/// membership and grants no custody. Resolve the state through its actual
/// typed owner: a fused graph's root does not own a callee's entry clauses.
pub(in crate::execution::terminal_unit) fn fold_parameter_entry_domains(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: SymbolHandle,
    binders: &[(SymbolHandle, String)],
    structural_parameters: &mut [CheckedUnitStructuralParameterPlan],
) -> Option<()> {
    let (machine, state) = validation::find_state(program, state)?;
    let parameters = program.state_parameters(state);
    let entry_state = program
        .machine_states(machine)
        .first()
        .map(|state| state.symbol);
    let contracts = program.state_contracts(state).iter().chain(
        program
            .machine_contracts(machine)
            .iter()
            .filter(|_| entry_state == Some(state.symbol)),
    );
    for contract in contracts {
        if contract.kind != SignatureContractKind::Requires || contract.binding.is_some() {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(membership) = fact.as_membership() else {
                continue;
            };
            let Some(root) = validation::single_segment_name_symbol(program, membership.value)
            else {
                continue;
            };
            let Some(source_position) = parameters
                .iter()
                .position(|parameter| parameter.symbol == root)
            else {
                continue;
            };
            let Some(parameter) = structural_parameters
                .iter_mut()
                .find(|parameter| parameter.position as usize == source_position)
            else {
                continue;
            };
            let routed = validation::field_domains::domain_requires_provenance(
                program,
                membership.domain_symbol,
            );
            let vacuous = validation::field_domains::domain_is_vacuous(
                program,
                membership.domain_symbol,
                &mut Vec::new(),
            );
            // Predicate-only membership keeps its independent predicate owner.
            if !routed && !vacuous {
                continue;
            }
            let Some(domain) = crate::facts::qualification_evidence::domain_definition(
                program,
                membership.domain_symbol,
            ) else {
                return None;
            };
            if !membership.semantic_domain.is_valid() {
                return None;
            }
            shapes.add_selected_domain(
                membership.semantic_domain,
                domain.symbol,
                parameters[source_position].type_reference,
                binders,
                false,
            )?;
            if !parameter
                .qualifications
                .contains(&membership.semantic_domain)
            {
                parameter.qualifications.push(membership.semantic_domain);
                parameter.qualifications.sort_by_key(|domain| domain.0);
            }
        }
    }
    Some(())
}

/// Internal state signatures retain the qualifications proved on every arrival,
/// including facts not spelled in a parameter's nominal type. Copies and later
/// projections consume this same contract. Never strengthen an invocation entry
/// from the particular callers encountered while compiling its body.
pub(in crate::execution::terminal_unit) fn retain_parameter_arrival_domains(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &mut [CheckedUnitStructuralParameterPlan],
) -> Result<(), &'static str> {
    if structural_parameters.is_empty()
        || program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol)
    {
        return Ok(());
    }
    let flow = state_flow(facts, machine.symbol, state.symbol)
        .ok_or("state graph: arrival domains: missing checked flow")?;
    let parameters = program.state_parameters(state);
    let declared_qualifications = parameters
        .iter()
        .map(|parameter| {
            validation::field_domains::declared_type_domain_rows(program, parameter.type_reference)
        })
        .collect::<Vec<_>>();
    for context in facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(flow.entry_semantic_contexts)
    {
        for fact in facts
            .semantic
            .context_view(facts.semantic.contexts.get(context.context))
            .facts()
        {
            if fact.origin != facts::FactOrigin::StatementTransfer {
                continue;
            }
            let Some((_, _, domain_symbol, domain)) = fact.payload.domain_membership() else {
                continue;
            };
            let place = fact.place.as_place().and_then(|place| {
                crate::flow::canonical_place_from_semantic_place(
                    program,
                    &facts.semantic,
                    facts.semantic.places.get(place),
                )
            });
            let Some(place) = place else {
                continue;
            };
            let Some(position) = parameters
                .iter()
                .position(|parameter| place.root == facts::PlaceRoot::Symbol(parameter.symbol))
            else {
                continue;
            };
            // The signature collector already publishes declaration obligations.
            // Their flow roster also contains derived all-element rollups, which
            // are logical contracts, not executable byte-range projections.
            // Retain only additional arrival facts here. An all-element
            // declaration also covers a runtime-indexed element; exact case,
            // field and selected domain identities still distinguish the fact.
            if declared_qualifications[position].iter().any(|declared| {
                declared.describes_projection(&place.segments, |expression| {
                    expression.is_valid()
                        && !matches!(
                            program.expression_table.expression(expression),
                            typed_trees::expression::ExpressionNode::Range(_)
                        )
                }) && declared.domain_symbol == domain_symbol
                    && declared.semantic_domain == domain
            }) {
                continue;
            }
            let Some(parameter) = structural_parameters
                .iter_mut()
                .find(|parameter| parameter.position as usize == position)
            else {
                continue;
            };
            let (carrier, path) = crate::execution::terminal_unit::calls::projected_argument_path(
                program,
                state.symbol,
                0,
                &place,
                None,
                None,
            )
            .ok_or("state graph: arrival domains: unsupported exact subject path")?;
            // Scalar qualifications have their own checked computation channel.
            if program.primitive_type_reference(carrier).is_some() {
                continue;
            }
            if path.is_empty() && parameter.qualifications.contains(&domain)
                || parameter
                    .projected_qualifications
                    .iter()
                    .any(|row| row.path == path && row.domain == domain)
            {
                continue;
            }
            shapes
                .add_selected_domain(domain, domain_symbol, carrier, &[], false)
                .ok_or("state graph: arrival domains: selected theory has no carrier plan")?;
            if path.is_empty() {
                parameter.qualifications.push(domain);
                parameter.qualifications.sort_by_key(|domain| domain.0);
            } else {
                parameter
                    .projected_qualifications
                    .push(CheckedStructuralPathQualification { path, domain });
            }
        }
    }
    Ok(())
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
    // A cast whose membership checking decided at its own occurrence, and
    // whose structural value this body produces, is owned by that value: the
    // checked membership travels with the produced place. A vacuous cast's
    // decision is its successful judgment; a predicate cast's is its proof.
    // Only a commitment with no such producer remains a body request.
    let decided = facts
        .qualifications
        .vacuous_uses
        .iter()
        .map(|use_| {
            (
                use_.machine,
                use_.state,
                use_.statement_index,
                use_.expression,
                use_.semantic_domain,
            )
        })
        .chain(
            facts
                .qualifications
                .value_formations
                .iter()
                .map(|(_, decision)| {
                    (
                        decision.machine,
                        decision.state,
                        decision.statement_index,
                        decision.expression,
                        decision.semantic_domain,
                    )
                }),
        )
        .filter(|(owner, ..)| *owner == machine)
        // Body commitments originate at casts (qualification_facts), not
        // every later use of the same domain. A copied descriptor or existing
        // place may discharge a call's requirement without producing another
        // structural value. That use must not make the original cast's
        // already-owned membership appear to lack a producer.
        .filter(|(_, _, _, expression, _)| {
            program.expression_table.cast_node(*expression).is_some()
        });
    let (owned, unowned): (Vec<_>, Vec<_>) =
        decided.partition(|(_, state, statement, expression, _)| {
            facts
                .values
                .structural_values
                .nodes
                .iter()
                .any(|(_, node)| {
                    node.machine == machine
                        && node.state == *state
                        && node.statement_index == *statement
                        && node.expression == *expression
                })
        });
    domains.retain(|domain| {
        !owned
            .iter()
            .any(|(.., semantic_domain)| semantic_domain == domain)
            || unowned
                .iter()
                .any(|(.., semantic_domain)| semantic_domain == domain)
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
    let contract = validation::structural_qualification_contract(program, type_reference).ok()?;
    for reference in contract.projection_types {
        shapes.add_type(reference, binders, &[])?;
    }
    for (domain, carrier) in contract.domains {
        shapes.add_domain(domain, carrier, binders)?;
    }
    Some(contract.projected_qualifications)
}

/// Returned values retain reference carriers; their projected memberships
/// use the same explicit pointer paths as the emitted result declaration.
pub(in crate::execution::terminal_unit) fn projected_result_qualifications(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<CheckedStructuralPathQualification>> {
    let contract =
        validation::structural_result_qualification_contract(program, type_reference).ok()?;
    for reference in contract.projection_types {
        shapes.add_type(reference, binders, &[])?;
    }
    for (domain, carrier) in contract.domains {
        shapes.add_domain(domain, carrier, binders)?;
    }
    Some(contract.projected_qualifications)
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
