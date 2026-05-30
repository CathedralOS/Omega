use super::*;

pub(crate) fn build_contract_operator_use_facts(
    program: &omega_typed_trees::TypedTrees,
    operators: &CheckedOperatorFacts,
    contract_facts: &mut omega_core::arena::Arena<ContractProofFact>,
    fact_refs: &mut omega_core::arena::Arena<ContractProofFactRef>,
) -> omega_core::arena::Arena<ContractOperatorUseFact> {
    let mut operator_uses =
        omega_core::arena::Arena::with_capacity(operators.resolved_contract_uses().count());

    for operator_use in operators.resolved_contract_uses() {
        append_contract_operator_use(
            program,
            contract_facts,
            fact_refs,
            &mut operator_uses,
            operator_use,
        );
    }

    operator_uses
}

fn append_contract_operator_use(
    program: &omega_typed_trees::TypedTrees,
    contract_facts: &mut omega_core::arena::Arena<ContractProofFact>,
    fact_refs: &mut omega_core::arena::Arena<ContractProofFactRef>,
    operator_uses: &mut omega_core::arena::Arena<ContractOperatorUseFact>,
    operator_use: CheckedOperatorContractUse<'_>,
) {
    let mut requires = HandleSpan::empty();
    let mut ensures = HandleSpan::empty();
    let mut boundary = HandleSpan::empty();

    for contract in program
        .signature_contracts
        .span_or_empty(operator_use.contracts())
    {
        let target = match super::contract_fact_kind(contract.kind) {
            ContractProofFactKind::Requires => &mut requires,
            ContractProofFactKind::Ensures => &mut ensures,
            ContractProofFactKind::Boundary => &mut boundary,
        };

        for fact in super::fact_handles(contract.facts) {
            let contract_fact = contract_facts.append(ContractProofFact {
                kind: super::contract_fact_kind(contract.kind),
                owner: ContractProofFactOwner::OperatorUse {
                    expression: operator_use.operator_use.expression,
                    origin: operator_use.operator_use.origin,
                    operator_symbol: operator_use.operator_symbol(),
                },
                fact,
            });
            fact_refs.append_to_span(
                target,
                ContractProofFactRef {
                    fact: contract_fact,
                },
            );
        }
    }

    if requires.is_empty() && ensures.is_empty() && boundary.is_empty() {
        return;
    }

    operator_uses.append(ContractOperatorUseFact {
        expression: operator_use.operator_use.expression,
        origin: operator_use.operator_use.origin,
        operator_symbol: operator_use.operator_symbol(),
        requires,
        ensures,
        boundary,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_checked_trees::{
        CheckedOperatorCandidateFact, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
        CheckedValueOrigin,
    };
    use omega_typed_trees::domain::ProofFact;
    use omega_typed_trees::operator::OperatorDefinition;
    use omega_typed_trees::signature::{SignatureContract, SignatureContractKind};
    use omega_typed_trees::types::TypeReferenceHandle;

    #[test]
    fn builds_contract_operator_use_facts_from_resolved_operator_contracts() {
        let expression = ExpressionHandle::from_arena_index(1);
        let operator_symbol = SymbolHandle::from_arena_index(2);
        let machine_symbol = SymbolHandle::from_arena_index(3);
        let state_symbol = SymbolHandle::from_arena_index(4);
        let origin = CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            statement_index: 5,
            role: Default::default(),
        };

        let mut program = omega_typed_trees::TypedTrees::default();
        let mut requires_facts = HandleSpan::empty();
        program
            .proof_facts
            .append_to_span(&mut requires_facts, ProofFact::Expression(expression));
        let mut ensures_facts = HandleSpan::empty();
        program
            .proof_facts
            .append_to_span(&mut ensures_facts, ProofFact::Expression(expression));

        let mut operator = OperatorDefinition {
            symbol: operator_symbol,
            contracts: HandleSpan::empty(),
            ..Default::default()
        };
        program.push_operator_contract(
            &mut operator,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: requires_facts,
                token_count: 1,
            },
        );
        program.push_operator_contract(
            &mut operator,
            SignatureContract {
                kind: SignatureContractKind::Ensures,
                facts: ensures_facts,
                token_count: 1,
            },
        );
        let contracts = operator.contracts;

        let mut candidates = omega_core::arena::Arena::default();
        let candidate_span =
            candidates.insert_many([CheckedOperatorCandidateFact::root(operator_symbol)
                .with_signature(
                    TypeReferenceHandle::invalid(),
                    TypeReferenceHandle::invalid(),
                    contracts,
                    0,
                    0,
                    false,
                )]);
        let mut uses = omega_core::arena::Arena::default();
        uses.append(CheckedOperatorUseFact {
            expression,
            origin,
            selected_operator_symbol: operator_symbol,
            candidates: candidate_span,
            candidate_count: 1,
            status: CheckedOperatorResolutionStatus::Resolved,
            ..Default::default()
        });
        let operators = CheckedOperatorFacts::with_roots(uses, candidates);
        let mut contract_facts = omega_core::arena::Arena::default();
        let mut fact_refs = omega_core::arena::Arena::default();

        let operator_uses = build_contract_operator_use_facts(
            &program,
            &operators,
            &mut contract_facts,
            &mut fact_refs,
        );

        assert_eq!(contract_facts.len(), 2);
        assert_eq!(operator_uses.len(), 1);
        let operator_use = operator_uses.iter().next().expect("operator use").1;
        assert_eq!(operator_use.expression, expression);
        assert_eq!(operator_use.origin, origin);
        assert_eq!(operator_use.operator_symbol, operator_symbol);
        assert_eq!(operator_use.requires.len(), 1);
        assert_eq!(operator_use.ensures.len(), 1);
        assert!(operator_use.boundary.is_empty());

        let require_ref = fact_refs.span_or_empty(operator_use.requires)[0];
        let require_fact = contract_facts.get(require_ref.fact);
        assert_eq!(require_fact.kind, ContractProofFactKind::Requires);
        assert_eq!(
            require_fact.owner,
            ContractProofFactOwner::OperatorUse {
                expression,
                origin,
                operator_symbol,
            }
        );
    }
}
