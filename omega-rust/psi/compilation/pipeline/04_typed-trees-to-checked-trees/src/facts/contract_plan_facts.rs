//! Contract plans, resource envelope coverage, mutation facts and closed
//! scalar value contract plans.

use crate::facts::crash_plan_facts::{build_crash_contract_capsules, build_published_crash_plan};
use crate::facts::{crash_calls, operator_crashes};
use validation::contract_structure::{
    callable_binders, declared_callable, declared_contracts, declared_termination, generic_binders,
};

/// Normalization scratch retained until every generated contract's envelope
/// settles. Publishing a head before its member graph closes would commit an
/// empty crash permission set and then silently change that public contract.
struct ContractPublication<'program> {
    type_identities: &'program arena::OwnedInterner<typed_trees::type_identity::TypeIdentityTable>,
    supply: language_semantics::MachineSupplyMode,
    external_binding: Option<language_semantics::ExternalBindingIdentity>,
    services: Vec<String>,
    invocations: language_semantics::SynchronousInvocationInterface,
    published_invocations: Vec<String>,
    suspension: language_semantics::SuspensionInterface,
    blocking: language_semantics::BlockingInterface,
    termination: checked_trees::DeclaredTermination,
    declaration: checked_trees::DeclaredContractStructure,
}

impl ContractPublication<'_> {
    fn identity(
        &self,
        crashes: checked_trees::CrashInterface,
    ) -> checked_trees::MachineContractIdentity {
        self.type_identities.with(|table| {
            checked_trees::contract_identity(
                self.supply,
                self.external_binding.as_ref(),
                &self.services,
                self.invocations,
                &self.published_invocations,
                self.suspension,
                self.blocking,
                crashes,
                &self.termination,
                &self.declaration,
                table,
            )
        })
    }
}

/// Add only causes inferred from private members, leaving authored head routes
/// guarded. The prepared graph excludes proof pruning: a stronger prover must
/// not narrow the published interface. Invocation-specific refinement happens
/// later against this finalized ceiling, never against a lexical body.
fn publish_family_crash_envelopes(
    contracts: &mut [checked_trees::MachineContractPlan],
    publications: &mut [ContractPublication<'_>],
    additions: &[(symbols::SymbolHandle, Vec<checked_trees::CrashCause>)],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    for (head, causes) in additions {
        let position = contracts
            .iter()
            .position(|contract| contract.machine == *head)
            .ok_or_else(|| {
                vec![diagnostics::Diagnostic::error(
                    "target-family crash envelope has no owning contract",
                )]
            })?;
        let publication = publications.get_mut(position).ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "target-family crash envelope has no declaration publication",
            )]
        })?;
        let contract = &mut contracts[position];
        let mut published = contract.crash.published().to_vec();
        for cause in causes {
            // An inferred unconditional permission subsumes authored guards
            // for this cause; both the runtime row and commitment normalize
            // that same complete contract rather than hashing redundant facts.
            published.retain(|bucket| bucket.cause() != *cause);
            published.push(checked_trees::CrashRouteBucket::unconditional(*cause));
            publication.declaration.clauses.retain(|clause| {
                clause.kind != checked_trees::DeclaredContractKind::Crashes(*cause)
            });
            publication
                .declaration
                .clauses
                .push(checked_trees::DeclaredContractClause {
                    kind: checked_trees::DeclaredContractKind::Crashes(*cause),
                    fact: None,
                });
        }
        publication.declaration.clauses.sort();
        publication.declaration.clauses.dedup();
        contract.crash = contract.crash.clone().with_published_ceiling(published);
    }
    Ok(())
}

