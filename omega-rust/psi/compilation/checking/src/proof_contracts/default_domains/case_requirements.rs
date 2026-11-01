//! The default-domain requirements one selected case must satisfy.
//!
//! Case predicates live in the data declaration's `requires` as
//! `self in Data::Case` disjunctions; a case declares none of its own. Every
//! established value satisfies that domain, so once dispatch has selected a
//! case, each disjunct guarded by a different case is refuted and the
//! remaining disjunct's other conjuncts hold. This owner performs that one
//! projection for every consumer that reasons from a selected case.
//!
//! Only a clause that mentions case membership is projected: a clause without
//! one is a common requirement owned by the data-level readers. A clause that
//! leaves more than one live disjunct has no conjunctive consequence and
//! contributes nothing; a disjunct naming two different cases is unreachable.
//! A missed requirement is sound; a wrong one is not.

use crate::frozen_program::expression_table;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

/// The conjunct atoms the default domain of `definition` implies for a value
/// whose active case is `case`. Atoms keep the declaration's spelling over the
/// predicate subject (`self.v >= 40`); consumers project `self` onto the
/// dispatched place.
pub(crate) fn selected_case_requirements(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    case: symbols::SymbolHandle,
) -> Vec<ExpressionHandle> {
    let Some(selected) = case_position(program, case) else {
        return Vec::new();
    };
    let mut requirements = Vec::new();
    for fact in program.proof_facts.span_or_empty(definition.where_facts) {
        let Some(expression) = fact.as_expression() else {
            continue;
        };
        let mut clauses = Vec::new();
        flatten(program, *expression, BinaryOperator::And, &mut clauses);
        for clause in clauses {
            project_clause(program, clause, selected, &mut requirements);
        }
    }
    requirements
}

fn project_clause(
    program: &dyn crate::CheckingInput,
    clause: ExpressionHandle,
    selected: u32,
    requirements: &mut Vec<ExpressionHandle>,
) {
    let mut disjuncts = Vec::new();
    flatten(program, clause, BinaryOperator::Or, &mut disjuncts);
    let mut mentions_case = false;
    let mut live = None;
    for disjunct in disjuncts {
        let mut atoms = Vec::new();
        flatten(program, disjunct, BinaryOperator::And, &mut atoms);
        let mut guards = Vec::new();
        let mut residual = Vec::new();
        for atom in atoms {
            match membership_case(program, atom) {
                Some(position) => guards.push(position),
                None => residual.push(atom),
            }
        }
        mentions_case |= !guards.is_empty();
        if guards.iter().any(|position| *position != selected) {
            continue;
        }
        if live.is_some() {
            return;
        }
        live = Some(residual);
    }
    if mentions_case && let Some(residual) = live {
        requirements.extend(residual);
    }
}

/// The member position of the case a `self in Data::Case` atom names. Generic
/// instances share their template's member order, so position identifies the
/// case across an applied declaration and the facts copied into it.
fn membership_case(program: &dyn crate::CheckingInput, atom: ExpressionHandle) -> Option<u32> {
    let ExpressionNode::Binary(binary) = expression_table(program).expression(atom) else {
        return None;
    };
    let membership = binary.operator == BinaryOperator::CaseMembership
        || expression_table(program)
            .authored_selection_occurrences(atom)
            .any(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| {
                        selection.kind()
                            == typed_trees::AuthoredDeclarationSelectionKind::CaseMembership
                    })
            });
    if !membership {
        return None;
    }
    let path = expression_table(program).name_path_node(binary.right)?;
    // The whole path names the case; the head may name its data type.
    [path.symbol, path.head_symbol]
        .into_iter()
        .chain(
            expression_table(program)
                .name_path_member_symbols(path.member_symbols)
                .last()
                .copied(),
        )
        .find_map(|symbol| case_position(program, symbol))
}

fn case_position(program: &dyn crate::CheckingInput, case: symbols::SymbolHandle) -> Option<u32> {
    if !case.is_valid() {
        return None;
    }
    program
        .data_variant_position_for_symbol(case)
        .map(|(_, position)| position)
}

fn flatten(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    out: &mut Vec<ExpressionHandle>,
) {
    if let ExpressionNode::Binary(binary) = expression_table(program).expression(expression)
        && binary.operator == operator
    {
        flatten(program, binary.left, operator, out);
        flatten(program, binary.right, operator, out);
        return;
    }
    out.push(expression);
}
