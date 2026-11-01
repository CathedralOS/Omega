//! An independently required invocation may consume only the retained complete
//! EntryInvariant judgment. RankInvariant does not establish extra entry facts,
//! and this consumer never selects or reproves another coordinate assignment.

pub(crate) fn proves_ranked_entry_requirement(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    goal: typed_trees::expression::ExpressionHandle,
    admitted: &checked_trees::CheckedRankArrivalPlan,
) -> bool {
    validation::arithmetic_entry_requirement_is_covered(program, machine, goal)
        && admitted
            .for_body(machine.symbol, machine.states)
            .is_some_and(|body| {
                body.premise_regime == checked_trees::CheckedRankPremiseRegime::EntryInvariant
            })
}
