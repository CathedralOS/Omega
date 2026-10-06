//! Bound generic cleanup inputs are ordinary call obligations. Their origin is
//! the retained generic death row, not an authored `requires` clause; only the
//! actual's evaluated operand facts can discharge the instantiated predicate.
use checked_trees::{
    CheckFacts, CheckedBooleanExpression, ContextualCleanupEntryRequirement, FlowCallFact,
    FlowStateFact,
};
use diagnostics::Diagnostic;
use facts::FactContextHandle;
use typed_trees::TypedTrees;

pub(crate) use super::call_values::target_machine;
use crate::flow::CanonicalPlace;

pub(crate) fn actual_place(
    program: &TypedTrees,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &ContextualCleanupEntryRequirement,
) -> Option<CanonicalPlace> {
    if target_machine(program, call)? != requirement.machine {
        return None;
    }
    let predicate = crate::facts::boolean_predicates::bind_entry(
        program,
        requirement.machine,
        requirement.consequence.clone(),
    )?;
    let observation = predicate.bindings.first()?;
    let actual = super::call_values::observation_actual(program, state, call, observation)?;
    crate::semantic::facts::projected_argument_place(
        program,
        state.state_symbol,
        call.statement_index,
        actual.argument?,
        actual.parameter.type_reference,
        &actual.path,
    )
}

pub(crate) fn entry_consequence(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &ContextualCleanupEntryRequirement,
) -> Option<CheckedBooleanExpression> {
    super::call_values::entry_predicate(
        program,
        facts,
        state,
        call,
        requirement.machine,
        requirement.consequence.clone(),
    )
}

/// Preserve the selector as a formula over authenticated caller entry values.
pub(crate) fn entry_antecedent(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &ContextualCleanupEntryRequirement,
) -> Option<CheckedBooleanExpression> {
    super::call_values::entry_predicate(
        program,
        facts,
        state,
        call,
        requirement.machine,
        requirement.antecedent.clone(),
    )
}

pub(crate) fn proves(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[FactContextHandle],
    requirement: &ContextualCleanupEntryRequirement,
) -> bool {
    super::call_values::proves_entry_predicate(
        program,
        facts,
        state,
        call,
        contexts,
        requirement.machine,
        requirement.predicate(),
    )
}

pub(super) fn check(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[FactContextHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(target) = target_machine(program, call) else {
        return;
    };
    let requirements = &facts.contextual_cleanup.entry_requirements;
    for (_, requirement) in requirements
        .rows
        .iter()
        .filter(|(_, row)| row.machine == target)
    {
        if proves(program, facts, state, call, contexts, requirement) {
            continue;
        }
        let subject = actual_place(program, state, call, requirement)
            .map(|place| place.label(program))
            .unwrap_or_else(|| "selected owned value".to_owned());
        let proposition = requirement
            .direct_boolean_subject()
            .map(|(_, _, expected)| format!("{subject} == {expected} required by"))
            .unwrap_or_else(|| format!("{subject} does not establish"));
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove contextual cleanup prerequisite for call {} from {}: {} {} clause `{}`",
            crate::labels::call_target_label(program, call.target_symbol),
            crate::labels::machine_name(program, state.machine_symbol),
            proposition,
            crate::labels::machine_name(program, requirement.hook_machine),
            program
                .expression_table
                .display_name(requirement.hook_clause),
        )));
    }
}
