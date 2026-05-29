use omega_core::symbols::SymbolHandle;

pub(crate) fn domain_proves_expression_label(
    program: &omega_typed_trees::TypedTrees,
    domain_symbol: SymbolHandle,
    base_label: &str,
    candidate_label: &str,
) -> bool {
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return false;
    };

    program.proof_facts(domain).iter().any(|fact| match fact {
        omega_typed_trees::domain::ProofFact::Expression(expression) => {
            instantiate_domain_expression_label(program, *expression, base_label) == candidate_label
        }
        omega_typed_trees::domain::ProofFact::Membership(membership) => {
            let nested_base =
                instantiate_domain_expression_label(program, membership.value, base_label);
            domain_proves_expression_label(
                program,
                membership.domain_symbol,
                &nested_base,
                candidate_label,
            )
        }
    })
}

fn instantiate_domain_expression_label(
    program: &omega_typed_trees::TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
    base_label: &str,
) -> String {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            let values = program
                .expression_table
                .expression_handles(*values)
                .iter()
                .map(|value| instantiate_domain_expression_label(program, *value, base_label))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => format!(
            "{} {} {}",
            instantiate_domain_expression_label(program, binary.left, base_label),
            binary.operator.display_name(),
            instantiate_domain_expression_label(program, binary.right, base_label),
        ),
        omega_typed_trees::expression::ExpressionNode::Boolean(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Cast(cast) => format!(
            "{} as {}",
            instantiate_domain_expression_label(program, cast.value, base_label),
            omega_typed_trees::expression::display_name_path(
                program.expression_table.name_path_members(cast.target_type),
                "::",
            )
        ),
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| instantiate_domain_expression_label(program, *argument, base_label))
                .collect::<Vec<_>>()
                .join(", ");
            if call.receiver.is_valid() {
                format!(
                    "{}.{}({arguments})",
                    instantiate_domain_expression_label(program, call.receiver, base_label),
                    call.target
                )
            } else {
                format!("{}({arguments})", call.target)
            }
        }
        omega_typed_trees::expression::ExpressionNode::Float(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Indexed(indexed) => format!(
            "{}[{}]",
            instantiate_domain_expression_label(program, indexed.collection, base_label),
            instantiate_domain_expression_label(program, indexed.index, base_label),
        ),
        omega_typed_trees::expression::ExpressionNode::Range(range) => {
            match (range.start.is_valid(), range.end.is_valid()) {
                (true, true) => format!(
                    "{}..{}",
                    instantiate_domain_expression_label(program, range.start, base_label),
                    instantiate_domain_expression_label(program, range.end, base_label),
                ),
                (true, false) => format!(
                    "{}..",
                    instantiate_domain_expression_label(program, range.start, base_label)
                ),
                (false, true) => format!(
                    "..{}",
                    instantiate_domain_expression_label(program, range.end, base_label)
                ),
                (false, false) => "..".to_owned(),
            }
        }
        omega_typed_trees::expression::ExpressionNode::Integer(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Member(member) => format!(
            "{}.{}",
            instantiate_domain_expression_label(program, member.receiver, base_label),
            member.member
        ),
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            format!(
                "mut {}",
                instantiate_domain_expression_label(program, *inner, base_label)
            )
        }
        omega_typed_trees::expression::ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members
                .first()
                .is_some_and(|member| member.as_str() == "self")
            {
                if members.len() == 1 {
                    base_label.to_owned()
                } else {
                    format!(
                        "{base_label}::{}",
                        omega_typed_trees::expression::display_name_path(&members[1..], "::")
                    )
                }
            } else {
                omega_typed_trees::expression::display_name_path(members, "::")
            }
        }
        omega_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            struct_literal.type_name.to_string()
        }
        omega_typed_trees::expression::ExpressionNode::String(value) => format!("{value:?}"),
    }
}
