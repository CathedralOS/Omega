use omega_core::arena::HandleSpan;

use crate::{FlowConstraintRef, FlowFacts, FlowSemanticContextRef};

pub(super) fn semantic_contexts(
    flow: &FlowFacts,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> &[FlowSemanticContextRef] {
    flow.contexts.semantic_context_refs.span_or_empty(contexts)
}

pub(super) fn constraints(
    flow: &FlowFacts,
    constraints: HandleSpan<FlowConstraintRef>,
) -> &[FlowConstraintRef] {
    flow.contexts.constraint_refs.span_or_empty(constraints)
}
