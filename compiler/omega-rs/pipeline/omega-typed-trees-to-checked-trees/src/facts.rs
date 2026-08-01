use crate::borrow::build_borrow_facts;
use crate::capabilities::build_capability_facts;
use crate::flow::{build_domain_facts, build_flow_facts_with_service_reaches};
use crate::invariants::build_invariant_facts;
use crate::operators::{build_operator_facts, select_pending_domain_operator_meanings};
use crate::proof::build_proof_facts_with_operators;
use crate::semantic::build_semantic_facts;
use crate::values::build_value_facts;
use omega_checked_trees::CheckFacts;
use omega_effects::OperationalPlan;
use omega_proof::obligations::ProofPlan;
use omega_typed_trees::TypedTrees;

mod carry;
mod index_compatibility;

pub(crate) fn build_check_facts(
    program: &TypedTrees,
    proof_plan: &ProofPlan<'_>,
    operations: OperationalPlan,
) -> Result<CheckFacts, Vec<omega_core::diagnostics::Diagnostic>> {
    let borrow = build_borrow_facts(program);
    let values = build_value_facts(program);
    let mut operators = build_operator_facts(program, &values);
    let proof = build_proof_facts_with_operators(program, proof_plan, &borrow, &operators);
    let invariants = build_invariant_facts(program);
    let mut semantic = build_semantic_facts(program, &proof);
    let domains = build_domain_facts(program, &semantic);
    let service_reach_inference = omega_effects::infer_service_reaches(program, &operations);
    let flow = build_flow_facts_with_service_reaches(
        program,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
        &service_reach_inference,
    );
    let index_compatibility = index_compatibility::build_index_compatibility_facts(
        program, &operators, &semantic, &flow,
    )?;
    // Domain-owned meanings are selected only from declarations, mints, and
    // signature `requires`; the selector accepts no flow/fact environment.
    select_pending_domain_operator_meanings(program, &mut operators);
    let capabilities = build_capability_facts(program, &service_reach_inference, &flow);
    // TPR3 slice 4: the checker-established termination summaries (built
    // from the same pure functions the termination CHECK uses -- facts and
    // diagnostics cannot disagree).
    let termination = crate::checks::termination::build_termination_facts(program);
    // EFX: the durable service-reach fixed point is built independently from
    // resolved boundary-trait identities and stored as a first-class checked
    // root with grouped machine/state/call arenas.
    let service_reaches = build_service_reach_facts(program, service_reach_inference);
    // STR4 checked plans, slice 2: semantic-domain commitments per machine.
    let qualifications = build_qualification_facts(program);
    // STR4 checked plans: the normalized machine contracts (published
    // halves + fingerprint; prover-independent by construction).
    let contract_plans = build_contract_plans(program, &service_reaches, &operations);
    // CRY1: materialize the effective structural policy once in the checked
    // fact layer; authored clauses remain minimum promises on typed data.
    let carry = carry::build_carry_facts(program);

    Ok(CheckFacts::with_roots(
        semantic,
        borrow,
        proof,
        values,
        invariants,
        domains,
        operators,
        operations,
        capabilities,
        flow,
        index_compatibility,
        termination,
        service_reaches,
        qualifications,
        contract_plans,
        carry,
    ))
}

