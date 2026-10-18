use crate::name::Identifier;
use crate::signature::SignatureContract;
use crate::signature::StateParameter;
use crate::statement::StatementNode;
use arena::HandleSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<SignatureContract>,
    pub statement_nodes: HandleSpan<StatementNode>,
}

/// The state keyed `symbol`, when declared. States are keyed by symbol rather
/// than name, so transition and evidence lookups resolve identity directly.
pub fn state_by_symbol(states: &[State], symbol: SymbolHandle) -> Option<&State> {
    states.iter().find(|state| state.symbol == symbol)
}

/// The position of the state keyed `symbol`, when declared.
pub fn state_position(states: &[State], symbol: SymbolHandle) -> Option<usize> {
    states.iter().position(|state| state.symbol == symbol)
}

impl crate::TypedTrees {
    /// Build the implicit `requires <param> in <domain>` membership contract for a
    /// normalized declared-domain constraint. The caller attaches the contract at
    /// the right level (machine vs trait signature).
    pub fn parameter_domain_membership_contract(
        &mut self,
        parameter_symbol: symbols::SymbolHandle,
        parameter_name: crate::name::Identifier,
        domain_symbol: symbols::SymbolHandle,
        domain_full_name: &str,
        domain_arguments: Vec<crate::types::TypeReferenceHandle>,
        semantic_domain: language_semantics::SemanticDomainId,
    ) -> crate::signature::SignatureContract {
        let domain_arguments = self
            .type_reference_table
            .insert_type_reference_handles(domain_arguments);
        let mut members = arena::HandleSpan::empty();
        self.expression_table
            .push_name_path_member(&mut members, parameter_name);
        let mut member_symbols = arena::HandleSpan::empty();
        self.expression_table
            .push_name_path_member_symbol(&mut member_symbols, parameter_symbol);
        let value = self
            .expression_table
            .insert(crate::expression::ExpressionNode::Name(
                crate::expression::TableNamePath::new(
                    members,
                    member_symbols,
                    parameter_symbol,
                    parameter_symbol,
                ),
            ));

        let mut domain = arena::HandleSpan::empty();
        for part in domain_full_name.split("::") {
            self.domain_path_members
                .append_to_span(&mut domain, crate::name::Identifier::generated(part));
        }

        let mut facts = arena::HandleSpan::empty();
        self.proof_facts.append_to_span(
            &mut facts,
            crate::domain::ProofFact::Membership(crate::domain::ProofMembershipFact {
                value,
                domain,
                domain_symbol,
                domain_arguments,
                semantic_domain,
                authored_domain_selection: None,
            }),
        );

        crate::signature::SignatureContract {
            kind: crate::signature::SignatureContractKind::Requires,
            keyword_source_span: None,
            binding: None,
            facts,
            token_count: 0,
        }
    }
}
