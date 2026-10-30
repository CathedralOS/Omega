//! Authored rank ranges are obligations, never inferred helper premises.
//!
//! The root proof uses only entry Requires, declared domains and geometry.
//! Ordinary invocation checking proves those same premises under exact actual
//! substitution, so its entry theorem supplies formation and membership at
//! each call. Re-proving them with a narrower arithmetic solver would reject
//! evidence-backed calls. The joint Nat₀ relation itself never assumes the
//! optional interval: each edge independently proves nonnegative nonincrease
//! and the weak-edge graph must be acyclic. Actual view parameters retain
//! their separate obligations in their own view owner.

use typed_trees::machine::Machine;

pub(super) fn entry(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    measure: validation::RankingRangeMeasure,
) -> Result<(), &'static str> {
    let Some(witness) = machine.termination_plan.implementation_witness.as_ref() else {
        return Ok(());
    };
    if witness.rank_range.is_none() {
        return Ok(());
    }
    let range = program
        .ranking_expression_custody_for(machine.symbol)
        .and_then(|custody| custody.rank_range)
        .filter(|range| range.is_valid())
        .ok_or("an authored rank range has no exact source custody")?;
    let root = program
        .machine_states(machine)
        .first()
        .ok_or("a ranged rank has no invocation entry")?;
    if !validation::prove_ranking_range_entry(program, machine, root, range, measure) {
        return Err("an authored natural rank range has no independent entry formation");
    }
    Ok(())
}
