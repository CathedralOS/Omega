use super::*;

enum Action<'proof> {
    Enter(&'proof ProofNode),
    Post(&'proof ProofNode),
    Children(&'proof [ProofNode]),
    Cases(&'proof ProofNode, &'proof [ProofNode]),
    Branches(&'proof [Proposition], &'proof [ProofNode]),
    PushAssumption(&'proof Proposition),
    PopAssumption,
}

pub(super) fn check_node(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ambient_count = assumptions.len();
    let mut assumptions = assumptions.to_vec();
    let mut pending = vec![Action::Enter(proof)];
    while let Some(action) = pending.pop() {
        match action {
            Action::Enter(proof) => {
                context
                    .validate(&proof.conclusion)
                    .map_err(ProofError::MalformedProposition)?;
                pending.push(Action::Post(proof));
                schedule_children(proof, &mut pending)?;
            }
            Action::Post(proof) => {
                check_node_locally(
                    context,
                    &assumptions,
                    semantic_axioms,
                    machine_parameter_values,
                    proof,
                    acceptance,
                )?;
            }
            Action::Children(children) => {
                if let Some((first, remaining)) = children.split_first() {
                    pending.push(Action::Children(remaining));
                    pending.push(Action::Enter(first));
                }
            }
            Action::Cases(disjunction, branches) => {
                let Proposition::Disjunction(disjuncts) = &disjunction.conclusion else {
                    return Err(ProofError::RulePremiseMismatch("disjunction elimination"));
                };
                if branches.len() != disjuncts.len() {
                    return Err(ProofError::DisjunctionArityMismatch);
                }
                pending.push(Action::Branches(disjuncts, branches));
            }
            Action::Branches(disjuncts, branches) => {
                if let Some((first, remaining)) = branches.split_first() {
                    let (disjunct, remaining_disjuncts) =
                        disjuncts.split_first().expect("checked case arity");
                    pending.push(Action::Branches(remaining_disjuncts, remaining));
                    pending.push(Action::PopAssumption);
                    pending.push(Action::Enter(first));
                    pending.push(Action::PushAssumption(disjunct));
                }
            }
            Action::PushAssumption(proposition) => assumptions.push(proposition.clone()),
            Action::PopAssumption => {
                debug_assert!(assumptions.len() > ambient_count);
                assumptions.pop().expect("scheduled local assumption");
            }
        }
    }
    debug_assert_eq!(assumptions.len(), ambient_count);
    Ok(())
}

fn schedule_children<'proof>(
    proof: &'proof ProofNode,
    pending: &mut Vec<Action<'proof>>,
) -> Result<(), ProofError> {
    match &proof.rule {
        ProofRule::Primitive(_)
        | ProofRule::SemanticAxiom { .. }
        | ProofRule::Assumption { .. }
        | ProofRule::IntegerCorrelatedForbiddenRoots { .. } => {}
        ProofRule::ConjunctionIntroduction(children) => {
            let Proposition::Conjunction(expected) = &proof.conclusion else {
                return Err(ProofError::RuleConclusionMismatch(
                    "conjunction introduction",
                ));
            };
            if children.len() != expected.len() {
                return Err(ProofError::ConjunctionArityMismatch);
            }
            pending.push(Action::Children(children));
        }
        ProofRule::ConjunctionElimination { conjunction, .. } => {
            pending.push(Action::Enter(conjunction));
        }
        ProofRule::DisjunctionIntroduction { disjunct, index } => {
            let Proposition::Disjunction(disjuncts) = &proof.conclusion else {
                return Err(ProofError::RuleConclusionMismatch(
                    "disjunction introduction",
                ));
            };
            disjuncts
                .get(*index)
                .ok_or(ProofError::UnknownDisjunct(*index))?;
            pending.push(Action::Enter(disjunct));
        }
        ProofRule::DisjunctionElimination {
            disjunction,
            branches,
        } => {
            pending.push(Action::Cases(disjunction, branches));
            pending.push(Action::Enter(disjunction));
        }
        ProofRule::ImplicationIntroduction { body } => {
            let Proposition::Implication { premise, .. } = &proof.conclusion else {
                return Err(ProofError::RuleConclusionMismatch(
                    "implication introduction",
                ));
            };
            pending.push(Action::PopAssumption);
            pending.push(Action::Enter(body));
            pending.push(Action::PushAssumption(premise));
        }
        ProofRule::ImplicationElimination {
            implication,
            premise,
        } => {
            pending.push(Action::Enter(premise));
            pending.push(Action::Enter(implication));
        }
        ProofRule::EqualitySymmetry { equality } => {
            pending.push(Action::Enter(equality));
        }
        ProofRule::IntegerOrderWeakening { relation } => {
            pending.push(Action::Enter(relation));
        }
        ProofRule::EqualityTransitivity {
            left_equals_middle,
            middle_equals_right,
        } => {
            pending.push(Action::Enter(middle_equals_right));
            pending.push(Action::Enter(left_equals_middle));
        }
        ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } => {
            pending.push(Action::Enter(middle_less_or_equal_right));
            pending.push(Action::Enter(left_less_or_equal_middle));
        }
        ProofRule::IntegerOrderSubstitution {
            relation, equality, ..
        } => {
            pending.push(Action::Enter(equality));
            pending.push(Action::Enter(relation));
        }
        ProofRule::IntegerAffineBound { root_bound, .. }
        | ProofRule::IntegerCastBound { root_bound, .. } => {
            pending.push(Action::Enter(root_bound));
        }
        ProofRule::IntegerExactAddDefinitionBound {
            left_bound,
            right_bound,
            ..
        } => {
            pending.push(Action::Enter(right_bound));
            pending.push(Action::Enter(left_bound));
        }
    }
    Ok(())
}