/// STR4 checked plans (wiki/spec/language/machines.md): assemble each machine's
/// normalized contract plan from the published halves already carried on
/// the records (supply mode, service/operational ceilings, published termination),
/// with a deterministic fingerprint over them. Checked service rows include
/// conservative call propagation, without proof-based pruning; a stronger
/// prover therefore cannot change an exported contract ID (acceptance 8).
pub(crate) fn build_contract_plans(
    program: &dyn ::validation::CheckingInput,
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
    scalar_expressions: &checked_trees::CheckedScalarExpressionPlans,
    scalar_computations: &checked_trees::CheckedScalarComputationPlans,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Result<checked_trees::MachineContractPlans, Vec<diagnostics::Diagnostic>> {
    let mut machines = Vec::new();
    let mut publications = Vec::new();
    let content_conservation = validation::content_conservation_plans(program);
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
        let generics = generic_binders(program.machine_type_parameters(machine));
        let mut callables = program
            .machine_states(machine)
            .iter()
            .map(|state| {
                declared_callable(
                    program,
                    program.state_parameters(state),
                    state.return_type,
                    program.state_contracts(state),
                    &generics,
                    &content_conservation,
                    true,
                )
            })
            .collect::<Vec<_>>();
        callables.sort();
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
        )
        .with_trapping_sites(trapping_sites(
            program,
            machine,
            operators,
            scalar_expressions,
            scalar_computations,
        )?);
        let binders = program
            .machine_states(machine)
            .first()
            .map(|entry| callable_binders(&generics, program.state_parameters(entry)))
            .unwrap_or_else(|| generics.clone());
        let declaration = checked_trees::DeclaredContractStructure {
            callables,
            clauses: declared_contracts(
                program,
                program.machine_contracts(machine),
                &binders,
                &content_conservation,
                true,
            ),
        };
        let published_termination = declared_termination(program, &termination.interface, &binders);
        let closed_scalar_values =
            build_closed_scalar_value_contract_plan(program, machine, operators);
        let external_binding = match machine.supply_mode {
            language_semantics::MachineSupplyMode::ExternalRealization {
                binding: Some(binding),
                ..
            } => Some(
                program
                    .external_bindings
                    .identity(binding)
                    .cloned()
                    .ok_or_else(|| {
                        vec![diagnostics::Diagnostic::error(
                            "checked external supply lost its owning binding identity",
                        )]
                    })?,
            ),
            _ => None,
        };
        publications.push(ContractPublication {
            type_identities: &program.type_identities,
            supply: machine.supply_mode,
            external_binding,
            services: published_service_names,
            invocations: synchronous_invocation.interface,
            published_invocations: synchronous_invocation.published.clone(),
            suspension: suspension.interface,
            blocking: blocking.interface,
            termination: published_termination,
            declaration,
        });
        machines.push(checked_trees::MachineContractPlan {
            machine: machine.symbol,
            closed_scalar_values,
            crash,
            // No consumer observes these construction coordinates. Family
            // envelopes settle before each final contract identity is minted.
            report_fingerprint: 0,
            commitment: checked_trees::MachineContractCommitment::from_digest([0; 32]),
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
    let summary_graph = crash_calls::prepare_crash_summary_graph(
        program,
        operators,
        flow,
        &crash_capsules,
        &machines,
    )?;
    publish_family_crash_envelopes(
        &mut machines,
        &mut publications,
        summary_graph.family_potential_crash_causes(),
    )?;
    for (contract, publication) in machines.iter_mut().zip(&publications) {
        let identity = publication.identity(contract.crash.interface());
        contract.report_fingerprint = identity.report_fingerprint;
        contract.commitment = identity.commitment;
    }
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
        &summary_graph,
    );
    let realized_envelopes =
        machines
            .iter()
            .map(|contract| {
                let machine = program
                    .machine(contract.machine)
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    checked_trees::MutationFacts {
        machines,
        ..Default::default()
    }
}

pub(crate) fn build_closed_scalar_value_contract_plan(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    operators: &checked_trees::CheckedOperatorFacts,
) -> checked_trees::ClosedScalarValueContractPlan {
    let parameters =
        crate::values::scalar::contract_entry::authored_entry_parameters(program, machine);
    let result_type = program
        .machine_states(machine)
        .first()
        .map(|state| state.return_type)
        .unwrap_or_default();
    build_callable_scalar_value_contract_plan(
        program,
        machine.symbol,
        parameters,
        result_type,
        program.machine_contracts(machine),
        operators,
    )
}

/// Executable bodies and opaque requirement signatures publish scalar facts
/// through the same declared formal telescope. Provider selection must not
/// replace an authored requirement with an empty contract, nor may publication
/// infer its requirements from whichever provider happens to be selected.
pub(crate) fn build_callable_scalar_value_contract_plan(
    program: &dyn ::validation::CheckingInput,
    owner: symbols::SymbolHandle,
    parameters: Option<&[typed_trees::signature::StateParameter]>,
    result_type: typed_trees::types::TypeReferenceHandle,
    contracts: &[typed_trees::signature::SignatureContract],
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
        let Some(binary) = program.expression_table.binary_node(*expression) else {
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
            owner,
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
        lower_closed_clause(contract)
            .or_else(|| {
                // Preserve legacy closed literal encoding, then read compositional
                // scalar predicates in their exact entry or normal-result namespace.
                let facts = program.proof_facts.span_or_empty(contract.facts);
                // One keyword is one clause however many facts it lists:
                // `requires 1 <= value; value <= 7;` parses to ONE contract
                // carrying TWO facts and means the conjunction that
                // `requires 1 <= value && value <= 7;` spells as one. One clause
                // per authored contract is load-bearing downstream --
                // `runtime_requirements::source` counts authored CONTRACTS to
                // find where the derived parameter-range tail begins, and reads
                // an authored two-bound range back as a single `And` -- so the
                // facts join here rather than becoming separate rows. Reading
                // only the one-fact spelling left every multi-fact clause
                // unlowered as `None`, so the clause could not be enforced. The
                // facts share one predicate budget
                // because they are one clause.
                let mut remaining = 4096;
                let mut conjunction: Option<checked_trees::CheckedBooleanExpression> = None;
                for fact in facts {
                    // A membership or proposition fact has no scalar predicate
                    // reading, and a clause is all-or-nothing: the whole clause
                    // stays unlowered rather than shrinking to the facts that do
                    // read, which would drop an authored requirement silently.
                    let expression = fact.as_expression()?;
                    let predicate = crate::values::lower_callable_scalar_contract_predicate(
                        program,
                        operators,
                        owner,
                        parameters?,
                        result_type,
                        *expression,
                        contract.kind == SignatureContractKind::Ensures,
                        &mut remaining,
                    )?;
                    conjunction = Some(match conjunction {
                        None => predicate,
                        // Left-associated, as `a && b && c` reads.
                        Some(left) => checked_trees::CheckedBooleanExpression::And {
                            left: Box::new(left),
                            right: Box::new(predicate),
                        },
                    });
                }
                conjunction.map(checked_trees::ClosedScalarContractValue::Predicate)
            })
            .or_else(|| {
                // Requires remain obligations on each caller. An authored ensures
                // has already been checked against normal-return paths: when this
                // scalar vocabulary has no useful consequence, retain its source
                // clause for checker consumers and erase it during publication.
                (contract.kind == SignatureContractKind::Ensures).then(|| {
                    let mut result_memberships = program
                        .proof_facts
                        .span_or_empty(contract.facts)
                        .iter()
                        .filter_map(|fact| match fact {
                            typed_trees::domain::ProofFact::Membership(membership)
                                if program
                                    .expression_table
                                    .names_reserved_result(membership.value) =>
                            {
                                Some(membership.semantic_domain)
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    result_memberships.sort_by_key(|domain| domain.0);
                    result_memberships.dedup();
                    checked_trees::ClosedScalarContractValue::SourceCheckedGuarantee {
                        facts: contract.facts,
                        result_memberships: result_memberships.into_boxed_slice(),
                    }
                })
            })
    };

    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    let mut has_crash_clauses = false;
    let mut has_outcome_specific_clauses = false;
    for contract in contracts {
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
    // One constraint walk produces the requires tail and the retained
    // rosters together so a range clause and its retained evidence can never
    // disagree about which authored range they describe.
    let ranges = crate::values::lower_callable_parameter_range_requirements(
        program,
        parameters.unwrap_or_default(),
    );
    requires.extend(ranges.scalar_clauses);
    checked_trees::ClosedScalarValueContractPlan::new(
        requires,
        ensures,
        has_crash_clauses,
        has_outcome_specific_clauses,
    )
    .with_authored_requires_len(authored_requires_len)
    .with_float_entry_ranges(ranges.float_entry_ranges)
    .with_integer_entry_ranges(ranges.integer_entry_ranges)
}

/// Body statements whose planned scalar value executes a Trapping primitive.
///
/// Each Trapping operation owns a `Trap` crash site at run time, so it is
/// crash evidence exactly like an explicit crash transition: an inferred body
/// widens its `Trap` cause to the unconditional route, and private callers
/// inherit it through the summary fixed point. The rows come from the same
/// checked scalar expression and computation plans lowering consumes, so a
/// Trapping operation reaching Terminal has its evidence here. A published
/// ceiling instead has to cover each such operation: the arithmetic-domain
/// walk decides that coverage over the operation's own operands while it
/// decides Exact formation (`validation`'s `trap_coverage`).
fn trapping_sites(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    operators: &checked_trees::CheckedOperatorFacts,
    scalar_expressions: &checked_trees::CheckedScalarExpressionPlans,
    scalar_computations: &checked_trees::CheckedScalarComputationPlans,
) -> Result<Vec<checked_trees::CrashSiteLocation>, Vec<diagnostics::Diagnostic>> {
    let states = program
        .machine_states(machine)
        .iter()
        .map(|state| state.symbol)
        .collect::<Vec<_>>();
    let expressions = scalar_expressions
        .expressions
        .iter()
        .filter(|located| {
            states.contains(&located.state) && located.expression.contains_trapping_operation()
        })
        .map(|located| {
            checked_trees::CrashSiteLocation::new(located.state, located.statement_ordinal)
        });
    let computations = scalar_computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.machine == machine.symbol
                && computation_contains_trapping_operation(scalar_computations, root.root)
        })
        .map(|root| checked_trees::CrashSiteLocation::new(root.state, root.statement_ordinal));
    // A selected float policy adapts the result after the boundary returns;
    // the boundary's own crash contract therefore cannot supply this site.
    // Read exact checked selections across spelled and named requirements,
    // not a cast's spelling or its operand storage. Selecting a future policy
    // without executing arithmetic has no result adapter and adds no Trap.
    // The existing summary fixed point propagates these body sites to private
    // callers just like integer Trapping sites, without a second call graph.
    let mut sites = expressions.chain(computations).collect::<Vec<_>>();
    for (_, origin, _, adapter) in operators.resolved_expression_selections() {
        if !matches!(
            adapter,
            checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { .. }
        ) {
            continue;
        }
        let checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            statement_index,
            ..
        } = origin
        else {
            continue;
        };
        if machine_symbol != machine.symbol {
            continue;
        }
        let invalid_site = || {
            vec![diagnostics::Diagnostic::error(
                "selected float result policy has no representable owning statement",
            )]
        };
        if !states.contains(&state_symbol) {
            return Err(invalid_site());
        }
        let statement = u32::try_from(statement_index).map_err(|_| invalid_site())?;
        sites.push(checked_trees::CrashSiteLocation::new(
            state_symbol,
            statement,
        ));
    }
    Ok(sites)
}

/// Whether evaluating one computation tree executes a Trapping primitive in
/// any of its pure expressions, operands, arms, or constructed arguments.
fn computation_contains_trapping_operation(
    plans: &checked_trees::CheckedScalarComputationPlans,
    root: checked_trees::CheckedScalarComputationHandle,
) -> bool {
    use checked_trees::{
        CheckedScalarComputationKind as Kind,
        CheckedScalarComputationStructuralArgument as Argument,
        CheckedScalarDispatchPattern as Pattern,
    };
    fn push_argument(
        plans: &checked_trees::CheckedScalarComputationPlans,
        argument: &Argument,
        pending: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
    ) -> bool {
        match argument {
            Argument::Place(place) => return push_place(place, pending),
            Argument::Case(case) => pending.extend(
                plans
                    .case_fields
                    .span_or_empty(case.fields)
                    .iter()
                    .map(|field| field.value),
            ),
        }
        false
    }
    fn push_place(
        place: &checked_trees::CheckedUnitStructuralArgumentPlan,
        pending: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
    ) -> bool {
        pending.extend(place.computation_operands());
        place.scalar_operands().any(|operand| match operand {
            checked_trees::CheckedCallScalarArgument::Pure(value) => {
                value.contains_trapping_operation()
            }
            checked_trees::CheckedCallScalarArgument::Computation(_) => false,
        })
    }
    let mut pending = vec![root];
    while let Some(handle) = pending.pop() {
        if !plans.nodes.is_valid(handle) {
            continue;
        }
        match &plans.nodes.get(handle).kind {
            Kind::RetainedConstant { .. } => {}
            Kind::Value(expression) => {
                if expression.contains_trapping_operation() {
                    return true;
                }
            }
            Kind::Apply {
                expression,
                operands,
                ..
            } => {
                if expression.contains_trapping_operation() {
                    return true;
                }
                pending.extend(plans.operands.span_or_empty(*operands).iter().copied());
            }
            Kind::ByteSequenceLiteralComparison { subject, .. }
            | Kind::ByteSequencePredicate { subject, .. } => {
                if push_place(subject, &mut pending) {
                    return true;
                }
            }
            Kind::StructuralField { .. } => {}
            Kind::CaseMembership { subject, .. } => {
                if push_argument(plans, subject, &mut pending) {
                    return true;
                }
            }
            Kind::SelectedComparison { left, right, .. } => pending.extend([*left, *right]),
            Kind::Qualification { operand, .. }
            | Kind::BooleanToInteger { operand, .. }
            | Kind::AuthoredCast { operand, .. } => pending.push(*operand),
            Kind::Dispatch { subject, arms, .. } => {
                pending.push(*subject);
                for arm in plans.dispatch_arms.span_or_empty(*arms) {
                    pending.push(arm.value);
                    if let Pattern::Value(value) = arm.pattern {
                        pending.push(value);
                    }
                }
            }
            Kind::Call {
                callee,
                arguments,
                structural_arguments,
                ..
            } => {
                pending.extend(callee.selector());
                pending.extend(plans.operands.span_or_empty(*arguments).iter().copied());
                // A boundary call keeps its own provider contract. Result-policy
                // Trap sites are collected from checked selections by
                // trapping_sites; this walk owns traps in its scalar operands.
                if matches!(callee, checked_trees::CheckedScalarCallee::Boundary { .. }) {
                    continue;
                }
                for structural in plans
                    .structural_arguments
                    .span_or_empty(*structural_arguments)
                {
                    if push_argument(plans, structural, &mut pending) {
                        return true;
                    }
                }
            }
            Kind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => pending.extend([*condition, *when_true, *when_false]),
        }
    }
    false
}
