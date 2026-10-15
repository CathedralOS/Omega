//! A postcondition about a completed owned argument describes its saved value.
//! It can describe the caller's source again only while that source still has
//! the captured contents. Borrowed formals instead describe live referents, so
//! their own postconditions remain publishable after writes through the call.

use validation::CanonicalPlace;

use super::super::FlowBuildContext;
use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowCallFact, ContractCallFact, FlowControlFacts, FlowOperandFact, FlowSemanticContextRef,
    ProofFacts,
};
use facts::{FactPayload, FactPlan, PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::{domain::ProofFact, expression::ExpressionHandle};

pub(in crate::flow) struct ArgumentSources<'program, 'writes> {
    program: &'program dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement: usize,
    parameters: &'program [typed_trees::signature::StateParameter],
    arguments: &'program [ExpressionHandle],
    writes: Option<&'writes [CanonicalPlace]>,
    captures: Vec<CapturedSource>,
}

/// Scratch custody copied before publication mutates the flow arenas. Paths
/// identify destination children; later writes retain each child's own epoch.
struct CapturedSource {
    position: usize,
    destination: Vec<PlaceSegment>,
    expression: ExpressionHandle,
    type_reference: typed_trees::types::TypeReferenceHandle,
    later_writes: Option<Vec<CanonicalPlace>>,
    referents: Vec<CanonicalPlace>,
}

fn writes_after_capture(
    control: &FlowControlFacts,
    captured: &FlowOperandFact,
    invocation: &FlowOperandFact,
) -> Option<Vec<CanonicalPlace>> {
    if captured.entry_origin_unknown_write || invocation.entry_origin_unknown_write {
        return None;
    }
    let prefix = control
        .operand_origin_writes
        .span(captured.entry_origin_writes)?;
    let complete = control
        .operand_origin_writes
        .span(invocation.entry_origin_writes)?;
    if prefix.len() > complete.len()
        || !prefix.iter().zip(complete).all(|(left, right)| {
            left.root == right.root
                && control
                    .operand_referent_segments
                    .span(left.segments)
                    .zip(control.operand_referent_segments.span(right.segments))
                    .is_some_and(|(left, right)| left == right)
        })
    {
        return None;
    }
    complete[prefix.len()..]
        .iter()
        .map(|write| {
            Some(CanonicalPlace {
                root: write.root,
                segments: control
                    .operand_referent_segments
                    .span(write.segments)?
                    .to_vec(),
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn retain_capture(
    program: &dyn ::validation::CheckingInput,
    control: &FlowControlFacts,
    capture: &FlowOperandFact,
    invocation: &FlowOperandFact,
    position: usize,
    destination: Vec<PlaceSegment>,
    type_reference: typed_trees::types::TypeReferenceHandle,
    active: &mut Vec<Handle<FlowOperandFact>>,
    captures: &mut Vec<CapturedSource>,
) -> Option<()> {
    if !type_reference.is_valid() {
        return None;
    }
    let referents = if super::super::operand_binds_live_view(program, type_reference) {
        control
            .operand_referents
            .span(capture.referents)?
            .iter()
            .map(|referent| {
                Some(CanonicalPlace {
                    root: referent.root,
                    segments: control
                        .operand_referent_segments
                        .span(referent.segments)?
                        .to_vec(),
                })
            })
            .collect::<Option<Vec<_>>>()?
    } else {
        Vec::new()
    };
    captures.push(CapturedSource {
        position,
        destination: destination.clone(),
        expression: capture.expression,
        type_reference,
        later_writes: writes_after_capture(control, capture, invocation),
        referents,
    });
    for child in control.operand_children.span(capture.children)? {
        if !control.operands.is_valid(child.value) || active.contains(&child.value) {
            return None;
        }
        let suffix = control.operand_child_paths.span(child.path)?;
        if suffix.is_empty() {
            return None;
        }
        let mut path = destination.clone();
        path.extend_from_slice(suffix);
        active.push(child.value);
        let child_type = super::super::operand_values::operand_child_type(
            program,
            type_reference,
            suffix,
            child.type_reference,
        )?;
        retain_capture(
            program,
            control,
            control.operands.get(child.value),
            invocation,
            position,
            path,
            child_type,
            active,
            captures,
        )?;
        active.pop();
    }
    Some(())
}

impl<'program, 'writes> ArgumentSources<'program, 'writes> {
    /// The completed reference's actual backing, not the source spelling or
    /// a replay of its initializer. An empty roster is unavailable evidence;
    /// several candidates do not identify which one the callee changed.
    pub(in crate::flow) fn referent_places(&self, position: usize) -> Option<&[CanonicalPlace]> {
        let capture = self
            .captures
            .iter()
            .find(|capture| capture.position == position && capture.destination.is_empty())?;
        super::super::operand_binds_live_view(self.program, capture.type_reference)
            .then_some(capture.referents.as_slice())
    }

    pub(in crate::flow) fn new(
        program: &'program dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &BorrowCallFact,
        writes: Option<&'writes [CanonicalPlace]>,
        control: &FlowControlFacts,
        captured_operands: &[FlowOperandFact],
    ) -> Option<Self> {
        let site = validation::find_call_site(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        )?;
        let parameters = validation::call_target_parameters(program, call.target_symbol)?;
        let arguments = validation::call_site_argument_expressions(program, &site);
        let receiver = match &site {
            validation::CallSite::Expression { call, .. } if call.receiver.is_valid() => {
                Some(call.receiver)
            }
            _ => None,
        };
        if parameters.len() != arguments.len()
            || !captured_operands
                .iter()
                .map(|capture| capture.expression)
                .eq(receiver.into_iter().chain(arguments.iter().copied()))
        {
            return None;
        }
        let mut captures = Vec::new();
        if let Some(invocation) = captured_operands.last() {
            let offset = usize::from(receiver.is_some());
            for (position, parameter) in parameters.iter().enumerate() {
                retain_capture(
                    program,
                    control,
                    captured_operands.get(position + offset)?,
                    invocation,
                    position,
                    Vec::new(),
                    parameter.type_reference,
                    &mut Vec::new(),
                    &mut captures,
                )?;
            }
        }
        Some(Self {
            program,
            state,
            statement: call.statement_index,
            parameters,
            arguments,
            writes,
            captures,
        })
    }

    pub(in crate::flow) fn observation_survives(
        &self,
        position: usize,
        path: &[PlaceSegment],
    ) -> bool {
        let Some(parameter) = self.parameters.get(position) else {
            return false;
        };
        let Some(argument) = self.arguments.get(position) else {
            return false;
        };
        let Some(projections) = super::super::literal_expression_projections(
            self.program,
            *argument,
            parameter.type_reference,
            path,
            false,
        ) else {
            return false;
        };
        !projections.is_empty()
            && projections.iter().all(|projection| {
                let mut candidates = self.captures.iter().filter(|capture| {
                    capture.position == position
                        && capture.destination.len() == projection.destination.len()
                        && capture.destination.iter().zip(&projection.destination).all(
                            |(left, right)| {
                                super::super::canonical_place_segments_equal(
                                    self.program,
                                    *left,
                                    *right,
                                )
                            },
                        )
                });
                let Some(capture) = candidates.next() else {
                    return false;
                };
                if candidates.next().is_some() || capture.expression != projection.expression {
                    return false;
                }
                if super::super::operand_binds_live_view(self.program, capture.type_reference) {
                    return true;
                }
                let Some(mut source) = validation::canonical_place_from_expression_in_state(
                    self.program,
                    self.state,
                    self.statement,
                    projection.expression,
                ) else {
                    return false;
                };
                let base_source = source.clone();
                source.extend_segments(&projection.remaining);
                // A computed result/literal has occurrence identity, not the
                // current storage of expressions evaluated to produce it.
                if matches!(source.root, PlaceRoot::Expression(_)) {
                    return true;
                }
                if !self.source_survives(capture, &source) {
                    return false;
                }
                // A changed runtime selector also prevents substitution onto the
                // current indexed place, even when its old element was untouched.
                let mut occurrences = Vec::new();
                crate::facts::contract_occurrences::append_expression_occurrences(
                    self.program,
                    projection.expression,
                    &mut occurrences,
                );
                occurrences.into_iter().all(|occurrence| {
                    let Some(dependency) = validation::canonical_place_from_expression_in_state(
                        self.program,
                        self.state,
                        self.statement,
                        occurrence,
                    ) else {
                        return false;
                    };
                    // The selected value was checked at its exact projected path;
                    // only independent selector observations are additional debt.
                    dependency == base_source || self.source_survives(capture, &dependency)
                })
            })
    }

    fn source_survives(&self, capture: &CapturedSource, source: &CanonicalPlace) -> bool {
        self.writes
            .zip(capture.later_writes.as_deref())
            .is_some_and(|(writes, later)| {
                writes.iter().chain(later).all(|write| {
                    source.root != write.root
                        || !super::super::canonical_place_segments_may_overlap(
                            self.program,
                            &source.segments,
                            &write.segments,
                        )
                })
            })
    }
}

pub(super) fn retain_guarantees(
    program: &dyn ::validation::CheckingInput,
    proof: &ProofFacts,
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    call: Option<&ContractCallFact>,
    sources: Option<&ArgumentSources<'_, '_>>,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> HandleSpan<FlowSemanticContextRef> {
    let Some(call) = call else {
        // Without the exact source contract occurrence, no imported ensures
        // context can establish which formal value its promise observes.
        return HandleSpan::empty();
    };
    let mut stale: Vec<Handle<ProofFact>> = Vec::new();
    for reference in proof.contract_fact_refs.span_or_empty(call.ensures) {
        let contract = proof.contract_facts.get(reference.fact);
        let survives =
            crate::facts::contract_occurrences::fact_referenced_occurrences(program, contract.fact)
                .into_iter()
                .all(|occurrence| {
                    let Some(place) =
                        validation::canonical_place_from_expression(program, occurrence)
                    else {
                        return true;
                    };
                    let Some(symbol) = place.root.as_symbol() else {
                        return true;
                    };
                    match crate::semantic::facts::contract_parameter_position(
                        program,
                        proof,
                        contract,
                        call.target_state_symbol,
                        symbol,
                    ) {
                        Ok(Some(position)) => sources.is_some_and(|sources| {
                            sources.observation_survives(position, &place.segments)
                        }),
                        Ok(None) => true,
                        Err(()) => false,
                    }
                });
        if !survives && !stale.contains(&contract.fact) {
            stale.push(contract.fact);
        }
    }
    if stale.is_empty() {
        return contexts;
    }
    let references = build
        .contexts
        .semantic_context_refs
        .span_or_empty(contexts)
        .to_vec();
    let mut retained = HandleSpan::empty();
    for reference in references {
        let context = semantic.contexts.get(reference.context);
        let invalid = semantic.context_view(context).facts().any(|fact| {
            let source = match fact.payload {
                FactPayload::Contract { fact, .. }
                | FactPayload::ContractBooleanExpression { fact, .. }
                | FactPayload::ContractDomainMembership { fact, .. }
                | FactPayload::ContractPropositionApplication { fact, .. }
                | FactPayload::ContractCarryPermission { fact, .. } => fact,
                _ => return false,
            };
            stale.contains(&source)
        });
        if !invalid {
            super::super::reference_spans::append_flow_reference(
                &mut build.contexts.semantic_context_refs,
                &mut retained,
                reference,
            );
        }
    }
    retained
}
