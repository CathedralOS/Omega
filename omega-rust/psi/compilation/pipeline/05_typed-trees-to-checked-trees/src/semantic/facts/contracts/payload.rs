use crate::labels::semantic_contract_fact_kind;
use arena::Handle;
use checked_trees::ContractProofFact;
use facts::FactPayload;
use language_semantics::CarryPermission;
use symbols::JoinedPath;
use typed_trees::domain::ProofFact;

pub(super) fn semantic_contract_payload(
    program: &dyn ::validation::CheckingInput,
    contract: &ContractProofFact,
) -> FactPayload {
    let kind = semantic_contract_fact_kind(contract.kind);
    match program.proof_facts.get(contract.fact) {
        ProofFact::Expression(expression) => FactPayload::ContractBooleanExpression {
            kind,
            fact: contract.fact,
            expression: *expression,
            instantiated: Handle::invalid(),
        },
        ProofFact::Membership(membership) => {
            let carry_permission = program.domain_path_members(membership.domain).joined_path();
            if let Some(permission) = CarryPermission::from_name(&carry_permission) {
                return FactPayload::ContractCarryPermission {
                    kind,
                    fact: contract.fact,
                    value: membership.value,
                    permission,
                };
            }
            FactPayload::ContractDomainMembership {
                kind,
                fact: contract.fact,
                value: membership.value,
                domain: membership.domain,
                domain_symbol: membership.domain_symbol,
                semantic_domain: membership.semantic_domain,
            }
        }
        ProofFact::Proposition(application) => FactPayload::ContractPropositionApplication {
            kind,
            fact: contract.fact,
            proposition: application.proposition,
            instantiated: Handle::invalid(),
        },
    }
}
