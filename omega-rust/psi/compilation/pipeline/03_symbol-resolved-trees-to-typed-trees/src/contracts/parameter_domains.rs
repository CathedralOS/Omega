use crate::lowerer::Lowerer;
use crate::signatures::parameters::lower_state_parameter;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) use validation::parameter_domain_constraints as domain_constraints;

/// Build the implicit `requires <param> in <domain>` membership contract for a
/// normalized declared-domain constraint. The caller attaches the contract at
/// the right level (machine vs trait signature).
pub(crate) fn build_domain_membership_contract(
    lowerer: &mut Lowerer,
    param_symbol: symbols::SymbolHandle,
    param_name: typed::name::Identifier,
    domain_symbol: symbols::SymbolHandle,
    domain_full_name: &str,
    domain_arguments: Vec<typed::types::TypeReferenceHandle>,
    semantic_domain: language_semantics::SemanticDomainId,
) -> typed::signature::SignatureContract {
    let domain_arguments = lowerer
        .typed_trees
        .type_reference_table
        .insert_type_reference_handles(domain_arguments);
    let mut members = arena::HandleSpan::empty();
    lowerer
        .typed_trees
        .expression_table
        .push_name_path_member(&mut members, param_name);
    let mut member_symbols = arena::HandleSpan::empty();
    lowerer
        .typed_trees
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, param_symbol);
    let value =
        lowerer
            .typed_trees
            .expression_table
            .insert(typed::expression::ExpressionNode::Name(
                typed::expression::TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: param_symbol,
                    symbol: param_symbol,
                },
            ));

    let mut domain = arena::HandleSpan::empty();
    for part in domain_full_name.split("::") {
        lowerer
            .typed_trees
            .domain_path_members
            .append_to_span(&mut domain, typed::name::Identifier::generated(part));
    }

    let mut facts = arena::HandleSpan::empty();
    lowerer.typed_trees.proof_facts.append_to_span(
        &mut facts,
        typed::domain::ProofFact::Membership(typed::domain::ProofMembershipFact {
            value,
            domain,
            domain_symbol,
            domain_arguments,
            semantic_domain,
            authored_domain_selection: None,
        }),
    );

    typed::signature::SignatureContract {
        kind: typed::signature::SignatureContractKind::Requires,
        keyword_source_span: None,
        binding: None,
        facts,
        token_count: 0,
    }
}

/// One `(parameter, declared domain)` pair collected while lowering state
/// parameters; each pair desugars into its own membership contract.
pub(crate) type DomainConstrainedParameter = (
    symbols::SymbolHandle,
    typed::name::Identifier,
    symbols::SymbolHandle,
    String,
    Vec<typed::types::TypeReferenceHandle>,
    language_semantics::SemanticDomainId,
);

/// Lower each authored state parameter, collecting every declared domain
/// constraint for `push_domain_membership_contracts`. `push_parameter`
/// attaches each lowered parameter at the right level (state vs trait/machine
/// signature).
pub(crate) fn lower_state_parameters_collecting_domain_constraints(
    lowerer: &mut Lowerer,
    parameters: arena::HandleSpan<resolved::signature::StateParameter>,
    mut push_parameter: impl FnMut(&mut typed::TypedTrees, typed::signature::StateParameter),
) -> Result<Vec<DomainConstrainedParameter>, diagnostics::Diagnostic> {
    let mut domain_constrained_parameters: Vec<DomainConstrainedParameter> = Vec::new();
    for parameter in lowerer.source_trees.state_parameters(parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        for (domain_symbol, domain_full_name, domain_arguments, semantic_domain) in
            domain_constraints(&lowerer.typed_trees, parameter.type_reference)
        {
            domain_constrained_parameters.push((
                parameter.symbol,
                parameter.name.clone(),
                domain_symbol,
                domain_full_name,
                domain_arguments,
                semantic_domain,
            ));
        }
        push_parameter(&mut lowerer.typed_trees, parameter);
    }
    Ok(domain_constrained_parameters)
}

/// Desugar each collected declared domain into an implicit `requires <param>
/// in <domain>` membership contract. `push_contract` attaches each contract at
/// the right level (state vs trait/machine signature).
pub(crate) fn push_domain_membership_contracts(
    lowerer: &mut Lowerer,
    domain_constrained_parameters: Vec<DomainConstrainedParameter>,
    mut push_contract: impl FnMut(&mut typed::TypedTrees, typed::signature::SignatureContract),
) {
    for (
        param_symbol,
        param_name,
        domain_symbol,
        domain_full_name,
        domain_arguments,
        semantic_domain,
    ) in domain_constrained_parameters
    {
        let contract = build_domain_membership_contract(
            lowerer,
            param_symbol,
            param_name,
            domain_symbol,
            &domain_full_name,
            domain_arguments,
            semantic_domain,
        );
        push_contract(&mut lowerer.typed_trees, contract);
    }
}
