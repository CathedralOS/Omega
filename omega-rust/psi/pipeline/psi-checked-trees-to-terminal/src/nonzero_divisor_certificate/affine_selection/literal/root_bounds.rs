//! Fixed root-bound orientations for affine-literal certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};

struct OrientedBound {
    proposition: Proposition,
    substitution_endpoint: usize,
}

pub(super) fn direct(
    root: &ScalarTerm,
    literal: &ScalarTerm,
    equality: &ProofNode,
) -> [ProofNode; 2] {
    ordered(root, literal).map(|bound| {
        substitute(
            bound.proposition,
            closed_relation(literal),
            equality,
            bound.substitution_endpoint,
        )
    })
}

pub(super) fn one_alias(
    root: &ScalarTerm,
    alias: &ScalarTerm,
    literal: &ScalarTerm,
    outer_equality: &ProofNode,
    inner_equality: &ProofNode,
) -> [ProofNode; 2] {
    let alias_bounds = ordered(alias, literal);
    let root_bounds = ordered(root, literal);
    [
        nested_substitution(
            &alias_bounds[0],
            &root_bounds[0],
            literal,
            outer_equality,
            inner_equality,
        ),
        nested_substitution(
            &alias_bounds[1],
            &root_bounds[1],
            literal,
            outer_equality,
            inner_equality,
        ),
    ]
}

fn ordered(value: &ScalarTerm, literal: &ScalarTerm) -> [OrientedBound; 2] {
    [
        OrientedBound {
            proposition: Proposition::LessOrEqual(literal.clone(), value.clone()),
            substitution_endpoint: 1,
        },
        OrientedBound {
            proposition: Proposition::LessOrEqual(value.clone(), literal.clone()),
            substitution_endpoint: 0,
        },
    ]
}

fn nested_substitution(
    alias_bound: &OrientedBound,
    root_bound: &OrientedBound,
    literal: &ScalarTerm,
    outer_equality: &ProofNode,
    inner_equality: &ProofNode,
) -> ProofNode {
    let alias_bound = substitute(
        alias_bound.proposition.clone(),
        closed_relation(literal),
        inner_equality,
        alias_bound.substitution_endpoint,
    );
    substitute(
        root_bound.proposition.clone(),
        alias_bound,
        outer_equality,
        root_bound.substitution_endpoint,
    )
}

fn closed_relation(literal: &ScalarTerm) -> ProofNode {
    ProofNode {
        conclusion: Proposition::LessOrEqual(literal.clone(), literal.clone()),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    }
}

fn substitute(
    conclusion: Proposition,
    relation: ProofNode,
    equality: &ProofNode,
    endpoint: usize,
) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(relation),
            equality: Box::new(equality.clone()),
            endpoint,
        },
    }
}
