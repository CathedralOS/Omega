//! Crash contract capsules, published crash plans, buckets and sites.

use crate::facts::crash_calls;
use validation::contract_structure::{
    callable_binders, declared_callable, declared_fact, declared_termination, generic_binders,
    is_true_crash_route, signature_generic_binders,
};

pub(crate) fn build_crash_contract_capsules(
    program: &dyn ::validation::CheckingInput,
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
        for declaration in program.trait_machine_signatures(definition) {
            // Execution replaces an open requirement with its closed family.
            // Package review still publishes the authored generic contract;
            // concrete applications cannot donate that declaration's ceiling.
            // Keep both coordinates, each derived from its own signature.
            for signature in std::iter::once(declaration).chain(
                program
                    .requirement_machine_instances(declaration)
                    .filter(|instance| instance.symbol != declaration.symbol),
            ) {
                signatures.extend(crate::proof::machine_parameter_evidence_signatures(
                    program,
                    program.state_signature_type_parameters(signature),
                ));
                signatures.push((definition.symbol, signature.symbol, signature));
            }
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
                CrashContractOwner::Signature(signature),
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
                            .trait_definition_by_symbol(symbol)
                            .map(|definition| format!("service:{}", definition.name))
                            .unwrap_or_else(|| format!("service:#{}", symbol.arena_index())),
                    })
                    .collect::<Vec<_>>();

            let generics = signature_generic_binders(program, signature);
            let declaration = checked_trees::DeclaredContractStructure {
                callables: vec![declared_callable(
                    program,
                    parameters,
                    signature.return_type,
                    contracts,
                    &generics,
                    content_conservation,
                    true,
                )],
                clauses: Vec::new(),
            };
            let termination = language_semantics::TerminationInterface::Published(
                signature.termination_guarantee.clone(),
            );
            let termination = declared_termination(
                program,
                &termination,
                &callable_binders(&generics, parameters),
            );
            let identity = program.type_identities.with(|table| {
                checked_trees::contract_identity(
                    language_semantics::MachineSupplyMode::Requirement,
                    None,
                    &published_service_names,
                    language_semantics::SynchronousInvocationInterface::PublishedCeiling,
                    &published_invocations,
                    language_semantics::SuspensionInterface::PublishedMaySuspend(
                        signature.suspends,
                    ),
                    language_semantics::BlockingInterface::PublishedMayBlock(signature.blocks),
                    crash.interface(),
                    &termination,
                    &declaration,
                    table,
                )
            });
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
    // Legacy declarations own their entire callable contract. Token-bearing
    // machine signature views are deliberately absent: those keep the ordinary
    // machine contract and its authored operational interfaces.
    capsules.extend(
        program
            .all_operators()
            .filter(|operator| operator.is_boundary)
            .map(|operator| {
                build_operator_crash_contract_capsule(
                    program,
                    operator,
                    content_conservation,
                    operators,
                )
            }),
    );
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

fn build_operator_crash_contract_capsule(
    program: &dyn ::validation::CheckingInput,
    operator: &typed_trees::operator::OperatorDefinition,
    content_conservation: &[validation::ContentConservationSourcePlan],
    operators: &checked_trees::CheckedOperatorFacts,
) -> checked_trees::CrashContractCapsule {
    let parameters = program.operator_parameters(operator);
    let generics = generic_binders(program.operator_type_parameters(operator));
    let declaration = checked_trees::DeclaredContractStructure {
        callables: vec![declared_callable(
            program,
            parameters,
            operator.return_type,
            program.operator_contracts(operator),
            &generics,
            content_conservation,
            true,
        )],
        clauses: Vec::new(),
    };
    let published =
        derive_authored_operator_crash_buckets(program, operator, operators, content_conservation);
    let crash = checked_trees::CrashPlan::published_ceiling(published.clone());

    // Legacy operator syntax has requires/ensures/crashes, but no services,
    // invokes, suspends, blocks or terminates clauses. Its existing admission
    // contract (provider-planning::service_schema::from_typed_operator) gives
    // realizations empty effect ceilings and no termination guarantee. Retain
    // that declaration envelope; neither a selected provider nor an arithmetic
    // spelling can donate effects or a termination proof to the requirement.
    let termination = language_semantics::TerminationGuarantee::NoGuarantee;
    let identity = program.type_identities.with(|table| {
        checked_trees::contract_identity(
            language_semantics::MachineSupplyMode::Requirement,
            None,
            &[],
            language_semantics::SynchronousInvocationInterface::PublishedCeiling,
            &[],
            language_semantics::SuspensionInterface::PublishedMaySuspend(false),
            language_semantics::BlockingInterface::PublishedMayBlock(false),
            crash.interface(),
            &checked_trees::DeclaredTermination::NoGuarantee,
            &declaration,
            table,
        )
    });
    checked_trees::CrashContractCapsule::new_with_commitment(
        operator.symbol,
        operator.symbol,
        identity.report_fingerprint,
        identity.commitment,
        published,
    )
    .with_operational_envelope(Vec::new(), Vec::new(), false, false, termination)
}

