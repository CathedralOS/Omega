//! Content-bearing signature conservation gates.
//!
//! A borrow lends access; it never supplies an owned claim that can survive
//! the call. This first P1c consumer rejects the exact retained-custody shape
//! where a content-bearing result has compatible content-bearing inputs, but
//! every compatible source is borrowed. It deliberately keys compatibility by
//! the retained compiler-owned algebra identity, never carrier or operation
//! names.

use psi_checked_trees::{
    CheckFacts, ContentIdentityReshuffleFact, ContentPartitionCompositionFact,
    ContentPartitionPlaceSubstitution, ContentPartitionResultRewrite, FlowClaimOutcomeSource,
};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::content::{
    ContentCaseSegment, ContentConservationEquation, ContentConservationOwnerKind,
    ContentConservationPlan, ContentConservationTerm, ContentFieldSegment, ContentPlaceRoot,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionPlan, ContentStructuralPlace,
    conservation_fingerprint, content_conservation_plan_bytes,
};
use psi_language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, SemanticDomainId,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use psi_typed_trees::statement::{StatementNode, TransitionTargetNode};
use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

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
        let Some(state) = crate::find_state(program, state_symbol) else {
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
                    && output_plan.fingerprint == input_plan.fingerprint
                    && output_plan.algebra == input_plan.algebra
            }) {
                let left = projection_term(input_plan, input_subject.clone());
                let right = projection_term(output_plan, output_subject.clone());
                let equation = ContentConservationEquation::new(left, right);
                let fingerprint = conservation_fingerprint(&input_plan.algebra, &equation);
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
                        fingerprint,
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

#[derive(Debug, Clone)]
struct ReturnedPartitionInvocation {
    statement_index: usize,
    target_symbol: SymbolHandle,
    receiver: Option<ExpressionHandle>,
    arguments: Vec<ExpressionHandle>,
    form: ReturnedPartitionInvocationForm,
}

#[derive(Debug, Clone, Copy)]
enum ReturnedPartitionInvocationForm {
    Expression(ExpressionHandle),
    NamedTransition,
    StagedLocal {
        call_expression: ExpressionHandle,
        local_symbol: SymbolHandle,
    },
}

#[derive(Debug, Default)]
struct PartitionCompositionEvidence {
    call_ordinal: Option<usize>,
    input_claim_identities: Vec<PermissionClaimIdentity>,
    input_claim_bindings: Vec<psi_checked_trees::ContentPartitionInputClaimBinding>,
    result_rewrites: Vec<ContentPartitionResultRewrite>,
    substitutions: Vec<ContentPartitionPlaceSubstitution>,
    observed_entry_projection: bool,
}

#[derive(Debug, Clone)]
struct AvailablePartitionSource {
    plan: ContentConservationPlan,
    derivation_depth: u32,
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
                                    && existing.fingerprint == composition.plan.fingerprint
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
                fact.source_fingerprint,
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

fn returned_partition_invocations(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
) -> Vec<ReturnedPartitionInvocation> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut invocations = statements
        .iter()
        .enumerate()
        .filter_map(|(statement_index, statement)| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            let mut invocation =
                direct_expression_invocation(program, statement_index, local.initial_value)?;
            invocation.form = ReturnedPartitionInvocationForm::StagedLocal {
                call_expression: local.initial_value,
                local_symbol: local.symbol,
            };
            Some(invocation)
        })
        .filter(|invocation| invocation.target_symbol.is_valid())
        .collect::<Vec<_>>();
    let mut returned = Vec::new();

    for (statement_index, statement) in statements.iter().enumerate() {
        match statement {
            StatementNode::Expression(expression) if statement_index + 1 == statements.len() => {
                if let Some(invocation) =
                    direct_expression_invocation(program, statement_index, *expression)
                {
                    returned.push(invocation);
                }
            }
            StatementNode::Transition(transition) => {
                for target in [transition.target, transition.continuation]
                    .into_iter()
                    .filter(|target| target.is_valid())
                {
                    match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named {
                            path, arguments, ..
                        } => {
                            returned.push(ReturnedPartitionInvocation {
                                statement_index,
                                target_symbol: path.symbol,
                                receiver: None,
                                arguments: program
                                    .statement_table
                                    .expression_handles(*arguments)
                                    .to_vec(),
                                form: ReturnedPartitionInvocationForm::NamedTransition,
                            });
                        }
                        TransitionTargetNode::Value(expression) => {
                            if let Some(invocation) =
                                direct_expression_invocation(program, statement_index, *expression)
                            {
                                returned.push(invocation);
                            }
                        }
                        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                    }
                }
            }
            _ => {}
        }
    }

    if let [invocation] = returned.as_slice()
        && invocation.target_symbol.is_valid()
    {
        invocations.push(invocation.clone());
    }
    invocations
}

