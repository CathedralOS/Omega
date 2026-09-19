//! Crash contract capsules, published crash plans, buckets and sites.

use crate::facts::canonical_encoding::{
    encode_contract_expression_canonical, encode_contract_fact_canonical,
    encode_contract_set_canonical, encode_type_spelling,
};
use crate::facts::crash_calls;
use typed_trees::TypedTrees;

pub(crate) fn build_crash_contract_capsules(
    program: &TypedTrees,
    content_conservation: &[validation::ContentConservationSourcePlan],
    operators: &checked_trees::CheckedOperatorFacts,
) -> Vec<checked_trees::CrashContractCapsule> {
    let mut signatures = Vec::new();
    for machine in program.machines() {
        for (owner_symbol, target_state, signature) in
            crate::proof::machine_parameter_evidence_signatures(
                program,
                program.machine_type_parameters(machine),
            )
        {
            // The binder symbol is the callable target inside the generic
            // body. A nominal requirement symbol remains authority metadata;
            // it is never a second alias for the parameter call target.
            signatures.push((owner_symbol, target_state, signature));
        }
    }
    for definition in program.data_definitions() {
        signatures.extend(crate::proof::machine_parameter_evidence_signatures(
            program,
            program.data_type_parameters(definition),
        ));
    }
    for definition in program.domain_definitions() {
        signatures.extend(crate::proof::machine_parameter_evidence_signatures(
            program,
            program.domain_type_parameters(definition),
        ));
    }
    for definition in program.traits() {
        signatures.extend(crate::proof::machine_parameter_evidence_signatures(
            program,
            program.trait_type_parameters(definition),
        ));
        for signature in program.trait_machine_signatures(definition) {
            signatures.extend(crate::proof::machine_parameter_evidence_signatures(
                program,
                program.state_signature_type_parameters(signature),
            ));
            signatures.push((definition.symbol, signature.symbol, signature));
        }
    }

    let mut capsules = signatures
        .into_iter()
        .map(|(target_machine, target_state, signature)| {
            let parameters = program.state_signature_parameters(signature);
            let parameter_names = parameters
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect::<Vec<_>>();
            let contracts = program.state_signature_contracts(signature);
            let published = build_published_crash_buckets(
                program,
                contracts,
                &parameter_names,
                content_conservation,
                Some(CrashContractOwner::Signature(signature)),
                Some(operators),
                &[],
            );
            let crash = checked_trees::CrashPlan::published_ceiling(published.clone());

            let published_service_names = program
                .service_reach_rows
                .services(signature.service_reach_row)
                .iter()
                .filter_map(|service| program.service_reaches.definition(*service))
                .map(|definition| definition.name.clone())
                .collect::<Vec<_>>();
            let published_invocations =
                validation::declared_signature_invocations(program, signature)
                    .into_iter()
                    .map(|invocation| match invocation {
                        flow_effects::InvocationTarget::Parameter(index) => {
                            format!("parameter:{index}")
                        }
                        flow_effects::InvocationTarget::Service(symbol) => program
                            .traits()
                            .iter()
                            .find(|definition| definition.symbol == symbol)
                            .map(|definition| format!("service:{}", definition.name))
                            .unwrap_or_else(|| format!("service:#{}", symbol.arena_index())),
                    })
                    .collect::<Vec<_>>();

            let generic_binders = program
                .state_signature_type_parameters(signature)
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    (parameter.name.as_str().to_owned(), format!("$G{index}"))
                })
                .collect::<Vec<_>>();
            let mut callable_shape = vec![0xa0];
            for parameter in parameters {
                callable_shape.push(u8::from(parameter.is_self));
                callable_shape.push(u8::from(parameter.is_mutable));
                callable_shape.push(u8::from(parameter.is_const));
                encode_type_spelling(
                    &program.display_type_reference(parameter.type_reference),
                    &generic_binders,
                    &mut callable_shape,
                );
            }
            callable_shape.push(0xaf);
            encode_type_spelling(
                &program.display_type_reference(signature.return_type),
                &generic_binders,
                &mut callable_shape,
            );
            for contract in encode_contract_set_canonical(
                program,
                contracts,
                &parameter_names,
                content_conservation,
                &[0xae],
                true,
                false,
            ) {
                callable_shape.extend(contract);
                callable_shape.push(0xad);
            }
            let canonical_facts = vec![callable_shape];
            let termination = language_semantics::TerminationInterface::Published(
                signature.termination_guarantee.clone(),
            );
            let identity = checked_trees::contract_identity(
                language_semantics::MachineSupplyMode::Requirement,
                &published_service_names,
                language_semantics::SynchronousInvocationInterface::PublishedCeiling,
                &published_invocations,
                language_semantics::SuspensionInterface::PublishedMaySuspend(signature.suspends),
                language_semantics::BlockingInterface::PublishedMayBlock(signature.blocks),
                &crash,
                &termination,
                &canonical_facts,
            );
            checked_trees::CrashContractCapsule::new_with_commitment(
                target_machine,
                target_state,
                identity.report_fingerprint,
                identity.commitment,
                published,
            )
            .with_operational_envelope(
                published_service_names,
                published_invocations,
                signature.suspends,
                signature.blocks,
                signature.termination_guarantee.clone(),
            )
        })
        .collect::<Vec<_>>();
    capsules.sort_by_key(|capsule| {
        (
            capsule.target_machine().arena_index(),
            capsule.target_machine().generation(),
            capsule.target_state().arena_index(),
            capsule.target_state().generation(),
        )
    });
    capsules.dedup();
    capsules
}