pub(crate) fn build_published_crash_plan(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> checked_trees::CrashPlan {
    let mut runtime_membership_subjects = Vec::new();
    let structural_runtime_requirements = build_structural_runtime_requirements(
        program,
        machine,
        operators,
        exact_integer_casts,
        &mut runtime_membership_subjects,
    );
    let structural_runtime_guarantees =
        build_structural_runtime_guarantees(program, machine, operators, exact_integer_casts);
    let published = build_published_crash_buckets(
        program,
        program.machine_contracts(machine),
        parameter_names,
        content_conservation,
        CrashContractOwner::Machine(machine),
        Some(operators),
        exact_integer_casts,
    );
    let plan = (if program.machine_publishes_crash_ceiling(machine) {
        checked_trees::CrashPlan::published_ceiling(published)
    } else {
        checked_trees::CrashPlan::default()
    })
    .with_structural_runtime_requirements(structural_runtime_requirements)
    .with_runtime_membership_subjects(runtime_membership_subjects)
    .with_structural_runtime_guarantees(structural_runtime_guarantees);
    let checked_sites = build_checked_crash_sites(program, machine, &plan);
    plan.with_checked_sites(checked_sites)
        .expect("one checked crash cause occupies each transition site")
}

fn build_structural_runtime_requirements(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    subjects: &mut Vec<checked_trees::CheckedRuntimeMembershipSubject>,
) -> Option<Vec<checked_trees::CheckedBooleanExpression>> {
    let entry = program.machine_states(machine).first()?;
    let facts = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(entry))
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .enumerate();
    let mut requirements = Vec::new();
    for (fact_ordinal, fact) in facts {
        // Structural qualification restatements retain their separate argument
        // presentation; scalar memberships retain their subject and full theory.
        if validation::restates_structural_parameter_qualification(program, entry, fact) {
            continue;
        }
        match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                requirements.push(crate::values::lower_machine_entry_boolean_expression(
                    program,
                    operators,
                    machine,
                    *expression,
                    exact_integer_casts,
                )?)
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                let primitive = validation::scalar_membership_carrier(program, membership)?;
                let subject = crate::values::lower_unit_scalar_argument(
                    program,
                    operators,
                    entry,
                    0,
                    membership.value,
                    primitive,
                )?;
                requirements.extend(validation::scalar_membership_predicates(
                    program, membership, &subject,
                )?);
                subjects.push(checked_trees::CheckedRuntimeMembershipSubject {
                    fact_ordinal: u32::try_from(fact_ordinal).ok()?,
                    membership: *membership,
                    subject,
                });
            }
            typed_trees::domain::ProofFact::Proposition(_) => return None,
        }
    }
    requirements.extend(
        crate::values::lower_scalar_parameter_refinement_predicates(program, machine)
            .into_iter()
            .collect::<Option<Vec<_>>>()?,
    );
    // The declaration predicate roster closes the package: every standing
    // `where` fact on storage an entry parameter reaches, which the caller
    // establishes at the call and the root installation at program entry.
    // Replay splits this derived tail off before rejoining authored clauses.
    requirements.extend(
        validation::machine_declaration_predicates(program, operators, machine)
            .into_iter()
            .map(|row| row.predicate),
    );
    Some(requirements)
}