fn direct_expression_invocation(
    program: &TypedTrees,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<ReturnedPartitionInvocation> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    Some(ReturnedPartitionInvocation {
        statement_index,
        target_symbol: call.target_symbol,
        receiver: call.receiver.is_valid().then_some(call.receiver),
        arguments: program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec(),
        form: ReturnedPartitionInvocationForm::Expression(expression),
    })
}

fn equation_contains_partition(equation: &ContentConservationEquation) -> bool {
    term_contains_partition(equation.left()) || term_contains_partition(equation.right())
}

fn term_contains_partition(term: &ContentConservationTerm) -> bool {
    match term {
        ContentConservationTerm::Projection { .. } => false,
        ContentConservationTerm::Separate(_) => true,
    }
}

fn instantiate_partition_wrapper(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state: &psi_typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    source: &ContentConservationPlan,
    source_derivation_depth: u32,
) -> Option<ContentPartitionCompositionFact> {
    let target_parameters = crate::call_target_parameters(program, invocation.target_symbol)?;
    let call_ordinal = partition_invocation_call_ordinal(
        program,
        facts,
        machine_symbol,
        state.symbol,
        invocation,
    )?;
    let mut evidence = PartitionCompositionEvidence {
        call_ordinal: Some(call_ordinal),
        ..PartitionCompositionEvidence::default()
    };
    let left = instantiate_partition_term(
        program,
        facts,
        state,
        invocation,
        target_parameters,
        source.equation.left(),
        &mut evidence,
    )?;
    let right = instantiate_partition_term(
        program,
        facts,
        state,
        invocation,
        target_parameters,
        source.equation.right(),
        &mut evidence,
    )?;
    if !evidence.observed_entry_projection {
        return None;
    }
    evidence.input_claim_bindings.sort_by_key(|binding| {
        (
            format!("{:?}", binding.claim_identity),
            format!("{:?}", binding.entry_place),
        )
    });
    evidence.input_claim_bindings.dedup();
    if evidence
        .input_claim_bindings
        .iter()
        .enumerate()
        .any(|(index, binding)| {
            evidence.input_claim_bindings[index + 1..]
                .iter()
                .any(|later| {
                    later.claim_identity == binding.claim_identity
                        && later.entry_place != binding.entry_place
                })
        })
    {
        return None;
    }
    evidence.input_claim_identities = evidence
        .input_claim_bindings
        .iter()
        .map(|binding| binding.claim_identity)
        .collect();
    evidence.input_claim_identities.dedup();
    evidence.result_rewrites.sort_by_key(|rewrite| {
        (
            format!("{:?}", rewrite.source),
            format!("{:?}", rewrite.target),
            format!("{:?}", rewrite.claim_identity),
        )
    });
    evidence.result_rewrites.dedup();
    evidence.substitutions.sort_by_key(|substitution| {
        (
            format!("{:?}", substitution.source),
            format!("{:?}", substitution.target),
        )
    });
    evidence.substitutions.dedup();
    let equation = ContentConservationEquation::new(left, right);
    let fingerprint = conservation_fingerprint(&source.algebra, &equation);
    let plan = ContentConservationPlan {
        owner_kind: ContentConservationOwnerKind::Machine,
        owner: machine_symbol,
        callable: state.symbol,
        algebra: source.algebra.clone(),
        equation,
        fingerprint,
    };
    Some(ContentPartitionCompositionFact {
        machine_symbol,
        state_symbol: state.symbol,
        source_callable: source.callable,
        source_fingerprint: source.fingerprint,
        source_derivation_depth,
        source_plan: source.clone(),
        statement_index: invocation.statement_index,
        call_ordinal,
        input_claim_identities: evidence.input_claim_identities,
        input_claim_bindings: evidence.input_claim_bindings,
        result_rewrites: evidence.result_rewrites,
        substitutions: evidence.substitutions,
        plan,
    })
}

