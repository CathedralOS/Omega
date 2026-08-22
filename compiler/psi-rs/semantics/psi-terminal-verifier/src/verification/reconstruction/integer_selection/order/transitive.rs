//! Independent exact two-fact integer transitivity reconstruction.

use psi_core::Proposition;

pub(super) fn retained(
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    requirements.iter().chain(semantic_axioms).any(|left_fact| {
        let Proposition::LessOrEqual(left, middle) = left_fact else {
            return false;
        };
        left == goal_left
            && requirements
                .iter()
                .chain(semantic_axioms)
                .any(|right_fact| {
                    matches!(
                        right_fact,
                        Proposition::LessOrEqual(right_middle, right)
                            if right_middle == middle && right == goal_right
                    )
                })
    })
}
