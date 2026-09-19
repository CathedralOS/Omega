//! Contract plans, resource envelope coverage, mutation facts and closed
//! scalar value contract plans.

use crate::facts::canonical_encoding::{encode_contract_set_canonical, encode_type_spelling};
use crate::facts::crash_plan_facts::{build_crash_contract_capsules, build_published_crash_plan};
use crate::facts::{crash_calls, operator_crashes};
use typed_trees::TypedTrees;

/// STR4 checked plans (wiki/spec/language/machines.md): assemble each machine's
/// normalized contract plan from the published halves already carried on
/// the records (supply mode, service/operational ceilings, published termination),
/// with a deterministic fingerprint over them. Checked service rows include
/// conservative call propagation, without proof-based pruning; a stronger
/// prover therefore cannot change an exported contract ID (acceptance 8).
pub(crate) fn build_contract_plans(
    program: &TypedTrees,
    service_reaches: &checked_trees::ServiceReachFacts,
    synchronous_invocations: &checked_trees::SynchronousInvocationFacts,
    suspensions: &checked_trees::SuspensionFacts,
    blocking: &checked_trees::BlockingFacts,
    termination: &checked_trees::TerminationFacts,
    mutation: &checked_trees::MutationFacts,
    capabilities: &flow_effects::CapabilityFlowPlan,
    flow: &checked_trees::FlowFacts,
    operators: &checked_trees::CheckedOperatorFacts,
    semantic: &facts::FactPlan,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Result<checked_trees::MachineContractPlans, Vec<diagnostics::Diagnostic>> {
    let mut machines = Vec::new();
    let content_conservation = validation::build_content_conservation_plans(program);
    for machine in program.machines() {
        let service_fact = service_reaches.for_machine(machine.symbol);
        let published_service_row = service_fact
            .map(|fact| fact.published_ceiling)
            .unwrap_or(language_semantics::ServiceReachRowTable::EMPTY_ROW);
        let published_service_names = service_reaches
            .rows
            .services(published_service_row)
            .iter()
            .filter_map(|service| service_reaches.services.definition(*service))
            .map(|definition| definition.name.clone())
            .collect::<Vec<_>>();
        let synchronous_invocation = synchronous_invocations
            .for_machine(machine.symbol)
            .expect("every checked machine must publish synchronous invocation facts");
        let suspension = suspensions
            .for_machine(machine.symbol)
            .expect("every checked machine must publish suspension facts");
        let blocking = blocking
            .for_machine(machine.symbol)
            .expect("every checked machine must publish blocking facts");
        let termination = termination
            .for_machine(machine.symbol)
            .expect("every checked machine must publish termination facts");
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
            let state_contracts = encode_contract_set_canonical(
                program,
                program.state_contracts(state),
                &parameter_names,
                &content_conservation,
                &[0xae],
                true,
                true,
            );
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
        let crash = build_published_crash_plan(
            program,
            machine,
            &parameter_names,
            &content_conservation,
            operators,
            exact_integer_casts,
        );
        canonical_facts.extend(encode_contract_set_canonical(
            program,
            program.machine_contracts(machine),
            &parameter_names,
            &content_conservation,
            &[],
            false,
            false,
        ));
        canonical_facts.sort();
        let closed_scalar_values =
            build_closed_scalar_value_contract_plan(program, machine, operators);
        let identity = checked_trees::contract_identity(
            machine.supply_mode,
            &published_service_names,
            synchronous_invocation.interface,
            &synchronous_invocation.published,
            suspension.interface,
            blocking.interface,
            &crash,
            &termination.interface,
            &canonical_facts,
        );
        machines.push(checked_trees::MachineContractPlan {
            machine: machine.symbol,
            closed_scalar_values,
            crash,
            report_fingerprint: identity.report_fingerprint,
            commitment: identity.commitment,
        });
    }
    let mut operator_crashes = operator_crashes::build(program, operators, flow, semantic)?;
    for plan in &mut machines {
        let mut sites = Vec::new();
        let mut pending = Vec::new();
        for (machine, site) in operator_crashes {
            if machine == plan.machine {
                sites.push(site);
            } else {
                pending.push((machine, site));
            }
        }
        operator_crashes = pending;
        plan.crash = plan
            .crash
            .clone()
            .with_checked_operators(sites)
            .ok_or_else(|| {
                vec![diagnostics::Diagnostic::error(
                    "invalid selected operator crash invocation roster",
                )]
            })?;
    }
    if !operator_crashes.is_empty() {
        return Err(vec![diagnostics::Diagnostic::error(
            "selected operator crash invocation has no owning machine contract",
        )]);
    }
    let crash_capsules = build_crash_contract_capsules(program, &content_conservation, operators);
    crash_calls::attach_checked_crash_calls(
        program,
        operators,
        exact_integer_casts,
        semantic,
        flow,
        &content_conservation,
        &crash_capsules,
        &mut machines,
        call_frames,
    );
    let realized_envelopes =
        machines
            .iter()
            .map(|contract| {
                let machine = program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == contract.machine)
                    .expect("every contract must retain its exact typed machine");
                let service_fact = service_reaches
                    .for_machine(contract.machine)
                    .expect("every checked machine must retain service-reach facts");
                let effective_service_reach = service_reaches
                    .rows
                    .services(service_fact.effective)
                    .iter()
                    .filter_map(|service| service_reaches.services.definition(*service))
                    .map(|definition| definition.name.clone())
                    .collect::<Vec<_>>();
                let concrete_service_reach = service_reaches
                    .rows
                    .services(service_fact.concrete_effective)
                    .iter()
                    .filter_map(|service| service_reaches.services.definition(*service))
                    .map(|definition| definition.name.clone())
                    .collect::<Vec<_>>();
                let invocation = synchronous_invocations
                    .for_machine(contract.machine)
                    .expect("every checked machine must retain invocation facts");
                let suspension = suspensions
                    .for_machine(contract.machine)
                    .expect("every checked machine must retain suspension facts");
                let blocking = blocking
                    .for_machine(contract.machine)
                    .expect("every checked machine must retain blocking facts");
                let termination = termination
                    .for_machine(contract.machine)
                    .expect("every checked machine must retain termination facts");
                let mutation = mutation
                    .for_machine(contract.machine)
                    .map(|fact| fact.state_write_frames.clone())
                    .unwrap_or_default();
                let capability_rows = capabilities
                    .flows()
                    .filter(|flow| flow.machine_symbol == contract.machine)
                    .copied()
                    .collect();
                checked_trees::RealizedMachineContractEnvelope {
                machine: contract.machine,
                contract_report_fingerprint: contract.report_fingerprint,
                contract_commitment: contract.commitment,
                effective_service_reach,
                concrete_service_reach,
                unresolved_installation_reaches: service_fact
                    .unresolved_installation_reaches
                    .clone(),
                effective_synchronous_invocations: invocation.checked_inferred.clone(),
                checked_may_suspend: suspension.checked_may_suspend,
                checked_may_block: blocking.checked_may_block,
                checked_termination: termination.checked_summary.clone(),
                checked_crash: contract.crash.clone(),
                mutation,
                capabilities: capability_rows,
                resources:
                    checked_trees::CheckedMachineResourceEnvelopes::from_checked_contract_entries(
                        contract.machine,
                        contract.report_fingerprint,
                        contract.commitment,
                        program.machine_states(machine).iter().map(|entry| entry.symbol),
                    ),
            }
            })
            .collect();
    let plans = checked_trees::MachineContractPlans {
        machines,
        crash_capsules,
        realized_envelopes,
    };
    plans.validate_resource_envelopes().map_err(|error| {
        vec![diagnostics::Diagnostic::error(format!(
            "checked resource-envelope replay failed: {error}"
        ))]
    })?;
    validate_checked_resource_envelope_coverage(program, &plans)?;
    Ok(plans)
}