fn partition_invocation_call_ordinal(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    invocation: &ReturnedPartitionInvocation,
) -> Option<usize> {
    let state_flow = facts.flow.control.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
    })?;
    let ordinals = facts
        .flow
        .control
        .calls
        .span_or_empty(state_flow.calls)
        .iter()
        .filter(|call| {
            call.statement_index == invocation.statement_index
                && call.target_symbol == invocation.target_symbol
        })
        .filter_map(|call| {
            let call_site = crate::find_call_site(
                program,
                machine_symbol,
                state_symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
            let exact = match (&invocation.form, call_site) {
                (
                    ReturnedPartitionInvocationForm::Expression(expected),
                    crate::CallSite::Expression { expression, .. },
                ) => *expected == expression,
                (
                    ReturnedPartitionInvocationForm::NamedTransition,
                    crate::CallSite::TransitionNamed { arguments, .. },
                ) => {
                    program.statement_table.expression_handles(arguments)
                        == invocation.arguments.as_slice()
                }
                (
                    ReturnedPartitionInvocationForm::StagedLocal {
                        call_expression, ..
                    },
                    crate::CallSite::Expression { expression, .. },
                ) => *call_expression == expression,
                _ => false,
            };
            exact.then_some(call.call_ordinal)
        })
        .fold(Vec::new(), |mut ordinals, ordinal| {
            if !ordinals.contains(&ordinal) {
                ordinals.push(ordinal);
            }
            ordinals
        });
    let [ordinal] = ordinals.as_slice() else {
        return None;
    };
    Some(*ordinal)
}

