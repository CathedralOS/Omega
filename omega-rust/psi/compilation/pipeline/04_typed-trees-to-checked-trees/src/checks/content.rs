//! Content-bearing signature conservation gates.
//!
//! A borrow lends access; it never supplies an owned claim. Retained custody
//! may nevertheless remain lifetime-bound to one explicit shared input loan.
//! This consumer admits only that exact whole-parameter/whole-result shape and
//! otherwise requires owned custody. Compatibility keys on the retained
//! compiler-owned algebra identity, never carrier or operation names.
//!
//! `call_results.rs` checks provisional result qualifications after ownership
//! reconstruction, including non-content provenance when no content plan exists.
//! `partition_wrappers.rs` composes
//! returned partition invocations into wrappers, `content_paths.rs` maps
//! content segments, contracts and projections onto fact paths, and
//! `retained_custody.rs` decides lifetime-bound borrow custody of retained
//! content.

mod call_results;
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

/// Derive the content equality attached to every exact input-relative claim
/// outcome. These are deliberately individual rewrite rows: distinct linear
/// claims do not imply that their projected content is disjoint, so this pass
/// never manufactures a `separate(...)` term. The later frontier theorem may
/// compose rows only when it also has the required partition evidence.
pub(crate) fn infer_identity_preserving_reshuffles(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) {
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
        let Some(machine) = program.machine_by_symbol(machine_symbol) else {
            continue;
        };
        let Some(state) = program.state_by_symbol(state_symbol) else {
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

/// Compose proved partition theorems at completed invocations. The shared
/// entailment owner decides which authored clauses are proved; requirements
/// and admitted boundary declarations supply the other roots. Merely writing
/// a guarantee cannot make an unproved cycle authorize itself.
///
/// The capture owner relates actual entry observations and returned contents,
/// while the permission ledger separately relates exact claims. Derived rows
/// bind one outgoing occurrence. Only a theorem established on every ordinary
/// outgoing edge becomes a callable source for the next fixed-point round.
/// Substitution preserves the source's partition algebra; it never invents
/// disjointness from independent claim identities or equal scalar totals.
pub(crate) fn compose_partition_wrappers(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    entailment: &mut super::contracts::ProvenExitExpressions<'_, '_>,
) {
    let source_plans = validation::content_conservation_plans(program);
    let mut available = source_plans
        .iter()
        .filter(|source| {
            source.plan.owner_kind == ContentConservationOwnerKind::TraitRequirement
                || program
                    .machine_by_symbol(source.plan.owner)
                    .is_some_and(|machine| {
                        machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim
                            || entailment
                                .for_machine(facts, machine.symbol)
                                .expressions
                                .contains(&source.source_expression)
                    })
        })
        .map(|source| AvailablePartitionSource {
            plan: source.plan.clone(),
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
                for invocation in returned_partition_invocations(program, facts, state) {
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
                        if compositions.iter().chain(round.iter()).any(
                            |existing: &ContentPartitionCompositionFact| {
                                existing.returned_at == composition.returned_at
                                    && existing.machine_symbol == composition.machine_symbol
                                    && existing.state_symbol == composition.state_symbol
                                    && existing.statement_index == composition.statement_index
                                    && existing.call_ordinal == composition.call_ordinal
                                    && existing.source_plan == composition.source_plan
                                    && existing.plan == composition.plan
                            },
                        ) {
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
        compositions.extend(round);
        let mut added_source = false;
        for composition in &compositions {
            if available
                .iter()
                .any(|source| source.plan == composition.plan)
            {
                continue;
            }
            let Some(state) = program.state_by_symbol(composition.state_symbol) else {
                continue;
            };
            let outgoing = partition_wrappers::ordinary_returns(program, facts, state);
            // A theorem on one arm is not a callable guarantee. Keep each
            // exact exit proof, and export a source only when the complete
            // ordinary outgoing roster proves the same normalized equation.
            if outgoing.is_empty()
                || !outgoing.iter().all(|returned_at| {
                    compositions.iter().any(|candidate| {
                        candidate.returned_at == *returned_at && candidate.plan == composition.plan
                    })
                })
            {
                continue;
            }
            available.push(AvailablePartitionSource {
                plan: composition.plan.clone(),
                derivation_depth: composition.source_derivation_depth.saturating_add(1),
            });
            added_source = true;
        }
        if !added_source {
            break;
        }
    }

    compositions.sort_by_key(|fact| {
        (
            fact.machine_symbol.arena_index(),
            fact.state_symbol.arena_index(),
            content_conservation_plan_bytes(&fact.plan),
        )
    });
    compositions.dedup();
    // A complete entry summary is a judgment over every ordinary machine
    // return, including returns in later states. Its substitution has already
    // carried their parameters back through the selected incoming calls.
    // Per-exit reference-origin machinery is not a prerequisite for this
    // independent owned-content proof. Never publish a local exit row here.
    for source in source_plans
        .iter()
        .filter(|source| source.plan.owner_kind == ContentConservationOwnerKind::Machine)
    {
        let Some(machine) = program.machine_by_symbol(source.plan.owner) else {
            continue;
        };
        if program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == source.plan.callable)
            && available
                .iter()
                .any(|complete| complete.plan == source.plan)
        {
            entailment.record_proved_normal_return(facts, machine.symbol, source.source_expression);
        }
    }
    facts.qualifications.content.partition_compositions = compositions;
}

/// Consume a derived theorem only at the outgoing occurrence whose current
/// result claims justified its substitution. Equal statement indices on two
/// transition arms are not the same completed invocation.
pub(super) fn proves_composed_exit(
    facts: &CheckFacts,
    exit: &checked_trees::FlowExitFact,
    fact: &facts::Fact,
    sources: &[validation::ContentConservationSourcePlan],
) -> bool {
    let facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return false;
    };
    sources
        .iter()
        .filter(|source| {
            source.source_expression == expression && source.plan.owner == exit.machine_symbol
        })
        .any(|source| {
            facts
                .qualifications
                .content
                .partition_compositions
                .iter()
                .any(|composition| {
                    let outgoing = facts.flow.control.outgoing.get(composition.returned_at);
                    outgoing.machine_symbol == exit.machine_symbol
                        && outgoing.state_symbol == exit.state_symbol
                        && outgoing.statement_index == exit.statement_index
                        && outgoing.transition_target == exit.transition_target
                        && composition.plan.algebra == source.plan.algebra
                        && composition.plan.equation == source.plan.equation
                })
        })
}

pub(crate) fn check_retained_content_custody(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    call_results::check_call_result_qualifications(program, facts, &mut diagnostics);
    if facts.qualifications.content.plans.is_empty() {
        facts
            .qualifications
            .content
            .retained_borrow_custodies
            .clear();
        return if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        };
    }

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

    program.for_each_machine_state_contract_frame(|machine, state, contracts, label| {
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
    });

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