/// STR4 checked plans (machine_taxonomy.md): assemble each machine's
/// normalized contract plan from the published halves already carried on
/// the records (supply mode, service/operational ceilings, published termination),
/// with a deterministic fingerprint over them. Only DECLARED material
/// enters -- acceptance 8 (a stronger prover cannot change an exported
/// contract ID) holds by construction.
fn build_contract_plans(
    program: &TypedTrees,
    service_reaches: &omega_checked_trees::ServiceReachFacts,
    operations: &OperationalPlan,
) -> omega_checked_trees::MachineContractPlans {
    let mut machines = Vec::new();
    let frame_resolver = omega_validation::CallFrameResolver::new(program);
    let invocation_inference = omega_effects::infer_synchronous_invocations(program);
    for machine in program.machines() {
        let service_fact = service_reaches.for_machine(machine.symbol);
        let published_service_row = machine.service_reach_row;
        let published_service_names = service_reaches
            .rows
            .services(published_service_row)
            .iter()
            .filter_map(|service| service_reaches.services.definition(*service))
            .map(|definition| definition.name.clone())
            .collect::<Vec<_>>();
        let publishes_service_contract = machine.supply_mode
            != omega_core::semantics::MachineSupplyMode::CheckedBody
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty();
        let service_reach = omega_core::semantics::ServiceReachPlan {
            interface: if publishes_service_contract {
                omega_core::semantics::ServiceReachInterface::PublishedCeiling(
                    published_service_row,
                )
            } else {
                omega_core::semantics::ServiceReachInterface::InternalInferred
            },
            checked_inferred: service_fact
                .map(|fact| fact.inferred_transitive)
                .unwrap_or(omega_core::semantics::ServiceReachRowTable::EMPTY_ROW),
        };
        let invocation_summary = invocation_inference.for_machine(machine.symbol);
        let canonical_invocation = |target: omega_effects::InvocationTarget| match target {
            omega_effects::InvocationTarget::Parameter(index) => format!("parameter:{index}"),
            omega_effects::InvocationTarget::Service(symbol) => program
                .traits()
                .iter()
                .find(|definition| definition.symbol == symbol)
                .map(|definition| format!("service:{}", definition.name))
                .unwrap_or_else(|| format!("service:#{}", symbol.arena_index())),
        };
        let mut published_invocations = invocation_summary
            .into_iter()
            .flat_map(|summary| summary.published.iter().copied())
            .map(canonical_invocation)
            .collect::<Vec<_>>();
        published_invocations.sort_unstable();
        published_invocations.dedup();
        let mut checked_invocations = invocation_summary
            .into_iter()
            .flat_map(|summary| summary.inferred_transitive.iter().copied())
            .map(canonical_invocation)
            .collect::<Vec<_>>();
        checked_invocations.sort_unstable();
        checked_invocations.dedup();
        let publishes_invocations = machine.supply_mode
            != omega_core::semantics::MachineSupplyMode::CheckedBody
            || !program.machine_invokes(machine).is_empty();
        let synchronous_invocation = omega_core::semantics::SynchronousInvocationPlan {
            interface: if publishes_invocations {
                omega_core::semantics::SynchronousInvocationInterface::PublishedCeiling
            } else {
                omega_core::semantics::SynchronousInvocationInterface::InternalInferred
            },
            published: published_invocations.clone(),
            checked_inferred: checked_invocations,
        };
        let termination = machine.termination_plan.interface.clone();
        // Slice 2: the declared requires/ensures facts in a CANONICAL,
        // clause-order-independent encoding (each fact serializes to a
        // stable byte form; the set sorts before folding). Parameter
        // RENAMES change the identity in v1 -- positional normalization is
        // the recorded follow-up.
        let mut canonical_facts: Vec<Vec<u8>> = Vec::new();
        // The callable shape is contract identity too. A selected static
        // machine changing parameter mode/type, result type, or state surface
        // must invalidate every specialization that recorded its contract ID.
        // Encode generic binders positionally so a rename remains invisible.
        let generic_binders: Vec<(String, String)> = program
            .machine_type_parameters(machine)
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.name.as_str().to_owned(), format!("$G{index}")))
            .collect();
        for state in program.machine_states(machine) {
            let mut encoded = vec![0xa0];
            let state_parameters = program.state_parameters(state);
            for parameter in state_parameters {
                encoded.push(u8::from(parameter.is_self));
                encoded.push(u8::from(parameter.is_mutable));
                encoded.push(u8::from(parameter.is_const));
                encode_type_spelling(
                    &program.display_type_reference(parameter.type_reference),
                    &generic_binders,
                    &mut encoded,
                );
            }
            encoded.push(0xaf);
            encode_type_spelling(
                &program.display_type_reference(state.return_type),
                &generic_binders,
                &mut encoded,
            );
            let parameter_names = state_parameters
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect::<Vec<_>>();
            let mut state_contracts = Vec::new();
            for contract in program.state_contracts(state) {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let mut contract_bytes = vec![0xae];
                    contract_bytes.push(match contract.kind {
                        omega_typed_trees::signature::SignatureContractKind::Requires => 1,
                        omega_typed_trees::signature::SignatureContractKind::Ensures => 2,
                        omega_typed_trees::signature::SignatureContractKind::Boundary => 3,
                    });
                    match fact {
                        omega_typed_trees::domain::ProofFact::Expression(expression) => {
                            contract_bytes.push(1);
                            encode_expression_canonical(
                                program,
                                *expression,
                                &parameter_names,
                                &mut contract_bytes,
                            );
                        }
                        omega_typed_trees::domain::ProofFact::Membership(membership) => {
                            contract_bytes.push(2);
                            encode_expression_canonical(
                                program,
                                membership.value,
                                &parameter_names,
                                &mut contract_bytes,
                            );
                            contract_bytes.push(0);
                            for member in program.domain_path_members(membership.domain) {
                                contract_bytes.extend(member.as_str().as_bytes());
                                contract_bytes.push(b':');
                            }
                        }
                    }
                    state_contracts.push(contract_bytes);
                }
            }
            state_contracts.sort();
            for contract in state_contracts {
                encoded.extend(contract);
                encoded.push(0xad);
            }
            canonical_facts.push(encoded);
        }
        // Positional parameter normalization: a contract fact naming the
        // machine's Nth parameter encodes as P<N>, so RENAMES never change
        // the identity (the substitutable contract is positional).
        let parameter_names: Vec<String> = program
            .machine_states(machine)
            .first()
            .map(|entry| {
                program
                    .state_parameters(entry)
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        for contract in program.machine_contracts(machine) {
            let kind_tag: u8 = match contract.kind {
                omega_typed_trees::signature::SignatureContractKind::Requires => 1,
                omega_typed_trees::signature::SignatureContractKind::Ensures => 2,
                omega_typed_trees::signature::SignatureContractKind::Boundary => 3,
            };
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let mut encoded = vec![kind_tag];
                match fact {
                    omega_typed_trees::domain::ProofFact::Expression(expression) => {
                        encoded.push(1);
                        encode_expression_canonical(
                            program,
                            *expression,
                            &parameter_names,
                            &mut encoded,
                        );
                    }
                    omega_typed_trees::domain::ProofFact::Membership(membership) => {
                        encoded.push(2);
                        encoded.extend(
                            program
                                .expression_table
                                .display_name(membership.value)
                                .as_bytes(),
                        );
                        encoded.push(0);
                        for member in program
                            .expression_table
                            .name_path_members(membership.domain)
                        {
                            encoded.extend(member.as_str().as_bytes());
                            encoded.push(b':');
                        }
                    }
                }
                canonical_facts.push(encoded);
            }
        }
        canonical_facts.sort();
        let operational_summary = operations
            .machines()
            .iter()
            .find(|summary| summary.symbol == machine.symbol);
        let publishes_operational_contract =
            machine.supply_mode != omega_core::semantics::MachineSupplyMode::CheckedBody;
        let checked_may_suspend =
            operational_summary.is_some_and(|summary| summary.transitive_may_suspend);
        let checked_may_block =
            operational_summary.is_some_and(|summary| summary.transitive_may_block);
        let suspension = omega_core::semantics::SuspensionPlan {
            interface: if publishes_operational_contract || machine.suspends {
                omega_core::semantics::SuspensionInterface::PublishedMaySuspend(machine.suspends)
            } else {
                omega_core::semantics::SuspensionInterface::InternalInferred
            },
            checked_may_suspend,
        };
        let blocking = omega_core::semantics::BlockingPlan {
            interface: if publishes_operational_contract || machine.blocks {
                omega_core::semantics::BlockingInterface::PublishedMayBlock(machine.blocks)
            } else {
                omega_core::semantics::BlockingInterface::InternalInferred
            },
            checked_may_block,
        };
        let fingerprint = omega_checked_trees::contract_fingerprint(
            machine.supply_mode,
            &published_service_names,
            synchronous_invocation.interface,
            &published_invocations,
            suspension.interface,
            blocking.interface,
            &termination,
            &canonical_facts,
        );
        let inferred_write_frames = program
            .machine_states(machine)
            .iter()
            .map(|state| omega_checked_trees::StateWriteFramePlan {
                state: state.symbol,
                frame: frame_resolver
                    .as_ref()
                    .map_or_else(omega_facts::NormalizedWriteFrame::opaque, |resolver| {
                        resolver.inferred_state_write_frame(machine, state)
                    }),
            })
            .collect();
        machines.push(omega_checked_trees::MachineContractPlan {
            machine: machine.symbol,
            supply_mode: machine.supply_mode,
            service_reach,
            synchronous_invocation,
            suspension,
            blocking,
            termination,
            inferred_write_frames,
            fingerprint,
        });
    }
    omega_checked_trees::MachineContractPlans {
        machines,
        task_activations: Vec::new(),
    }
}