#[allow(clippy::too_many_arguments)]
fn instantiate_partition_term(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_state: &psi_typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    target_parameters: &[StateParameter],
    term: &ContentConservationTerm,
    evidence: &mut PartitionCompositionEvidence,
) -> Option<ContentConservationTerm> {
    match term {
        ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            projection_fingerprint,
            subject,
        } => Some(ContentConservationTerm::Projection {
            domain: *domain,
            semantic_domain: *semantic_domain,
            projection_machine: *projection_machine,
            projection_fingerprint: *projection_fingerprint,
            subject: instantiate_partition_subject(
                program,
                facts,
                caller_state,
                invocation,
                target_parameters,
                subject,
                evidence,
            )?,
        }),
        ContentConservationTerm::Separate(children) => Some(ContentConservationTerm::separate(
            children
                .iter()
                .map(|child| {
                    instantiate_partition_term(
                        program,
                        facts,
                        caller_state,
                        invocation,
                        target_parameters,
                        child,
                        evidence,
                    )
                })
                .collect::<Option<Vec<_>>>()?,
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn instantiate_partition_subject(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_state: &psi_typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    target_parameters: &[StateParameter],
    subject: &ContentStructuralPlace,
    evidence: &mut PartitionCompositionEvidence,
) -> Option<ContentStructuralPlace> {
    let target = match (&subject.root, subject.version) {
        (ContentPlaceRoot::Result, ContentPlaceVersion::Current) => {
            instantiate_partition_result_subject(
                program,
                facts,
                caller_state,
                invocation,
                subject,
                evidence,
            )?
        }
        (
            ContentPlaceRoot::Parameter {
                position, symbol, ..
            },
            ContentPlaceVersion::Entry,
        ) => {
            evidence.observed_entry_projection = true;
            let parameter = target_parameters
                .get(usize::try_from(*position).ok()?)
                .filter(|parameter| !symbol.is_valid() || parameter.symbol == *symbol)
                .or_else(|| {
                    symbol
                        .is_valid()
                        .then(|| {
                            target_parameters
                                .iter()
                                .find(|parameter| parameter.symbol == *symbol)
                        })
                        .flatten()
                })?;
            let argument = argument_for_target_parameter(
                target_parameters,
                &invocation.arguments,
                invocation.receiver,
                parameter.symbol,
            )?;
            let actual = partition_argument_place(
                program,
                caller_state.symbol,
                invocation.statement_index,
                argument,
                &subject.segments,
            )?;
            let psi_facts::PlaceRoot::Symbol(actual_root) = actual.root else {
                return None;
            };
            let (caller_position, caller_parameter) = program
                .state_parameters(caller_state)
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == actual_root)?;
            let claim_identity = unique_entry_claim_identity(
                facts,
                caller_state.symbol,
                actual_root,
                &actual.segments,
            )?;
            let call_ordinal = evidence.call_ordinal?;
            let transferred_to_invocation =
                facts.flow.ownership.permissions.iter().any(|(_, event)| {
                    let source_matches = matches!(
                        event.source,
                        PermissionEventSource::Call {
                            statement_index,
                            call_ordinal: event_call_ordinal,
                            target_symbol,
                            ..
                        } if statement_index == invocation.statement_index
                            && event_call_ordinal == call_ordinal
                            && target_symbol == invocation.target_symbol
                    ) || event.source
                        == PermissionEventSource::Statement {
                            statement_index: invocation.statement_index,
                        };
                    event.state_symbol == caller_state.symbol
                        && source_matches
                        && event.kind == PermissionEventKind::Transfer
                        && event.access == PermissionAccess::Owned
                        && event.obligation_live
                        && event.claim_identity == claim_identity
                        && event.root == actual.root
                        && facts.flow.ownership.segments.span_or_empty(event.segments)
                            == actual.segments
                });
            if !transferred_to_invocation {
                return None;
            }
            let entry_place = ContentStructuralPlace {
                version: ContentPlaceVersion::Entry,
                root: ContentPlaceRoot::Parameter {
                    position: u32::try_from(caller_position).ok()?,
                    symbol: caller_parameter.symbol,
                    name: caller_parameter.name.as_str().to_owned(),
                    is_self: caller_parameter.is_self,
                },
                segments: content_path(program, &actual.segments)?,
            };
            evidence.input_claim_bindings.push(
                psi_checked_trees::ContentPartitionInputClaimBinding {
                    claim_identity,
                    entry_place: entry_place.clone(),
                },
            );
            entry_place
        }
        _ => return None,
    };
    if let Some(previous) = evidence
        .substitutions
        .iter()
        .find(|substitution| substitution.source == *subject)
    {
        if previous.target != target {
            return None;
        }
    } else {
        evidence
            .substitutions
            .push(ContentPartitionPlaceSubstitution {
                source: subject.clone(),
                target: target.clone(),
            });
    }
    Some(target)
}

fn instantiate_partition_result_subject(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_state: &psi_typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    subject: &ContentStructuralPlace,
    evidence: &mut PartitionCompositionEvidence,
) -> Option<ContentStructuralPlace> {
    let ReturnedPartitionInvocationForm::StagedLocal { local_symbol, .. } = invocation.form else {
        return Some(subject.clone());
    };
    let local_segments = content_segments_to_fact_path(&subject.segments)?;
    let call_ordinal = evidence.call_ordinal?;
    let identities = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            let source_matches = matches!(
                event.source,
                PermissionEventSource::Call {
                    statement_index,
                    call_ordinal: event_call_ordinal,
                    target_symbol,
                    ..
                } if statement_index == invocation.statement_index
                    && event_call_ordinal == call_ordinal
                    && target_symbol == invocation.target_symbol
            ) || event.source
                == PermissionEventSource::Statement {
                    statement_index: invocation.statement_index,
                };
            event.state_symbol == caller_state.symbol
                && source_matches
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
                && event.obligation_live
                && event.claim_identity != PermissionClaimIdentity::Unknown
                && event.root == psi_facts::PlaceRoot::Symbol(local_symbol)
                && facts.flow.ownership.segments.span_or_empty(event.segments) == local_segments
        })
        .map(|(_, event)| event.claim_identity)
        .fold(Vec::new(), |mut identities, identity| {
            if !identities.contains(&identity) {
                identities.push(identity);
            }
            identities
        });
    let [claim_identity] = identities.as_slice() else {
        return None;
    };
    let output_paths = facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .filter(|(_, map)| map.state_symbol == caller_state.symbol)
        .flat_map(|(_, map)| {
            facts
                .flow
                .ownership
                .claim_outcome_entries
                .span_or_empty(map.entries)
        })
        .filter_map(|entry| match entry.source {
            FlowClaimOutcomeSource::Established {
                claim_identity: outcome_identity,
                ..
            } if outcome_identity == *claim_identity => Some(
                facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(entry.output_segments)
                    .to_vec(),
            ),
            FlowClaimOutcomeSource::Input { .. }
            | FlowClaimOutcomeSource::Established { .. }
            | FlowClaimOutcomeSource::Unknown => None,
        })
        .fold(Vec::new(), |mut paths, path| {
            if !paths.contains(&path) {
                paths.push(path);
            }
            paths
        });
    let [output_path] = output_paths.as_slice() else {
        return None;
    };
    let target = ContentStructuralPlace {
        version: ContentPlaceVersion::Current,
        root: ContentPlaceRoot::Result,
        segments: content_path(program, output_path)?,
    };
    evidence
        .result_rewrites
        .push(ContentPartitionResultRewrite {
            claim_identity: *claim_identity,
            source: subject.clone(),
            target: target.clone(),
        });
    Some(target)
}

fn argument_for_target_parameter(
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    receiver: Option<ExpressionHandle>,
    parameter_symbol: SymbolHandle,
) -> Option<ExpressionHandle> {
    let includes_explicit_self =
        parameters.iter().any(|parameter| parameter.is_self) && arguments.len() == parameters.len();
    let mut argument_index = 0usize;
    for parameter in parameters {
        let argument = if parameter.is_self && !includes_explicit_self {
            receiver
        } else {
            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            argument
        };
        if parameter.symbol == parameter_symbol {
            return argument;
        }
    }
    None
}

