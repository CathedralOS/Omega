//! Membership and sum-pattern relabeling.

use super::super::*;

/// Destructure syntax lowers to `subject in Base::Case` before this pass. When
/// the subject is a state parameter or local with an exact synthesized type,
/// that annotation selects the corresponding closed case identity even when
/// another closed instance of the same generic sum exists in the program.
pub(in crate::generic_data) fn relabel_closed_sum_memberships_from_local_types(
    syntax: &mut SyntaxTrees,
    synthesized_origins: &HashMap<String, String>,
) {
    let concrete_states = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) if machine.type_parameters.is_empty() => Some(machine),
            _ => None,
        })
        .flat_map(|machine| {
            syntax
                .tables
                .items
                .state_handles(machine.states)
                .iter()
                .map(|state| (*state, machine.attached_data.clone()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for (state_handle, attached_data) in concrete_states {
        let state = syntax.tables.items.state(state_handle).clone();
        let mut local_types = HashMap::<String, String>::new();
        for parameter in syntax.tables.items.state_parameters(state.parameters) {
            let parameter = syntax.tables.items.state_parameter(*parameter);
            if let Some(type_name) = named_type_name(syntax, parameter.type_reference) {
                local_types.insert(parameter.name.as_str().to_owned(), type_name);
            }
        }
        let statements = syntax.tables.items.statements(state.statements).to_vec();
        for statement in &statements {
            if let StatementNode::LocalData(local) = syntax.tables.statements.statement(*statement)
                && let Some(type_name) = named_type_name(syntax, local.type_reference)
            {
                local_types.insert(local.name.as_str().to_owned(), type_name);
            }
        }
        let self_field_types = attached_data
            .as_ref()
            .and_then(|attached| {
                syntax.root_items().find_map(|item| match item {
                    Item::Data(definition) if definition.name == *attached => Some(
                        syntax
                            .tables
                            .items
                            .data_members(definition.members)
                            .iter()
                            .filter_map(|member| match member {
                                DataMember::Field(field) => {
                                    named_type_name(syntax, field.type_reference).map(|type_name| {
                                        (field.name.as_str().to_owned(), type_name)
                                    })
                                }
                                _ => None,
                            })
                            .collect::<HashMap<_, _>>(),
                    ),
                    _ => None,
                })
            })
            .unwrap_or_default();

        let mut reachable = HashSet::new();
        for statement in statements {
            collect_statement_expression_handles(syntax, statement, &mut reachable);
        }
        let replacements = syntax
            .expressions
            .iter_expressions()
            .filter(|(handle, _)| reachable.contains(&handle.arena_index()))
            .filter_map(|(handle, expression)| {
                let ExpressionNode::Membership(membership) = expression else {
                    return None;
                };
                let closed = match syntax.expressions.expression(membership.value) {
                    ExpressionNode::Name(subject_path) => {
                        let [subject] = syntax.expressions.identifier_path_members(*subject_path)
                        else {
                            return None;
                        };
                        local_types.get(subject.as_str())?
                    }
                    ExpressionNode::Member(member)
                        if matches!(
                            syntax.expressions.expression(member.receiver),
                            ExpressionNode::SelfValue
                        ) =>
                    {
                        self_field_types.get(member.member.as_str())?
                    }
                    _ => return None,
                };
                let base = synthesized_origins.get(closed)?;
                let [domain_base, case] = syntax
                    .expressions
                    .identifier_path_members(membership.domain)
                else {
                    return None;
                };
                (domain_base.as_str() == base).then(|| {
                    (
                        handle,
                        membership.value,
                        closed.clone(),
                        domain_base.source_span(),
                        case.clone(),
                    )
                })
            })
            .collect::<Vec<_>>();
        for (handle, value, closed, carrier_span, case) in replacements {
            let domain = closed_sum_path(syntax, &closed, carrier_span, case);
            syntax.expressions.replace_expression(
                handle,
                ExpressionNode::Membership(syntax_trees::expression::TableMembershipExpression {
                    value,
                    domain,
                }),
            );
        }
    }
}

pub(in crate::generic_data) fn named_type_name(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
) -> Option<String> {
    let type_reference = match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => *base_type,
        TypeReferenceNode::Named(_) => type_reference,
        _ => return None,
    };
    match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Named(name) => Some(name.as_str().to_owned()),
        _ => None,
    }
}

/// When a generic sum has exactly one closed instance, every remaining
/// `Base::Case` constructor and pattern path is unambiguous. Rewrite that
/// fallback cohort to the synthesized nominal identity before symbol
/// resolution. Multiple-instance uses must already have been selected by exact
/// context above. Generic template bodies remain parameterized declarations.
pub(in crate::generic_data) fn relabel_unique_closed_sum_paths(
    syntax: &mut SyntaxTrees,
    synthesized_sum_instances: &HashMap<String, String>,
) {
    if synthesized_sum_instances.is_empty() {
        return;
    }

    let concrete_expressions = concrete_machine_expression_handles(syntax);
    let variants = generic_sum_variant_names(syntax, synthesized_sum_instances);
    let replacements = syntax
        .expressions
        .iter_expressions()
        .filter(|(handle, _)| concrete_expressions.contains(&handle.arena_index()))
        .filter_map(|(handle, expression)| {
            let (path, kind) = match expression {
                ExpressionNode::Name(path) => (*path, SumPathExpressionKind::Name),
                ExpressionNode::Membership(membership) => (
                    membership.domain,
                    SumPathExpressionKind::Membership(membership.value),
                ),
                ExpressionNode::StructLiteral(literal) => {
                    let (carrier, case) = literal.constructor_name.as_str().rsplit_once("::")?;
                    if !variants
                        .get(carrier)
                        .is_some_and(|names| names.contains(case))
                    {
                        return None;
                    }
                    let closed = synthesized_sum_instances.get(carrier)?;
                    return Some((
                        handle,
                        SumPathExpressionKind::StructLiteral(literal.clone()),
                        closed.clone(),
                        literal.constructor_name.source_span(),
                        Identifier::new(case, literal.constructor_name.source_span()),
                    ));
                }
                _ => return None,
            };
            let [base, case] = syntax.expressions.identifier_path_members(path) else {
                return None;
            };
            let closed = synthesized_sum_instances.get(base.as_str())?;
            if !variants
                .get(base.as_str())
                .is_some_and(|names| names.contains(case.as_str()))
            {
                return None;
            }
            Some((
                handle,
                kind,
                closed.clone(),
                base.source_span(),
                case.clone(),
            ))
        })
        .collect::<Vec<_>>();

    for (handle, kind, closed, carrier_span, case) in replacements {
        let replacement = match kind {
            SumPathExpressionKind::Name => {
                ExpressionNode::Name(closed_sum_path(syntax, &closed, carrier_span, case))
            }
            SumPathExpressionKind::Membership(value) => {
                ExpressionNode::Membership(syntax_trees::expression::TableMembershipExpression {
                    value,
                    domain: closed_sum_path(syntax, &closed, carrier_span, case),
                })
            }
            SumPathExpressionKind::StructLiteral(mut literal) => {
                literal.constructor_name = Identifier::new(
                    format!("{closed}::{case}"),
                    literal.constructor_name.source_span(),
                );
                ExpressionNode::StructLiteral(literal)
            }
        };
        syntax.expressions.replace_expression(handle, replacement);
    }
}

pub(in crate::generic_data) fn generic_sum_variant_names(
    syntax: &SyntaxTrees,
    instances: &HashMap<String, String>,
) -> HashMap<String, HashSet<String>> {
    syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Data(definition) if instances.contains_key(definition.name.as_str()) => Some((
                definition.name.as_str().to_owned(),
                syntax
                    .items
                    .data_members(definition.members)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Variant(variant) => Some(variant.name.as_str().to_owned()),
                        _ => None,
                    })
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

pub(in crate::generic_data) fn closed_sum_path(
    syntax: &mut SyntaxTrees,
    closed: &str,
    carrier_span: source::SourceSpan,
    case: Identifier,
) -> HandleSpan<Identifier> {
    let mut path = HandleSpan::empty();
    // Specialization changes the selected carrier, not its authored occurrence.
    // Membership checking rejoins this carrier receipt with the case receipt.
    syntax
        .expressions
        .append_identifier_path_member_to_span(&mut path, Identifier::new(closed, carrier_span));
    syntax
        .expressions
        .append_identifier_path_member_to_span(&mut path, case);
    path
}

#[cfg(test)]
mod tests {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
    };
    use source::{SourceId, SourceSpan, Span};
    use source_files_to_tokens::Lexer;
    use tokens_to_syntax_trees::parse_syntax_trees_with_id;

    #[test]
    fn closed_sum_membership_retains_both_authored_selection_spans() {
        let source = "data Maybe<T> { case None; case Some(value: T); }
            machine present(value: Maybe<i32>) -> bool { value in Maybe::Some }";
        let source_id = SourceId(0);
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize generic membership");
        let syntax =
            parse_syntax_trees_with_id(source_id, &tokens).expect("parse generic membership");
        let syntax = crate::normalize_generic_data(syntax).expect("select closed carrier");
        let resolved = crate::lower_syntax_trees(&syntax).expect("resolve closed membership");
        let start = source
            .rfind("Maybe::Some")
            .expect("authored membership path");
        for (kind, path, span) in [
            (
                AuthoredDeclarationSelectionKind::CaseReference,
                "Maybe<i32>",
                Span::new(start, start + 5),
            ),
            (
                AuthoredDeclarationSelectionKind::CaseMembership,
                "Maybe<i32>::Some",
                Span::new(start + 7, start + 11),
            ),
        ] {
            let selections = resolved
                .authored_declaration_selections()
                .iter()
                .filter(|selection| {
                    selection.kind() == kind
                        && selection.source_span() == SourceSpan::new(source_id, span)
                })
                .collect::<Vec<_>>();
            assert_eq!(
                selections.len(),
                1,
                "one exact authored {kind:?} occurrence"
            );
            let AuthoredDeclarationSelectionTarget::Resolved(target) = selections[0].target()
            else {
                panic!("closed membership selects a declaration");
            };
            assert_eq!(
                resolved
                    .symbols
                    .display_path(target.selected_symbol(), "::"),
                path
            );
        }
    }
}

#[derive(Clone)]
pub(in crate::generic_data) enum SumPathExpressionKind {
    Name,
    Membership(ExpressionHandle),
    StructLiteral(syntax_trees::expression::TableStructLiteral),
}
