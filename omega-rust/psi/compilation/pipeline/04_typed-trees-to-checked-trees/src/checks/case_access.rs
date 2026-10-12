//! Payload access requires the live tag of its exact source place.
//! Flow records the access after receiver effects and before subsequent
//! operands. Readable access and a declared payload type supply no tag proof;
//! consume the ordinary live-case judgment, independently of scalar format,
//! qualification, or whether the accessed payload is copied or borrowed.

use checked_trees::CheckFacts;
use diagnostics::Diagnostic;

pub(super) fn check(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, state) in facts.flow.control.states.iter() {
        for access in facts
            .flow
            .control
            .case_accesses
            .span_or_empty(state.case_accesses)
        {
            let subject = validation::canonical_place_from_expression_in_state(
                program,
                state.state_symbol,
                access.statement_index,
                access.expression,
            );
            let contexts = facts
                .flow
                .semantic_constraint_contexts(access.constraints)
                .collect::<Vec<_>>();
            if subject.as_ref().is_none_or(|subject| {
                !crate::flow::place_cases_are_selected(
                    program,
                    &facts.semantic,
                    &contexts,
                    state.machine_symbol,
                    state.state_symbol,
                    access.statement_index,
                    subject,
                )
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove selected case for payload access `{}` in `{}` at statement {}",
                    program.expression_table.display_name(access.expression),
                    program.symbols.display_path(state.state_symbol, "::"),
                    access.statement_index,
                )));
            }
        }
    }
}