fn partition_argument_place(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    argument: ExpressionHandle,
    projection_path: &[ContentPlaceSegment],
) -> Option<crate::flow::CanonicalPlace> {
    let mut direct = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        argument,
    )?;
    if matches!(direct.root, psi_facts::PlaceRoot::Symbol(_)) {
        direct
            .segments
            .extend(content_segments_to_fact_path(projection_path)?);
        return Some(direct);
    }
    let leaf = aggregate_argument_projection(program, argument, projection_path)?;
    let leaf = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        leaf,
    )?;
    matches!(leaf.root, psi_facts::PlaceRoot::Symbol(_)).then_some(leaf)
}

fn aggregate_argument_projection(
    program: &TypedTrees,
    expression: ExpressionHandle,
    projection_path: &[ContentPlaceSegment],
) -> Option<ExpressionHandle> {
    let Some((head, tail)) = projection_path.split_first() else {
        return Some(expression);
    };
    match (head, program.expression_table.expression(expression)) {
        (ContentPlaceSegment::Case(expected), ExpressionNode::StructLiteral(literal))
            if literal
                .case_name
                .as_ref()
                .is_some_and(|case| case.as_str() == expected.name) =>
        {
            aggregate_argument_projection(program, expression, tail)
        }
        (ContentPlaceSegment::Field(expected), ExpressionNode::StructLiteral(literal)) => {
            let field = program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .find(|field| field.name.as_str() == expected.name)?;
            aggregate_argument_projection(program, field.value, tail)
        }
        (ContentPlaceSegment::FixedIndex(index), ExpressionNode::ArrayLiteral(values)) => {
            let value = program
                .expression_table
                .expression_handles(*values)
                .get(usize::try_from(*index).ok()?)?;
            aggregate_argument_projection(program, *value, tail)
        }
        _ => None,
    }
}

fn content_segments_to_fact_path(
    segments: &[ContentPlaceSegment],
) -> Option<Vec<psi_facts::PlaceSegment>> {
    segments
        .iter()
        .map(|segment| match segment {
            ContentPlaceSegment::Case(case) if case.symbol.is_valid() => {
                Some(psi_facts::PlaceSegment::Case {
                    variant: case.symbol,
                })
            }
            ContentPlaceSegment::Field(field) if field.symbol.is_valid() => {
                Some(psi_facts::PlaceSegment::Field {
                    symbol: field.symbol,
                })
            }
            ContentPlaceSegment::FixedIndex(index) => Some(psi_facts::PlaceSegment::FixedIndex {
                index: usize::try_from(*index).ok()?,
            }),
            ContentPlaceSegment::Case(_) | ContentPlaceSegment::Field(_) => None,
        })
        .collect()
}

fn projection_term(
    plan: &ContentProjectionPlan,
    subject: ContentStructuralPlace,
) -> ContentConservationTerm {
    ContentConservationTerm::Projection {
        domain: plan.domain,
        semantic_domain: plan.semantic_domain,
        projection_machine: plan.machine,
        projection_fingerprint: plan.fingerprint,
        subject,
    }
}

fn unique_entry_claim_identity(
    facts: &CheckFacts,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    input_path: &[psi_facts::PlaceSegment],
) -> Option<PermissionClaimIdentity> {
    let identities = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.state_symbol == state_symbol
                && event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
                && event.obligation_live
                && event.root == psi_facts::PlaceRoot::Symbol(parameter_symbol)
                && facts.flow.ownership.segments.span_or_empty(event.segments) == input_path
                && event.claim_identity != PermissionClaimIdentity::Unknown
        })
        .map(|(_, event)| event.claim_identity)
        .fold(Vec::new(), |mut identities, identity| {
            if !identities.contains(&identity) {
                identities.push(identity);
            }
            identities
        });
    let [identity] = identities.as_slice() else {
        return None;
    };
    Some(*identity)
}

fn applicable_projection_plans<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    type_reference: TypeReferenceHandle,
    subject: &ContentStructuralPlace,
) -> Vec<&'facts ContentProjectionPlan> {
    let Some(carrier) = unwrapped_type_reference(program, type_reference) else {
        return Vec::new();
    };
    let carrier_identity = program.normalized_type_identity(carrier).into_string();
    facts
        .qualifications
        .content
        .plans
        .iter()
        .filter(|plan| {
            plan.carrier_identity == carrier_identity
                && (type_has_domain(program, type_reference, plan.semantic_domain)
                    || contracts_establish_domain(
                        program,
                        machine,
                        state,
                        subject,
                        plan.domain,
                        plan.semantic_domain,
                    ))
        })
        .collect()
}

fn unwrapped_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => unwrapped_type_reference(program, *referee),
        _ => Some(type_reference),
    }
}

fn type_has_domain(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    domain: SemanticDomainId,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_has_domain(program, *referee, domain)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| {
                    matches!(constraint, TypeConstraintNode::Domain(candidate) if candidate.semantic_id == domain)
                })
                || type_has_domain(program, *base_type, domain)
        }
        _ => false,
    }
}