/// Reconstruct the complete per-entry resource roster from typed ownership
/// and declaration order. Structural checked validation above does not reopen
/// typed trees, so this second gate is what rejects a missing, foreign, or
/// reordered entry before the source-independent carrier leaves this stage.
fn validate_checked_resource_envelope_coverage(
    program: &TypedTrees,
    plans: &checked_trees::MachineContractPlans,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    for machine in program.machines() {
        let contract = plans.for_machine(machine.symbol).ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "checked resource-envelope replay is missing an exact machine contract",
            )]
        })?;
        let realized = plans.realized_envelope(machine.symbol).ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "checked resource-envelope replay is missing an exact realized machine row",
            )]
        })?;
        let expected_entries = program.machine_states(machine);
        if realized.resources.len() != expected_entries.len() {
            return Err(vec![diagnostics::Diagnostic::error(
                "checked resource-envelope replay does not cover every owned machine entry",
            )]);
        }
        for (resource, entry) in realized.resources.iter().zip(expected_entries) {
            let replayed = checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
                machine.symbol,
                entry.symbol,
                contract.report_fingerprint,
                contract.commitment,
            );
            if resource != &replayed {
                return Err(vec![diagnostics::Diagnostic::error(
                    "checked resource-envelope replay changed entry ownership or declaration order",
                )]);
            }
        }
    }
    Ok(())
}

