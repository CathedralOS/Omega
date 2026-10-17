//! Boundary scalar contracts retain source obligations separately from structural
//! qualification identity. The published parameter roster decides which lane
//! owns a membership; a scalar theory must project completely or fail closed.

use symbols::SymbolHandle;

/// Publish scalar obligations in the declaration's formal namespace. Only an
/// exact structural qualification restatement belongs to the structural lane;
/// scalar membership retains its complete predicate, including implicit entry
/// clauses introduced by parameter qualifications. Provider bodies do not
/// choose which part of the published contract survives.
pub(super) fn boundary_scalar_requires(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    owner: SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    structural_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    domain_requirements: &[checked_trees::CheckedUnitStructuralDomainRequirementPlan],
    result_type: typed_trees::types::TypeReferenceHandle,
    contracts: &[typed_trees::signature::SignatureContract],
    entry_contracts: &[typed_trees::signature::SignatureContract],
    closed: &checked_trees::ClosedScalarValueContractPlan,
) -> Option<Vec<checked_trees::ClosedScalarContractValue>> {
    use checked_trees::ClosedScalarContractValue;
    use typed_trees::domain::ProofFact;
    use typed_trees::signature::SignatureContractKind;

    let authored = contracts
        .iter()
        .filter(|clause| clause.kind == SignatureContractKind::Requires && clause.binding.is_none())
        .collect::<Vec<_>>();
    if authored.len() != closed.authored_requires().len() {
        return None;
    }
    let mut rows = Vec::new();
    let entries = entry_contracts
        .iter()
        .filter(|clause| clause.kind == SignatureContractKind::Requires);
    for (clause, row) in authored
        .into_iter()
        .zip(closed.authored_requires().iter().map(Option::as_ref))
        .chain(entries.map(|clause| (clause, None)))
    {
        if clause.binding.is_some() {
            return None;
        }
        if let Some(
            value @ (ClosedScalarContractValue::Predicate(_)
            | ClosedScalarContractValue::Boolean(_)
            | ClosedScalarContractValue::Integer(_)),
        ) = row
        {
            rows.push(value.clone());
            continue;
        }
        let mut remaining = 4096;
        let mut conjunction = None;
        for fact in program.proof_facts.span_or_empty(clause.facts) {
            let predicate = match fact {
                ProofFact::Membership(membership) => {
                    let structural =
                        validation::single_segment_name_symbol(program, membership.value)
                            .and_then(|symbol| {
                                parameters
                                    .iter()
                                    .position(|parameter| parameter.symbol == symbol)
                            })
                            .is_some_and(|position| {
                                structural_parameters
                                    .iter()
                                    .position(|parameter| parameter.position as usize == position)
                                    .is_some_and(|argument_index| {
                                        domain_requirements.iter().any(|requirement| {
                                            requirement.argument_index as usize == argument_index
                                                && requirement.domain == membership.semantic_domain
                                        })
                                    })
                            });
                    if structural {
                        continue;
                    }
                    crate::values::scalar::result_contract::lower_callable_scalar_membership(
                        program,
                        operators,
                        owner,
                        parameters,
                        membership,
                        &mut remaining,
                    )?
                }
                ProofFact::Expression(expression) => {
                    crate::values::lower_callable_scalar_contract_predicate(
                        program,
                        operators,
                        owner,
                        parameters,
                        result_type,
                        *expression,
                        false,
                        &mut remaining,
                    )?
                }
                ProofFact::Proposition(_) => return None,
            };
            conjunction = Some(match conjunction {
                None => predicate,
                Some(left) => checked_trees::CheckedBooleanExpression::And {
                    left: Box::new(left),
                    right: Box::new(predicate),
                },
            });
        }
        if let Some(predicate) = conjunction {
            rows.push(ClosedScalarContractValue::Predicate(predicate));
        }
    }
    for row in &closed.requires()[closed.authored_requires().len()..] {
        match row {
            Some(value @ ClosedScalarContractValue::Predicate(_)) => rows.push(value.clone()),
            Some(ClosedScalarContractValue::FloatRange(_)) => {}
            _ => return None,
        }
    }
    Some(rows)
}
