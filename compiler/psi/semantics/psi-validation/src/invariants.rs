use psi_diagnostics::Diagnostic;
use psi_facts::{FactPlan, ProgramPoint};
use psi_typed_trees::TypedTrees;

pub(crate) fn validate_invariant_definitions(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for invariant in program.invariant_definitions() {
        let constraint_fact_count = fact_plan
            .contexts_at_point(ProgramPoint::Definition {
                symbol: invariant.symbol,
            })
            .flat_map(|context| context.type_constraints())
            .count();

        if constraint_fact_count != invariant.constraints.len() {
            diagnostics.push(Diagnostic::error(format!(
                "invariant `{}` references invalid constraint storage",
                invariant.name
            )));
            continue;
        }
    }
}