/// Lower every unbound `ensures` clause of the machine signature into the
/// same structural scalar vocabulary the entry requirements use. The clauses
/// observe post-state parameter storage through the identical authored
/// positions; bound witness lanes and outcome-specific clauses stay outside
/// this capsule because they carry separate evidence plans.
fn build_structural_runtime_guarantees(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<Vec<checked_trees::CheckedBooleanExpression>> {
    program
        .machine_contracts(machine)
        .iter()
        .chain(
            program
                .machine_states(machine)
                .first()
                .into_iter()
                .flat_map(|entry| program.state_contracts(entry)),
        )
        .filter(|contract| {
            contract.kind == typed_trees::signature::SignatureContractKind::Ensures
                && contract.binding.is_none()
        })
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .map(|fact| {
            let expression = fact.as_expression()?;
            crate::values::lower_machine_entry_boolean_expression(
                program,
                operators,
                machine,
                *expression,
                exact_integer_casts,
            )
        })
        .collect()
}

/// The entry state's declared parameter names, in declaration order — the
/// positional row every machine-level contract and crash surface builds.
pub(crate) fn machine_entry_parameter_names(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<String> {
    program
        .machine_states(machine)
        .first()
        .map(|entry| {
            program
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn derive_authored_machine_crash_buckets(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<checked_trees::CrashRouteBucket> {
    let parameter_names = machine_entry_parameter_names(program, machine);
    let conservation = validation::content_conservation_plans(program);
    build_published_crash_buckets(
        program,
        program.machine_contracts(machine),
        &parameter_names,
        &conservation,
        CrashContractOwner::Machine(machine),
        None,
        &[],
    )
}

pub(crate) fn derive_authored_signature_crash_buckets(
    program: &dyn ::validation::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<checked_trees::CrashRouteBucket> {
    let parameter_names = program
        .state_signature_parameters(signature)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let conservation = validation::content_conservation_plans(program);
    build_published_crash_buckets(
        program,
        program.state_signature_contracts(signature),
        &parameter_names,
        &conservation,
        CrashContractOwner::Signature(signature),
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
    program: &dyn ::validation::CheckingInput,
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
        CrashContractOwner::Operator(operator),
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
    program: &dyn ::validation::CheckingInput,
    contracts: &[typed_trees::signature::SignatureContract],
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    owner: CrashContractOwner<'_>,
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
    let parameters = match &owner {
        CrashContractOwner::Machine(machine) => program
            .machine_states(machine)
            .first()
            .map(|entry| program.state_parameters(entry))
            .unwrap_or(&[]),
        CrashContractOwner::Signature(signature) => program.state_signature_parameters(signature),
        CrashContractOwner::Operator(operator) => program.operator_parameters(operator),
    };
    let binders = match &owner {
        CrashContractOwner::Machine(machine) => {
            let generics = generic_binders(program.machine_type_parameters(machine));
            program
                .machine_states(machine)
                .first()
                .map(|entry| callable_binders(&generics, program.state_parameters(entry)))
                .unwrap_or(generics)
        }
        CrashContractOwner::Signature(signature) => callable_binders(
            &signature_generic_binders(program, signature),
            program.state_signature_parameters(signature),
        ),
        CrashContractOwner::Operator(operator) => callable_binders(
            &generic_binders(program.operator_type_parameters(operator)),
            program.operator_parameters(operator),
        ),
    };
    for contract in contracts {
        let typed_trees::signature::SignatureContractKind::Crashes { cause } = &contract.kind
        else {
            continue;
        };
        let cause = checked_trees::CrashCause::from(*cause);
        let bucket = buckets.entry(cause).or_default();
        let facts = program.proof_facts.span_or_empty(contract.facts);
        if facts.is_empty() || facts.iter().any(|fact| is_true_crash_route(program, fact)) {
            bucket.unconditional = true;
            continue;
        }
        for fact in facts {
            let identity = match fact {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    let structured = crash_calls::proof_crash_predicate_from_expression(
                        program,
                        *expression,
                        parameters,
                        parameter_names,
                        Some(content_conservation),
                    );
                    let scalar = operators.and_then(|operators| match &owner {
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
                    if let Some(scalar) = scalar {
                        checked_trees::CrashPredicateIdentity::from_expression_and_scalar(
                            structured, scalar,
                        )
                    } else {
                        checked_trees::CrashPredicateIdentity::from_expression(structured)
                    }
                }
                _ => {
                    let declaration = declared_fact(program, fact, &binders, content_conservation);
                    program.type_identities.with(|table| {
                        checked_trees::CrashPredicateIdentity::from_declared_fact(
                            &declaration,
                            table,
                        )
                    })
                }
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
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    crash_plan: &checked_trees::CrashPlan,
) -> Vec<checked_trees::CheckedCrashSite> {
    let mut sites = Vec::new();
    for state in program.machine_states(machine) {
        for (statement_ordinal, statement) in program
            .statement_table
            .indexed_statements(state.statement_nodes)
        {
            let Some(transition) = statement.as_transition() else {
                continue;
            };
            let typed_trees::statement::TransitionExit::Crash(cause) = transition.exit else {
                continue;
            };
            let cause = checked_trees::CrashCause::from(cause);
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

/// Canonical identity of one body-derived path predicate in the namespace of
/// a machine's published crash routes. This deliberately shares the exact
/// encoder used by [`build_published_crash_plan`]: checked guard coverage may
/// join a source expression to a published bucket only after both normalize to
/// the same source-handle-free bytes.
pub(crate) fn canonical_crash_path_predicate(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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

/// Proof assumptions and published ceilings use the same typed identity
/// producer; executable guard keys remain in their independent namespace.
pub(crate) fn canonical_crash_contract_predicate(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    negated: bool,
    parameters: &[typed_trees::signature::StateParameter],
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> checked_trees::CrashPredicateIdentity {
    let expression = crash_calls::proof_crash_predicate_from_expression(
        program,
        expression,
        parameters,
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

/// Canonical source-handle-free identity for one operand participating in a
/// checker-derived crash-predicate relation. This is an internal join key;
/// only complete predicate identities enter checked crash plans.
pub(crate) fn canonical_crash_operand_identity(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> checked_trees::CrashPredicateIdentity {
    checked_trees::CrashPredicateIdentity::from_operand(
        crash_calls::crash_predicate_from_expression(
            program,
            expression,
            parameter_names,
            Some(content_conservation),
        ),
    )
}
