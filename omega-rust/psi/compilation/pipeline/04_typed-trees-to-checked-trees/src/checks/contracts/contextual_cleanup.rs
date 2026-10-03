//! Bound generic cleanup inputs are ordinary call obligations. Their origin is
//! the retained generic death row, not an authored `requires` clause; only the
//! actual's current call-entry facts can discharge the instantiated predicate.
use arena::Arena;
use checked_trees::{
    CheckFacts, CheckedStructuralPredicatePathSegment, ContextualCleanupEntryRequirement,
    FlowCallFact, FlowStateFact,
};
use diagnostics::Diagnostic;
use facts::FactContextHandle;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, expression::ExpressionHandle, signature::StateParameter};

use crate::flow::CanonicalPlace;

pub(crate) fn target_machine(program: &TypedTrees, call: &FlowCallFact) -> Option<SymbolHandle> {
    crate::lookup::machine_by_symbol(program, call.target_symbol)
        .map(|machine| machine.symbol)
        .or_else(|| {
            crate::semantic::calls::find_machine_by_entry_state(program, call.target_symbol)
                .map(|(machine, _)| machine.symbol)
        })
}

struct Actual<'program> {
    parameter: &'program StateParameter,
    argument: Option<ExpressionHandle>,
    place: CanonicalPlace,
    path: Vec<facts::PlaceSegment>,
}

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
    let site = crate::semantic::calls::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let parameters = crate::semantic::calls::call_target_parameters(program, call.target_symbol)?;
    let position = usize::try_from(requirement.parameter_position).ok()?;
    let parameter = parameters.get(position)?;
    let (_, path, endpoint, _) = crate::values::resolve_structural_parameter_path(
        program,
        parameters,
        requirement.parameter_position,
        paths.span(requirement.path)?,
    )?;
    if program.primitive_type_reference(endpoint) != Some(typed_trees::types::PrimitiveType::Bool) {
        return None;
    }
    let argument = if parameter.is_self {
        None
    } else {
        let ordinal = parameters[..position]
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
        Some(*crate::semantic::calls::call_site_argument_expressions(program, &site).get(ordinal)?)
    };
    let place = if let Some(argument) = argument {
        crate::flow::canonical_place_from_expression_in_state(
            program,
            state.state_symbol,
            call.statement_index,
            argument,
        )?
    } else {
        crate::flow::canonical_receiver_place_for_call_site(
            program,
            state.machine_symbol,
            state.state_symbol,
            &site,
            call.statement_index,
        )?
    };
    Some(Actual {
        parameter,
        argument,
        place,
        path,
    })
}

pub(crate) fn actual_place(
    program: &TypedTrees,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &ContextualCleanupEntryRequirement,
    paths: &Arena<CheckedStructuralPredicatePathSegment>,
) -> Option<CanonicalPlace> {
    let mut actual = actual(program, state, call, requirement, paths)?;
    actual.place.segments.extend(actual.path);
    Some(actual.place)
}

pub(crate) fn proves(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[FactContextHandle],
    requirement: &ContextualCleanupEntryRequirement,
) -> bool {
    let Some(actual) = actual(
        program,
        state,
        call,
        requirement,
        &facts.contextual_cleanup.entry_requirements.paths,
    ) else {
        return false;
    };
    let mut subject = actual.place;
    subject.segments.extend_from_slice(&actual.path);
    if super::prover::boolean_value_at_place(program, &facts.semantic, contexts, &subject)
        == Some(requirement.expected)
    {
        return true;
    }
    let Some(argument) = actual.argument else {
        return false;
    };
    let Some(projections) = crate::flow::literal_value_projections(
        program,
        argument,
        actual.parameter.type_reference,
        &actual.path,
        false,
    ) else {
        return false;
    };
    // Only closed constructor leaves may be evaluated here. Stored names keep
    // their live place above; replaying a local initializer or rereading an
    // earlier copied operand after a later operand writes would lose custody.
    !projections.is_empty()
        && projections.iter().all(|projection| {
            projection.remaining.is_empty()
                && super::prover::closed_boolean_value(
                    program,
                    &facts.operators,
                    projection.expression,
                ) == Some(requirement.expected)
        })
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
