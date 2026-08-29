//! Deterministic control-flow edge fact reconstruction.

use std::collections::BTreeMap;

use psi_core::{IntegerSign, IntegerValue, Proposition, ScalarTerm, ValueId};

use super::super::substitution::{substitute_proposition_values, substitute_scalar_term_values};

pub(super) fn true_condition_fact(
    condition: ValueId,
    axioms: &[Proposition],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
) -> Option<Proposition> {
    let condition = value_term(condition);
    let predicate = axioms.iter().rev().find_map(|axiom| match axiom {
        Proposition::Equal(left, right) if left == &condition => Some(right),
        Proposition::Equal(left, right) if right == &condition => Some(left),
        _ => None,
    })?;
    let constants = axioms
        .iter()
        .filter_map(|axiom| match axiom {
            Proposition::Equal(ScalarTerm::Value { id, .. }, value)
                if matches!(value, ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. }) =>
            {
                Some((*id, value.clone()))
            }
            Proposition::Equal(value, ScalarTerm::Value { id, .. })
                if matches!(value, ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. }) =>
            {
                Some((*id, value.clone()))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let predicate = substitute_scalar_term_values(predicate, &constants);
    match &predicate {
        ScalarTerm::BooleanEqual { left, right } | ScalarTerm::IntegerEqual { left, right, .. } => {
            Some(Proposition::Equal((**left).clone(), (**right).clone()))
        }
        ScalarTerm::IntegerLessThan { left, right, .. } => {
            Some(Proposition::LessThan((**left).clone(), (**right).clone()))
        }
        ScalarTerm::IntegerLessOrEqual { left, right, .. } => Some(Proposition::LessOrEqual(
            (**left).clone(),
            (**right).clone(),
        )),
        _ => None,
    }
}

pub(super) fn bind_successor_axioms(
    axioms: &mut Vec<Proposition>,
    target_block: &psi_terminal::Block,
    arguments: &[ValueId],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    rewrite_path_facts: bool,
) {
    let substitutions = target_block
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (*argument, value_term(parameter.id)))
        .collect::<BTreeMap<_, _>>();
    let established = axioms.clone();
    // Emit edge equalities before rewritten path facts so independently
    // reconstructed axiom indexes are deterministic.
    for (parameter, argument) in target_block.parameters.iter().zip(arguments) {
        axioms.push(Proposition::Equal(
            value_term(parameter.id),
            value_term(*argument),
        ));
    }
    if rewrite_path_facts {
        for proposition in &established {
            let rewritten = substitute_proposition_values(proposition, &substitutions);
            push_unique(axioms, rewritten);
        }
    }
}

pub(super) fn append_successor_fact(
    axioms: &mut Vec<Proposition>,
    proposition: &Proposition,
    target_block: &psi_terminal::Block,
    arguments: &[ValueId],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    derive_discrete_unsigned_positive: bool,
) {
    let substitutions = target_block
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (*argument, value_term(parameter.id)))
        .collect::<BTreeMap<_, _>>();
    push_unique(axioms, proposition.clone());
    let rewritten = substitute_proposition_values(proposition, &substitutions);
    if derive_discrete_unsigned_positive {
        append_discrete_unsigned_positive_fact(axioms, proposition);
        append_discrete_unsigned_positive_fact(axioms, &rewritten);
    }
    push_unique(axioms, rewritten);
}

/// Fixed unsigned integers are discrete: a taken `0 < value` edge also
/// establishes the exact `1 <= value` premise needed by subtraction of one.
fn append_discrete_unsigned_positive_fact(
    propositions: &mut Vec<Proposition>,
    proposition: &Proposition,
) {
    let Proposition::LessThan(
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(0),
        },
        right,
    ) = proposition
    else {
        return;
    };
    if scalar_type.sign() != IntegerSign::Unsigned {
        return;
    }
    let one = ScalarTerm::integer(*scalar_type, IntegerValue::Unsigned(1))
        .expect("one belongs to every validated unsigned fixed carrier");
    push_unique(propositions, Proposition::LessOrEqual(one, right.clone()));
}

fn push_unique(propositions: &mut Vec<Proposition>, proposition: Proposition) {
    if !propositions.contains(&proposition) {
        propositions.push(proposition);
    }
}
