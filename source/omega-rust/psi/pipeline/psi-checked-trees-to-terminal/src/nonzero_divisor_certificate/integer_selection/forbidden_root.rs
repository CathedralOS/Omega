//! Definition-local same-root affine divide/remainder certificates.

use std::collections::BTreeSet;

use psi_core::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};
use psi_proof_admission::{
    CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness,
    IntegerCorrelatedForbiddenRootWitness, ProofNode, ProofRule,
    check_integer_correlated_forbidden_root_witness,
};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    let (integer_type, dividend, divisor) = exact_division_operands(goal)?;
    let definition_axiom_count = semantic_axioms.len();
    let dividend_branch = affine_branch(
        integer_type,
        dividend.clone(),
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    let divisor_branch = affine_branch(
        integer_type,
        divisor.clone(),
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    if dividend_branch.root != divisor_branch.root
        || !dividend_branch
            .definition_axioms
            .is_disjoint(&divisor_branch.definition_axioms)
        || dividend_branch.definition_axioms.iter().next_back()?
            >= divisor_branch.definition_axioms.iter().next()?
    {
        return None;
    }

    let mut available = semantic_axioms.to_vec();
    available.extend_from_slice(assumptions);
    let (lower_bound_axiom, lower, upper_bound_axiom, upper) = tight_bounds(
        integer_type,
        &dividend_branch.root,
        &available,
        definition_axiom_count,
    )?;
    let witness = IntegerCorrelatedForbiddenRootWitness {
        dividend: CorrelatedAffineBranchWitness {
            root: dividend_branch.root.clone(),
            target: dividend_branch.target,
            steps: dividend_branch.steps,
        },
        divisor: CorrelatedAffineBranchWitness {
            root: divisor_branch.root,
            target: divisor_branch.target,
            steps: divisor_branch.steps,
        },
        definition_axiom_count,
        lower_bound_axiom,
        upper_bound_axiom,
        conclusion: Proposition::Conjunction(vec![lower, upper]),
    };
    let checked = check_integer_correlated_forbidden_root_witness(
        context,
        &available,
        machine_parameter_values,
        &witness,
    )
    .ok()?;
    if !checked.forbidden_roots().is_empty()
        || checked.dividend() != &dividend
        || checked.divisor() != &divisor
    {
        return None;
    }
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerCorrelatedForbiddenRoots { witness },
    })
}

#[derive(Debug)]
struct AffineBranch {
    root: ScalarTerm,
    target: ScalarTerm,
    definition_axioms: BTreeSet<usize>,
    steps: Vec<CorrelatedAffineStepWitness>,
}

fn affine_branch(
    integer_type: IntegerType,
    mut current: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<AffineBranch> {
    if integer_type.carrier() != IntegerCarrier::Fixed
        || integer_type.sign() != IntegerSign::Signed
        || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
    {
        return None;
    }
    let target = current.clone();
    let mut prior = definition_axiom_count.min(semantic_axioms.len());
    let mut definition_axioms = BTreeSet::new();
    let mut reverse_steps = Vec::new();
    for _ in 0..=prior {
        if !definition_axioms.is_empty()
            && matches!(
                &current,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == integer_type && machine_parameter_values.contains(id)
            )
        {
            reverse_steps.reverse();
            return Some(AffineBranch {
                root: current,
                target,
                definition_axioms,
                steps: reverse_steps,
            });
        }
        let (definition_axiom, expression) =
            semantic_axioms[..prior].iter().enumerate().rev().find_map(
                |(index, axiom)| match axiom {
                    Proposition::Equal(left, right) if left == &current => Some((index, right)),
                    _ => None,
                },
            )?;
        let (next, sibling) = match expression {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (left, right),
            _ => return None,
        };
        if landed_literal(integer_type, next, semantic_axioms, definition_axiom).is_some()
            || landed_literal(integer_type, sibling, semantic_axioms, definition_axiom).is_none()
            || !definition_axioms.insert(definition_axiom)
        {
            return None;
        }
        reverse_steps.push(CorrelatedAffineStepWitness {
            definition_axiom,
            literal_axiom: landed_literal_axiom(
                integer_type,
                sibling,
                semantic_axioms,
                definition_axiom,
            )?,
        });
        current = (**next).clone();
        prior = definition_axiom;
    }
    None
}

fn landed_literal(
    integer_type: IntegerType,
    term: &ScalarTerm,
    semantic_axioms: &[Proposition],
    prior: usize,
) -> Option<i128> {
    signed_literal(integer_type, term).or_else(|| {
        semantic_axioms[..prior.min(semantic_axioms.len())]
            .iter()
            .rev()
            .find_map(|axiom| match axiom {
                Proposition::Equal(left, right) if left == term => {
                    signed_literal(integer_type, right)
                }
                _ => None,
            })
    })
}

fn landed_literal_axiom(
    integer_type: IntegerType,
    term: &ScalarTerm,
    semantic_axioms: &[Proposition],
    prior: usize,
) -> Option<Option<usize>> {
    if signed_literal(integer_type, term).is_some() {
        return Some(None);
    }
    semantic_axioms[..prior.min(semantic_axioms.len())]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, axiom)| match axiom {
            Proposition::Equal(left, right)
                if left == term && signed_literal(integer_type, right).is_some() =>
            {
                Some(Some(index))
            }
            _ => None,
        })
}