pub(crate) fn encode_signature_contract_kind(
    kind: &typed_trees::signature::SignatureContractKind,
    output: &mut Vec<u8>,
) {
    match kind {
        typed_trees::signature::SignatureContractKind::Requires => output.push(1),
        typed_trees::signature::SignatureContractKind::Ensures => output.push(2),
        typed_trees::signature::SignatureContractKind::EnsuresForResultCase {
            result_data,
            result_case,
        } => {
            output.push(3);
            output.extend_from_slice(&result_data.arena_index().to_le_bytes());
            output.extend_from_slice(&result_case.arena_index().to_le_bytes());
        }
        typed_trees::signature::SignatureContractKind::Crashes { cause } => {
            output.push(4);
            output.push(match cause {
                typed_trees::signature::CrashCause::Trap => 1,
                typed_trees::signature::CrashCause::Abort => 2,
            });
        }
    }
}

pub(crate) fn build_published_crash_plan(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> checked_trees::CrashPlan {
    let structural_runtime_requirements =
        build_structural_runtime_requirements(program, machine, operators, exact_integer_casts);
    let published = build_published_crash_buckets(
        program,
        program.machine_contracts(machine),
        parameter_names,
        content_conservation,
        Some(CrashContractOwner::Machine(machine)),
        Some(operators),
        exact_integer_casts,
    );
    let plan = (if machine.is_public
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !published.is_empty()
    {
        checked_trees::CrashPlan::published_ceiling(published)
    } else {
        checked_trees::CrashPlan::default()
    })
    .with_structural_runtime_requirements(structural_runtime_requirements);
    let checked_sites = build_checked_crash_sites(program, machine, &plan);
    plan.with_checked_sites(checked_sites)
        .expect("one checked crash cause occupies each transition site")
}

fn build_structural_runtime_requirements(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<Vec<checked_trees::CheckedBooleanExpression>> {
    let entry = program.machine_states(machine).first()?;
    let mut requirements = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(entry))
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .map(|fact| {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                return None;
            };
            crate::values::lower_machine_entry_boolean_expression(
                program,
                operators,
                machine,
                *expression,
                exact_integer_casts,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    requirements.extend(
        crate::values::lower_integer_parameter_range_requirements(program, machine)
            .into_iter()
            .collect::<Option<Vec<_>>>()?,
    );
    requirements.extend(
        crate::facts::where_requirements::machine_entry_where_requirements(
            program, machine, operators,
        ),
    );
    Some(requirements)
}

pub(crate) fn derive_authored_machine_crash_buckets(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<checked_trees::CrashRouteBucket> {
    let parameter_names = program
        .machine_states(machine)
        .first()
        .map(|entry| {
            program
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let conservation = validation::build_content_conservation_plans(program);
    build_published_crash_buckets(
        program,
        program.machine_contracts(machine),
        &parameter_names,
        &conservation,
        Some(CrashContractOwner::Machine(machine)),
        None,
        &[],
    )
}

pub(crate) fn derive_authored_signature_crash_buckets(
    program: &TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<checked_trees::CrashRouteBucket> {
    let parameter_names = program
        .state_signature_parameters(signature)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let conservation = validation::build_content_conservation_plans(program);
    build_published_crash_buckets(
        program,
        program.state_signature_contracts(signature),
        &parameter_names,
        &conservation,
        None,
        None,
        &[],
    )
}

/// An operator declaration's published buckets. Each guarded route carries
/// its structured scalar form over the operator's own formal parameters
/// (dense scalar position, the telescope Terminal operation crash contracts
/// read as formal `k + 1`); a route the operator reader cannot structure
/// keeps its identity only, so downstream lowering still fails closed on it.
pub(crate) fn derive_authored_operator_crash_buckets(
    program: &TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
    operators: &checked_trees::CheckedOperatorFacts,
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> Vec<checked_trees::CrashRouteBucket> {
    let parameter_names = program
        .operator_parameters(operator)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect::<Vec<_>>();
    build_published_crash_buckets(
        program,
        program
            .signature_contracts
            .span_or_empty(operator.contracts),
        &parameter_names,
        content_conservation,
        Some(CrashContractOwner::Operator(operator)),
        Some(operators),
        &[],
    )
}

enum CrashContractOwner<'program> {
    Machine(&'program typed_trees::machine::Machine),
    Signature(&'program typed_trees::signature::StateSignature),
    Operator(&'program typed_trees::operator::OperatorDefinition),
}

fn build_published_crash_buckets(
    program: &TypedTrees,
    contracts: &[typed_trees::signature::SignatureContract],
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    owner: Option<CrashContractOwner<'_>>,
    operators: Option<&checked_trees::CheckedOperatorFacts>,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Vec<checked_trees::CrashRouteBucket> {
    pub(crate) use std::collections::BTreeMap;

    #[derive(Default)]
    struct Bucket {
        unconditional: bool,
        routes: Vec<checked_trees::CrashPredicateIdentity>,
    }

    let mut buckets = BTreeMap::<checked_trees::CrashCause, Bucket>::new();
    for contract in contracts {
        let typed_trees::signature::SignatureContractKind::Crashes { cause } = &contract.kind
        else {
            continue;
        };
        let cause = match cause {
            typed_trees::signature::CrashCause::Trap => checked_trees::CrashCause::Trap,
            typed_trees::signature::CrashCause::Abort => checked_trees::CrashCause::Abort,
        };
        let bucket = buckets.entry(cause).or_default();
        let facts = program.proof_facts.span_or_empty(contract.facts);
        if facts.is_empty() || facts.iter().any(|fact| is_true_crash_route(program, fact)) {
            bucket.unconditional = true;
            continue;
        }
        for fact in facts {
            let mut route = Vec::new();
            encode_contract_fact_canonical(
                program,
                fact,
                parameter_names,
                content_conservation,
                false,
                &mut route,
            );
            let identity = match fact {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    let structured = crash_calls::crash_predicate_from_expression(
                        program,
                        *expression,
                        parameter_names,
                        Some(content_conservation),
                    );
                    let scalar = owner
                        .as_ref()
                        .zip(operators)
                        .and_then(|(owner, operators)| match owner {
                            CrashContractOwner::Machine(machine) => {
                                crate::values::lower_machine_entry_boolean_expression(
                                    program,
                                    operators,
                                    machine,
                                    *expression,
                                    exact_integer_casts,
                                )
                            }
                            CrashContractOwner::Signature(signature) => {
                                crate::values::lower_signature_crash_contract_expression(
                                    program,
                                    operators,
                                    signature,
                                    *expression,
                                )
                            }
                            CrashContractOwner::Operator(operator) => {
                                crate::values::lower_operator_crash_contract_expression(
                                    program,
                                    operators,
                                    operator,
                                    *expression,
                                )
                            }
                        });
                    let identity = if let Some(scalar) = scalar {
                        checked_trees::CrashPredicateIdentity::from_expression_and_scalar(
                            structured, scalar,
                        )
                    } else {
                        checked_trees::CrashPredicateIdentity::from_expression(structured)
                    };
                    debug_assert_eq!(identity.canonical_bytes(), route);
                    identity
                }
                _ => checked_trees::CrashPredicateIdentity::from_canonical_bytes(route),
            };
            bucket.routes.push(identity);
        }
    }

    buckets
        .into_iter()
        .map(|(cause, mut bucket)| {
            let alternative_guards = if bucket.unconditional {
                vec![checked_trees::CrashRouteGuard::Truth]
            } else {
                bucket.routes.sort();
                bucket.routes.dedup();
                bucket
                    .routes
                    .into_iter()
                    .map(checked_trees::CrashRouteGuard::Predicate)
                    .collect()
            };
            checked_trees::CrashRouteBucket::new(cause, alternative_guards)
                .expect("an authored crash bucket has a canonical nonempty route set")
        })
        .collect()
}

fn build_checked_crash_sites(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    crash_plan: &checked_trees::CrashPlan,
) -> Vec<checked_trees::CheckedCrashSite> {
    let mut sites = Vec::new();
    for state in program.machine_states(machine) {
        for (statement_ordinal, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            let typed_trees::statement::StatementNode::Transition(transition) = statement else {
                continue;
            };
            let typed_trees::statement::TransitionExit::Crash(cause) = transition.exit else {
                continue;
            };
            let cause = match cause {
                typed_trees::signature::CrashCause::Trap => checked_trees::CrashCause::Trap,
                typed_trees::signature::CrashCause::Abort => checked_trees::CrashCause::Abort,
            };
            // An unconditional same-cause route covers every possible path
            // guard. Guarded buckets join only after path-conditioned
            // entailment exists.
            let guard_covering_buckets = crash_plan
                .published_with_ids()
                .filter_map(|(id, bucket)| {
                    (bucket.cause() == cause && bucket.is_unconditional()).then_some(id)
                })
                .collect();
            sites.push(checked_trees::CheckedCrashSite::new(
                checked_trees::CrashSiteLocation::new(
                    state.symbol,
                    u32::try_from(statement_ordinal)
                        .expect("state-local statement ordinal exceeds checked identity range"),
                ),
                cause,
                guard_covering_buckets,
                Vec::new(),
            ));
        }
    }
    sites
}

pub(crate) fn is_true_crash_route(
    program: &TypedTrees,
    fact: &typed_trees::domain::ProofFact,
) -> bool {
    matches!(
        fact,
        typed_trees::domain::ProofFact::Expression(expression)
            if matches!(
                program.expression_table.expression(*expression),
                typed_trees::expression::ExpressionNode::Boolean(true)
            )
    )
}

/// Canonical identity of one body-derived path predicate in the namespace of
/// a machine's published crash routes. This deliberately shares the exact
/// encoder used by [`build_published_crash_plan`]: checked guard coverage may
/// join a source expression to a published bucket only after both normalize to
/// the same source-handle-free bytes.
pub(crate) fn canonical_crash_path_predicate(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    negated: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> checked_trees::CrashPredicateIdentity {
    let expression = crash_calls::crash_predicate_from_expression(
        program,
        expression,
        parameter_names,
        Some(content_conservation),
    );
    let expression = if negated {
        checked_trees::CrashPredicateExpression::Unary {
            operator: typed_trees::expression::UnaryOperator::LogicalNot as u8,
            operand: Box::new(expression),
        }
    } else {
        expression
    };
    checked_trees::CrashPredicateIdentity::from_expression(expression)
}

/// Canonical identity of a checker-derived binary predicate assembled from
/// existing typed operands. This uses the published crash-route encoder but
/// never rewrites the published contract itself.
pub(crate) fn canonical_crash_binary_path_predicate(
    program: &TypedTrees,
    operator: typed_trees::expression::BinaryOperator,
    left: typed_trees::expression::ExpressionHandle,
    right: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> checked_trees::CrashPredicateIdentity {
    let left = crash_calls::crash_predicate_from_expression(
        program,
        left,
        parameter_names,
        Some(content_conservation),
    );
    let right = crash_calls::crash_predicate_from_expression(
        program,
        right,
        parameter_names,
        Some(content_conservation),
    );
    checked_trees::CrashPredicateIdentity::from_expression(
        checked_trees::CrashPredicateExpression::Binary {
            operator: operator as u8,
            left: Box::new(left),
            right: Box::new(right),
        },
    )
}

/// Canonical source-handle-free identity for one operand participating in a
/// checker-derived crash-predicate relation. This is an internal join key;
/// only complete predicate identities enter checked crash plans.
pub(crate) fn canonical_crash_operand_identity(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> checked_trees::CrashPredicateIdentity {
    let mut bytes = vec![0x6f]; // checker-private operand namespace
    encode_contract_expression_canonical(
        program,
        expression,
        parameter_names,
        content_conservation,
        &mut bytes,
    );
    checked_trees::CrashPredicateIdentity::from_canonical_bytes(bytes)
}