fn encode_type_spelling(text: &str, binders: &[(String, String)], output: &mut Vec<u8>) {
    let mut word = String::new();
    let flush = |word: &mut String, output: &mut Vec<u8>| {
        if word.is_empty() {
            return;
        }
        if let Some((_, replacement)) = binders.iter().find(|(name, _)| name == word) {
            output.extend(replacement.as_bytes());
        } else {
            output.extend(word.as_bytes());
        }
        word.clear();
    };
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            word.push(character);
        } else {
            flush(&mut word, output);
            output.extend(character.to_string().as_bytes());
        }
    }
    flush(&mut word, output);
    output.push(0);
}

/// STR4 checked plans, slice 2 (decision 19): collect each machine's
/// semantic-domain COMMITMENTS -- v1 walks its statements' expressions for
/// arithmetic-policy casts (`x as u8 in Saturating`; the compiler-blessed
/// closed semantic-facet subset) and normalizes the policy to its FIXED
/// SemanticDomainTable identity. Sorted + deduped; cast-free machines carry
/// no entry.
fn build_qualification_facts(program: &TypedTrees) -> omega_checked_trees::QualificationFacts {
    use omega_checked_trees::VacuousQualificationUse;
    use omega_core::semantics::SemanticDomainTable;
    use omega_core::symbols::SymbolHandle;
    use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use std::collections::HashSet;

    fn domain_is_vacuous(
        program: &TypedTrees,
        domain_symbol: SymbolHandle,
        stack: &mut Vec<SymbolHandle>,
    ) -> bool {
        if !domain_symbol.is_valid() || stack.contains(&domain_symbol) {
            return false;
        }
        let Some(domain) = program
            .domain_definitions()
            .iter()
            .find(|candidate| candidate.symbol == domain_symbol)
        else {
            return false;
        };
        if let Some(alias) = domain.alias.as_ref() {
            if alias.constituents.is_empty() {
                return false;
            }
            stack.push(domain_symbol);
            let vacuous = alias
                .constituents
                .iter()
                .all(|constituent| domain_is_vacuous(program, constituent.domain_symbol, stack));
            stack.pop();
            return vacuous;
        }
        !domain.predicate_body.is_present() && domain.establishment_routes.is_empty()
    }

    fn collect_casts(
        program: &TypedTrees,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: u32,
        expression: ExpressionHandle,
        committed: &mut Vec<omega_core::semantics::SemanticDomainId>,
        vacuous_uses: &mut Vec<VacuousQualificationUse>,
        visited: &mut HashSet<u32>,
    ) {
        if !expression.is_valid() || !visited.insert(expression.arena_index()) {
            return;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Cast(cast) => {
                let policy = match cast.domain {
                    omega_core::arithmetic::ArithmeticDomain::Exact => None,
                    omega_core::arithmetic::ArithmeticDomain::Wrapping => {
                        Some(SemanticDomainTable::WRAPPING)
                    }
                    omega_core::arithmetic::ArithmeticDomain::Saturating => {
                        Some(SemanticDomainTable::SATURATING)
                    }
                    omega_core::arithmetic::ArithmeticDomain::Trapping => {
                        Some(SemanticDomainTable::TRAPPING)
                    }
                };
                if let Some(policy) = policy {
                    committed.push(policy);
                }
                // Declared-domain casts already carry the normalized symbol
                // selected before validation. Checked facts consume that
                // identity directly rather than repeating short-name lookup.
                if cast.semantic_domain_symbol.is_valid()
                    && let Some(domain) = program
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == cast.semantic_domain_symbol)
                {
                    let semantic_id = if cast.semantic_domain_id.is_valid() {
                        cast.semantic_domain_id
                    } else {
                        domain.semantic_id
                    };
                    if semantic_id.is_valid() {
                        committed.push(semantic_id);
                    }
                    if domain_is_vacuous(program, domain.symbol, &mut Vec::new()) {
                        vacuous_uses.push(VacuousQualificationUse {
                            machine,
                            state,
                            statement_index,
                            expression,
                            domain: domain.symbol,
                            semantic_domain: semantic_id,
                        });
                    }
                }
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    cast.value,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Binary(binary) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    binary.left,
                    committed,
                    vacuous_uses,
                    visited,
                );
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    binary.right,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Unary(unary) => collect_casts(
                program,
                machine,
                state,
                statement_index,
                unary.operand,
                committed,
                vacuous_uses,
                visited,
            ),
            ExpressionNode::Member(member) => collect_casts(
                program,
                machine,
                state,
                statement_index,
                member.receiver,
                committed,
                vacuous_uses,
                visited,
            ),
            ExpressionNode::Mutable(inner) => collect_casts(
                program,
                machine,
                state,
                statement_index,
                *inner,
                committed,
                vacuous_uses,
                visited,
            ),
            ExpressionNode::Indexed(indexed) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    indexed.collection,
                    committed,
                    vacuous_uses,
                    visited,
                );
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    indexed.index,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Range(range) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    range.start,
                    committed,
                    vacuous_uses,
                    visited,
                );
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    range.end,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Call(call) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    call.receiver,
                    committed,
                    vacuous_uses,
                    visited,
                );
                for argument in program.expression_table.expression_handles(call.arguments) {
                    collect_casts(
                        program,
                        machine,
                        state,
                        statement_index,
                        *argument,
                        committed,
                        vacuous_uses,
                        visited,
                    );
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    collect_casts(
                        program,
                        machine,
                        state,
                        statement_index,
                        field.value,
                        committed,
                        vacuous_uses,
                        visited,
                    );
                }
            }
            ExpressionNode::ArrayLiteral(items) => {
                for item in program.expression_table.expression_handles(*items) {
                    collect_casts(
                        program,
                        machine,
                        state,
                        statement_index,
                        *item,
                        committed,
                        vacuous_uses,
                        visited,
                    );
                }
            }
            _ => {}
        }
    }

    let mut machines = Vec::new();
    let mut vacuous_uses = Vec::new();
    for machine in program.machines() {
        let mut committed = Vec::new();
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                use omega_typed_trees::statement::StatementNode;
                let statement_index =
                    u32::try_from(statement_index).expect("qualification statement index overflow");
                let mut visited = HashSet::new();
                match statement {
                    StatementNode::AssemblyFact(_) => {}
                    StatementNode::Assignment(assignment) => {
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            assignment.target,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            assignment.value,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                    }
                    StatementNode::Expression(expression) => {
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            *expression,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                    }
                    StatementNode::LocalData(local) => {
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            local.initial_value,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            collect_casts(
                                program,
                                machine.symbol,
                                state.symbol,
                                statement_index,
                                *argument,
                                &mut committed,
                                &mut vacuous_uses,
                                &mut visited,
                            );
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if let omega_typed_trees::statement::TransitionGuardNode::When(guard) =
                            &transition.guard
                        {
                            collect_casts(
                                program,
                                machine.symbol,
                                state.symbol,
                                statement_index,
                                *guard,
                                &mut committed,
                                &mut vacuous_uses,
                                &mut visited,
                            );
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            match program.statement_table.transition_target(target) {
                                omega_typed_trees::statement::TransitionTargetNode::Value(
                                    value,
                                ) => collect_casts(
                                    program,
                                    machine.symbol,
                                    state.symbol,
                                    statement_index,
                                    *value,
                                    &mut committed,
                                    &mut vacuous_uses,
                                    &mut visited,
                                ),
                                omega_typed_trees::statement::TransitionTargetNode::Named {
                                    arguments,
                                    ..
                                } => {
                                    for argument in
                                        program.statement_table.expression_handles(*arguments)
                                    {
                                        collect_casts(
                                            program,
                                            machine.symbol,
                                            state.symbol,
                                            statement_index,
                                            *argument,
                                            &mut committed,
                                            &mut vacuous_uses,
                                            &mut visited,
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        committed.sort_by_key(|id| id.0);
        committed.dedup();
        if !committed.is_empty() {
            machines.push(omega_checked_trees::MachineQualifications {
                machine: machine.symbol,
                body_committed: committed,
            });
        }
    }
    omega_checked_trees::QualificationFacts {
        machines,
        vacuous_uses,
        content: omega_checked_trees::ContentProjectionFacts {
            plans: omega_validation::build_content_projection_plans(program),
        },
    }
}

/// Build the boundary-symbol service fixed point without consulting the
/// legacy global effect catalog. A direct boundary-signature call contributes
/// its containing service plus the signature's explicitly reached services.
/// Checked local callees contribute honest inferred bodies; requirements,
/// boundaries, external realizations, and authored checked ceilings contribute
/// their published row.
fn build_service_reach_facts(
    program: &TypedTrees,
    inferred: omega_effects::ServiceReachInferencePlan,
) -> omega_checked_trees::ServiceReachFacts {
    omega_checked_trees::ServiceReachFacts {
        services: program.service_reaches.clone(),
        rows: inferred.rows,
        root_machines: remap_service_reach_span(inferred.root_machines),
        machines: inferred
            .machines
            .map(|machine| omega_checked_trees::MachineServiceReachRows {
                machine: machine.machine,
                published_ceiling: machine.published,
                inferred_direct: machine.inferred_direct,
                inferred_transitive: machine.inferred_transitive,
                effective: machine.effective,
                states: remap_service_reach_span(machine.states),
            }),
        states: inferred
            .states
            .map(|state| omega_checked_trees::StateServiceReachRows {
                state: state.state,
                inferred_direct: state.inferred_direct,
                inferred_transitive: state.inferred_transitive,
                calls: remap_service_reach_span(state.calls),
            }),
        calls: inferred
            .calls
            .map(|call| omega_checked_trees::CallServiceReachRows {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_state: call.target_state,
                target_machine: call.target_machine,
                inferred_direct: call.inferred_direct,
                inferred_transitive: call.inferred_transitive,
            }),
    }
}

fn remap_service_reach_span<From, To>(
    span: omega_core::arena::HandleSpan<From>,
) -> omega_core::arena::HandleSpan<To> {
    let start = span.start();
    omega_core::arena::HandleSpan::from_parts(
        omega_core::arena::Handle::from_parts(start.arena_index(), start.generation()),
        span.count(),
    )
}

/// A stable, spelling-independent byte encoding of a contract fact
/// expression: prefix walk with operator tags, name paths as text, integer
/// literals as text (exact at any magnitude). Deterministic across
/// programs for the same declared clause.
fn encode_expression_canonical(
    program: &TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    out: &mut Vec<u8>,
) {
    use omega_typed_trees::expression::ExpressionNode;
    if !expression.is_valid() {
        out.push(0);
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            out.push(1);
            out.push(binary.operator as u8);
            encode_expression_canonical(program, binary.left, parameter_names, out);
            encode_expression_canonical(program, binary.right, parameter_names, out);
        }
        ExpressionNode::Unary(unary) => {
            out.push(2);
            out.push(unary.operator as u8);
            encode_expression_canonical(program, unary.operand, parameter_names, out);
        }
        ExpressionNode::Integer(value) => {
            out.push(3);
            out.extend(value.text().as_bytes());
            out.push(0);
        }
        ExpressionNode::Boolean(value) => {
            out.push(4);
            out.push(u8::from(*value));
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            // A bare parameter name normalizes to its POSITION -- renames
            // never change the contract identity.
            if let [single] = members
                && let Some(index) = parameter_names
                    .iter()
                    .position(|name| name == single.as_str())
            {
                out.push(9);
                out.extend(
                    u32::try_from(index)
                        .expect("parameter index fits u32")
                        .to_le_bytes(),
                );
                return;
            }
            out.push(5);
            for member in members {
                out.extend(member.as_str().as_bytes());
                out.push(b'.');
            }
            out.push(0);
        }
        ExpressionNode::Member(member) => {
            out.push(6);
            encode_expression_canonical(program, member.receiver, parameter_names, out);
            out.extend(member.member.as_str().as_bytes());
            out.push(0);
        }
        ExpressionNode::Call(call) => {
            out.push(7);
            out.extend(call.target.as_str().as_bytes());
            out.push(0);
            encode_expression_canonical(program, call.receiver, parameter_names, out);
            for argument in program.expression_table.expression_handles(call.arguments) {
                encode_expression_canonical(program, *argument, parameter_names, out);
            }
            out.push(0xfe);
        }
        // Anything else falls back to the display name -- stable per
        // spelling (a conservative widening; refine per-node as shapes
        // arrive in contracts).
        other => {
            let _ = other;
            out.push(8);
            out.extend(program.expression_table.display_name(expression).as_bytes());
            out.push(0);
        }
    }
}