fn tight_bounds(
    integer_type: IntegerType,
    root: &ScalarTerm,
    available: &[Proposition],
    definition_axiom_count: usize,
) -> Option<(usize, Proposition, usize, Proposition)> {
    let IntegerValue::Signed(mut minimum) = integer_type.minimum_value() else {
        return None;
    };
    let IntegerValue::Signed(mut maximum) = integer_type.maximum_value() else {
        return None;
    };
    let mut lower = None;
    let mut upper = None;
    for (index, proposition) in available.iter().enumerate().skip(definition_axiom_count) {
        let Proposition::LessOrEqual(left, right) = proposition else {
            continue;
        };
        if right == root
            && let Some(candidate) = signed_literal(integer_type, left)
            && candidate > minimum
        {
            minimum = candidate;
            lower = Some((index, proposition.clone()));
        }
        if left == root
            && let Some(candidate) = signed_literal(integer_type, right)
            && candidate < maximum
        {
            maximum = candidate;
            upper = Some((index, proposition.clone()));
        }
    }
    let (lower_axiom, lower) = lower?;
    let (upper_axiom, upper) = upper?;
    (minimum <= maximum).then_some((lower_axiom, lower, upper_axiom, upper))
}

fn exact_division_operands(goal: &Proposition) -> Option<(IntegerType, ScalarTerm, ScalarTerm)> {
    let Proposition::Disjunction(disjuncts) = goal else {
        return None;
    };
    let [
        Proposition::LessOrEqual(divisor, negative_two),
        Proposition::LessOrEqual(one, positive_divisor),
        Proposition::Conjunction(exception),
    ] = disjuncts.as_slice()
    else {
        return None;
    };
    let [
        Proposition::LessOrEqual(exception_divisor, negative_one),
        Proposition::LessOrEqual(minimum_plus_one, dividend),
    ] = exception.as_slice()
    else {
        return None;
    };
    if divisor != positive_divisor || divisor != exception_divisor {
        return None;
    }
    let ScalarType::Integer(integer_type) = divisor.scalar_type() else {
        return None;
    };
    let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
        return None;
    };
    if signed_literal(integer_type, negative_two) != Some(-2)
        || signed_literal(integer_type, one) != Some(1)
        || signed_literal(integer_type, negative_one) != Some(-1)
        || signed_literal(integer_type, minimum_plus_one) != minimum.checked_add(1)
        || dividend.scalar_type() != ScalarType::Integer(integer_type)
    {
        return None;
    }
    Some((integer_type, dividend.clone(), divisor.clone()))
}

fn signed_literal(integer_type: IntegerType, term: &ScalarTerm) -> Option<i128> {
    match term.integer_value()? {
        (actual_type, IntegerValue::Signed(value)) if actual_type == integer_type => Some(value),
        _ => None,
    }
}
