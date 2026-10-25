//! Congruence of content projections across a nonmutating activation.

use checked_trees::{CheckFacts, FlowStateFact};
use language_semantics::content::{
    ContentConservationOwnerKind, ContentConservationPlan, ContentConservationTerm,
    ContentPlaceRoot, ContentPlaceVersion,
};

mod inherited;

pub(super) fn proves_exit(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    exit: &checked_trees::FlowExitFact,
    fact: &facts::Fact,
    content_plans: &[validation::ContentConservationSourcePlan],
    available: &[ContentConservationPlan],
) -> bool {
    if inherited::proves_exit(program, facts, exit, fact, content_plans, available) {
        return true;
    }
    let facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return false;
    };
    let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
        return false;
    };
    content_plans
        .iter()
        .filter(|source| {
            source.source_expression == expression && source.plan.owner == machine.symbol
        })
        .any(|source| unchanged_projection(program, facts, machine, &source.plan))
}

fn unchanged_projection(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    plan: &ContentConservationPlan,
) -> bool {
    let Some(mutation) = facts.mutation.for_machine(machine.symbol) else {
        return false;
    };
    // A missing or opaque frame is not preservation evidence. Check every
    // state in the activation, including writes reached through other calls.
    if !machine.body_is_present
        || program.machine_states(machine).is_empty()
        || program.machine_states(machine).iter().any(|state| {
            !mutation.state_write_frames.iter().any(|frame| {
                frame.state == state.symbol
                    && frame.frame.is_complete()
                    && frame.frame.paths().is_empty()
            })
        })
    {
        return false;
    }
    let ContentConservationTerm::Projection {
        domain: left_domain,
        semantic_domain: left_semantic_domain,
        projection_machine: left_projection,
        subject: left,
        ..
    } = plan.equation.left()
    else {
        return false;
    };
    let ContentConservationTerm::Projection {
        domain: right_domain,
        semantic_domain: right_semantic_domain,
        projection_machine: right_projection,
        subject: right,
        ..
    } = plan.equation.right()
    else {
        return false;
    };
    // Normalization supplies exact projection and structural-place
    // identities; report fingerprints never establish equality.
    left_domain.is_valid()
        && left_projection.is_valid()
        && left_domain == right_domain
        && left_semantic_domain == right_semantic_domain
        && left_projection == right_projection
        && matches!((&left.root, &right.root),
            (ContentPlaceRoot::Parameter { symbol: left, .. },
             ContentPlaceRoot::Parameter { symbol: right, .. })
             if left.is_valid() && left == right)
        && left.segments == right.segments
        && matches!(
            (left.version, right.version),
            (ContentPlaceVersion::Entry, ContentPlaceVersion::Current)
                | (ContentPlaceVersion::Current, ContentPlaceVersion::Entry)
        )
}

/// Publish read-only congruence only when the actual complete exit roster and
/// each exact incoming-origin obligation check. An exitless callable supplies
/// no independently observed theorem, while its ordinary partial contract is
/// left intact.
pub(super) fn proves_readonly_source(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    source: &validation::ContentConservationSourcePlan,
) -> bool {
    if source.plan.owner_kind != ContentConservationOwnerKind::Machine {
        return false;
    }
    let Some(machine) = program.machine_by_symbol(source.plan.owner) else {
        return false;
    };
    if program
        .machine_states(machine)
        .first()
        .map(|state| state.symbol)
        != Some(source.plan.callable)
        || !unchanged_projection(program, facts, machine, &source.plan)
    {
        return false;
    }
    let mut observed = false;
    for state in program.machine_states(machine) {
        let Some(flow) = facts.flow.control.state(machine.symbol, state.symbol) else {
            return false;
        };
        for exit in facts.flow.control.exits.span_or_empty(flow.exits) {
            observed = true;
            let mut matched = false;
            for context in facts
                .flow
                .semantic_constraint_contexts(exit.ensures_constraints)
            {
                for fact in facts
                    .semantic
                    .context_view(facts.semantic.contexts.get(context))
                    .facts()
                {
                    let facts::FactPayload::ContractBooleanExpression {
                        fact: contract,
                        expression,
                        ..
                    } = fact.payload
                    else {
                        continue;
                    };
                    if expression != source.source_expression {
                        continue;
                    }
                    matched = true;
                    if facts
                        .flow
                        .control
                        .exit_parameter_origins
                        .span_or_empty(exit.parameter_origins)
                        .iter()
                        .any(|origin| {
                            origin.contract == contract
                                && !origin.state_parameter.is_valid()
                                && !origin.retained_referent_type.is_valid()
                        })
                    {
                        return false;
                    }
                }
            }
            if !matched {
                return false;
            }
        }
    }
    observed
}
