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
    match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::Match(dispatch) => {
            super::render::match_label(program, dispatch, &render)
        }
        typed_trees::expression::ExpressionNode::Atomic(atomic) => {
            super::render::atomic_label(atomic, &render)
        }
        typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            super::render::array_literal_label(program, *values, &render)
        }
        typed_trees::expression::ExpressionNode::Binary(binary) => {
            super::render::binary_label(binary, &render)
        }
        typed_trees::expression::ExpressionNode::Boolean(value) => value.to_string(),
        typed_trees::expression::ExpressionNode::Cast(cast) => {
            super::render::cast_label(program, cast, &render)
        }
        typed_trees::expression::ExpressionNode::Call(call) => {
            super::render::call_label(program, call, &render)
        }
        typed_trees::expression::ExpressionNode::Float(value) => value.to_string(),
        typed_trees::expression::ExpressionNode::UndeterminedNaN(format) => {
            format!("<{} NaN without payload>", format.name())
        }
        typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            super::render::indexed_label(indexed, &render)
        }
        typed_trees::expression::ExpressionNode::Integer(value) => value.to_string(),
        typed_trees::expression::ExpressionNode::Member(member) => {
            super::render::member_label(member, &render)
        }
        typed_trees::expression::ExpressionNode::Borrow(inner) => {
            super::render::borrow_label(inner, &render)
        }
        typed_trees::expression::ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if typed_trees::domain::domain_subject_reference(program, domain, expression) {
                base_label.to_owned()
            } else {
                typed_trees::expression::display_name_path(members, "::")
            }
        }
        typed_trees::expression::ExpressionNode::Range(range) => {
            super::render::range_label(range, &render)
        }
        typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            struct_literal.type_name.to_string()
        }
        typed_trees::expression::ExpressionNode::String(value) => format!("{value:?}"),
        typed_trees::expression::ExpressionNode::Unary(unary) => {
            super::render::unary_label(unary, &render)
        }
        typed_trees::expression::ExpressionNode::ZeroValue(type_reference) => format!(
            "zero_value<{}>()",
            program.display_type_reference(*type_reference)
        ),
    }
}
