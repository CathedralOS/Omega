//! Content-bearing signature conservation gates.
//!
//! A borrow lends access; it never supplies an owned claim. Retained custody
//! may nevertheless remain lifetime-bound to one explicit shared input loan.
//! This consumer admits only that exact whole-parameter/whole-result shape and
//! otherwise requires owned custody. Compatibility keys on the retained
//! compiler-owned algebra identity, never carrier or operation names.
//!
//! This file owns the three gates. `partition_wrappers.rs` composes
//! returned partition invocations into wrappers, `content_paths.rs` maps
//! content segments, contracts and projections onto fact paths, and
//! `retained_custody.rs` decides lifetime-bound borrow custody of retained
//! content.

mod content_paths;
mod partition_wrappers;
mod retained_custody;

use crate::checks::content::content_paths::{
    applicable_projection_plans, content_path, projection_term, unique_entry_claim_identity,
};
use crate::checks::content::partition_wrappers::{
    AvailablePartitionSource, equation_contains_partition, instantiate_partition_wrapper,
    returned_partition_invocations,
};
use crate::checks::content::retained_custody::{check_boundary_partition_results, check_callable};
use checked_trees::{
    CheckFacts, ContentIdentityReshuffleFact, ContentPartitionCompositionFact,
    FlowClaimOutcomeSource,
};
use diagnostics::Diagnostic;
use language_semantics::content::{
    ContentConservationEquation, ContentConservationOwnerKind, ContentConservationPlan,
    ContentPlaceRoot, ContentPlaceVersion, ContentStructuralPlace, conservation_report_fingerprint,
    content_conservation_plan_bytes,
};
use typed_trees::TypedTrees;

/// Derive the content equality attached to every exact input-relative claim
/// outcome. These are deliberately individual rewrite rows: distinct linear
/// claims do not imply that their projected content is disjoint, so this pass
/// never manufactures a `separate(...)` term. The later frontier theorem may
/// compose rows only when it also has the required partition evidence.
pub(crate) fn infer_identity_preserving_reshuffles(program: &TypedTrees, facts: &mut CheckFacts) {
    let outcomes = facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .flat_map(|(_, map)| {
            facts
                .flow
                .ownership
                .claim_outcome_entries
                .span_or_empty(map.entries)
                .iter()
                .map(|entry| {
                    (
                        map.machine_symbol,
                        map.state_symbol,
                        entry.output_segments,
                        entry.source,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut reshuffles = Vec::new();

    for (machine_symbol, state_symbol, output_segments, source) in outcomes {
        let FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments: input_segments,
        } = source
        else {
            continue;
        };
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
        else {
            continue;
        };
        let Some(state) = crate::semantic_calls::find_state(program, state_symbol) else {
            continue;
        };
        let Some((parameter_position, parameter)) = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == parameter_symbol)
        else {
            continue;
        };
        let input_path = facts.flow.ownership.segments.span_or_empty(input_segments);
        let output_path = facts.flow.ownership.segments.span_or_empty(output_segments);
        let Some(input_claim) =
            super::multiplicity::linear_claim_frontier(program, parameter.type_reference)
                .into_iter()
                .find(|claim| claim.path == input_path)
        else {
            continue;
        };
        let Some(output_claim) =
            super::multiplicity::linear_claim_frontier(program, state.return_type)
                .into_iter()
                .find(|claim| claim.path == output_path)
        else {
            continue;
        };
        let Some(input_content_path) = content_path(program, input_path) else {
            continue;
        };
        let Some(output_content_path) = content_path(program, output_path) else {
            continue;
        };
        let input_subject = ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: ContentPlaceRoot::Parameter {
                position: u32::try_from(parameter_position)
                    .expect("state parameter position fits in u32"),
                symbol: parameter.symbol,
                name: parameter.name.as_str().to_owned(),
                is_self: parameter.is_self,
            },
            segments: input_content_path,
        };
        let output_subject = ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments: output_content_path,
        };
        let input_plans = applicable_projection_plans(
            program,
            facts,
            machine,
            state,
            input_claim.type_reference,
            &input_subject,
        );
        let output_plans = applicable_projection_plans(
            program,
            facts,
            machine,
            state,
            output_claim.type_reference,
            &output_subject,
        );
        let Some(claim_identity) =
            unique_entry_claim_identity(facts, state_symbol, parameter_symbol, input_path)
        else {
            continue;
        };

        for input_plan in input_plans {
            for output_plan in output_plans.iter().copied().filter(|output_plan| {
                output_plan.semantic_domain == input_plan.semantic_domain
                    && output_plan.report_fingerprint == input_plan.report_fingerprint
                    && output_plan.algebra == input_plan.algebra
            }) {
                let left = projection_term(input_plan, input_subject.clone());
                let right = projection_term(output_plan, output_subject.clone());
                let equation = ContentConservationEquation::new(left, right);
                let report_fingerprint =
                    conservation_report_fingerprint(&input_plan.algebra, &equation);
                reshuffles.push(ContentIdentityReshuffleFact {
                    machine_symbol,
                    state_symbol,
                    claim_identity,
                    input_parameter_symbol: parameter_symbol,
                    input_segments,
                    output_segments,
                    plan: ContentConservationPlan {
                        owner_kind: ContentConservationOwnerKind::Machine,
                        owner: machine_symbol,
                        callable: state_symbol,
                        algebra: input_plan.algebra.clone(),
                        equation,
                        report_fingerprint,
                    },
                });
            }
        }
    }

    reshuffles.sort_by_key(|fact| {
        (
            fact.machine_symbol.arena_index(),
            fact.state_symbol.arena_index(),
            content_conservation_plan_bytes(&fact.plan),
        )
    });
    reshuffles.dedup();
    facts.qualifications.content.identity_reshuffles = reshuffles;
}

/// Instantiate an already-authored partition theorem through an exact wrapper.
/// This pass can substitute caller-entry paths and either a directly returned
/// result or a result staged through exact local-chain and aggregate identity
/// rewrites;
/// it cannot construct a `separate(...)` node. Every entry projection must bind
/// to one caller parameter claim whose transfer-stable identity reaches the
/// exact returned call site.
///
/// Derived rows are made available to later rounds so wrapper chains close to
/// a fixed point. Every staged source-result projection must retain one exact
/// call-established claim identity into a unique callable-result path. When
/// several staged calls independently contribute to one returned aggregate,
/// each call retains its own authored theorem and exact structural rewrite row.
pub(crate) fn compose_partition_wrappers(program: &TypedTrees, facts: &mut CheckFacts) {
    let mut available = facts
        .qualifications
        .content
        .conservation_plans
        .iter()
        .cloned()
        .map(|plan| AvailablePartitionSource {
            plan,
            derivation_depth: 0,
        })
        .collect::<Vec<_>>();
    let state_count = program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum::<usize>();
    let mut compositions = Vec::new();

    for _ in 0..state_count.max(1) {
        let sources = available.clone();
        let mut round = Vec::new();

        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for invocation in returned_partition_invocations(program, state) {
                    for source in sources.iter().filter(|source| {
                        source.plan.callable == invocation.target_symbol
                            && equation_contains_partition(&source.plan.equation)
                    }) {
                        let Some(composition) = instantiate_partition_wrapper(
                            program,
                            facts,
                            machine.symbol,
                            state,
                            &invocation,
                            &source.plan,
                            source.derivation_depth,
                        ) else {
                            continue;
                        };
                        if available
                            .iter()
                            .map(|source| &source.plan)
                            .chain(
                                round
                                    .iter()
                                    .map(|fact: &ContentPartitionCompositionFact| &fact.plan),
                            )
                            .any(|existing| {
                                existing.callable == state.symbol
                                    && existing.algebra == composition.plan.algebra
                                    && existing.report_fingerprint
                                        == composition.plan.report_fingerprint
                                    && existing.equation == composition.plan.equation
                            })
                        {
                            continue;
                        }
                        round.push(composition);
                    }
                }
            }
        }

        round.sort_by_key(|fact| {
            (
                fact.machine_symbol.arena_index(),
                fact.state_symbol.arena_index(),
                fact.source_callable.arena_index(),
                fact.source_report_fingerprint,
                content_conservation_plan_bytes(&fact.plan),
            )
        });
        round.dedup();
        if round.is_empty() {
            break;
        }
        available.extend(round.iter().map(|fact| AvailablePartitionSource {
            plan: fact.plan.clone(),
            derivation_depth: fact.source_derivation_depth.saturating_add(1),
        }));
        compositions.extend(round);
    }

    compositions.sort_by_key(|fact| {
        (
            fact.machine_symbol.arena_index(),
            fact.state_symbol.arena_index(),
            content_conservation_plan_bytes(&fact.plan),
        )
    });
    compositions.dedup();
    facts.qualifications.content.partition_compositions = compositions;
}

