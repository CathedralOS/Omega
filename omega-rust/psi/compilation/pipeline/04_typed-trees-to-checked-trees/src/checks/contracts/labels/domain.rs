use symbols::SymbolHandle;

pub(crate) fn domain_proves_expression_label(
    program: &typed_trees::TypedTrees,
    domain_symbol: SymbolHandle,
    base_label: &str,
    candidate_label: &str,
) -> bool {
    // Rendering a family predicate cannot substitute its retained indices.
    // Keep this fallback restricted to wholly unindexed proof theories.
    if !typed_trees::domain::supports_symbol_only_proof(program, domain_symbol) {
        return false;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return false;
    };

    program.proof_facts(domain).iter().any(|fact| match fact {
        typed_trees::domain::ProofFact::Expression(expression) => {
            instantiate_domain_expression_label(program, domain, *expression, base_label)
                == candidate_label
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            let nested_base =
                instantiate_domain_expression_label(program, domain, membership.value, base_label);
            domain_proves_expression_label(
                program,
                membership.domain_symbol,
                &nested_base,
                candidate_label,
            )
        }
        typed_trees::domain::ProofFact::Proposition(_) => false,
    })
}

pub(crate) fn instantiate_domain_expression_label(
    program: &typed_trees::TypedTrees,
    domain: &typed_trees::domain::DomainDefinition,
    expression: typed_trees::expression::ExpressionHandle,
    base_label: &str,
) -> String {
    let render = |value: typed_trees::expression::ExpressionHandle| {
        instantiate_domain_expression_label(program, domain, value, base_label)
    };
    super::render::expression_label(
        program,
        program.expression_table.expression(expression),
        &render,
        &|path| {
            let members = program.expression_table.name_path_members(path.members);
            if typed_trees::domain::domain_subject_reference(program, domain, expression) {
                base_label.to_owned()
            } else {
                typed_trees::expression::display_name_path(members, "::")
            }
        },
        &|literal| literal.type_name.to_string(),
    )
}