fn contracts_establish_domain(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    subject: &ContentStructuralPlace,
    domain: SymbolHandle,
    semantic_domain: SemanticDomainId,
) -> bool {
    if program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == domain)
        .is_none_or(|definition| definition.semantic_id != semantic_domain)
    {
        // Contract proof facts currently retain only the nominal family. They
        // cannot establish one exact indexed application without laundering
        // another family member into it.
        return false;
    }
    let mut contracts = program.state_contracts(state).iter().collect::<Vec<_>>();
    if program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol)
    {
        contracts.extend(program.machine_contracts(machine));
    }
    contracts.into_iter().any(|contract| {
        let allowed = match (&subject.root, subject.version) {
            (ContentPlaceRoot::Parameter { .. }, ContentPlaceVersion::Entry) => {
                contract.kind == SignatureContractKind::Requires
            }
            (ContentPlaceRoot::Result, ContentPlaceVersion::Current) => {
                contract.kind == SignatureContractKind::Ensures
            }
            _ => false,
        };
        allowed
            && program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .any(|fact| {
                    let ProofFact::Membership(membership) = fact else {
                        return false;
                    };
                    membership.domain_symbol == domain
                        && contract_place_matches(program, membership.value, subject)
                })
    })
}

fn contract_place_matches(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: &ContentStructuralPlace,
) -> bool {
    let Some((root_name, root_symbol, segments)) = contract_structural_place(program, expression)
    else {
        return false;
    };
    let root_matches = match &expected.root {
        ContentPlaceRoot::Result => root_name == "result",
        ContentPlaceRoot::Parameter { symbol, name, .. } => {
            (*symbol == root_symbol && symbol.is_valid()) || *name == root_name
        }
    };
    root_matches && content_paths_match(&expected.segments, &segments)
}

fn contract_structural_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(String, SymbolHandle, Vec<ContentPlaceSegment>)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let names = program.expression_table.name_path_members(path.members);
            let root = names.first()?.as_str().to_owned();
            let symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let root_symbol = symbols.first().copied().unwrap_or(path.head_symbol);
            let segments =
                names
                    .iter()
                    .enumerate()
                    .skip(1)
                    .fold(Vec::new(), |mut segments, (index, name)| {
                        push_contract_field(
                            program,
                            &mut segments,
                            symbols
                                .get(index)
                                .copied()
                                .unwrap_or(SymbolHandle::invalid()),
                            name.as_str(),
                        );
                        segments
                    });
            Some((root, root_symbol, segments))
        }
        ExpressionNode::Member(member) => {
            let (root, root_symbol, mut segments) =
                contract_structural_place(program, member.receiver)?;
            push_contract_field(
                program,
                &mut segments,
                member.member_symbol,
                member.member.as_str(),
            );
            Some((root, root_symbol, segments))
        }
        ExpressionNode::Indexed(indexed) => {
            let (root, root_symbol, mut segments) =
                contract_structural_place(program, indexed.collection)?;
            let ExpressionNode::Integer(index) = program.expression_table.expression(indexed.index)
            else {
                return None;
            };
            segments.push(ContentPlaceSegment::FixedIndex(index.value_u64()?));
            Some((root, root_symbol, segments))
        }
        ExpressionNode::Borrow(inner) => contract_structural_place(program, inner.target),
        _ => None,
    }
}

fn push_contract_field(
    program: &TypedTrees,
    segments: &mut Vec<ContentPlaceSegment>,
    field_symbol: SymbolHandle,
    field_name: &str,
) {
    if let Some(variant_symbol) = psi_facts::payload_variant_for_field(program, field_symbol)
        && let Some(variant_name) = data_variant_name(program, variant_symbol)
    {
        segments.push(ContentPlaceSegment::Case(ContentCaseSegment {
            symbol: variant_symbol,
            name: variant_name.to_owned(),
        }));
    }
    segments.push(ContentPlaceSegment::Field(ContentFieldSegment {
        symbol: field_symbol,
        name: field_name.to_owned(),
    }));
}

fn content_paths_match(left: &[ContentPlaceSegment], right: &[ContentPlaceSegment]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| match (left, right) {
                (ContentPlaceSegment::Case(left), ContentPlaceSegment::Case(right)) => {
                    left.name == right.name
                        && (!left.symbol.is_valid()
                            || !right.symbol.is_valid()
                            || left.symbol == right.symbol)
                }
                (ContentPlaceSegment::FixedIndex(left), ContentPlaceSegment::FixedIndex(right)) => {
                    left == right
                }
                (ContentPlaceSegment::Field(left), ContentPlaceSegment::Field(right)) => {
                    left.name == right.name
                        && (!left.symbol.is_valid()
                            || !right.symbol.is_valid()
                            || left.symbol == right.symbol)
                }
                _ => false,
            })
}