pub(crate) fn check_retained_content_custody(
    program: &TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    if facts.qualifications.content.plans.is_empty() {
        facts
            .qualifications
            .content
            .retained_borrow_custodies
            .clear();
        return Ok(());
    }

    let mut diagnostics = Vec::new();
    let mut retained_borrow_custodies = Vec::new();

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            let contracts = program
                .state_signature_contracts(signature)
                .iter()
                .collect::<Vec<_>>();
            check_callable(
                program,
                facts,
                &format!("{}::{}", trait_definition.name, signature.name),
                signature.symbol,
                &signature.lifetime_parameters,
                program.state_signature_parameters(signature),
                signature.return_type,
                &contracts,
                &mut diagnostics,
                &mut retained_borrow_custodies,
            );
            if trait_definition.is_boundary {
                check_boundary_partition_results(
                    program,
                    facts,
                    &format!("{}::{}", trait_definition.name, signature.name),
                    signature.symbol,
                    program.state_signature_parameters(signature),
                    signature.return_type,
                    &contracts,
                    &mut diagnostics,
                );
            }
        }
    }

    for machine in program.machines() {
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            let mut contracts = program.state_contracts(state).iter().collect::<Vec<_>>();
            if state_index == 0 {
                contracts.extend(program.machine_contracts(machine));
            }
            let label = if state_index == 0 {
                machine.name.to_string()
            } else {
                format!("{}::{}", machine.name, state.name)
            };
            check_callable(
                program,
                facts,
                &label,
                state.symbol,
                &machine.lifetime_parameters,
                program.state_parameters(state),
                state.return_type,
                &contracts,
                &mut diagnostics,
                &mut retained_borrow_custodies,
            );
            if !machine.body_is_present {
                check_boundary_partition_results(
                    program,
                    facts,
                    &label,
                    state.symbol,
                    program.state_parameters(state),
                    state.return_type,
                    &contracts,
                    &mut diagnostics,
                );
            }
        }
    }

    if diagnostics.is_empty() {
        facts.qualifications.content.retained_borrow_custodies = retained_borrow_custodies;
        Ok(())
    } else {
        facts
            .qualifications
            .content
            .retained_borrow_custodies
            .clear();
        Err(diagnostics)
    }
}
