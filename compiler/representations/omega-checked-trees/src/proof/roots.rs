use omega_core::arena::Arena;

use crate::{
    ContractCallFact, ContractExitFact, ContractProofFact, ContractProofFactRef,
    ProofObligationFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofFacts {
    pub obligations: Arena<ProofObligationFact>,
    pub contract_facts: Arena<ContractProofFact>,
    pub contract_fact_refs: Arena<ContractProofFactRef>,
    pub contract_calls: Arena<ContractCallFact>,
    pub contract_exits: Arena<ContractExitFact>,
}

impl ProofFacts {
    pub fn with_roots(
        obligations: Arena<ProofObligationFact>,
        contract_facts: Arena<ContractProofFact>,
        contract_fact_refs: Arena<ContractProofFactRef>,
        contract_calls: Arena<ContractCallFact>,
        contract_exits: Arena<ContractExitFact>,
    ) -> Self {
        Self {
            obligations,
            contract_facts,
            contract_fact_refs,
            contract_calls,
            contract_exits,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ContractCallFact, ContractExitFact, ContractProofFact, ContractProofFactRef, ProofFacts,
        ProofObligationFact,
    };
    use omega_core::arena::Arena;

    #[test]
    fn proof_facts_constructor_keeps_proof_roots_explicit() {
        let obligations = Arena::<ProofObligationFact>::with_capacity(1);
        let contract_facts = Arena::<ContractProofFact>::with_capacity(2);
        let contract_fact_refs = Arena::<ContractProofFactRef>::with_capacity(3);
        let contract_calls = Arena::<ContractCallFact>::with_capacity(4);
        let contract_exits = Arena::<ContractExitFact>::with_capacity(5);

        let facts = ProofFacts::with_roots(
            obligations.clone(),
            contract_facts.clone(),
            contract_fact_refs.clone(),
            contract_calls.clone(),
            contract_exits.clone(),
        );

        assert_eq!(facts.obligations, obligations);
        assert_eq!(facts.contract_facts, contract_facts);
        assert_eq!(facts.contract_fact_refs, contract_fact_refs);
        assert_eq!(facts.contract_calls, contract_calls);
        assert_eq!(facts.contract_exits, contract_exits);
    }
}