fn content_path(
    program: &TypedTrees,
    path: &[psi_facts::PlaceSegment],
) -> Option<Vec<ContentPlaceSegment>> {
    path.iter()
        .map(|segment| match segment {
            psi_facts::PlaceSegment::Case { variant } => {
                Some(ContentPlaceSegment::Case(ContentCaseSegment {
                    symbol: *variant,
                    name: data_variant_name(program, *variant)?.to_owned(),
                }))
            }
            psi_facts::PlaceSegment::Field { symbol } => {
                Some(ContentPlaceSegment::Field(ContentFieldSegment {
                    symbol: *symbol,
                    name: data_field_name(program, *symbol)?.to_owned(),
                }))
            }
            psi_facts::PlaceSegment::FixedIndex { index } => Some(ContentPlaceSegment::FixedIndex(
                u64::try_from(*index).expect("fixed index fits u64"),
            )),
            psi_facts::PlaceSegment::Index { .. } => None,
        })
        .collect()
}

fn data_variant_name(program: &TypedTrees, variant_symbol: SymbolHandle) -> Option<&str> {
    program.data_definitions().iter().find_map(|definition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                psi_typed_trees::data::DataMember::Variant(variant) => {
                    (variant.symbol == variant_symbol).then_some(variant.name.as_str())
                }
                psi_typed_trees::data::DataMember::Field(_) => None,
            })
    })
}

fn data_field_name(program: &TypedTrees, field_symbol: SymbolHandle) -> Option<&str> {
    program.data_definitions().iter().find_map(|definition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) => {
                    (field.symbol == field_symbol).then_some(field.name.as_str())
                }
                psi_typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find_map(|field| {
                        (field.symbol == field_symbol).then_some(field.name.as_str())
                    }),
            })
    })
}

pub(crate) fn check_retained_content_custody(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    if facts.qualifications.content.plans.is_empty() {
        return Ok(());
    }

    let mut diagnostics = Vec::new();

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
                program.state_signature_parameters(signature),
                signature.return_type,
                &contracts,
                &mut diagnostics,
            );
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
                program.state_parameters(state),
                state.return_type,
                &contracts,
                &mut diagnostics,
            );
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[allow(clippy::too_many_arguments)]
fn check_callable(
    program: &TypedTrees,
    facts: &CheckFacts,
    label: &str,
    callable: SymbolHandle,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    contracts: &[&SignatureContract],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut result_domains = Vec::new();
    append_type_domains(program, return_type, &mut result_domains);
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Membership(membership) = fact else {
                continue;
            };
            if expression_is_bare_result(program, membership.value)
                && let Some(domain) = nominal_domain_application(program, membership.domain_symbol)
            {
                push_unique_domain_application(&mut result_domains, domain);
            }
        }
    }

    for result_domain in result_domains {
        let Some(result_plan) = facts
            .qualifications
            .content
            .for_semantic_domain(result_domain.semantic_domain)
        else {
            continue;
        };
        let mut borrowed_sources = Vec::new();
        let mut owned_sources = Vec::new();

        for parameter in parameters {
            let mut parameter_domains = Vec::new();
            append_type_domains(program, parameter.type_reference, &mut parameter_domains);
            for contract in contracts
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Requires)
            {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Membership(membership) = fact else {
                        continue;
                    };
                    if expression_names_parameter(program, membership.value, parameter) {
                        if let Some(domain) =
                            nominal_domain_application(program, membership.domain_symbol)
                        {
                            push_unique_domain_application(&mut parameter_domains, domain);
                        }
                    }
                }
            }

            let compatible = parameter_domains.iter().any(|domain| {
                facts
                    .qualifications
                    .content
                    .for_semantic_domain(domain.semantic_domain)
                    .is_some_and(|input_plan| compatible_content(input_plan, result_plan))
            });
            if !compatible {
                continue;
            }

            if type_contains_reference(program, parameter.type_reference) {
                borrowed_sources.push(parameter.name.as_str());
            } else if program.type_multiplicity(parameter.type_reference) == Multiplicity::Linear {
                owned_sources.push(parameter.name.as_str());
            }
        }

        if owned_sources.len() == 1 || (owned_sources.is_empty() && borrowed_sources.is_empty()) {
            continue;
        }

        if let Some(selected) = authored_retention_source(
            facts,
            callable,
            result_domain.semantic_domain,
            result_plan,
            parameters,
        ) && owned_sources.iter().any(|name| *name == selected)
        {
            continue;
        }

        let result_name = domain_name(program, result_domain.symbol);
        if owned_sources.len() > 1 {
            let owned = owned_sources
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", ");
            diagnostics.push(Diagnostic::error(format!(
                "callable `{label}` returns content-bearing custody `{result_name}` with ambiguous compatible consumed inputs {owned}; retained-after-return authority requires one unambiguous owned source or an exact postcondition correspondence",
            )));
            continue;
        }
        let borrowed = borrowed_sources
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        diagnostics.push(Diagnostic::error(format!(
            "callable `{label}` returns content-bearing custody `{result_name}` sourced only from borrowed parameter{} {borrowed}; retained-after-return authority requires a consumed owned input",
            if borrowed_sources.len() == 1 { "" } else { "s" },
        )));
    }
}

