use super::lower_expression_handle_from_table_with_self_substitution;
use crate::name::lower_name;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

/// `value in Type::Case` is the tag test (frozen decision 11): it lowers to
/// a tag-equality compare against the case name. The lowered form is the
/// SAME binary-equality shape a payload-less `==` produces, on purpose --
/// the guard tag clamp, value-position enum-constant folding, and the
/// interpreter's tag-only enum equality all consume it unchanged. The
/// user-facing bare-case `==` error cannot re-flag it because that check
/// runs on the RESOLVED trees, before this lowering (see `crate::equality`).
///
/// Returns `None` when the membership domain does not name a case of a data
/// definition, so the caller falls back to declared-domain lowering.
pub(super) fn lower_case_membership_expression(
    program: &resolved::SymbolResolvedTrees,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::TypedTrees,
    value: typed::expression::ExpressionHandle,
    domain: arena::HandleSpan<resolved::name::DiagnosticName>,
    type_symbol: symbols::SymbolHandle,
    case_symbol: symbols::SymbolHandle,
) -> Option<typed::expression::ExpressionHandle> {
    lower_case_membership_expression_from_members(
        program,
        target,
        value,
        source.name_path_members(domain),
        type_symbol,
        case_symbol,
    )
}

fn lower_case_membership_expression_from_members(
    program: &resolved::SymbolResolvedTrees,
    target: &mut typed::TypedTrees,
    value: typed::expression::ExpressionHandle,
    domain_members: &[resolved::name::DiagnosticName],
    type_symbol: symbols::SymbolHandle,
    case_symbol: symbols::SymbolHandle,
) -> Option<typed::expression::ExpressionHandle> {
    let [_type_name, _case_name] = domain_members else {
        return None;
    };
    if !type_symbol.is_valid() || !case_symbol.is_valid() {
        return None;
    }

    let data_definition = program
        .data_definitions
        .iter()
        .find(|definition| definition.symbol == type_symbol)?;
    let exact_case_belongs_to_type = program
        .data_members(data_definition.members)
        .iter()
        .any(|member| matches!(member, resolved::data::DataMember::Variant(variant) if variant.symbol == case_symbol));
    if !exact_case_belongs_to_type {
        return None;
    }

    // The case reference must carry its symbols: the backend's guard tag
    // clamp keys the tag-only compare off a symbol-stamped `Type::Case` path.
    let mut members = arena::HandleSpan::empty();
    for member in domain_members {
        target
            .expression_table
            .push_name_path_member(&mut members, lower_name(member));
    }
    let mut member_symbols = arena::HandleSpan::empty();
    target
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, type_symbol);
    target
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, case_symbol);
    let case_reference = target
        .expression_table
        .insert(typed::expression::ExpressionNode::Name(
            typed::expression::TableNamePath {
                members,
                member_symbols,
                head_symbol: type_symbol,
                symbol: case_symbol,
            },
        ));

    Some(
        target
            .expression_table
            .insert(typed::expression::ExpressionNode::Binary(
                typed::expression::TableBinaryExpression {
                    left: value,
                    operator: typed::expression::BinaryOperator::Equal,
                    right: case_reference,
                },
            )),
    )
}

pub(super) fn lower_domain_membership_expression(
    program: &resolved::SymbolResolvedTrees,
    target: &mut typed::TypedTrees,
    value: typed::expression::ExpressionHandle,
    domain_symbol: symbols::SymbolHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    let Some(domain_definition) = program
        .domain_definitions
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return Err(Diagnostic::error(format!(
            "cannot lower executable membership for unknown domain symbol {}",
            domain_symbol.arena_index()
        )));
    };
    let expanded = crate::domain::expand_domain_reference(
        program,
        domain_symbol,
        vec![domain_definition.name.clone()],
    )?;
    let mut lowered = Vec::with_capacity(expanded.len());
    for atom in expanded {
        lowered.push(lower_atomic_domain_membership_expression(
            program,
            target,
            value,
            atom.symbol,
        )?);
    }
    combine_conjunction(target, lowered)
}

fn lower_atomic_domain_membership_expression(
    program: &resolved::SymbolResolvedTrees,
    target: &mut typed::TypedTrees,
    value: typed::expression::ExpressionHandle,
    domain_symbol: symbols::SymbolHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    let Some(domain_definition) = program
        .domain_definitions
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return Err(Diagnostic::error(format!(
            "cannot lower executable membership for unknown domain symbol {}",
            domain_symbol.arena_index()
        )));
    };
    let source = &program.tables.bodies.expressions;
    let mut lowered_facts = Vec::new();

    for fact in program.proof_facts(domain_definition.facts) {
        let lowered = match fact {
            resolved::domain::ProofFact::Expression(expression) => {
                lower_expression_handle_from_table_with_self_substitution(
                    Some(program),
                    source,
                    target,
                    *expression,
                    Some(value),
                )?
            }
            resolved::domain::ProofFact::Membership(membership) => {
                let nested_value = lower_expression_handle_from_table_with_self_substitution(
                    Some(program),
                    source,
                    target,
                    membership.value,
                    Some(value),
                )?;
                let (case_type_symbol, case_symbol) =
                    case_symbols_for_domain_fact(program, membership.domain);
                if let Some(case_membership) = lower_case_membership_expression_from_members(
                    program,
                    target,
                    nested_value,
                    program.domain_path_members(membership.domain),
                    case_type_symbol,
                    case_symbol,
                ) {
                    case_membership
                } else {
                    lower_domain_membership_expression(
                        program,
                        target,
                        nested_value,
                        membership.domain_symbol,
                    )?
                }
            }
        };
        lowered_facts.push(lowered);
    }

    combine_conjunction(target, lowered_facts)
}

fn case_symbols_for_domain_fact(
    program: &resolved::SymbolResolvedTrees,
    domain: arena::HandleSpan<resolved::name::DiagnosticName>,
) -> (symbols::SymbolHandle, symbols::SymbolHandle) {
    let [type_name, case_name] = program.domain_path_members(domain) else {
        return (
            symbols::SymbolHandle::invalid(),
            symbols::SymbolHandle::invalid(),
        );
    };
    let Some(data) = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == type_name.as_str())
    else {
        return (
            symbols::SymbolHandle::invalid(),
            symbols::SymbolHandle::invalid(),
        );
    };
    let case_symbol = program
        .data_members(data.members)
        .iter()
        .find_map(|member| match member {
            resolved::data::DataMember::Variant(variant)
                if variant.name.as_str() == case_name.as_str() =>
            {
                Some(variant.symbol)
            }
            _ => None,
        })
        .unwrap_or_else(symbols::SymbolHandle::invalid);
    (data.symbol, case_symbol)
}

fn combine_conjunction(
    target: &mut typed::TypedTrees,
    lowered_facts: Vec<typed::expression::ExpressionHandle>,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    let mut lowered_facts = lowered_facts.into_iter();
    let Some(mut combined) = lowered_facts.next() else {
        return Ok(target
            .expression_table
            .insert(typed::expression::ExpressionNode::Boolean(true)));
    };

    for fact in lowered_facts {
        combined = target
            .expression_table
            .insert(typed::expression::ExpressionNode::Binary(
                typed::expression::TableBinaryExpression {
                    left: combined,
                    operator: typed::expression::BinaryOperator::And,
                    right: fact,
                },
            ));
    }

    Ok(combined)
}