pub(crate) fn build_mutation_facts(
    program: &TypedTrees,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> checked_trees::MutationFacts {
    let mut owned_resolver = None;
    let frame_resolver =
        crate::flow::shared_call_frames_or(call_frames, program, &mut owned_resolver);
    let machines = program
        .machines()
        .iter()
        .map(|machine| {
            let states = program.machine_states(machine);
            let frames = frame_resolver.map_or_else(
                || {
                    (0..states.len())
                        .map(|_| facts::NormalizedWriteFrame::opaque())
                        .collect()
                },
                |resolver| resolver.inferred_machine_state_write_frames(machine),
            );
            checked_trees::MachineMutationFact {
                machine: machine.symbol,
                state_write_frames: states
                    .iter()
                    .zip(frames)
                    .map(|(state, frame)| checked_trees::StateWriteFramePlan {
                        state: state.symbol,
                        frame,
                    })
                    .collect(),
            }
        })
        .collect();
    checked_trees::MutationFacts { machines }
}

pub(crate) fn build_closed_scalar_value_contract_plan(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    operators: &checked_trees::CheckedOperatorFacts,
) -> checked_trees::ClosedScalarValueContractPlan {
    use typed_trees::{
        domain::ProofFact,
        expression::{BinaryOperator, ExpressionNode},
        signature::SignatureContractKind,
    };

    let boolean_type =
        program
            .type_reference_table
            .named_references()
            .find_map(|(type_reference, symbol, _)| {
                (program.symbols.builtin_type_atom(symbol) == Some(symbols::BuiltinTypeAtom::Bool))
                    .then_some(type_reference)
            });

    let lower_closed_clause = |contract: &typed_trees::signature::SignatureContract| {
        let [ProofFact::Expression(expression)] = program.proof_facts.span_or_empty(contract.facts)
        else {
            return None;
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
        else {
            return None;
        };
        if binary.operator != BinaryOperator::Equal {
            return None;
        }
        // Boolean literals have one exact builtin carrier. Integer literals
        // retain wildcard typing rather than guessing a contextual landing.
        let operand_types = [binary.left, binary.right].map(|operand| {
            matches!(
                program.expression_table.expression(operand),
                ExpressionNode::Boolean(_)
            )
            .then_some(boolean_type)
            .flatten()
        });
        if operators.uses.iter().any(|(_, operator)| {
            operator.expression == *expression
                && operator.status
                    != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
        }) || !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            *expression,
            language_core::OperatorSpelling::Equal,
            &operand_types,
        ) {
            return None;
        }
        match (
            program.expression_table.expression(binary.left),
            program.expression_table.expression(binary.right),
        ) {
            (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) if left == right => {
                Some(checked_trees::ClosedScalarContractValue::Boolean(*left))
            }
            (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) if left == right => {
                Some(checked_trees::ClosedScalarContractValue::Integer(
                    left.clone(),
                ))
            }
            _ => None,
        }
    };

    let lower_clause = |contract: &typed_trees::signature::SignatureContract| {
        lower_closed_clause(contract).or_else(|| {
            // Preserve legacy closed literal encoding, then read compositional
            // scalar predicates in their exact entry or normal-result namespace.
            let [ProofFact::Expression(expression)] =
                program.proof_facts.span_or_empty(contract.facts)
            else {
                return None;
            };
            crate::values::lower_scalar_contract_predicate(
                program,
                operators,
                machine,
                *expression,
                contract.kind == SignatureContractKind::Ensures,
                &mut 4096,
            )
            .map(checked_trees::ClosedScalarContractValue::Predicate)
        })
    };

    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    let mut has_crash_clauses = false;
    let mut has_outcome_specific_clauses = false;
    for contract in program.machine_contracts(machine) {
        if matches!(
            &contract.kind,
            SignatureContractKind::EnsuresForResultCase { .. }
        ) {
            has_outcome_specific_clauses = true;
            continue;
        }
        // Named witness-bearing lanes are checked and lowered through the
        // evidence contract plan. They do not participate in the independent
        // closed scalar value contract used by terminal scalar production.
        if contract.binding.is_some() {
            continue;
        }
        match contract.kind {
            SignatureContractKind::Requires => requires.push(lower_clause(contract)),
            SignatureContractKind::Ensures => ensures.push(lower_clause(contract)),
            SignatureContractKind::EnsuresForResultCase { .. } => unreachable!(
                "outcome-specific clauses were separated from unconditional scalar contracts"
            ),
            SignatureContractKind::Crashes { .. } => has_crash_clauses = true,
        }
    }
    // Authored requires clauses lead the roster; parameter-range rows follow
    // as a derived tail so consumers can publish the authored block alone.
    let authored_requires_len = requires.len();
    // One constraint walk produces the requires tail and the floating roster
    // together so a `FloatRange` clause and its retained evidence can never
    // disagree about which authored range they describe.
    let ranges = crate::values::lower_scalar_parameter_range_requirements(program, machine);
    requires.extend(ranges.scalar_clauses);
    checked_trees::ClosedScalarValueContractPlan::new(
        requires,
        ensures,
        has_crash_clauses,
        has_outcome_specific_clauses,
    )
    .with_authored_requires_len(authored_requires_len)
    .with_float_entry_ranges(ranges.float_entry_ranges)
}
