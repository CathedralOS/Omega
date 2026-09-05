//! Congruence of content projections across a nonmutating activation.

use checked_trees::{CheckFacts, FlowStateFact};
use language_semantics::content::{ContentConservationTerm, ContentPlaceRoot, ContentPlaceVersion};
use typed_trees::TypedTrees;

pub(super) fn proves_exit(
    program: &TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    fact: &facts::Fact,
    content_plans: &[validation::ContentConservationSourcePlan],
) -> bool {
    let facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return false;
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state_flow.machine_symbol)
    else {
        return false;
    };
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
    content_plans
        .iter()
        .filter(|source| {
            source.source_expression == expression && source.plan.owner == machine.symbol
        })
        .any(|source| {
            let ContentConservationTerm::Projection {
                domain: left_domain,
                semantic_domain: left_semantic_domain,
                projection_machine: left_projection,
                subject: left,
                ..
            } = source.plan.equation.left()
            else {
                return false;
            };
            let ContentConservationTerm::Projection {
                domain: right_domain,
                semantic_domain: right_semantic_domain,
                projection_machine: right_projection,
                subject: right,
                ..
            } = source.plan.equation.right()
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
        })
}
