use crate::lowerer::Lowerer;
use typed_trees as typed;

/// Every normalized declared domain on a parameter type, looking through a
/// leading reference. Arithmetic policy is represented by a distinct
/// constraint node and does not become a membership contract.
pub(crate) fn domain_constraints(
    typed_trees: &typed::TypedTrees,
    type_reference: typed::types::TypeReferenceHandle,
) -> Vec<(
    symbols::SymbolHandle,
    String,
    Vec<typed::types::TypeReferenceHandle>,
    language_semantics::SemanticDomainId,
)> {
    match typed_trees
        .type_reference_table
        .type_reference(type_reference)
    {
        typed::types::TypeReferenceNode::Reference { referee, .. } => {
            domain_constraints(typed_trees, *referee)
        }
        typed::types::TypeReferenceNode::Constrained { constraints, .. } => typed_trees
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                typed::types::TypeConstraintNode::Domain(domain) if domain.symbol.is_valid() => {
                    typed_trees
                        .domain_definitions()
                        .iter()
                        .find(|definition| definition.symbol == domain.symbol)
                        .map(|definition| {
                            (
                                domain.symbol,
                                definition.name.as_str().to_owned(),
                                domain.arguments.clone(),
                                domain.semantic_id,
                            )
                        })
                }
                typed::types::TypeConstraintNode::Domain(domain)
                    if language_semantics::CarryPermission::from_name(domain.name.as_str())
                        .is_some() =>
                {
                    Some((
                        symbols::SymbolHandle::invalid(),
                        domain.name.as_str().to_owned(),
                        Vec::new(),
                        language_semantics::SemanticDomainId::NULL,
                    ))
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

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
