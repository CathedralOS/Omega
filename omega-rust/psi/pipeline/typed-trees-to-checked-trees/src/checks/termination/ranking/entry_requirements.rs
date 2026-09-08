//! Export only requirements covered by a complete inductive graph judgment.

use super::*;

pub(crate) fn proves_ranked_entry_requirement(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    goal: ExpressionHandle,
) -> bool {
    prove(program, machine, goal).unwrap_or(false)
}

fn prove(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    goal: ExpressionHandle,
) -> Option<bool> {
    if !validation::arithmetic_entry_requirement_is_covered(program, machine, goal) {
        return None;
    }
    let root = program.machine_states(machine).first()?;
    let witness = machine.termination_plan.implementation_witness.as_ref()?;
    let subjects = resolve_machine_witness_subjects(program, machine)?;
    let arguments = resolve_machine_witness_view_arguments(program, machine)?;
    let path = witness
        .view_path
        .split("::")
        .filter(|member| !member.is_empty())
        .collect::<Vec<_>>();
    let OrderResolution::Resolved(order) =
        RankingOrder::resolve(program, root, &subjects, &path, &arguments)
    else {
        return None;
    };
    let measure = match (&order, subjects.as_slice()) {
        (RankingOrder::IncreasingTo(limit), [subject]) => DecreaseMeasure::Distance {
            lower: *subject,
            upper: *limit,
        },
        (_, [subject]) => DecreaseMeasure::Single(*subject),
        (_, [lower, upper]) => DecreaseMeasure::Distance {
            lower: *lower,
            upper: *upper,
        },
        _ => return None,
    };
    // This is not acceptance of an authored witness. The full graph establishes
    // entry membership and re-establishes its entry premises on EVERY arrival.
    // RankInvariant alone would not prove the extra required comparison.
    Some(ranges::proves_entry_requirements(
        program, machine, &order, measure,
    ))
}