/// Return the one parameter selected by an exact authored custody equality.
/// The equation must relate the whole current result projection directly to
/// the whole entry projection of one parameter in the same algebra. Partition
/// terms and structural subplaces describe transformations rather than the
/// one-to-one correspondence needed to resolve retained ownership.
fn authored_retention_source<'a>(
    facts: &CheckFacts,
    callable: SymbolHandle,
    result_domain: SemanticDomainId,
    result_plan: &ContentProjectionPlan,
    parameters: &'a [StateParameter],
) -> Option<&'a str> {
    facts
        .qualifications
        .content
        .conservation_plans
        .iter()
        .filter(|plan| plan.callable == callable && plan.algebra == result_plan.algebra)
        .find_map(|plan| {
            let left = exact_projection_subject(plan.equation.left())?;
            let right = exact_projection_subject(plan.equation.right())?;
            let parameter = match (&left.root, left.version, &right.root, right.version) {
                (
                    ContentPlaceRoot::Result,
                    ContentPlaceVersion::Current,
                    ContentPlaceRoot::Parameter { symbol, .. },
                    ContentPlaceVersion::Entry,
                ) if projection_domain(plan.equation.left()) == Some(result_domain) => *symbol,
                (
                    ContentPlaceRoot::Parameter { symbol, .. },
                    ContentPlaceVersion::Entry,
                    ContentPlaceRoot::Result,
                    ContentPlaceVersion::Current,
                ) if projection_domain(plan.equation.right()) == Some(result_domain) => *symbol,
                _ => return None,
            };
            parameters
                .iter()
                .find(|candidate| candidate.symbol == parameter)
                .map(|candidate| candidate.name.as_str())
        })
}

fn exact_projection_subject(term: &ContentConservationTerm) -> Option<&ContentStructuralPlace> {
    let ContentConservationTerm::Projection { subject, .. } = term else {
        return None;
    };
    subject.segments.is_empty().then_some(subject)
}

fn projection_domain(term: &ContentConservationTerm) -> Option<SemanticDomainId> {
    let ContentConservationTerm::Projection {
        semantic_domain, ..
    } = term
    else {
        return None;
    };
    Some(*semantic_domain)
}

fn compatible_content(left: &ContentProjectionPlan, right: &ContentProjectionPlan) -> bool {
    left.algebra == right.algebra
}

fn append_type_domains(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    domains: &mut Vec<DomainApplication>,
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            append_type_domains(program, *referee, domains);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            append_type_domains(program, *base_type, domains);
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    push_unique_domain_application(
                        domains,
                        DomainApplication {
                            symbol: domain.symbol,
                            semantic_domain: domain.semantic_id,
                        },
                    );
                }
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DomainApplication {
    symbol: SymbolHandle,
    semantic_domain: SemanticDomainId,
}

fn nominal_domain_application(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<DomainApplication> {
    let definition = program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)?;
    definition
        .semantic_id
        .is_valid()
        .then_some(DomainApplication {
            symbol,
            semantic_domain: definition.semantic_id,
        })
}

fn type_contains_reference(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_contains_reference(program, *base_type)
        }
        _ => false,
    }
}

fn expression_is_bare_result(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    matches!(program.expression_table.name_path_members(path.members), [name] if name.as_str() == "result")
}

fn expression_names_parameter(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameter: &StateParameter,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    matches!(program.expression_table.name_path_members(path.members), [name]
        if path.symbol == parameter.symbol || name.as_str() == parameter.name.as_str())
}

fn push_unique_domain_application(domains: &mut Vec<DomainApplication>, domain: DomainApplication) {
    if domain.symbol.is_valid() && domain.semantic_domain.is_valid() && !domains.contains(&domain) {
        domains.push(domain);
    }
}

fn domain_name(program: &TypedTrees, symbol: SymbolHandle) -> &str {
    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == symbol)
        .map(|domain| domain.name.as_str())
        .unwrap_or("<unknown domain>")
}
