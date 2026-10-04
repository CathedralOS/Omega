use crate::proof::proposition_vocabulary::{contract_fact_kind, fact_handles};
use arena::HandleSpan;
use checked_trees::{
    CheckedOperatorContractUse, CheckedOperatorFacts, ContractOperatorUseFact, ContractProofFact,
    ContractProofFactKind, ContractProofFactOwner, ContractProofFactRef,
};

pub(crate) fn build_contract_operator_use_facts(
    program: &typed_trees::TypedTrees,
    operators: &CheckedOperatorFacts,
    contract_facts: &mut arena::Arena<ContractProofFact>,
    fact_refs: &mut arena::Arena<ContractProofFactRef>,
) -> arena::Arena<ContractOperatorUseFact> {
    let mut operator_uses = arena::Arena::with_capacity(operators.resolved_contract_uses().count());

    for operator_use in operators.resolved_contract_uses() {
        // These declaration-backed rows are expression-scoped. An implicit
        // comparison needs arm-local invocation custody before its ensures
        // can become flow evidence; its requires are checked separately.
        if operator_use.operator_use.occurrence
            != checked_trees::CheckedOperatorOccurrence::Expression
        {
            continue;
        }
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
    program: &typed_trees::TypedTrees,
    contract_facts: &mut arena::Arena<ContractProofFact>,
    fact_refs: &mut arena::Arena<ContractProofFactRef>,
    operator_uses: &mut arena::Arena<ContractOperatorUseFact>,
    operator_use: CheckedOperatorContractUse<'_>,
) {
    let mut requires = HandleSpan::empty();
    let mut ensures = HandleSpan::empty();

    for contract in program
        .signature_contracts
        .span_or_empty(operator_use.contracts())
    {
        let Some(kind) = contract_fact_kind(&contract.kind) else {
            continue;
        };
        let target = match kind {
            ContractProofFactKind::Requires => &mut requires,
            ContractProofFactKind::Ensures => &mut ensures,
        };

        for fact in fact_handles(contract.facts) {
            let contract_fact = contract_facts.append(ContractProofFact {
                kind,
                owner: ContractProofFactOwner::OperatorUse {
                    expression: operator_use.operator_use.expression,
                    origin: operator_use.operator_use.origin,
                    operator_symbol: operator_use.operator_symbol(),
                },
                fact,
                evidence_term: None,
                qualification_authorization: None,
                inherited_scope: None,
            });
            fact_refs.append_to_span(
                target,
                ContractProofFactRef {
                    fact: contract_fact,
                },
            );
        }
    }

    if requires.is_empty() && ensures.is_empty() {
        return;
    }

    operator_uses.append(ContractOperatorUseFact {
        expression: operator_use.operator_use.expression,
        origin: operator_use.operator_use.origin,
        operator_symbol: operator_use.operator_symbol(),
        requires,
        ensures,
    });
}
