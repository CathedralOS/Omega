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
    match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::Match(dispatch) => {
            let render =
                |value| instantiate_domain_expression_label(program, domain, value, base_label);
            let arms = program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .map(|arm| {
                    let pattern = match arm.pattern {
                        typed_trees::expression::MatchPattern::Value(value) => render(value),
                        typed_trees::expression::MatchPattern::Wildcard => "_".to_owned(),
                    };
                    format!("{pattern} -> {}", render(arm.value))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("match {} {{ {arms} }}", render(dispatch.subject))
        }
        typed_trees::expression::ExpressionNode::Atomic(atomic) => format!(
            "atomic[{:?}]({})",
            atomic.ordering,
            instantiate_domain_expression_label(program, domain, atomic.value, base_label),
        ),
        typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            let values = program
                .expression_table
                .expression_handles(*values)
                .iter()
                .map(|value| {
                    instantiate_domain_expression_label(program, domain, *value, base_label)
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        typed_trees::expression::ExpressionNode::Binary(binary) => format!(
            "{} {} {}",
            instantiate_domain_expression_label(program, domain, binary.left, base_label),
            binary.operator.display_name(),
            instantiate_domain_expression_label(program, domain, binary.right, base_label),
        ),
        typed_trees::expression::ExpressionNode::Boolean(value) => value.to_string(),
        typed_trees::expression::ExpressionNode::Cast(cast) => format!(
            "{} as {}",
            instantiate_domain_expression_label(program, domain, cast.value, base_label),
            typed_trees::expression::display_name_path(
                program
                    .expression_table
                    .name_path_members(cast.target_label),
                "::",
            )
        ),
        typed_trees::expression::ExpressionNode::Call(call) => {
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| {
                    instantiate_domain_expression_label(program, domain, *argument, base_label)
                })
                .collect::<Vec<_>>()
                .join(", ");
            if call.receiver.is_valid() {
                format!(
                    "{}.{}({arguments})",
                    instantiate_domain_expression_label(program, domain, call.receiver, base_label),
                    call.target
                )
            } else {
                format!("{}({arguments})", call.target)
            }
        }
        typed_trees::expression::ExpressionNode::Float(value) => value.to_string(),
        typed_trees::expression::ExpressionNode::UndeterminedNaN(format) => {
            format!("<{} NaN without payload>", format.name())
        }
        typed_trees::expression::ExpressionNode::Indexed(indexed) => format!(
            "{}[{}]",
            instantiate_domain_expression_label(program, domain, indexed.collection, base_label),
            instantiate_domain_expression_label(program, domain, indexed.index, base_label),
        ),
        typed_trees::expression::ExpressionNode::Range(range) => {
            match (range.start.is_valid(), range.end.is_valid()) {
                (true, true) => format!(
                    "{}..{}",
                    instantiate_domain_expression_label(program, domain, range.start, base_label),
                    instantiate_domain_expression_label(program, domain, range.end, base_label),
                ),
                (true, false) => format!(
                    "{}..",
                    instantiate_domain_expression_label(program, domain, range.start, base_label)
                ),
                (false, true) => format!(
                    "..{}",
                    instantiate_domain_expression_label(program, domain, range.end, base_label)
                ),
                (false, false) => "..".to_owned(),
            }
        }
        typed_trees::expression::ExpressionNode::Integer(value) => value.to_string(),
        typed_trees::expression::ExpressionNode::Member(member) => format!(
            "{}.{}",
            instantiate_domain_expression_label(program, domain, member.receiver, base_label),
            member.member
        ),
        typed_trees::expression::ExpressionNode::Borrow(inner) => {
            let target =
                instantiate_domain_expression_label(program, domain, inner.target, base_label);
            match inner.access {
                language_semantics::ReferenceAccess::Shared => target,
                language_semantics::ReferenceAccess::Mutable => format!("mut {target}"),
                language_semantics::ReferenceAccess::WriteOnly => format!("write {target}"),
            }
        }
        typed_trees::expression::ExpressionNode::Unary(unary) => format!(
            "{}{}",
            unary.operator.display_name(),
            instantiate_domain_expression_label(program, domain, unary.operand, base_label)
        ),
        typed_trees::expression::ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if typed_trees::domain::domain_subject_reference(program, domain, expression) {
                base_label.to_owned()
            } else {
                typed_trees::expression::display_name_path(members, "::")
            }
        }
        typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            struct_literal.type_name.to_string()
        }
        typed_trees::expression::ExpressionNode::String(value) => format!("{value:?}"),
        typed_trees::expression::ExpressionNode::ZeroValue(type_reference) => format!(
            "zero_value<{}>()",
            program.display_type_reference(*type_reference)
        ),
    }
}
