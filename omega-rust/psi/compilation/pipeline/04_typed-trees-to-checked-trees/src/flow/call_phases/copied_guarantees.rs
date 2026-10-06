//! A postcondition about a completed owned argument describes its saved value.
//! It can describe the caller's source again only while that source still has
//! the captured contents. Borrowed formals instead describe live referents, so
//! their own postconditions remain publishable after writes through the call.

use super::super::{CanonicalPlace, FlowBuildContext};
use arena::{Handle, HandleSpan};
use checked_trees::{BorrowCallFact, ContractCallFact, FlowSemanticContextRef, ProofFacts};
use facts::{FactPayload, FactPlan, PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, domain::ProofFact, expression::ExpressionHandle};

pub(in crate::flow) struct ArgumentSources<'program, 'writes> {
    program: &'program TypedTrees,
    state: SymbolHandle,
    statement: usize,
    parameters: &'program [typed_trees::signature::StateParameter],
    arguments: &'program [ExpressionHandle],
    writes: Option<&'writes [CanonicalPlace]>,
    changed_operands: &'writes [CanonicalPlace],
}

impl<'program, 'writes> ArgumentSources<'program, 'writes> {
    pub(in crate::flow) fn new(
        program: &'program TypedTrees,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &BorrowCallFact,
        writes: Option<&'writes [CanonicalPlace]>,
        changed_operands: &'writes [CanonicalPlace],
    ) -> Option<Self> {
        let site = crate::semantic::calls::find_call_site(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        )?;
        Some(Self {
            program,
            state,
            statement: call.statement_index,
            parameters: crate::semantic::calls::call_target_parameters(
                program,
                call.target_symbol,
            )?,
            arguments: crate::semantic::calls::call_site_argument_expressions(program, &site),
            writes,
            changed_operands,
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
        if !super::super::operand_binds_stable_copy(self.program, parameter.type_reference) {
            return true;
        }
        let Some(argument) = self.arguments.get(position) else {
            return false;
        };
        let Some(projections) = super::super::literal_value_projections(
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
                let Some(mut source) = super::super::canonical_place_from_expression_in_state(
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
                if !self.source_survives(&source) {
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
                    let Some(dependency) = super::super::canonical_place_from_expression_in_state(
                        self.program,
                        self.state,
                        self.statement,
                        occurrence,
                    ) else {
                        return false;
                    };
                    // The selected value was checked at its exact projected path;
                    // only independent selector observations are additional debt.
                    dependency == base_source || self.source_survives(&dependency)
                })
            })
    }

    fn source_survives(&self, source: &CanonicalPlace) -> bool {
        self.writes.is_some_and(|writes| {
            writes.iter().chain(self.changed_operands).all(|write| {
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
    program: &TypedTrees,
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
                        super::super::canonical_place_from_expression(program, occurrence)
                    else {
                        return true;
                    };
                    let PlaceRoot::Symbol(symbol) = place.root else {
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
