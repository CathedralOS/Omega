//! Bound generic cleanup inputs are ordinary call obligations. Their origin is
//! the retained generic death row, not an authored `requires` clause; only the
//! actual's evaluated operand facts can discharge the instantiated predicate.
use arena::Arena;
use checked_trees::{
    CheckFacts, CheckedBooleanExpression, CheckedStructuralPredicatePathSegment,
    ContextualCleanupEntryRequirement, FlowCallFact, FlowStateFact,
};
use diagnostics::Diagnostic;
use facts::FactContextHandle;
use typed_trees::TypedTrees;

pub(crate) use super::call_values::target_machine;
use super::call_values::{Actual, actual_at, captured_actual};
use crate::flow::CanonicalPlace;

fn actual<'program>(
    program: &'program TypedTrees,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &ContextualCleanupEntryRequirement,
    paths: &Arena<CheckedStructuralPredicatePathSegment>,
) -> Option<Actual<'program>> {
    if target_machine(program, call)? != requirement.machine {
        return None;
    }
    let parameters = crate::semantic::calls::call_target_parameters(program, call.target_symbol)?;
    let (_, path, endpoint, _) = crate::values::resolve_structural_parameter_path(
        program,
        parameters,
        requirement.parameter_position,
        paths.span(requirement.path)?,
    )?;
    if program.primitive_type_reference(endpoint) != Some(typed_trees::types::PrimitiveType::Bool) {
        return None;
    }
    actual_at(
        program,
        state,
        call,
        requirement.parameter_position as usize,
        path,
    )
}

pub(crate) fn actual_place(
    program: &TypedTrees,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &ContextualCleanupEntryRequirement,
    paths: &Arena<CheckedStructuralPredicatePathSegment>,
) -> Option<CanonicalPlace> {
    let actual = actual(program, state, call, requirement, paths)?;
    crate::semantic::facts::projected_argument_place(
        program,
        state.state_symbol,
        call.statement_index,
        actual.argument?,
        actual.parameter.type_reference,
        &actual.path,
    )
}

pub(crate) fn entry_subject(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &ContextualCleanupEntryRequirement,
) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)> {
    let actual = actual(
        program,
        state,
        call,
        requirement,
        &facts.contextual_cleanup.entry_requirements.paths,
    )?;
    let operand = captured_actual(program, facts, state, call, &actual)?;
    let place = crate::semantic::facts::projected_argument_place(
        program,
        state.state_symbol,
        call.statement_index,
        actual.argument?,
        actual.parameter.type_reference,
        &actual.path,
    )?;
    crate::flow::operand_entry_origin_is_stable(program, &facts.flow, operand, &place)
        .then_some(())?;
    crate::facts::entry_parameter_place_before_statement(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        &place,
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
        requirement.predicate(&facts.contextual_cleanup.entry_requirements.paths),
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
        let subject = actual_place(program, state, call, requirement, &requirements.paths)
            .map(|place| place.label(program))
            .unwrap_or_else(|| format!("parameter {}", requirement.parameter_position));
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove contextual cleanup prerequisite for call {} from {}: {} == {} required by {} clause `{}`",
            crate::labels::call_target_label(program, call.target_symbol),
            crate::labels::machine_name(program, state.machine_symbol),
            subject, requirement.expected,
            crate::labels::machine_name(program, requirement.hook_machine),
            program.expression_table.display_name(requirement.hook_clause),
        )));
    }
}
