//! Finite live memberships carried by exact named-state argument values.
//!
//! Origins alone supply no row. Every row is captured from an available fact;
//! incoming joins can only remove claims, and root entry is never narrowed.

use super::*;
use typed_trees::signature::StateParameter;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::flow) struct QualifiedInput {
    parameter: SymbolHandle,
    segments: Vec<facts::PlaceSegment>,
    domain: HandleSpan<Identifier>,
    domain_symbol: SymbolHandle,
    evidence: QualificationEvidence,
}

impl QualifiedInput {
    fn same_claim(&self, other: &Self) -> bool {
        self.parameter == other.parameter
            && self.segments == other.segments
            && self.domain_symbol == other.domain_symbol
            && self.evidence == other.evidence
    }
}

pub(in crate::flow) struct CapturedQualification {
    input: QualifiedInput,
    context: facts::FactContextHandle,
    isolated: bool,
}

pub(super) fn meet(previous: &mut Vec<QualifiedInput>, incoming: &[QualifiedInput]) -> bool {
    let before = previous.len();
    previous.retain(|claim| incoming.iter().any(|next| claim.same_claim(next)));
    previous.len() != before
}

fn unconstrained(
    program: &typed_trees::TypedTrees,
    mut reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    reference
}

/// A field chain may pass through owned records, but never another reference.
/// The final qualified value must itself be isolated from referenced storage.
fn owned_projection(
    program: &typed_trees::TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    mut reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> bool {
    reference = unconstrained(program, reference);
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *referee;
    }
    for segment in segments {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return false;
        };
        let TypeReferenceNode::Named { symbol: owner, .. } = program
            .type_reference_table
            .type_reference(unconstrained(program, reference))
        else {
            return false;
        };
        let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *owner)
        else {
            return false;
        };
        if !symbol.is_valid() || program.symbols.get(*symbol).parent != *owner {
            return false;
        }
        let Some(field) = program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == *symbol => {
                    Some(field)
                }
                _ => None,
            })
        else {
            return false;
        };
        reference = field.type_reference;
    }
    frames.proof_value_is_caller_isolated(reference)
}

fn capture_parameter(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    flow_context: &FlowBuildContext,
    source: &StateParameter,
    destination: SymbolHandle,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> Vec<CapturedQualification> {
    let Some(frames) = flow_context.call_frames else {
        return Vec::new();
    };
    let isolated = frames.proof_value_is_caller_isolated(source.type_reference);
    let mut captured = Vec::new();
    for context in flow_context
        .contexts
        .semantic_context_refs
        .span_or_empty(contexts)
    {
        for fact in semantic
            .context_view(semantic.contexts.get(context.context))
            .facts()
        {
            if matches!(fact.origin, FactOrigin::CallRequires) {
                continue;
            }
            let (domain, domain_symbol) = match fact.payload {
                FactPayload::DomainMembership {
                    domain,
                    domain_symbol,
                    ..
                }
                | FactPayload::ContractDomainMembership {
                    domain,
                    domain_symbol,
                    ..
                } => (domain, domain_symbol),
                _ => continue,
            };
            let FactPlace::Place(place) = fact.place else {
                continue;
            };
            let place = semantic.places.get(place);
            let segments = semantic.place_segments.span_or_empty(place.segments);
            if !domain_symbol.is_valid()
                || place.root != facts::PlaceRoot::Symbol(source.symbol)
                || !owned_projection(program, frames, source.type_reference, segments)
            {
                continue;
            }
            captured.push(CapturedQualification {
                input: QualifiedInput {
                    parameter: destination,
                    segments: segments.to_vec(),
                    domain,
                    domain_symbol,
                    evidence: fact.evidence,
                },
                context: context.context,
                isolated,
            });
        }
    }
    captured
}

#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn capture_argument(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    flow_context: &FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    target: typed_trees::statement::TransitionTargetHandle,
    ordinal: usize,
    argument: ExpressionHandle,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> Vec<CapturedQualification> {
    let ExpressionNode::Name(name) = program.expression_table.expression(argument) else {
        return Vec::new();
    };
    if name.symbol != name.head_symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return Vec::new();
    }
    let Some(source) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == name.symbol && !parameter.is_self)
    else {
        return Vec::new();
    };
    let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(target)
    else {
        return Vec::new();
    };
    let Some(destination) = program
        .machine_states(machine)
        .iter()
        .find(|candidate| candidate.symbol == path.symbol)
    else {
        return Vec::new();
    };
    let Some(parameter) = program
        .state_parameters(destination)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .nth(ordinal)
    else {
        return Vec::new();
    };
    capture_parameter(
        program,
        semantic,
        flow_context,
        source,
        parameter.symbol,
        contexts,
    )
}

pub(in crate::flow) fn finish(
    flow_context: &FlowBuildContext,
    contexts: HandleSpan<FlowSemanticContextRef>,
    captured: Vec<CapturedQualification>,
    bindings_stable: bool,
) -> Vec<QualifiedInput> {
    let live = flow_context
        .contexts
        .semantic_context_refs
        .span_or_empty(contexts);
    let mut inputs: Vec<QualifiedInput> = Vec::new();
    for captured in captured {
        if (captured.isolated
            || (bindings_stable
                && live
                    .iter()
                    .any(|context| context.context == captured.context)))
            && !inputs.iter().any(|input| input.same_claim(&captured.input))
        {
            inputs.push(captured.input);
        }
    }
    inputs
}

pub(super) fn capture_self(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    flow_context: &FlowBuildContext,
    state: &typed_trees::state::State,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> Vec<QualifiedInput> {
    let captured = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .flat_map(|parameter| {
            capture_parameter(
                program,
                semantic,
                flow_context,
                parameter,
                parameter.symbol,
                contexts,
            )
        })
        .collect();
    finish(flow_context, contexts, captured, true)
}

pub(super) fn append(semantic: &mut FactPlan, inputs: &[QualifiedInput], point: ProgramPoint) {
    for input in inputs {
        let place = semantic.append_symbol_place(input.parameter);
        for segment in &input.segments {
            semantic.push_place_segment(place, *segment);
        }
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: input.evidence,
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain: input.domain,
                domain_symbol: input.domain_symbol,
            },
        });
        let mut references = HandleSpan::empty();
        semantic.append_ref(&mut references, fact);
        semantic.append_context(point, references);
    }
}
