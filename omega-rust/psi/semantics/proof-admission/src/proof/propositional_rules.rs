//! The propositional rules: primitive judgments, cited axioms and
//! assumptions, and the introduction and elimination of conjunction,
//! disjunction and implication.

use super::integer_math_normalization::propositions_match_under_integer_math_normalization;
use super::{AcceptanceBuilder, AcceptedProofRule, ProofError, RuleScope};
use crate::decide_primitive;
use semantic_vocabulary::Proposition;
use terminal_psi::{ProofNode, ProofRule};

pub(super) fn check_primitive(
    scope: &RuleScope<'_>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::Primitive(judgment) = &proof.rule else {
        unreachable!("dispatched check_primitive")
    };
    let RuleScope { context, .. } = *scope;
    acceptance.rules.insert(AcceptedProofRule::Primitive);
    decide_primitive(context, &proof.conclusion, *judgment).map_err(ProofError::PrimitiveJudgment)
}

pub(super) fn check_semantic_axiom(
    scope: &RuleScope<'_>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::SemanticAxiom { index } = &proof.rule else {
        unreachable!("dispatched check_semantic_axiom")
    };
    let RuleScope {
        semantic_axioms, ..
    } = *scope;
    acceptance.rules.insert(AcceptedProofRule::SemanticAxiom);
    let axiom = semantic_axioms
        .get(*index)
        .ok_or(ProofError::UnknownSemanticAxiom(*index))?;
    if !propositions_match_under_integer_math_normalization(axiom, &proof.conclusion) {
        return Err(ProofError::SemanticAxiomConclusionMismatch(*index));
    }
    acceptance.record_semantic_axiom(*index, axiom);
    Ok(())
}

pub(super) fn check_assumption(
    scope: &RuleScope<'_>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::Assumption { index } = &proof.rule else {
        unreachable!("dispatched check_assumption")
    };
    let RuleScope { assumptions, .. } = *scope;
    acceptance.rules.insert(AcceptedProofRule::Assumption);
    let assumption = assumptions
        .get(*index)
        .ok_or(ProofError::UnknownAssumption(*index))?;
    if !propositions_match_under_integer_math_normalization(assumption, &proof.conclusion) {
        return Err(ProofError::AssumptionConclusionMismatch(*index));
    }
    acceptance.record_assumption(*index, assumption);
    Ok(())
}

pub(super) fn check_conjunction_introduction(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::ConjunctionIntroduction(conjuncts) = &proof.rule else {
        unreachable!("dispatched check_conjunction_introduction")
    };
    acceptance
        .rules
        .insert(AcceptedProofRule::ConjunctionIntroduction);
    let Proposition::Conjunction(expected) = &proof.conclusion else {
        return Err(ProofError::RuleConclusionMismatch(
            "conjunction introduction",
        ));
    };
    if expected.len() != conjuncts.len() {
        return Err(ProofError::ConjunctionArityMismatch);
    }
    for (expected, conjunct) in expected.iter().zip(conjuncts) {
        if &conjunct.conclusion != expected {
            return Err(ProofError::ConjunctConclusionMismatch);
        }
    }
    Ok(())
}

pub(super) fn check_conjunction_elimination(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::ConjunctionElimination {
        conjunction,
        conjunct,
    } = &proof.rule
    else {
        unreachable!("dispatched check_conjunction_elimination")
    };
    acceptance
        .rules
        .insert(AcceptedProofRule::ConjunctionElimination);
    let Proposition::Conjunction(conjuncts) = &conjunction.conclusion else {
        return Err(ProofError::RulePremiseMismatch("conjunction elimination"));
    };
    let selected = conjuncts
        .get(*conjunct)
        .ok_or(ProofError::UnknownConjunct(*conjunct))?;
    (selected == &proof.conclusion)
        .then_some(())
        .ok_or(ProofError::ConjunctConclusionMismatch)
}

pub(super) fn check_disjunction_introduction(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::DisjunctionIntroduction { disjunct, index } = &proof.rule else {
        unreachable!("dispatched check_disjunction_introduction")
    };
    acceptance
        .rules
        .insert(AcceptedProofRule::DisjunctionIntroduction);
    let Proposition::Disjunction(disjuncts) = &proof.conclusion else {
        return Err(ProofError::RuleConclusionMismatch(
            "disjunction introduction",
        ));
    };
    let selected = disjuncts
        .get(*index)
        .ok_or(ProofError::UnknownDisjunct(*index))?;
    (selected == &disjunct.conclusion)
        .then_some(())
        .ok_or(ProofError::DisjunctConclusionMismatch)
}

pub(super) fn check_disjunction_elimination(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::DisjunctionElimination {
        disjunction,
        branches,
    } = &proof.rule
    else {
        unreachable!("dispatched check_disjunction_elimination")
    };
    acceptance
        .rules
        .insert(AcceptedProofRule::DisjunctionElimination);
    let Proposition::Disjunction(disjuncts) = &disjunction.conclusion else {
        return Err(ProofError::RulePremiseMismatch("disjunction elimination"));
    };
    if branches.len() != disjuncts.len() {
        return Err(ProofError::DisjunctionArityMismatch);
    }
    for branch in branches {
        if branch.conclusion != proof.conclusion {
            return Err(ProofError::DisjunctionBranchConclusionMismatch);
        }
    }
    Ok(())
}

pub(super) fn check_implication_introduction(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::ImplicationIntroduction { body } = &proof.rule else {
        unreachable!("dispatched check_implication_introduction")
    };
    acceptance
        .rules
        .insert(AcceptedProofRule::ImplicationIntroduction);
    let Proposition::Implication { conclusion, .. } = &proof.conclusion else {
        return Err(ProofError::RuleConclusionMismatch(
            "implication introduction",
        ));
    };
    (&body.conclusion == conclusion.as_ref())
        .then_some(())
        .ok_or(ProofError::ImplicationConclusionMismatch)
}

pub(super) fn check_implication_elimination(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::ImplicationElimination {
        implication,
        premise,
    } = &proof.rule
    else {
        unreachable!("dispatched check_implication_elimination")
    };
    acceptance
        .rules
        .insert(AcceptedProofRule::ImplicationElimination);
    let Proposition::Implication {
        premise: required,
        conclusion,
    } = &implication.conclusion
    else {
        return Err(ProofError::RulePremiseMismatch("implication elimination"));
    };
    if premise.conclusion != **required {
        return Err(ProofError::ImplicationPremiseMismatch);
    }
    (&proof.conclusion == conclusion.as_ref())
        .then_some(())
        .ok_or(ProofError::ImplicationConclusionMismatch)
}
