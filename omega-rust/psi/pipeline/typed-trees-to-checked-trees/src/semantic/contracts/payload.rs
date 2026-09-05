use crate::context::*;

pub(super) fn semantic_contract_payload(
    program: &typed_trees::TypedTrees,
    contract: &ContractProofFact,
) -> FactPayload {
    let kind = semantic_contract_fact_kind(contract.kind);
    match program.proof_facts.get(contract.fact) {
        typed_trees::domain::ProofFact::Expression(expression) => {
            FactPayload::ContractBooleanExpression {
                kind,
                fact: contract.fact,
                expression: *expression,
                instantiated: arena::Handle::invalid(),
            }
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            let carry_permission = program
                .domain_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if let Some(permission) =
                language_semantics::CarryPermission::from_name(&carry_permission)
            {
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
            }
        }
        typed_trees::domain::ProofFact::Proposition(application) => {
            FactPayload::ContractPropositionApplication {
                kind,
                fact: contract.fact,
                proposition: application.proposition,
                instantiated: arena::Handle::invalid(),
            }
        }
    }
}
