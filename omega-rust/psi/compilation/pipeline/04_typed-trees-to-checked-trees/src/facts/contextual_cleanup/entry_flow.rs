//! Seed bound cleanup predicates into ordinary entry flow as relational facts.
//! Rebuilding starts from declarations; observations from an earlier flow sweep
//! never become assumptions, and implications never seed their consequences.
use checked_trees::ContextualCleanupEntryRequirements;
use diagnostics::Diagnostic;
use facts::{FactOrigin, FactPlan, ProgramPoint};
use typed_trees::TypedTrees;

pub(crate) fn seed_entry_requirements(
    program: &TypedTrees,
    semantic: &mut FactPlan,
    requirements: &ContextualCleanupEntryRequirements,
) -> Result<(), Vec<Diagnostic>> {
    for (_, requirement) in requirements.rows.iter() {
        let Some(predicate) = super::super::boolean_predicates::bind_entry(
            program,
            requirement.machine,
            requirement.predicate(),
        ) else {
            return Err(vec![Diagnostic::error(
                "bound cleanup prerequisite has no exact invocation-entry predicate bindings",
            )]);
        };
        super::super::boolean_predicates::seed(
            semantic,
            &predicate,
            ProgramPoint::State {
                machine_symbol: predicate.machine,
                state_symbol: predicate.entry_state,
            },
            FactOrigin::ContextualCleanup {
                machine_symbol: predicate.machine,
                state_symbol: predicate.entry_state,
            },
        );
    }
    Ok(())
}
