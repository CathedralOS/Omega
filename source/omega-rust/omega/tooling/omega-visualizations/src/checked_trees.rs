use psi_checked_trees::CheckedTrees;
use psi_symbols::SymbolHandle;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;

mod capability_manifest;

pub use capability_manifest::{
    capability_manifest_json, capability_manifest_json_with_composition,
    capability_manifest_json_with_selection,
};

/// Public checked qualification-evidence surface. The fact's program point and
/// its establishment origin remain independent, and admitted rows retain their
/// normalized receipt identity when provider admission supplied one.
pub fn qualification_evidence_manifest_json(
    program: &CheckedTrees,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> String {
    use psi_facts::FactPayload;
    use psi_language_semantics::QualificationEvidenceOrigin;

    let rows = program
        .facts
        .semantic
        .facts
        .iter()
        .filter(|(_, fact)| fact.evidence.origin != QualificationEvidenceOrigin::None)
        .filter_map(|(_, fact)| {
            let domain_label = match fact.payload {
                FactPayload::DomainMembership {
                    domain,
                    domain_symbol,
                    ..
                }
                | FactPayload::ContractDomainMembership {
                    domain,
                    domain_symbol,
                    ..
                } => {
                    if domain_symbol.is_valid() {
                        program
                            .domain_definitions()
                            .iter()
                            .find(|definition| definition.symbol == domain_symbol)
                            .expect("qualification evidence must name an exact declared domain");
                        qualification_symbol_label(program, domain_symbol)
                    } else {
                        program
                            .domain_path_members(domain)
                            .iter()
                            .map(|member| member.as_str())
                            .collect::<Vec<_>>()
                            .join("::")
                    }
                }
                FactPayload::CarryPermission { permission, .. }
                | FactPayload::ContractCarryPermission { permission, .. } => {
                    permission.name().to_owned()
                }
                _ => return None,
            };
            Some((fact, domain_label))
        })
        .collect::<Vec<_>>();

    let selected_provider_closure_digest = selected_provider_plans
        .identity_digest()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let mut json = format!(
        "{{\n  \"selected_provider_closure_report_fingerprint\": \"0x{:016x}\",\n  \"selected_provider_closure_digest\": \"{}\",\n  \"qualification_evidence\": [",
        selected_provider_plans.compatibility_report_identity(),
        selected_provider_closure_digest,
    );
    for (index, (fact, domain_label)) in rows.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        validate_qualification_program_point(program, fact.point);
        let requirement_identity = qualification_requirement_identity(program, &fact.evidence);
        validate_qualification_receipt(
            selected_provider_plans,
            fact.evidence.origin,
            fact.evidence.receipt_identity,
        );
        validate_qualification_source(program, &fact.evidence);
        json.push_str("\n    {\n      \"subject\": ");
        push_json_string(&mut json, &qualification_subject(program, fact));
        json.push_str(",\n      \"domain\": ");
        push_json_string(&mut json, domain_label);
        json.push_str(",\n      \"origin\": ");
        push_json_string(&mut json, fact.evidence.origin.as_str());
        json.push_str(",\n      \"program_point\": ");
        push_json_string(&mut json, program_point_name(fact.point));
        json.push_str(",\n      \"program_point_identity\": ");
        push_json_string(&mut json, &exact_program_point_label(program, fact.point));
        json.push_str(",\n      \"source\": ");
        if fact.evidence.source_symbol.is_valid() {
            push_json_string(
                &mut json,
                &qualification_symbol_label(program, fact.evidence.source_symbol),
            );
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"requirement\": ");
        if requirement_identity.is_some() {
            push_json_string(
                &mut json,
                &qualification_symbol_label(program, fact.evidence.requirement_symbol),
            );
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"requirement_identity\": ");
        if let Some(requirement_identity) = requirement_identity {
            push_json_string(&mut json, &requirement_identity);
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"receipt_identity\": ");
        if fact.evidence.receipt_identity == 0 {
            json.push_str("null");
        } else {
            push_json_string(
                &mut json,
                &format!("0x{:016x}", fact.evidence.receipt_identity),
            );
        }
        json.push_str("\n    }");
    }
    let mut boundary_authority_rows = selected_provider_plans
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.schema.methods.iter().flat_map(move |method| {
                method.entry_claims.iter().map(move |claim| {
                    (
                        plan,
                        method,
                        claim,
                        method
                            .parameter_type_identities
                            .get(claim.parameter_index)
                            .map(String::as_str),
                    )
                })
            })
        })
        .collect::<Vec<_>>();
    boundary_authority_rows.sort_by(
        |(left_plan, left_method, left_claim, _), (right_plan, right_method, right_claim, _)| {
            left_plan
                .name
                .cmp(&right_plan.name)
                .then_with(|| left_method.name.cmp(&right_method.name))
                .then_with(|| left_claim.parameter_index.cmp(&right_claim.parameter_index))
                .then_with(|| left_claim.domain.cmp(&right_claim.domain))
        },
    );
    let mut boundary_result_rows = selected_provider_plans
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.schema.methods.iter().flat_map(move |method| {
                method
                    .result_claims
                    .iter()
                    .map(move |claim| (plan, method, claim))
            })
        })
        .collect::<Vec<_>>();
    boundary_result_rows.sort_by(
        |(left_plan, left_method, left_claim), (right_plan, right_method, right_claim)| {
            left_plan
                .name
                .cmp(&right_plan.name)
                .then_with(|| left_method.name.cmp(&right_method.name))
                .then_with(|| left_claim.domain.cmp(&right_claim.domain))
        },
    );

    json.push_str("\n  ],\n  \"boundary_authority_flow\": [");
    for (index, (plan, method, claim, subject_type)) in boundary_authority_rows.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"flow\": ");
        push_json_string(&mut json, claim.authority_flow.as_str());
        json.push_str(",\n      \"boundary\": ");
        push_json_string(&mut json, &plan.schema.trait_name);
        json.push_str(",\n      \"requirement\": ");
        push_json_string(&mut json, &service_requirement_label(plan, method));
        json.push_str(",\n      \"requirement_owner\": ");
        push_json_string(&mut json, &method.requirement_owner);
        json.push_str(",\n      \"requirement_identity\": ");
        push_json_string(&mut json, &method.requirement_identity);
        json.push_str(",\n      \"parameter_index\": ");
        json.push_str(&claim.parameter_index.to_string());
        json.push_str(",\n      \"subject_type\": ");
        if let Some(subject_type) = subject_type {
            push_json_string(&mut json, subject_type);
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"domain\": ");
        push_json_string(&mut json, &claim.domain);
        json.push_str(",\n      \"predicate_body\": ");
        push_json_string(&mut json, claim.predicate_body.as_str());
        json.push_str(",\n      \"effective_carry\": ");
        push_carry_policy_json(&mut json, claim.effective_carry);
        json.push_str(",\n      \"provider_plan\": ");
        push_json_string(&mut json, &plan.name);
        json.push_str(",\n      \"provider_origin_package\": ");
        if plan.origin_package.is_empty() {
            json.push_str("null");
        } else {
            push_json_string(&mut json, &plan.origin_package);
        }
        json.push_str(",\n      \"receipt_identity\": ");
        push_json_string(&mut json, &format!("0x{:016x}", plan.report_fingerprint()));
        json.push_str("\n    }");
    }
    for (index, (plan, method, claim)) in boundary_result_rows.iter().enumerate() {
        if !boundary_authority_rows.is_empty() || index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"flow\": \"returns\"");
        json.push_str(",\n      \"boundary\": ");
        push_json_string(&mut json, &plan.schema.trait_name);
        json.push_str(",\n      \"requirement\": ");
        push_json_string(&mut json, &service_requirement_label(plan, method));
        json.push_str(",\n      \"requirement_owner\": ");
        push_json_string(&mut json, &method.requirement_owner);
        json.push_str(",\n      \"requirement_identity\": ");
        push_json_string(&mut json, &method.requirement_identity);
        json.push_str(",\n      \"parameter_index\": null");
        json.push_str(",\n      \"subject_type\": ");
        if let Some(subject_type) = &method.result_type_identity {
            push_json_string(&mut json, subject_type);
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"domain\": ");
        push_json_string(&mut json, &claim.domain);
        json.push_str(",\n      \"predicate_body\": \"bodyless\"");
        json.push_str(",\n      \"effective_carry\": ");
        push_carry_policy_json(&mut json, claim.effective_carry);
        json.push_str(",\n      \"provider_plan\": ");
        push_json_string(&mut json, &plan.name);
        json.push_str(",\n      \"provider_origin_package\": ");
        if plan.origin_package.is_empty() {
            json.push_str("null");
        } else {
            push_json_string(&mut json, &plan.origin_package);
        }
        json.push_str(",\n      \"receipt_identity\": ");
        push_json_string(&mut json, &format!("0x{:016x}", plan.report_fingerprint()));
        json.push_str("\n    }");
    }

    json.push_str("\n  ],\n  \"machine_semantic_domain_commitments\": [");
    for (index, (machine, domains)) in validated_machine_semantic_domain_commitments(program)
        .iter()
        .enumerate()
    {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, machine.symbol),
        );
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, machine.symbol)
                .expect("semantic-domain commitment must name an exact owning machine"),
        );
        json.push_str(",\n      \"semantic_domains\": [");
        for (domain_index, (domain, name)) in domains.iter().enumerate() {
            if domain_index > 0 {
                json.push_str(", ");
            }
            json.push_str("{\"semantic_domain_id\": ");
            json.push_str(&domain.0.to_string());
            json.push_str(", \"semantic_domain\": ");
            push_json_string(&mut json, name);
            json.push('}');
        }
        json.push_str("]\n    }");
    }
    json.push_str("\n  ],\n  \"vacuous_qualification_uses\": [");
    for (index, use_fact) in program.facts.qualifications.vacuous_uses.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let semantic_domain_name = validate_vacuous_qualification_use(program, use_fact);
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, use_fact.machine),
        );
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, use_fact.machine)
                .expect("vacuous qualification use must name an exact owning machine"),
        );
        json.push_str(",\n      \"state\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, use_fact.state),
        );
        json.push_str(",\n      \"statement_index\": ");
        json.push_str(&use_fact.statement_index.to_string());
        json.push_str(",\n      \"origin\": \"vacuous_qualification\"");
        json.push_str(",\n      \"domain\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, use_fact.domain),
        );
        json.push_str(",\n      \"semantic_domain_id\": ");
        json.push_str(&use_fact.semantic_domain.0.to_string());
        json.push_str(",\n      \"semantic_domain\": ");
        push_json_string(&mut json, semantic_domain_name);
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

fn validated_machine_semantic_domain_commitments(
    program: &CheckedTrees,
) -> Vec<(
    &psi_typed_trees::machine::Machine,
    Vec<(psi_language_semantics::SemanticDomainId, &str)>,
)> {
    let mut seen_machines = Vec::new();
    program
        .facts
        .qualifications
        .machines
        .iter()
        .map(|fact| {
            assert!(
                !seen_machines.contains(&fact.machine),
                "semantic-domain commitments must have one row per exact machine",
            );
            seen_machines.push(fact.machine);
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == fact.machine)
                .expect("semantic-domain commitment must name an exact owning machine");
            assert!(
                !fact.body_committed.is_empty(),
                "semantic-domain commitment row must retain at least one domain",
            );
            assert!(
                fact.body_committed
                    .windows(2)
                    .all(|domains| domains[0].0 < domains[1].0),
                "semantic-domain commitments must be strictly increasing",
            );
            let domains = fact
                .body_committed
                .iter()
                .map(|domain| {
                    let name = program
                        .semantic_domains
                        .name(*domain)
                        .expect("semantic-domain commitment must name a registered domain");
                    (*domain, name)
                })
                .collect();
            (machine, domains)
        })
        .collect()
}

fn validate_qualification_receipt(
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    origin: psi_language_semantics::QualificationEvidenceOrigin,
    receipt_identity: u64,
) {
    if receipt_identity != 0 {
        assert_eq!(
            origin,
            psi_language_semantics::QualificationEvidenceOrigin::AdmittedReceipt,
            "nonzero qualification evidence receipt must use admitted-receipt origin",
        );
        selected_provider_plans
            .plan_by_report_fingerprint(receipt_identity)
            .expect(
                "qualification evidence receipt must name an exact retained selected provider plan",
            );
    }
}

fn validate_qualification_source(
    program: &CheckedTrees,
    evidence: &psi_facts::QualificationEvidence,
) {
    use psi_language_semantics::QualificationEvidenceOrigin;
    use psi_typed_trees::data::{MachineParameterContract, TypeParameterKind};

    if evidence.origin == QualificationEvidenceOrigin::AdmittedReceipt {
        qualification_requirement_identity(program, evidence);
        return;
    }

    assert!(
        !evidence.requirement_symbol.is_valid(),
        "non-admitted qualification evidence must not name a boundary requirement",
    );
    assert_eq!(
        evidence.receipt_identity, 0,
        "non-admitted qualification evidence must not retain an admitted receipt",
    );
    assert!(
        evidence.source_symbol.is_valid(),
        "non-admitted qualification evidence must retain a nonempty exact source symbol",
    );

    let source = evidence.source_symbol;
    let machine_matches = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == source)
        .count();
    let state_matches = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .filter(|state| state.symbol == source)
        .count();
    let root_operator_matches = program
        .operators()
        .iter()
        .filter(|operator| operator.symbol == source)
        .count();
    let domain_operator_matches = program
        .domain_definitions()
        .iter()
        .flat_map(|domain| program.domain_operators(domain))
        .filter(|operator| operator.symbol == source)
        .count();
    let trait_matches = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == source)
        .count();
    let trait_requirement_matches = program
        .traits()
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .filter(|requirement| requirement.symbol == source)
        .count();
    let generic_parameter_matches = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_type_parameters(machine))
        .filter(|parameter| {
            matches!(&parameter.kind, TypeParameterKind::Machine { .. })
                && parameter.symbol == source
        })
        .count();
    let structural_contract_matches = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_type_parameters(machine))
        .filter(|parameter| {
            matches!(
                &parameter.kind,
                TypeParameterKind::Machine {
                    contract: MachineParameterContract::Structural(contract),
                } if contract.symbol == source
            )
        })
        .count();
    let matches = machine_matches
        + state_matches
        + root_operator_matches
        + domain_operator_matches
        + trait_matches
        + trait_requirement_matches
        + generic_parameter_matches
        + structural_contract_matches;
    assert_eq!(
        matches, 1,
        "non-admitted qualification evidence source must resolve to exactly one retained typed semantic declaration",
    );
}

fn validate_qualification_program_point(program: &CheckedTrees, point: psi_facts::ProgramPoint) {
    use psi_facts::ProgramPoint;

    let (machine_symbol, state_symbol, statement_index, call_ordinal) = match point {
        ProgramPoint::Global | ProgramPoint::Definition { .. } => return,
        ProgramPoint::Machine { machine_symbol } => {
            program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)
                .expect("qualification evidence program point must name an exact typed machine");
            return;
        }
        ProgramPoint::State {
            machine_symbol,
            state_symbol,
        } => (machine_symbol, state_symbol, None, None),
        ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index,
        } => (machine_symbol, state_symbol, Some(statement_index), None),
        ProgramPoint::Call {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        }
        | ProgramPoint::CallRequires {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        }
        | ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } => (
            machine_symbol,
            state_symbol,
            Some(statement_index),
            Some(call_ordinal),
        ),
        ProgramPoint::Exit {
            machine_symbol,
            state_symbol,
            statement_index,
        } => (machine_symbol, state_symbol, Some(statement_index), None),
    };
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("qualification evidence program point must name an exact typed machine");
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .expect(
            "qualification evidence program point state must belong to its exact typed machine",
        );
    if let Some(statement_index) = statement_index {
        assert!(
            statement_index
                < program
                    .statement_table
                    .statements(state.statement_nodes)
                    .len(),
            "qualification evidence program point statement index must be within its exact typed state",
        );
    }
    if let (Some(statement_index), Some(call_ordinal)) = (statement_index, call_ordinal) {
        let flow_state = program
            .facts
            .flow
            .control
            .states
            .iter()
            .find(|(_, state)| {
                state.machine_symbol == machine_symbol && state.state_symbol == state_symbol
            })
            .map(|(_, state)| state)
            .expect("qualification evidence call point must name an exact checked flow state");
        assert!(
            program
                .facts
                .flow
                .control
                .calls
                .span_or_empty(flow_state.calls)
                .iter()
                .any(|call| {
                    call.statement_index == statement_index && call.call_ordinal == call_ordinal
                }),
            "qualification evidence call point must name an exact owned checked flow call",
        );
    }
}

fn validate_vacuous_qualification_use<'program>(
    program: &'program CheckedTrees,
    use_fact: &psi_checked_trees::VacuousQualificationUse,
) -> &'program str {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == use_fact.machine)
        .expect("vacuous qualification use must name an exact owning machine");
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == use_fact.state)
        .expect("vacuous qualification use state must belong to its exact owning machine");
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(
            usize::try_from(use_fact.statement_index)
                .expect("vacuous qualification statement index must fit the host"),
        )
        .expect("vacuous qualification use statement index must be within its exact state");
    assert!(
        use_fact.expression.is_valid()
            && program
                .expression_table
                .expression_entries()
                .any(|(handle, _)| handle == use_fact.expression),
        "vacuous qualification use must name a valid retained expression",
    );
    assert!(
        matches!(
            program.expression_table.expression(use_fact.expression),
            psi_typed_trees::expression::ExpressionNode::Cast(_)
        ),
        "vacuous qualification use must name its exact retained cast",
    );
    assert!(
        qualification_statement_contains_expression(
            program,
            statement,
            use_fact.expression,
            &mut Vec::new(),
        ),
        "vacuous qualification use cast must belong to its exact statement",
    );
    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == use_fact.domain)
        .expect("vacuous qualification use must name an exact declared domain");
    program
        .semantic_domains
        .name(use_fact.semantic_domain)
        .expect("vacuous qualification use must name a registered semantic-domain instance")
}

fn qualification_statement_contains_expression(
    program: &CheckedTrees,
    statement: &psi_typed_trees::statement::StatementNode,
    target: psi_typed_trees::expression::ExpressionHandle,
    visited: &mut Vec<psi_typed_trees::expression::ExpressionHandle>,
) -> bool {
    use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

    let mut contains =
        |expression| qualification_expression_contains(program, expression, target, visited);
    match statement {
        StatementNode::AssemblyFact(_) => false,
        StatementNode::Assignment(assignment) => {
            contains(assignment.target) || contains(assignment.value)
        }
        StatementNode::Call(call) => program
            .statement_table
            .expression_handles(call.arguments)
            .iter()
            .copied()
            .any(contains),
        StatementNode::Expression(expression) => contains(*expression),
        StatementNode::LocalData(local) => contains(local.initial_value),
        StatementNode::Transition(transition) => {
            if matches!(transition.guard, TransitionGuardNode::When(guard) if contains(guard)) {
                return true;
            }
            [transition.target, transition.continuation]
                .into_iter()
                .filter(|target| target.is_valid())
                .any(|transition_target| {
                    match program.statement_table.transition_target(transition_target) {
                        TransitionTargetNode::Named { arguments, .. } => program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .copied()
                            .any(&mut contains),
                        TransitionTargetNode::Value(expression) => contains(*expression),
                        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => false,
                    }
                })
        }
    }
}

fn qualification_expression_contains(
    program: &CheckedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    target: psi_typed_trees::expression::ExpressionHandle,
    visited: &mut Vec<psi_typed_trees::expression::ExpressionHandle>,
) -> bool {
    use psi_typed_trees::expression::ExpressionNode;

    if expression == target {
        return true;
    }
    if !expression.is_valid() || visited.contains(&expression) {
        return false;
    }
    let Some((_, expression_node)) = program
        .expression_table
        .expression_entries()
        .find(|(handle, _)| *handle == expression)
    else {
        return false;
    };
    visited.push(expression);
    let mut contains = |child| qualification_expression_contains(program, child, target, visited);
    match expression_node {
        ExpressionNode::ArrayLiteral(items) => program
            .expression_table
            .expression_handles(*items)
            .iter()
            .copied()
            .any(contains),
        ExpressionNode::Binary(binary) => contains(binary.left) || contains(binary.right),
        ExpressionNode::Call(call) => {
            contains(call.receiver)
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(&mut contains)
        }
        ExpressionNode::Cast(cast) => contains(cast.value),
        ExpressionNode::Indexed(indexed) => contains(indexed.collection) || contains(indexed.index),
        ExpressionNode::Member(member) => contains(member.receiver),
        ExpressionNode::Borrow(inner) => contains(inner.target),
        ExpressionNode::Range(range) => contains(range.start) || contains(range.end),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| contains(field.value)),
        ExpressionNode::Unary(unary) => contains(unary.operand),
        ExpressionNode::Atomic(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

/// Render the authored owner of an exact inherited requirement. The selected
/// schema is the deployment boundary and may be a descendant that only refines
/// calling policy, so reconstructing `Schema::method` would misattribute the
/// semantic requirement. Transitional singleton schemas have no exact identity
/// and retain their existing display label.
fn service_requirement_label(
    plan: &omega_effects::provider_plan::ProviderPlan,
    method: &omega_effects::provider_plan::ServiceMethod,
) -> String {
    let owner = if method.requirement_owner.is_empty() {
        &plan.schema.trait_name
    } else {
        &method.requirement_owner
    };
    format!("{owner}::{}", method.name)
}

/// Public PDI3 compatibility surface. The named condition and its exact
/// discharge route are retained independently of indexed-domain identity.
pub fn index_compatibility_manifest_json(program: &CheckedTrees) -> String {
    use psi_checked_trees::IndexCompatibilityDischarge;

    let mut rows = program
        .facts
        .index_compatibility
        .conditions
        .iter()
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.name.cmp(&right.name));

    let mut json = String::from("{\n  \"index_compatibility\": [");
    for (index, condition) in rows.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let (route, operation_count, evidence_facts) = match &condition.discharge {
            IndexCompatibilityDischarge::ClosedEvaluation => {
                ("closed_evaluation", None, Vec::new())
            }
            IndexCompatibilityDischarge::LicensedNormalization { operation_count } => {
                ("licensed_normalization", Some(*operation_count), Vec::new())
            }
            IndexCompatibilityDischarge::EstablishedLocalFacts { facts } => {
                ("established_local_fact", None, facts.clone())
            }
        };
        json.push_str("\n    {\n      \"name\": ");
        push_json_string(&mut json, &condition.name);
        json.push_str(",\n      \"program_point\": ");
        push_json_string(
            &mut json,
            &exact_program_point_label(program, condition.point),
        );
        json.push_str(",\n      \"family\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, condition.family),
        );
        json.push_str(",\n      \"actual_instance\": ");
        json.push_str(&condition.actual_instance.0.to_string());
        json.push_str(",\n      \"expected_instance\": ");
        json.push_str(&condition.expected_instance.0.to_string());
        json.push_str(",\n      \"actual_expression\": ");
        push_json_string(&mut json, &condition.actual_label);
        json.push_str(",\n      \"expected_expression\": ");
        push_json_string(&mut json, &condition.expected_label);
        json.push_str(",\n      \"discharge\": ");
        push_json_string(&mut json, route);
        json.push_str(",\n      \"operation_count\": ");
        match operation_count {
            Some(count) => json.push_str(&count.to_string()),
            None => json.push_str("null"),
        }
        json.push_str(",\n      \"evidence_facts\": [");
        for (index, fact) in evidence_facts.iter().enumerate() {
            if index > 0 {
                json.push_str(", ");
            }
            json.push_str(&fact.arena_index().to_string());
        }
        json.push(']');
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

/// Normalized per-state claim outcome maps and content projections retained by
/// the checked ownership and qualification passes. This proof/debug artifact
/// exposes exact output paths, input-or-established sources, and the closed
/// symbolic content expression without making presentation spelling part of
/// public contract identity. Exact identity-preserving content rewrites are
/// retained beside the outcome rows that justify them; admitted backing and
/// complete frontier witnesses join this same artifact when their source
/// surfaces land.
pub fn claim_outcome_manifest_json(program: &CheckedTrees) -> String {
    let ownership = &program.facts.flow.ownership;
    let mut json = String::from("{\n  \"claim_outcome_maps\": [");
    let mut claim_outcome_coordinates = Vec::new();
    for (map_index, (_, map)) in ownership.claim_outcome_maps.iter().enumerate() {
        if map_index > 0 {
            json.push(',');
        }
        let coordinate = (map.machine_symbol, map.state_symbol);
        assert!(
            !claim_outcome_coordinates.contains(&coordinate),
            "claim outcome maps must retain one row per exact machine and state",
        );
        claim_outcome_coordinates.push(coordinate);
        let entries = validated_claim_outcome_entries(program, map);
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, &symbol_label(program, map.machine_symbol));
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, map.machine_symbol)
                .expect("claim outcome map must name an exact owning machine"),
        );
        json.push_str(",\n      \"state\": ");
        push_json_string(
            &mut json,
            &state_label_from_symbol(program, map.state_symbol),
        );
        json.push_str(",\n      \"entries\": [");
        for (entry_index, entry) in entries.iter().enumerate() {
            if entry_index > 0 {
                json.push(',');
            }
            json.push_str("\n        {\n          \"output_path\": ");
            push_claim_path_json(
                &mut json,
                program,
                ownership.segments.span_or_empty(entry.output_segments),
            );
            json.push_str(",\n          \"source\": ");
            push_claim_outcome_source_json(&mut json, program, entry.source);
            json.push_str("\n        }");
        }
        json.push_str("\n      ]\n    }");
    }
    let mut content_projections = validated_content_projection_plans(program);
    content_projections.sort_by_key(|plan| {
        (
            qualification_symbol_label(program, plan.domain),
            plan.report_fingerprint,
        )
    });
    json.push_str("\n  ],\n  \"content_projections\": [");
    for (index, plan) in content_projections.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"domain\": ");
        push_json_string(&mut json, &qualification_symbol_label(program, plan.domain));
        json.push_str(",\n      \"semantic_domain_id\": ");
        json.push_str(&plan.semantic_domain.0.to_string());
        json.push_str(",\n      \"carrier\": ");
        push_json_string(&mut json, &plan.carrier_identity);
        json.push_str(",\n      \"projection_machine\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, plan.machine),
        );
        json.push_str(",\n      \"projection_machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, plan.machine)
                .expect("content projection plan must name an exact projection machine"),
        );
        json.push_str(",\n      \"algebra\": ");
        push_content_algebra_json(&mut json, &plan.algebra);
        json.push_str(",\n      \"normalized_projection\": ");
        push_content_projection_json(&mut json, &plan.expression);
        json.push_str(",\n      \"report_fingerprint\": ");
        push_json_string(&mut json, &format!("0x{:016x}", plan.report_fingerprint));
        json.push_str("\n    }");
    }
    let mut identity_reshuffles = program
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .collect::<Vec<_>>();
    identity_reshuffles.sort_by_key(|row| {
        (
            state_label_from_symbol(program, row.state_symbol),
            psi_language_semantics::content::content_conservation_plan_bytes(&row.plan),
        )
    });
    json.push_str("\n  ],\n  \"content_identity_reshuffles\": [");
    let mut identity_reshuffle_keys = Vec::new();
    for (index, row) in identity_reshuffles.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        validate_content_conservation_plan(program, &content_projections, &row.plan);
        assert!(
            row.plan.owner_kind
                == psi_language_semantics::content::ContentConservationOwnerKind::Machine
                && row.machine_symbol == row.plan.owner
                && row.state_symbol == row.plan.callable,
            "content identity reshuffle must retain its exact plan owner and callable",
        );
        validate_content_identity_reshuffle(program, row);
        let key = (
            row.machine_symbol,
            row.state_symbol,
            row.claim_identity,
            row.input_parameter_symbol,
            row.input_segments,
            row.output_segments,
            row.plan.report_fingerprint,
        );
        assert!(
            !identity_reshuffle_keys.contains(&key),
            "content identity reshuffles must retain one exact witness row per plan",
        );
        identity_reshuffle_keys.push(key);
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, &symbol_label(program, row.machine_symbol));
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, row.machine_symbol)
                .expect("content identity reshuffle must name an exact owning machine"),
        );
        json.push_str(",\n      \"state\": ");
        push_json_string(
            &mut json,
            &state_label_from_symbol(program, row.state_symbol),
        );
        json.push_str(",\n      \"claim_identity\": ");
        push_claim_identity_json(&mut json, program, row.claim_identity);
        json.push_str(",\n      \"input\": {\"parameter\": ");
        push_json_string(
            &mut json,
            &symbol_label(program, row.input_parameter_symbol),
        );
        json.push_str(", \"path\": ");
        push_claim_path_json(
            &mut json,
            program,
            ownership
                .segments
                .span(row.input_segments)
                .expect("validated content identity reshuffle input path"),
        );
        json.push_str("},\n      \"output_path\": ");
        push_claim_path_json(
            &mut json,
            program,
            ownership
                .segments
                .span(row.output_segments)
                .expect("validated content identity reshuffle output path"),
        );
        json.push_str(",\n      \"algebra\": ");
        push_content_algebra_json(&mut json, &row.plan.algebra);
        json.push_str(",\n      \"equation\": {\"left\": ");
        push_content_conservation_term_json(&mut json, program, row.plan.equation.left());
        json.push_str(", \"right\": ");
        push_content_conservation_term_json(&mut json, program, row.plan.equation.right());
        json.push_str("},\n      \"report_fingerprint\": ");
        push_json_string(
            &mut json,
            &format!("0x{:016x}", row.plan.report_fingerprint),
        );
        json.push_str("\n    }");
    }
    let mut partition_compositions = program
        .facts
        .qualifications
        .content
        .partition_compositions
        .iter()
        .collect::<Vec<_>>();
    validate_content_partition_lineage(program);
    partition_compositions.sort_by_key(|row| {
        (
            state_label_from_symbol(program, row.state_symbol),
            psi_language_semantics::content::content_conservation_plan_bytes(&row.plan),
        )
    });
    json.push_str("\n  ],\n  \"content_partition_compositions\": [");
    let mut partition_composition_keys = Vec::new();
    for (index, row) in partition_compositions.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        validate_content_conservation_plan(program, &content_projections, &row.source_plan);
        validate_content_conservation_plan(program, &content_projections, &row.plan);
        assert!(
            row.plan.owner_kind
                == psi_language_semantics::content::ContentConservationOwnerKind::Machine
                && row.machine_symbol == row.plan.owner
                && row.state_symbol == row.plan.callable,
            "content partition composition must retain its exact derived-plan owner and callable",
        );
        assert!(
            row.source_callable == row.source_plan.callable
                && row.source_report_fingerprint == row.source_plan.report_fingerprint,
            "content partition composition must retain its exact source-plan coordinates",
        );
        validate_content_partition_input_custody(program, row);
        validate_content_partition_substitution_replay(row);
        validate_content_partition_result_rewrites(program, row);
        let key = (
            row.machine_symbol,
            row.state_symbol,
            row.statement_index,
            row.call_ordinal,
            row.source_report_fingerprint,
            row.plan.report_fingerprint,
        );
        assert!(
            !partition_composition_keys.contains(&key),
            "content partition compositions must retain one exact row per call and plan",
        );
        partition_composition_keys.push(key);
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, &symbol_label(program, row.machine_symbol));
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, row.machine_symbol)
                .expect("content partition composition must name an exact owning machine"),
        );
        json.push_str(",\n      \"state\": ");
        push_json_string(
            &mut json,
            &state_label_from_symbol(program, row.state_symbol),
        );
        json.push_str(",\n      \"source_callable\": ");
        push_json_string(&mut json, &symbol_label(program, row.source_callable));
        json.push_str(",\n      \"source_callable_overload_identity\": ");
        push_json_string(
            &mut json,
            &callable_overload_identity(program, row.source_plan.owner, row.source_callable)
                .expect("content partition composition must name an exact source callable"),
        );
        json.push_str(",\n      \"source_report_fingerprint\": ");
        push_json_string(
            &mut json,
            &format!("0x{:016x}", row.source_report_fingerprint),
        );
        json.push_str(",\n      \"source_derivation_depth\": ");
        json.push_str(&row.source_derivation_depth.to_string());
        json.push_str(",\n      \"source_equation\": {\"left\": ");
        push_content_conservation_term_json(&mut json, program, row.source_plan.equation.left());
        json.push_str(", \"right\": ");
        push_content_conservation_term_json(&mut json, program, row.source_plan.equation.right());
        json.push_str("},\n      \"substitutions\": [");
        for (substitution_index, substitution) in row.substitutions.iter().enumerate() {
            if substitution_index > 0 {
                json.push_str(", ");
            }
            json.push_str("{\"source\": ");
            push_content_structural_place_json(&mut json, &substitution.source);
            json.push_str(", \"target\": ");
            push_content_structural_place_json(&mut json, &substitution.target);
            json.push('}');
        }
        json.push(']');
        json.push_str(",\n      \"call\": {\"statement_index\": ");
        json.push_str(&row.statement_index.to_string());
        json.push_str(", \"call_ordinal\": ");
        json.push_str(&row.call_ordinal.to_string());
        json.push_str("},\n      \"input_claim_identities\": [");
        for (claim_index, identity) in row.input_claim_identities.iter().enumerate() {
            if claim_index > 0 {
                json.push_str(", ");
            }
            push_claim_identity_json(&mut json, program, *identity);
        }
        json.push_str("],\n      \"input_claim_bindings\": [");
        for (binding_index, binding) in row.input_claim_bindings.iter().enumerate() {
            if binding_index > 0 {
                json.push_str(", ");
            }
            json.push_str("{\"claim_identity\": ");
            push_claim_identity_json(&mut json, program, binding.claim_identity);
            json.push_str(", \"entry_place\": ");
            push_content_structural_place_json(&mut json, &binding.entry_place);
            json.push('}');
        }
        json.push_str("],\n      \"result_rewrites\": [");
        for (rewrite_index, rewrite) in row.result_rewrites.iter().enumerate() {
            if rewrite_index > 0 {
                json.push_str(", ");
            }
            json.push_str("{\"claim_identity\": ");
            push_claim_identity_json(&mut json, program, rewrite.claim_identity);
            json.push_str(", \"source\": ");
            push_content_structural_place_json(&mut json, &rewrite.source);
            json.push_str(", \"target\": ");
            push_content_structural_place_json(&mut json, &rewrite.target);
            json.push('}');
        }
        json.push_str("],\n      \"algebra\": ");
        push_content_algebra_json(&mut json, &row.plan.algebra);
        json.push_str(",\n      \"equation\": {\"left\": ");
        push_content_conservation_term_json(&mut json, program, row.plan.equation.left());
        json.push_str(", \"right\": ");
        push_content_conservation_term_json(&mut json, program, row.plan.equation.right());
        json.push_str("},\n      \"report_fingerprint\": ");
        push_json_string(
            &mut json,
            &format!("0x{:016x}", row.plan.report_fingerprint),
        );
        json.push_str("\n    }");
    }
    let mut conservation = program
        .facts
        .qualifications
        .content
        .conservation_plans
        .iter()
        .collect::<Vec<_>>();
    conservation.sort_by_key(|plan| {
        (
            symbol_label(program, plan.callable),
            plan.report_fingerprint,
        )
    });
    json.push_str("\n  ],\n  \"content_conservation\": [");
    let mut authored_keys = Vec::new();
    for (index, plan) in conservation.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        validate_content_conservation_plan(program, &content_projections, plan);
        let key = (
            plan.owner_kind,
            plan.owner,
            plan.callable,
            plan.algebra.clone(),
        );
        assert!(
            !authored_keys.contains(&key),
            "content conservation plans must retain one authored row per exact owner, callable, and algebra",
        );
        authored_keys.push(key);
        json.push_str("\n    {\n      \"owner_kind\": ");
        push_json_string(
            &mut json,
            match plan.owner_kind {
                psi_language_semantics::content::ContentConservationOwnerKind::Machine => "machine",
                psi_language_semantics::content::ContentConservationOwnerKind::TraitRequirement => {
                    "trait_requirement"
                }
            },
        );
        json.push_str(",\n      \"owner\": ");
        push_json_string(&mut json, &symbol_label(program, plan.owner));
        json.push_str(",\n      \"callable\": ");
        push_json_string(&mut json, &symbol_label(program, plan.callable));
        json.push_str(",\n      \"callable_overload_identity\": ");
        push_json_string(
            &mut json,
            &callable_overload_identity(program, plan.owner, plan.callable)
                .expect("content conservation plan must name an exact callable"),
        );
        json.push_str(",\n      \"algebra\": ");
        push_content_algebra_json(&mut json, &plan.algebra);
        json.push_str(",\n      \"equation\": {\"left\": ");
        push_content_conservation_term_json(&mut json, program, plan.equation.left());
        json.push_str(", \"right\": ");
        push_content_conservation_term_json(&mut json, program, plan.equation.right());
        json.push_str("},\n      \"report_fingerprint\": ");
        push_json_string(&mut json, &format!("0x{:016x}", plan.report_fingerprint));
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

fn validated_claim_outcome_entries<'program>(
    program: &'program CheckedTrees,
    map: &psi_checked_trees::FlowClaimOutcomeMapFact,
) -> &'program [psi_checked_trees::FlowClaimOutcomeEntryFact] {
    use psi_checked_trees::FlowClaimOutcomeSource;
    use psi_language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
        PermissionProvenance,
    };

    let ownership = &program.facts.flow.ownership;
    let mut machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == map.machine_symbol);
    let machine = machines
        .next()
        .expect("claim outcome map must name an exact typed machine");
    assert!(
        machines.next().is_none(),
        "claim outcome map machine must resolve to exactly one typed machine",
    );
    let mut states = program
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == map.state_symbol);
    let state = states
        .next()
        .expect("claim outcome map state must belong to its exact typed machine");
    assert!(
        states.next().is_none(),
        "claim outcome map state must resolve to exactly one state owned by its machine",
    );
    let entries = ownership
        .claim_outcome_entries
        .span(map.entries)
        .expect("claim outcome map must retain an exact valid entry span");
    let mut output_paths = Vec::new();
    for entry in entries {
        let output_path = ownership
            .segments
            .span(entry.output_segments)
            .expect("claim outcome entry must retain an exact valid output path span");
        assert!(
            !output_paths.contains(&output_path),
            "claim outcome map must retain one entry per exact output path",
        );
        output_paths.push(output_path);
        match entry.source {
            FlowClaimOutcomeSource::Unknown => {
                panic!("claim outcome entry must retain an exact known source")
            }
            FlowClaimOutcomeSource::Input {
                parameter_symbol,
                segments,
            } => {
                assert!(
                    program
                        .state_parameters(state)
                        .iter()
                        .any(|parameter| parameter.symbol == parameter_symbol),
                    "claim outcome input source must name an exact parameter owned by its state",
                );
                let source_path = ownership
                    .segments
                    .span(segments)
                    .expect("claim outcome input source must retain an exact valid path span");
                let mut origins = Vec::new();
                for (_, event) in ownership.permissions.iter().filter(|(_, event)| {
                    event.machine_symbol == map.machine_symbol
                        && event.state_symbol == map.state_symbol
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                        && event.obligation_live
                        && event.root == psi_facts::PlaceRoot::Symbol(parameter_symbol)
                        && ownership.segments.span(event.segments) == Some(source_path)
                        && event.claim_identity != PermissionClaimIdentity::Unknown
                        && event.provenance != PermissionProvenance::Unknown
                }) {
                    let origin = (event.claim_identity, event.provenance);
                    if !origins.contains(&origin) {
                        origins.push(origin);
                    }
                }
                assert_eq!(
                    origins.len(),
                    1,
                    "claim outcome input source must resolve to one distinct live retained permission origin",
                );
            }
            FlowClaimOutcomeSource::Established {
                claim_identity,
                provenance,
            } => {
                assert!(
                    claim_identity != PermissionClaimIdentity::Unknown,
                    "claim outcome established source must retain a non-unknown claim identity",
                );
                assert!(
                    provenance != PermissionProvenance::Unknown,
                    "claim outcome established source must retain non-unknown provenance",
                );
                assert!(
                    ownership.permissions.iter().any(|(_, event)| {
                        event.claim_identity == claim_identity && event.provenance == provenance
                    }),
                    "claim outcome established source must match one retained permission event",
                );
            }
        }
    }
    entries
}

fn validated_content_projection_plans(
    program: &CheckedTrees,
) -> Vec<&psi_language_semantics::content::ContentProjectionPlan> {
    use psi_language_semantics::content::projection_report_fingerprint;

    let mut seen_domains = Vec::new();
    let mut seen_semantic_domains = Vec::new();
    program
        .facts
        .qualifications
        .content
        .plans
        .iter()
        .map(|plan| {
            assert!(
                plan.domain.is_valid(),
                "content projection plan must name a nonempty exact declared domain",
            );
            let mut domains = program
                .domain_definitions()
                .iter()
                .filter(|domain| domain.symbol == plan.domain);
            let domain = domains
                .next()
                .expect("content projection plan must name a nonempty exact declared domain");
            assert!(
                domains.next().is_none(),
                "content projection plan domain must resolve to exactly one declaration",
            );
            assert!(
                plan.semantic_domain.is_valid()
                    && domain.semantic_id == plan.semantic_domain
                    && program
                        .semantic_domains
                        .name(plan.semantic_domain)
                        .is_some(),
                "content projection plan must retain its exact registered semantic domain",
            );
            assert!(
                !plan.carrier_identity.is_empty()
                    && domain.target_type.is_valid()
                    && plan.carrier_identity
                        == program
                            .normalized_type_identity(domain.target_type)
                            .into_string(),
                "content projection plan must retain its exact normalized carrier identity",
            );
            let mut machines = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == plan.machine);
            machines
                .next()
                .expect("content projection plan must name an exact typed projection machine");
            assert!(
                machines.next().is_none(),
                "content projection plan machine must resolve to exactly one typed machine",
            );
            assert_eq!(
                plan.report_fingerprint,
                projection_report_fingerprint(&plan.algebra, &plan.expression),
                "content projection plan must retain its exact normalized report_fingerprint",
            );
            assert!(
                !seen_domains.contains(&plan.domain),
                "content projection plans must retain one row per exact domain",
            );
            seen_domains.push(plan.domain);
            assert!(
                !seen_semantic_domains.contains(&plan.semantic_domain),
                "content projection plans must retain one row per exact semantic domain",
            );
            seen_semantic_domains.push(plan.semantic_domain);
            plan
        })
        .collect()
}

fn validate_content_conservation_plan(
    program: &CheckedTrees,
    projection_plans: &[&psi_language_semantics::content::ContentProjectionPlan],
    plan: &psi_language_semantics::content::ContentConservationPlan,
) {
    use psi_language_semantics::content::{
        ContentConservationOwnerKind, conservation_report_fingerprint,
    };

    match plan.owner_kind {
        ContentConservationOwnerKind::Machine => {
            let mut owners = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == plan.owner);
            let owner = owners
                .next()
                .expect("content conservation machine owner must name an exact typed machine");
            assert!(
                owners.next().is_none(),
                "content conservation machine owner must resolve to exactly one typed machine",
            );
            let mut callables = program
                .machine_states(owner)
                .iter()
                .filter(|state| state.symbol == plan.callable);
            callables.next().expect(
                "content conservation machine callable must be a state owned by its exact machine",
            );
            assert!(
                callables.next().is_none(),
                "content conservation machine callable must resolve to exactly one owned state",
            );
        }
        ContentConservationOwnerKind::TraitRequirement => {
            let mut owners = program
                .traits()
                .iter()
                .filter(|definition| definition.symbol == plan.owner);
            let owner = owners.next().expect(
                "content conservation trait owner must name an exact typed trait definition",
            );
            assert!(
                owners.next().is_none(),
                "content conservation trait owner must resolve to exactly one trait definition",
            );
            let mut callables = program
                .trait_machine_signatures(owner)
                .iter()
                .filter(|signature| signature.symbol == plan.callable);
            callables.next().expect(
                "content conservation trait callable must be a requirement owned by its exact trait",
            );
            assert!(
                callables.next().is_none(),
                "content conservation trait callable must resolve to exactly one owned requirement",
            );
        }
    }
    assert_eq!(
        plan.report_fingerprint,
        conservation_report_fingerprint(&plan.algebra, &plan.equation),
        "content conservation plan must retain its exact normalized report_fingerprint",
    );
    validate_content_conservation_term(projection_plans, &plan.algebra, plan.equation.left());
    validate_content_conservation_term(projection_plans, &plan.algebra, plan.equation.right());
}

fn validate_content_conservation_term(
    projection_plans: &[&psi_language_semantics::content::ContentProjectionPlan],
    algebra: &psi_language_semantics::content::ContentAlgebraIdentity,
    term: &psi_language_semantics::content::ContentConservationTerm,
) {
    use psi_language_semantics::content::ContentConservationTerm;

    match term {
        ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            projection_report_fingerprint,
            ..
        } => {
            let mut matches = projection_plans.iter().filter(|plan| {
                plan.domain == *domain
                    && plan.semantic_domain == *semantic_domain
                    && plan.machine == *projection_machine
                    && plan.report_fingerprint == *projection_report_fingerprint
            });
            let projection = matches.next().expect(
                "content conservation projection term must join one exact retained projection plan",
            );
            assert!(
                matches.next().is_none(),
                "content conservation projection term must join exactly one retained projection plan",
            );
            assert_eq!(
                &projection.algebra, algebra,
                "content conservation projection term must retain the plan's exact algebra",
            );
        }
        ContentConservationTerm::Separate(terms) => {
            for term in terms {
                validate_content_conservation_term(projection_plans, algebra, term);
            }
        }
    }
}

fn validate_content_identity_reshuffle(
    program: &CheckedTrees,
    row: &psi_checked_trees::ContentIdentityReshuffleFact,
) {
    use psi_checked_trees::FlowClaimOutcomeSource;
    use psi_language_semantics::content::{
        ContentConservationTerm, ContentPlaceRoot, ContentPlaceVersion, ContentStructuralPlace,
    };
    use psi_language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    };

    let ownership = &program.facts.flow.ownership;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == row.machine_symbol)
        .expect("content identity reshuffle must name an exact typed machine");
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == row.state_symbol)
        .expect("content identity reshuffle state must belong to its exact typed machine");
    let mut parameters = program
        .state_parameters(state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.symbol == row.input_parameter_symbol);
    let (parameter_position, parameter) = parameters
        .next()
        .expect("content identity reshuffle input must name an exact parameter owned by its state");
    assert!(
        parameters.next().is_none(),
        "content identity reshuffle input must resolve to exactly one parameter owned by its state",
    );
    let input_path = ownership
        .segments
        .span(row.input_segments)
        .expect("content identity reshuffle input must retain an exact valid path span");
    let output_path = ownership
        .segments
        .span(row.output_segments)
        .expect("content identity reshuffle output must retain an exact valid path span");
    assert!(
        row.claim_identity != PermissionClaimIdentity::Unknown,
        "content identity reshuffle must retain a non-unknown claim identity",
    );
    let mut entry_identities = Vec::new();
    for (_, event) in ownership.permissions.iter().filter(|(_, event)| {
        event.machine_symbol == row.machine_symbol
            && event.state_symbol == row.state_symbol
            && event.source == PermissionEventSource::StateEntry
            && event.kind == PermissionEventKind::Establish
            && event.access == PermissionAccess::Owned
            && event.obligation_live
            && event.root == psi_facts::PlaceRoot::Symbol(row.input_parameter_symbol)
            && ownership.segments.span(event.segments) == Some(input_path)
            && event.claim_identity != PermissionClaimIdentity::Unknown
    }) {
        if !entry_identities.contains(&event.claim_identity) {
            entry_identities.push(event.claim_identity);
        }
    }
    let [entry_identity] = entry_identities.as_slice() else {
        panic!(
            "content identity reshuffle input must resolve to one distinct live retained permission identity"
        )
    };
    assert_eq!(
        row.claim_identity, *entry_identity,
        "content identity reshuffle must retain its exact input permission identity",
    );

    let mut maps = ownership.claim_outcome_maps.iter().filter(|(_, map)| {
        map.machine_symbol == row.machine_symbol && map.state_symbol == row.state_symbol
    });
    let map = maps
        .next()
        .expect("content identity reshuffle must name one exact retained claim outcome map")
        .1;
    assert!(
        maps.next().is_none(),
        "content identity reshuffle must name exactly one retained claim outcome map",
    );
    let matching_outcomes = validated_claim_outcome_entries(program, map)
        .iter()
        .filter(|entry| {
            ownership.segments.span(entry.output_segments) == Some(output_path)
                && matches!(
                    entry.source,
                    FlowClaimOutcomeSource::Input {
                        parameter_symbol,
                        segments,
                    } if parameter_symbol == row.input_parameter_symbol
                        && ownership.segments.span(segments) == Some(input_path)
                )
        })
        .count();
    assert_eq!(
        matching_outcomes, 1,
        "content identity reshuffle must retain one exact input-relative claim outcome",
    );

    let input_subject = ContentStructuralPlace {
        version: ContentPlaceVersion::Entry,
        root: ContentPlaceRoot::Parameter {
            position: u32::try_from(parameter_position)
                .expect("content identity reshuffle parameter position must fit u32"),
            symbol: parameter.symbol,
            name: parameter.name.as_str().to_owned(),
            is_self: parameter.is_self,
        },
        segments: exact_content_path(program, input_path),
    };
    let output_subject = ContentStructuralPlace {
        version: ContentPlaceVersion::Current,
        root: ContentPlaceRoot::Result,
        segments: exact_content_path(program, output_path),
    };
    fn projection_subject(term: &ContentConservationTerm) -> Option<&ContentStructuralPlace> {
        match term {
            ContentConservationTerm::Projection { subject, .. } => Some(subject),
            ContentConservationTerm::Separate(_) => None,
        }
    }
    let left = projection_subject(row.plan.equation.left());
    let right = projection_subject(row.plan.equation.right());
    assert!(
        matches!(
            (left, right),
            (Some(left), Some(right))
                if (left == &input_subject && right == &output_subject)
                    || (left == &output_subject && right == &input_subject)
        ),
        "content identity reshuffle equation must retain its exact input and output projection subjects",
    );
}

fn exact_content_path(
    program: &CheckedTrees,
    path: &[psi_facts::PlaceSegment],
) -> Vec<psi_language_semantics::content::ContentPlaceSegment> {
    use psi_language_semantics::content::{
        ContentCaseSegment, ContentFieldSegment, ContentPlaceSegment,
    };

    path.iter()
        .map(|segment| match segment {
            psi_facts::PlaceSegment::Case { variant } => {
                let mut variants = program.data_definitions().iter().flat_map(|definition| {
                    program
                        .data_members(definition)
                        .iter()
                        .filter_map(|member| match member {
                            psi_typed_trees::data::DataMember::Variant(candidate)
                                if candidate.symbol == *variant =>
                            {
                                Some(candidate)
                            }
                            psi_typed_trees::data::DataMember::Field(_)
                            | psi_typed_trees::data::DataMember::Variant(_) => None,
                        })
                });
                let candidate = variants.next().expect(
                    "content identity reshuffle case path must name an exact typed variant",
                );
                assert!(
                    variants.next().is_none(),
                    "content identity reshuffle case path must resolve to exactly one typed variant",
                );
                ContentPlaceSegment::Case(ContentCaseSegment {
                    symbol: candidate.symbol,
                    name: candidate.name.as_str().to_owned(),
                })
            }
            psi_facts::PlaceSegment::Field { symbol } => {
                let mut fields = program.data_definitions().iter().flat_map(|definition| {
                    program
                        .data_members(definition)
                        .iter()
                        .flat_map(|member| match member {
                            psi_typed_trees::data::DataMember::Field(field) => {
                                std::slice::from_ref(field)
                            }
                            psi_typed_trees::data::DataMember::Variant(variant) => {
                                program.data_payload_fields(variant)
                            }
                        })
                        .filter(|field| field.symbol == *symbol)
                });
                let field = fields.next().expect(
                    "content identity reshuffle field path must name an exact typed field",
                );
                assert!(
                    fields.next().is_none(),
                    "content identity reshuffle field path must resolve to exactly one typed field",
                );
                ContentPlaceSegment::Field(ContentFieldSegment {
                    symbol: field.symbol,
                    name: field.name.as_str().to_owned(),
                })
            }
            psi_facts::PlaceSegment::FixedIndex { index } => {
                ContentPlaceSegment::FixedIndex(
                    u64::try_from(*index)
                        .expect("content identity reshuffle fixed index must fit u64"),
                )
            }
            psi_facts::PlaceSegment::FixedRange { .. } => {
                panic!("content identity reshuffle paths must not retain a range")
            }
            psi_facts::PlaceSegment::Index { .. } => {
                panic!("content identity reshuffle paths must not retain a runtime index")
            }
        })
        .collect()
}

fn validate_content_partition_input_custody(
    program: &CheckedTrees,
    row: &psi_checked_trees::ContentPartitionCompositionFact,
) {
    use psi_language_semantics::content::{ContentPlaceRoot, ContentPlaceVersion};
    use psi_language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    };

    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == row.machine_symbol)
        .expect("content partition composition must name an exact typed machine");
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == row.state_symbol)
        .expect("content partition composition state must belong to its exact typed machine");
    program
        .statement_table
        .statements(state.statement_nodes)
        .get(row.statement_index)
        .expect("content partition composition statement index must be within its exact state");
    let mut flow_states = program
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, fact)| {
            fact.machine_symbol == row.machine_symbol && fact.state_symbol == row.state_symbol
        });
    let flow_state = flow_states
        .next()
        .expect("content partition composition must name one exact checked flow state")
        .1;
    assert!(
        flow_states.next().is_none(),
        "content partition composition must name exactly one checked flow state",
    );
    let calls = program
        .facts
        .flow
        .control
        .calls
        .span(flow_state.calls)
        .expect("content partition composition flow state must retain an exact valid call span");
    let mut coordinate_calls = calls.iter().filter(|call| {
        call.statement_index == row.statement_index && call.call_ordinal == row.call_ordinal
    });
    let call = coordinate_calls
        .next()
        .expect("content partition composition must retain one exact checked call coordinate");
    assert!(
        coordinate_calls.next().is_none(),
        "content partition composition must retain exactly one checked call coordinate",
    );
    assert_eq!(
        call.target_symbol, row.source_callable,
        "content partition composition must retain its exact checked source target",
    );

    assert!(
        !row.input_claim_identities.is_empty(),
        "content partition composition must retain at least one input claim identity",
    );
    let mut identities = Vec::new();
    for identity in &row.input_claim_identities {
        assert!(
            *identity != PermissionClaimIdentity::Unknown,
            "content partition composition input claim identities must be non-unknown",
        );
        assert!(
            !identities.contains(identity),
            "content partition composition input claim identities must be unique",
        );
        identities.push(*identity);
    }
    assert_eq!(
        row.input_claim_identities,
        row.input_claim_bindings
            .iter()
            .map(|binding| binding.claim_identity)
            .collect::<Vec<_>>(),
        "content partition composition input identities must exactly match ordered bindings",
    );
    for binding in &row.input_claim_bindings {
        let (position, symbol, name, is_self) = match &binding.entry_place.root {
            ContentPlaceRoot::Parameter {
                position,
                symbol,
                name,
                is_self,
            } if binding.entry_place.version == ContentPlaceVersion::Entry => {
                (*position, *symbol, name, *is_self)
            }
            _ => panic!(
                "content partition composition input binding must retain an entry parameter place"
            ),
        };
        let parameter = program
            .state_parameters(state)
            .get(usize::try_from(position).expect("partition input position must fit usize"))
            .filter(|parameter| {
                parameter.symbol == symbol
                    && parameter.name.as_str() == name
                    && parameter.is_self == is_self
            })
            .expect(
                "content partition composition input binding must name its exact caller parameter",
            );
        let matching_events = program
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == row.machine_symbol
                    && event.state_symbol == row.state_symbol
                    && event.source == PermissionEventSource::StateEntry
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.claim_identity == binding.claim_identity
                    && event.root == psi_facts::PlaceRoot::Symbol(parameter.symbol)
            })
            .filter(|(_, event)| {
                let path = program
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span(event.segments)
                    .expect("partition input permission event must retain an exact valid path");
                exact_content_path(program, path) == binding.entry_place.segments
            })
            .count();
        assert_eq!(
            matching_events, 1,
            "content partition composition input binding must match one live retained permission event",
        );
    }
}

fn validate_content_partition_substitution_replay(
    row: &psi_checked_trees::ContentPartitionCompositionFact,
) {
    use psi_language_semantics::content::{ContentConservationEquation, ContentConservationTerm};

    fn contains_separate(term: &ContentConservationTerm) -> bool {
        match term {
            ContentConservationTerm::Projection { .. } => false,
            ContentConservationTerm::Separate(_) => true,
        }
    }

    fn collect_subjects<'term>(
        term: &'term ContentConservationTerm,
        subjects: &mut Vec<&'term psi_language_semantics::content::ContentStructuralPlace>,
    ) {
        match term {
            ContentConservationTerm::Projection { subject, .. } => subjects.push(subject),
            ContentConservationTerm::Separate(terms) => {
                for term in terms {
                    collect_subjects(term, subjects);
                }
            }
        }
    }

    fn replay(
        term: &ContentConservationTerm,
        substitutions: &[psi_checked_trees::ContentPartitionPlaceSubstitution],
    ) -> ContentConservationTerm {
        match term {
            ContentConservationTerm::Projection {
                domain,
                semantic_domain,
                projection_machine,
                projection_report_fingerprint,
                subject,
            } => {
                let target = substitutions
                    .iter()
                    .find(|substitution| substitution.source == *subject)
                    .expect(
                        "content partition substitution replay must cover every source subject",
                    );
                ContentConservationTerm::Projection {
                    domain: *domain,
                    semantic_domain: *semantic_domain,
                    projection_machine: *projection_machine,
                    projection_report_fingerprint: *projection_report_fingerprint,
                    subject: target.target.clone(),
                }
            }
            ContentConservationTerm::Separate(terms) => ContentConservationTerm::Separate(
                terms
                    .iter()
                    .map(|term| replay(term, substitutions))
                    .collect(),
            ),
        }
    }

    assert!(
        contains_separate(row.source_plan.equation.left())
            || contains_separate(row.source_plan.equation.right()),
        "content partition composition source equation must retain an authored partition",
    );
    assert!(
        !row.substitutions.is_empty(),
        "content partition composition must retain a nonempty exact substitution map",
    );
    for (index, substitution) in row.substitutions.iter().enumerate() {
        assert!(
            row.substitutions[..index]
                .iter()
                .all(|previous| previous.source != substitution.source),
            "content partition composition substitution sources must be unique",
        );
        assert!(
            row.substitutions[..index]
                .iter()
                .all(|previous| previous.target != substitution.target),
            "content partition composition substitution targets must be unique",
        );
    }
    let mut subjects = Vec::new();
    collect_subjects(row.source_plan.equation.left(), &mut subjects);
    collect_subjects(row.source_plan.equation.right(), &mut subjects);
    for substitution in &row.substitutions {
        assert!(
            subjects.contains(&&substitution.source),
            "content partition composition substitution source must occur in the source equation",
        );
    }
    for subject in subjects {
        assert_eq!(
            row.substitutions
                .iter()
                .filter(|substitution| substitution.source == *subject)
                .count(),
            1,
            "content partition composition must cover every source subject exactly once",
        );
    }
    let replayed = ContentConservationEquation::new(
        replay(row.source_plan.equation.left(), &row.substitutions),
        replay(row.source_plan.equation.right(), &row.substitutions),
    );
    assert_eq!(
        row.source_plan.algebra, row.plan.algebra,
        "content partition composition replay must preserve the exact source algebra",
    );
    assert_eq!(
        replayed, row.plan.equation,
        "content partition composition derived equation must equal exact substitution replay",
    );
}

fn validate_content_partition_result_rewrites(
    program: &CheckedTrees,
    row: &psi_checked_trees::ContentPartitionCompositionFact,
) {
    use psi_checked_trees::FlowClaimOutcomeSource;
    use psi_language_semantics::content::{ContentPlaceRoot, ContentPlaceVersion};
    use psi_language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    };

    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == row.machine_symbol)
        .expect("content partition result rewrite must name an exact typed machine");
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == row.state_symbol)
        .expect("content partition result rewrite state must belong to its exact typed machine");
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(row.statement_index)
        .expect("content partition result rewrite statement must be within its exact state");

    for (index, rewrite) in row.result_rewrites.iter().enumerate() {
        assert!(
            rewrite.claim_identity != PermissionClaimIdentity::Unknown,
            "content partition result rewrite must retain a non-unknown claim identity",
        );
        assert!(
            row.result_rewrites[..index]
                .iter()
                .all(|previous| previous.claim_identity != rewrite.claim_identity),
            "content partition result rewrite claim identities must be unique",
        );
        assert!(
            row.result_rewrites[..index]
                .iter()
                .all(|previous| previous.source != rewrite.source),
            "content partition result rewrite sources must be unique",
        );
        assert!(
            row.result_rewrites[..index]
                .iter()
                .all(|previous| previous.target != rewrite.target),
            "content partition result rewrite targets must be unique",
        );
        assert!(
            rewrite.source.version == ContentPlaceVersion::Current
                && rewrite.source.root == ContentPlaceRoot::Result,
            "content partition result rewrite source must be an exact current result place",
        );
        assert!(
            rewrite.target.version == ContentPlaceVersion::Current
                && rewrite.target.root == ContentPlaceRoot::Result,
            "content partition result rewrite target must be an exact current result place",
        );
        assert_eq!(
            row.substitutions
                .iter()
                .filter(|substitution| {
                    substitution.source == rewrite.source && substitution.target == rewrite.target
                })
                .count(),
            1,
            "content partition result rewrite must retain one exact substitution pair",
        );
        let psi_typed_trees::statement::StatementNode::LocalData(local) = statement else {
            panic!("content partition result rewrite must belong to its exact staged local")
        };
        let matching_events = program
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                let source_matches = matches!(
                    event.source,
                    PermissionEventSource::Call {
                        statement_index,
                        call_ordinal,
                        target_symbol,
                        ..
                    } if statement_index == row.statement_index
                        && call_ordinal == row.call_ordinal
                        && target_symbol == row.source_callable
                ) || event.source
                    == PermissionEventSource::Statement {
                        statement_index: row.statement_index,
                    };
                event.machine_symbol == row.machine_symbol
                    && event.state_symbol == row.state_symbol
                    && source_matches
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.claim_identity == rewrite.claim_identity
                    && event.root == psi_facts::PlaceRoot::Symbol(local.symbol)
            })
            .filter(|(_, event)| {
                let path = program
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span(event.segments)
                    .expect("content partition result event must retain an exact valid path");
                exact_content_path(program, path) == rewrite.source.segments
            })
            .count();
        assert_eq!(
            matching_events, 1,
            "content partition result rewrite must match one live staged-local permission event",
        );

        let mut outcome_maps = program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .iter()
            .filter(|(_, map)| {
                map.machine_symbol == row.machine_symbol && map.state_symbol == row.state_symbol
            });
        let outcome_map = outcome_maps
            .next()
            .expect("content partition result rewrite must name one exact checked outcome map")
            .1;
        assert!(
            outcome_maps.next().is_none(),
            "content partition result rewrite must name exactly one checked outcome map",
        );
        let entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .span(outcome_map.entries)
            .expect("content partition result outcome map must retain an exact valid entry span");
        let matching_outcomes = entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.source,
                    FlowClaimOutcomeSource::Established { claim_identity, .. }
                        if claim_identity == rewrite.claim_identity
                )
            })
            .filter(|entry| {
                let path = program
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span(entry.output_segments)
                    .expect("content partition result outcome must retain an exact valid path");
                exact_content_path(program, path) == rewrite.target.segments
            })
            .count();
        assert_eq!(
            matching_outcomes, 1,
            "content partition result rewrite must match one exact established outcome",
        );
    }
}

fn validate_content_partition_lineage(program: &CheckedTrees) {
    let authored = &program.facts.qualifications.content.conservation_plans;
    let compositions = &program.facts.qualifications.content.partition_compositions;

    for (index, row) in compositions.iter().enumerate() {
        if row.source_derivation_depth == 0 {
            assert_eq!(
                authored
                    .iter()
                    .filter(|plan| **plan == row.source_plan)
                    .count(),
                1,
                "content partition depth-zero source must match one exact authored plan",
            );
            continue;
        }

        let mut parents = compositions
            .iter()
            .enumerate()
            .filter(|(candidate_index, candidate)| {
                *candidate_index != index && candidate.plan == row.source_plan
            });
        let parent = parents
            .next()
            .expect("content partition derived source must match one distinct exact parent row")
            .1;
        assert!(
            parents.next().is_none(),
            "content partition derived source must match exactly one distinct parent row",
        );
        let expected_depth = parent
            .source_derivation_depth
            .checked_add(1)
            .expect("content partition source derivation depth must not overflow");
        assert_eq!(
            row.source_derivation_depth, expected_depth,
            "content partition source derivation depth must exactly increment its parent",
        );
    }
}

fn push_content_algebra_json(
    json: &mut String,
    algebra: &psi_language_semantics::content::ContentAlgebraIdentity,
) {
    use psi_language_semantics::content::ContentAlgebraIdentity;

    match algebra {
        ContentAlgebraIdentity::IntervalSet { coordinate_space } => {
            json.push_str("{\"kind\": \"interval_set\", \"coordinate_space\": ");
            push_json_string(json, coordinate_space);
            json.push('}');
        }
        ContentAlgebraIdentity::CountedQuantity { unit } => {
            json.push_str("{\"kind\": \"counted_quantity\", \"unit\": ");
            push_json_string(json, unit);
            json.push('}');
        }
    }
}

fn push_content_projection_json(
    json: &mut String,
    projection: &psi_language_semantics::content::ContentProjectionExpression,
) {
    use psi_language_semantics::content::ContentProjectionExpression;

    match projection {
        ContentProjectionExpression::IntervalSet { members } => {
            json.push_str("{\"kind\": \"interval_set\", \"members\": [");
            for (index, member) in members.iter().enumerate() {
                if index > 0 {
                    json.push_str(", ");
                }
                json.push_str("{\"start\": ");
                push_content_scalar_json(json, member.start());
                json.push_str(", \"end\": ");
                push_content_scalar_json(json, member.end());
                json.push('}');
            }
            json.push_str("]}");
        }
        ContentProjectionExpression::CountedQuantity { magnitude } => {
            json.push_str("{\"kind\": \"counted_quantity\", \"magnitude\": ");
            push_content_scalar_json(json, magnitude);
            json.push('}');
        }
    }
}

fn push_content_scalar_json(
    json: &mut String,
    scalar: &psi_language_semantics::content::ContentScalarExpression,
) {
    use psi_language_semantics::content::{ContentArithmeticOperator, ContentScalarExpression};

    match scalar {
        ContentScalarExpression::SubjectField(path) => {
            json.push_str("{\"kind\": \"subject_field\", \"path\": ");
            push_content_field_path_json(json, path);
            json.push('}');
        }
        ContentScalarExpression::RuntimeScalarEmbedding(path) => {
            json.push_str("{\"kind\": \"runtime_scalar_embedding\", \"path\": ");
            push_content_field_path_json(json, path);
            json.push('}');
        }
        ContentScalarExpression::Natural(value) => {
            json.push_str("{\"kind\": \"natural\", \"value\": ");
            push_json_string(json, value);
            json.push('}');
        }
        ContentScalarExpression::Successor(value) => {
            json.push_str("{\"kind\": \"successor\", \"value\": ");
            push_content_scalar_json(json, value);
            json.push('}');
        }
        ContentScalarExpression::Arithmetic {
            operator,
            left,
            right,
        } => {
            json.push_str("{\"kind\": \"arithmetic\", \"operator\": ");
            push_json_string(
                json,
                match operator {
                    ContentArithmeticOperator::Add => "add",
                    ContentArithmeticOperator::Subtract => "subtract",
                    ContentArithmeticOperator::Multiply => "multiply",
                },
            );
            json.push_str(", \"left\": ");
            push_content_scalar_json(json, left);
            json.push_str(", \"right\": ");
            push_content_scalar_json(json, right);
            json.push('}');
        }
    }
}

fn push_content_field_path_json(
    json: &mut String,
    path: &[psi_language_semantics::content::ContentFieldSegment],
) {
    json.push('[');
    for (index, segment) in path.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(json, &segment.name);
    }
    json.push(']');
}

fn push_content_conservation_term_json(
    json: &mut String,
    program: &CheckedTrees,
    term: &psi_language_semantics::content::ContentConservationTerm,
) {
    use psi_language_semantics::content::ContentConservationTerm;

    match term {
        ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            projection_report_fingerprint,
            subject,
        } => {
            json.push_str("{\"kind\": \"projection\", \"domain\": ");
            push_json_string(json, &qualification_symbol_label(program, *domain));
            json.push_str(", \"semantic_domain_id\": ");
            json.push_str(&semantic_domain.0.to_string());
            json.push_str(", \"projection_machine\": ");
            push_json_string(
                json,
                &qualification_symbol_label(program, *projection_machine),
            );
            json.push_str(", \"projection_report_fingerprint\": ");
            push_json_string(json, &format!("0x{projection_report_fingerprint:016x}"));
            json.push_str(", \"place\": ");
            push_content_structural_place_json(json, subject);
            json.push('}');
        }
        ContentConservationTerm::Separate(terms) => {
            json.push_str("{\"kind\": \"separate\", \"terms\": [");
            for (index, term) in terms.iter().enumerate() {
                if index > 0 {
                    json.push_str(", ");
                }
                push_content_conservation_term_json(json, program, term);
            }
            json.push_str("]}");
        }
    }
}

fn push_content_structural_place_json(
    json: &mut String,
    subject: &psi_language_semantics::content::ContentStructuralPlace,
) {
    use psi_language_semantics::content::{
        ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
    };

    json.push_str("{\"version\": ");
    push_json_string(
        json,
        match subject.version {
            ContentPlaceVersion::Entry => "entry",
            ContentPlaceVersion::Current => "current",
        },
    );
    json.push_str(", \"root\": ");
    match &subject.root {
        ContentPlaceRoot::Parameter {
            position,
            name,
            is_self,
            ..
        } => {
            json.push_str("{\"kind\": ");
            push_json_string(json, if *is_self { "self" } else { "parameter" });
            json.push_str(", \"position\": ");
            json.push_str(&position.to_string());
            json.push_str(", \"name\": ");
            push_json_string(json, name);
            json.push('}');
        }
        ContentPlaceRoot::Result => json.push_str("{\"kind\": \"result\"}"),
    }
    json.push_str(", \"path\": [");
    for (index, segment) in subject.segments.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        match segment {
            ContentPlaceSegment::Case(case) => {
                json.push_str("{\"kind\": \"case\", \"name\": ");
                push_json_string(json, &case.name);
                json.push('}');
            }
            ContentPlaceSegment::Field(field) => {
                json.push_str("{\"kind\": \"field\", \"name\": ");
                push_json_string(json, &field.name);
                json.push('}');
            }
            ContentPlaceSegment::FixedIndex(index) => {
                json.push_str("{\"kind\": \"fixed_index\", \"index\": ");
                json.push_str(&index.to_string());
                json.push('}');
            }
        }
    }
    json.push_str("]}");
}

fn push_claim_outcome_source_json(
    json: &mut String,
    program: &CheckedTrees,
    source: psi_checked_trees::FlowClaimOutcomeSource,
) {
    match source {
        psi_checked_trees::FlowClaimOutcomeSource::Unknown => {
            json.push_str("{\"kind\": \"unknown\"}");
        }
        psi_checked_trees::FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments,
        } => {
            json.push_str("{\"kind\": \"input\", \"parameter\": ");
            push_json_string(json, &symbol_label(program, parameter_symbol));
            json.push_str(", \"path\": ");
            push_claim_path_json(
                json,
                program,
                program
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(segments),
            );
            json.push('}');
        }
        psi_checked_trees::FlowClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => {
            json.push_str("{\"kind\": \"established\", \"claim_identity\": ");
            push_claim_identity_json(json, program, claim_identity);
            json.push_str(", \"provenance\": ");
            push_claim_provenance_json(json, program, provenance);
            json.push('}');
        }
    }
}

fn push_claim_path_json(
    json: &mut String,
    program: &CheckedTrees,
    path: &[psi_facts::PlaceSegment],
) {
    json.push('[');
    for (index, segment) in path.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        match segment {
            psi_facts::PlaceSegment::Field { symbol } => {
                json.push_str("{\"field\": ");
                push_json_string(json, &symbol_label(program, *symbol));
                json.push('}');
            }
            psi_facts::PlaceSegment::Case { variant } => {
                json.push_str("{\"case\": ");
                push_json_string(json, &symbol_label(program, *variant));
                json.push('}');
            }
            psi_facts::PlaceSegment::FixedIndex { index } => {
                json.push_str("{\"fixed_index\": ");
                json.push_str(&index.to_string());
                json.push('}');
            }
            psi_facts::PlaceSegment::FixedRange { start, end } => {
                json.push_str("{\"fixed_range\": {\"start\": ");
                json.push_str(&start.to_string());
                json.push_str(", \"end\": ");
                json.push_str(&end.to_string());
                json.push_str("}}");
            }
            psi_facts::PlaceSegment::Index { expression } => {
                json.push_str("{\"index\": ");
                push_json_string(json, &program.expression_table.display_name(*expression));
                json.push('}');
            }
        }
    }
    json.push(']');
}

fn push_claim_identity_json(
    json: &mut String,
    program: &CheckedTrees,
    identity: psi_language_semantics::PermissionClaimIdentity,
) {
    match identity {
        psi_language_semantics::PermissionClaimIdentity::Unknown => {
            json.push_str("{\"kind\": \"unknown\"}");
        }
        psi_language_semantics::PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source,
            ordinal,
        } => {
            json.push_str("{\"kind\": \"established\", \"machine\": ");
            push_json_string(json, &symbol_label(program, machine_symbol));
            json.push_str(", \"state\": ");
            push_json_string(json, &state_label_from_symbol(program, state_symbol));
            json.push_str(", \"source\": ");
            push_permission_event_source_json(json, program, source);
            json.push_str(", \"ordinal\": ");
            json.push_str(&ordinal.to_string());
            json.push('}');
        }
    }
}

fn push_claim_provenance_json(
    json: &mut String,
    program: &CheckedTrees,
    provenance: psi_language_semantics::PermissionProvenance,
) {
    match provenance {
        psi_language_semantics::PermissionProvenance::Unknown => {
            json.push_str("{\"kind\": \"unknown\"}");
        }
        psi_language_semantics::PermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source,
        } => {
            json.push_str("{\"kind\": \"established\", \"machine\": ");
            push_json_string(json, &symbol_label(program, machine_symbol));
            json.push_str(", \"state\": ");
            push_json_string(json, &state_label_from_symbol(program, state_symbol));
            json.push_str(", \"source\": ");
            push_permission_event_source_json(json, program, source);
            json.push('}');
        }
    }
}

fn push_permission_event_source_json(
    json: &mut String,
    program: &CheckedTrees,
    source: psi_language_semantics::PermissionEventSource,
) {
    use psi_language_semantics::PermissionEventSource;
    match source {
        PermissionEventSource::StateEntry => json.push_str("{\"kind\": \"state_entry\"}"),
        PermissionEventSource::Statement { statement_index } => {
            json.push_str("{\"kind\": \"statement\", \"statement_index\": ");
            json.push_str(&statement_index.to_string());
            json.push('}');
        }
        PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => {
            json.push_str("{\"kind\": \"call\", \"statement_index\": ");
            json.push_str(&statement_index.to_string());
            json.push_str(", \"call_ordinal\": ");
            json.push_str(&call_ordinal.to_string());
            json.push_str(", \"target\": ");
            push_json_string(json, &state_label_from_symbol(program, target_symbol));
            json.push('}');
        }
        PermissionEventSource::StateExit => json.push_str("{\"kind\": \"state_exit\"}"),
    }
}

fn qualification_subject(program: &CheckedTrees, fact: &psi_facts::Fact) -> String {
    use psi_facts::{FactPlace, PlaceRoot, PlaceSegment};

    let FactPlace::Place(place) = fact.place else {
        return match fact.place {
            FactPlace::Symbol(symbol) => qualification_symbol_label(program, symbol),
            FactPlace::Expression(expression) => program.expression_table.display_name(expression),
            FactPlace::TypeReference(type_reference) => {
                program.display_type_reference(type_reference)
            }
            FactPlace::Unknown => {
                panic!("qualification evidence must retain a semantic subject position")
            }
            FactPlace::Place(_) => unreachable!("place subject handled above"),
        };
    };
    let place = program.facts.semantic.places.get(place);
    let mut subject = match place.root {
        PlaceRoot::Unknown => {
            panic!("qualification evidence must retain a semantic subject position")
        }
        PlaceRoot::Symbol(symbol) => qualification_symbol_label(program, symbol),
        PlaceRoot::Expression(expression) => program.expression_table.display_name(expression),
        PlaceRoot::TypeReference(type_reference) => program.display_type_reference(type_reference),
    };
    for segment in program
        .facts
        .semantic
        .place_segments
        .span_or_empty(place.segments)
    {
        match segment {
            PlaceSegment::Field { symbol } => {
                subject.push('.');
                subject.push_str(&qualification_symbol_label(program, *symbol));
            }
            PlaceSegment::Case { variant } => {
                subject.push_str("::");
                subject.push_str(&qualification_symbol_label(program, *variant));
            }
            PlaceSegment::FixedIndex { index } => {
                subject.push('[');
                subject.push_str(&index.to_string());
                subject.push(']');
            }
            PlaceSegment::FixedRange { start, end } => {
                subject.push('[');
                subject.push_str(&start.to_string());
                subject.push_str("..");
                subject.push_str(&end.to_string());
                subject.push(']');
            }
            PlaceSegment::Index { expression } => {
                subject.push('[');
                subject.push_str(&program.expression_table.display_name(*expression));
                subject.push(']');
            }
        }
    }
    subject
}

fn qualification_symbol_label(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    if !symbol.is_valid() {
        return "<unknown>".to_owned();
    }
    let path = program.symbols.display_path(symbol, "::");
    if path.is_empty() {
        format!("#{}", symbol.arena_index())
    } else {
        path
    }
}

fn qualification_requirement_identity(
    program: &CheckedTrees,
    evidence: &psi_facts::QualificationEvidence,
) -> Option<String> {
    if evidence.origin != psi_language_semantics::QualificationEvidenceOrigin::AdmittedReceipt {
        assert!(
            !evidence.requirement_symbol.is_valid(),
            "non-admitted qualification evidence must not name a boundary requirement",
        );
        return None;
    }
    let definition = program
        .traits()
        .iter()
        .find(|definition| {
            definition.is_boundary && definition.symbol == evidence.source_symbol
        })
        .expect(
            "admitted qualification evidence must name an exact boundary requirement owner/signature pair",
        );
    let requirement = program
        .trait_machine_signatures(definition)
        .iter()
        .find(|requirement| requirement.symbol == evidence.requirement_symbol)
        .expect(
            "admitted qualification evidence must name an exact boundary requirement owner/signature pair",
        );
    Some(
        program
            .normalized_trait_requirement_overload_identity(definition, requirement)
            .identity(),
    )
}

fn machine_overload_identity(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
) -> Option<String> {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .and_then(|machine| program.normalized_machine_overload_identity(machine))
        .map(|identity| identity.identity())
}

fn callable_overload_identity(
    program: &CheckedTrees,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> Option<String> {
    if let Some(identity) = machine_overload_identity(program, target_machine) {
        return Some(identity);
    }
    if target_machine == target_state {
        return program.machine_parameter_signature(target_state).map(
            |(declaring_machine, requirement)| {
                program
                    .normalized_machine_parameter_overload_identity(declaring_machine, requirement)
                    .identity()
            },
        );
    }
    program.traits().iter().find_map(|definition| {
        (definition.symbol == target_machine)
            .then(|| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|requirement| requirement.symbol == target_state)
                    .map(|requirement| {
                        program
                            .normalized_trait_requirement_overload_identity(definition, requirement)
                            .identity()
                    })
            })
            .flatten()
    })
}

fn program_point_name(point: psi_facts::ProgramPoint) -> &'static str {
    use psi_facts::ProgramPoint;
    match point {
        ProgramPoint::Global => "global",
        ProgramPoint::Definition { .. } => "definition",
        ProgramPoint::Machine { .. } => "machine",
        ProgramPoint::State { .. } => "state",
        ProgramPoint::Statement { .. } => "statement",
        ProgramPoint::Call { .. } => "call",
        ProgramPoint::CallRequires { .. } => "call_requires",
        ProgramPoint::CallEnsures { .. } => "call_ensures",
        ProgramPoint::Exit { .. } => "exit",
    }
}

fn exact_program_point_label(program: &CheckedTrees, point: psi_facts::ProgramPoint) -> String {
    use psi_facts::ProgramPoint;

    let symbol = |symbol| qualification_symbol_label(program, symbol);
    match point {
        ProgramPoint::Global => "global".to_owned(),
        ProgramPoint::Definition { symbol: definition } => symbol(definition),
        ProgramPoint::Machine { machine_symbol } => symbol(machine_symbol),
        ProgramPoint::State { state_symbol, .. } => symbol(state_symbol),
        ProgramPoint::Statement {
            state_symbol,
            statement_index,
            ..
        } => format!("{}:statement-{statement_index}", symbol(state_symbol)),
        ProgramPoint::Call {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::CallRequires {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-requires-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::CallEnsures {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-ensures-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::Exit {
            state_symbol,
            statement_index,
            ..
        } => format!("{}:exit-{statement_index}", symbol(state_symbol)),
    }
}

/// Checked carry-policy artifact. The authored clause is retained only as a
/// diagnostic/publication input; `effective` is the checker-derived policy
/// later liveness, runtime-admission, and model-export consumers must use.
/// Keeping the axes structured avoids making presentation spelling part of
/// artifact identity.
pub fn carry_manifest_json(program: &CheckedTrees) -> String {
    let mut json = String::from("{\n  \"data\": [");
    for (index, fact) in program.facts.carry.data.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let name = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == fact.data)
            .map(|definition| definition.name.as_str())
            .unwrap_or("<unknown>");
        json.push_str("\n    {\n      \"type\": ");
        push_json_string(&mut json, name);
        json.push_str(",\n      \"opaque\": ");
        let opaque = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == fact.data)
            .is_some_and(|definition| {
                definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque
            });
        json.push_str(if opaque { "true" } else { "false" });
        json.push_str(",\n      \"declared\": ");
        if let Some(declared) = fact.declared {
            push_carry_policy_json(&mut json, declared);
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"effective\": ");
        push_carry_policy_json(&mut json, fact.effective);
        json.push_str("\n    }");
    }
    json.push_str("\n  ],\n  \"claim_policies\": [");
    for (index, fact) in program.facts.carry.claim_policies.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"claim_identity\": ");
        push_claim_identity_json(&mut json, program, fact.claim_identity);
        json.push_str(",\n      \"contributing_origins\": ");
        json.push_str(&fact.contributing_origins.to_string());
        json.push_str(",\n      \"effective\": ");
        push_carry_policy_json(&mut json, fact.effective);
        json.push_str("\n    }");
    }
    json.push_str("\n  ],\n  \"safe_point_crossings\": [");
    for (index, fact) in program.facts.carry.suspension_crossings.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, carry_machine_name(program, fact.machine));
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, fact.machine)
                .expect("safe-point carry crossing must name an exact owning machine"),
        );
        json.push_str(",\n      \"state\": ");
        push_json_string(&mut json, carry_state_name(program, fact.state));
        json.push_str(",\n      \"statement_index\": ");
        json.push_str(&fact.statement_index.to_string());
        json.push_str(",\n      \"call_ordinal\": ");
        json.push_str(&fact.call_ordinal.to_string());
        json.push_str(",\n      \"target\": ");
        push_json_string(&mut json, carry_call_target_name(program, fact.target));
        json.push_str(",\n      \"effective\": ");
        push_carry_policy_json(&mut json, fact.effective);
        json.push_str(",\n      \"live_values\": [");
        for (live_index, live) in fact.live_values.iter().enumerate() {
            if live_index > 0 {
                json.push_str(", ");
            }
            json.push_str("{\"type\": ");
            push_json_string(
                &mut json,
                &program.display_type_reference_with_constraints(live.type_reference),
            );
            json.push_str(", \"storage\": ");
            push_json_string(
                &mut json,
                match live.storage {
                    psi_checked_trees::SuspensionCrossingStorage::Persistent => "persistent",
                    psi_checked_trees::SuspensionCrossingStorage::Parameter => "parameter",
                    psi_checked_trees::SuspensionCrossingStorage::Local => "local",
                    psi_checked_trees::SuspensionCrossingStorage::CallArgument => "call_argument",
                },
            );
            json.push_str(", \"effective\": ");
            push_carry_policy_json(&mut json, live.effective);
            json.push('}');
        }
        json.push_str("]\n    }");
    }
    json.push_str("\n  ],\n  \"activation_wide_carry\": [");
    for (index, fact) in program.facts.carry.activation_wide_carry.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let name = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == fact.machine)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, name);
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, fact.machine)
                .expect("activation-wide carry must name an exact owning machine"),
        );
        json.push_str(",\n      \"analysis_complete\": ");
        json.push_str(if fact.analysis_complete {
            "true"
        } else {
            "false"
        });
        json.push_str(",\n      \"subtree_machine_count\": ");
        json.push_str(
            &program
                .facts
                .carry
                .machine_subtree_symbols(fact.machine)
                .len()
                .to_string(),
        );
        json.push_str(",\n      \"effective\": ");
        push_carry_policy_json(&mut json, fact.effective);
        json.push_str(",\n      \"contributing_type_count\": ");
        json.push_str(&fact.contributing_types.len().to_string());
        json.push_str(",\n      \"unnamed_strict_values\": ");
        json.push_str(&fact.unnamed_strict_values.to_string());
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

fn carry_machine_name(program: &CheckedTrees, symbol: SymbolHandle) -> &str {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .map(|machine| machine.name.as_str())
        .unwrap_or("<unknown>")
}

fn carry_state_name(program: &CheckedTrees, symbol: SymbolHandle) -> &str {
    program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == symbol)
                .map(|state| state.name.as_str())
        })
        .unwrap_or("<unknown>")
}

fn carry_call_target_name(program: &CheckedTrees, symbol: SymbolHandle) -> &str {
    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
    {
        return machine.name.as_str();
    }
    program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == symbol)
                .map(|_| machine.name.as_str())
        })
        .unwrap_or("<unknown>")
}

/// Provider-independent task activation demands. Runtime/provider admission
/// consumes these normalized facts; the artifact keeps target/layout and
/// canonical carry derivation inspectable without exposing provider handles.
pub fn task_activation_manifest_json(
    program: &CheckedTrees,
    task_activations: &omega_task_plans::TaskActivationPlanSet,
) -> String {
    use omega_task_plans::TaskStartOperation;
    use psi_checked_trees::machine::Machine;

    fn machine_name<'a>(machines: &'a [Machine], symbol: SymbolHandle) -> &'a str {
        machines
            .iter()
            .find(|machine| machine.symbol == symbol)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>")
    }
    fn callable_name(program: &CheckedTrees, symbol: SymbolHandle) -> String {
        if let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == symbol)
        {
            return machine.name.as_str().to_owned();
        }
        program
            .traits()
            .iter()
            .find_map(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.symbol == symbol)
                    .map(|signature| format!("{}::{}", definition.name, signature.name))
            })
            .unwrap_or_else(|| "<unknown>".to_owned())
    }
    let mut json = String::from("{\n  \"activations\": [");
    for (index, activation) in task_activations.as_slice().iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let plan = activation.plan.candidate();
        json.push_str("\n    {\n      \"operation\": ");
        push_json_string(
            &mut json,
            match activation.operation {
                TaskStartOperation::Start => "start",
                TaskStartOperation::TryStart => "try_start",
            },
        );
        json.push_str(",\n      \"start_requirement\": ");
        push_json_string(
            &mut json,
            &callable_name(program, activation.start_requirement),
        );
        json.push_str(",\n      \"selected_runtime\": {\"provider_plan\": ");
        push_json_string(&mut json, &activation.selected_runtime.provider_plan_name);
        json.push_str(", \"runtime_identity\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            activation.selected_runtime.runtime.normalized_identity()
        ));
        json.push_str("\", \"requirement_identity\": ");
        push_json_string(&mut json, &activation.selected_runtime.requirement_identity);
        json.push('}');
        json.push_str(",\n      \"target_machine\": ");
        push_json_string(
            &mut json,
            machine_name(program.machines(), activation.target_machine),
        );
        json.push_str(",\n      \"target_machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, activation.target_machine)
                .expect("task activation must name an exact target machine"),
        );
        json.push_str(",\n      \"specialization_report_fingerprint\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            activation.specialization_report_fingerprint
        ));
        json.push_str("\",\n      \"specialization_commitment\": \"");
        for byte in activation.specialization_commitment.as_bytes() {
            json.push_str(&format!("{byte:02x}"));
        }
        json.push_str("\",\n      \"activation_plan_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            activation.plan.normalized_identity().normalized_identity()
        ));
        json.push_str("\",\n      \"machine_contract_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.machine_contract.normalized_identity()
        ));
        json.push_str("\",\n      \"entry_id\": \"0x");
        json.push_str(&format!("{:016x}", plan.entry.normalized_identity()));
        json.push_str("\",\n      \"argument_layout_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.argument_layout.normalized_identity()
        ));
        json.push_str("\",\n      \"terminal_outcome_layout_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.terminal_outcome_layout.normalized_identity()
        ));
        json.push_str("\",\n      \"calling_plan_id\": \"0x");
        json.push_str(&format!("{:016x}", plan.calling_plan.normalized_identity()));
        json.push_str("\",\n      \"stack_plan\": {\"bytes\": ");
        json.push_str(&plan.stack_plan.bytes.to_string());
        json.push_str(", \"alignment\": ");
        json.push_str(&plan.stack_plan.alignment.to_string());
        json.push_str(", \"representation\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.stack_plan.representation.normalized_identity()
        ));
        json.push_str("\"},\n      \"may_suspend\": ");
        json.push_str(if plan.may_suspend { "true" } else { "false" });
        json.push_str(",\n      \"may_block\": ");
        json.push_str(if plan.may_block { "true" } else { "false" });
        json.push_str(",\n      \"canonical_suspension_crossings\": [");
        for (crossing_index, crossing) in plan.canonical_suspension_crossings.iter().enumerate() {
            if crossing_index > 0 {
                json.push(',');
            }
            json.push_str("{\"identity\": \"0x");
            json.push_str(&format!("{:016x}", crossing.identity.normalized_identity()));
            json.push_str("\", \"suspension_allowed\": ");
            json.push_str(if crossing.suspension_allowed {
                "true"
            } else {
                "false"
            });
            json.push_str(", \"preserve_cpu\": ");
            json.push_str(if crossing.preserve_cpu {
                "true"
            } else {
                "false"
            });
            json.push_str(", \"preserve_host_thread\": ");
            json.push_str(if crossing.preserve_host_thread {
                "true"
            } else {
                "false"
            });
            json.push('}');
        }
        json.push_str("],\n      \"cpu_thread_preservation\": {\"preserve_cpu\": ");
        json.push_str(if plan.carry_obligations.preserve_cpu {
            "true"
        } else {
            "false"
        });
        json.push_str(", \"preserve_host_thread\": ");
        json.push_str(if plan.carry_obligations.preserve_host_thread {
            "true"
        } else {
            "false"
        });
        json.push('}');
        json.push_str(",\n      \"cancellation_required\": ");
        json.push_str(if plan.cancellation_required {
            "true"
        } else {
            "false"
        });
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

fn push_carry_policy_json(output: &mut String, policy: psi_language_semantics::CarryPolicy) {
    use psi_language_semantics::{CarryAddress, CarryCpu, CarryHostThread, CarrySuspension};

    output.push_str("{\"suspension\": ");
    push_json_string(
        output,
        match policy.suspension {
            CarrySuspension::Forbidden => "forbidden",
            CarrySuspension::Allowed => "allowed",
        },
    );
    output.push_str(", \"cpu\": ");
    push_json_string(
        output,
        match policy.cpu {
            CarryCpu::Origin => "same",
            CarryCpu::Any => "any",
        },
    );
    output.push_str(", \"thread\": ");
    push_json_string(
        output,
        match policy.host_thread {
            CarryHostThread::Origin => "same",
            CarryHostThread::Any => "any",
        },
    );
    output.push_str(", \"address\": ");
    push_json_string(
        output,
        match policy.address {
            CarryAddress::Stable => "stable",
            CarryAddress::Movable => "movable",
        },
    );
    output.push('}');
}

/// Decision 20/23's externally inspectable machine-contract artifact. The
/// object shape is the firewall: authored interface identity and checked
/// implementation evidence are siblings, never one flattened bag. Consumers
/// pin `contract.report_fingerprint`; proof/debug tooling may inspect
/// `implementation` without changing that identity.
fn exact_manifest_machine_contract<'program>(
    program: &'program CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
) -> &'program psi_checked_trees::MachineContractPlan {
    let mut matches = program
        .facts
        .contract_plans
        .machines
        .iter()
        .filter(|plan| plan.machine == machine.symbol);
    let plan = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact machine contract row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact machine contract rows",
        machine.name
    );
    plan
}

fn exact_manifest_service_reach(
    program: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
) -> psi_language_semantics::ServiceReachPlan {
    let mut matches = program
        .facts
        .service_reaches
        .machines()
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact service-reach row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact service-reach rows",
        machine.name
    );
    psi_language_semantics::ServiceReachPlan {
        interface: fact.interface,
        checked_inferred: fact.inferred_transitive,
    }
}

fn exact_manifest_synchronous_invocation<'program>(
    program: &'program CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
) -> &'program psi_language_semantics::SynchronousInvocationPlan {
    let mut matches = program
        .facts
        .synchronous_invocations
        .machines
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact synchronous-invocation row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact synchronous-invocation rows",
        machine.name
    );
    &fact.plan
}

fn exact_manifest_suspension(
    program: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
) -> psi_language_semantics::SuspensionPlan {
    let mut matches = program
        .facts
        .suspensions
        .machines
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact suspension row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact suspension rows",
        machine.name
    );
    fact.plan
}

fn exact_manifest_blocking(
    program: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
) -> psi_language_semantics::BlockingPlan {
    let mut matches = program
        .facts
        .blocking
        .machines
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact blocking row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact blocking rows",
        machine.name
    );
    fact.plan
}

fn exact_manifest_termination<'program>(
    program: &'program CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
) -> &'program psi_language_semantics::MachineTerminationPlan {
    let mut matches = program
        .facts
        .termination
        .machines
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact termination row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact termination rows",
        machine.name
    );
    &fact.plan
}

fn exact_manifest_mutation<'program>(
    program: &'program CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
) -> &'program psi_checked_trees::MachineMutationFact {
    let mut matches = program
        .facts
        .mutation
        .machines
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact mutation row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact mutation rows",
        machine.name
    );
    let states = program.machine_states(machine);
    assert_eq!(
        fact.state_write_frames.len(),
        states.len(),
        "machine contract manifest `{}` mutation frames must cover its exact typed state table one-for-one",
        machine.name
    );
    for (frame, state) in fact.state_write_frames.iter().zip(states) {
        assert_eq!(
            frame.state, state.symbol,
            "machine contract manifest `{}` mutation frames must retain exact typed state-table carrier order",
            machine.name
        );
    }
    fact
}

struct ValidatedManifestSpecialization<'program> {
    specialization: &'program psi_typed_trees::typed_trees::MachineSpecialization,
    template: &'program Machine,
    instance: &'program Machine,
    instance_contract_report_fingerprint: u64,
    instance_contract_commitment: psi_checked_trees::MachineContractCommitment,
}

struct ValidatedManifestCrashTarget {
    owner_label: String,
    state_label: String,
    overload_identity: String,
    is_requirement: bool,
}

struct ValidatedManifestCrashCapsule<'program> {
    capsule: &'program psi_checked_trees::CrashContractCapsule,
    target: ValidatedManifestCrashTarget,
}

fn exact_manifest_crash_target(
    program: &CheckedTrees,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
    source_kind: &str,
) -> ValidatedManifestCrashTarget {
    let local_owners = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == target_machine)
        .collect::<Vec<_>>();
    assert!(
        local_owners.len() <= 1,
        "{source_kind} has duplicate exact local target-machine owners"
    );
    let trait_owners = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == target_machine)
        .collect::<Vec<_>>();
    assert!(
        trait_owners.len() <= 1,
        "{source_kind} has duplicate exact trait target owners"
    );

    let mut candidates = Vec::new();
    if let Some(machine) = local_owners.first().copied() {
        let states = program
            .machine_states(machine)
            .iter()
            .filter(|state| state.symbol == target_state)
            .collect::<Vec<_>>();
        assert!(
            states.len() <= 1,
            "{source_kind} has duplicate exact local target states"
        );
        if let Some(state) = states.first().copied() {
            let overload_identity = program
                .normalized_machine_overload_identity(machine)
                .unwrap_or_else(|| {
                    panic!("{source_kind} local target must retain an exact overload identity")
                })
                .identity();
            candidates.push(ValidatedManifestCrashTarget {
                owner_label: machine.name.as_str().to_owned(),
                state_label: state.name.as_str().to_owned(),
                overload_identity,
                is_requirement: false,
            });
        }
    }

    let mut generic_targets = Vec::new();
    if target_machine == target_state {
        for declaring_machine in program.machines() {
            for parameter in program.machine_type_parameters(declaring_machine) {
                let psi_typed_trees::data::TypeParameterKind::Machine { contract } =
                    &parameter.kind
                else {
                    continue;
                };
                if parameter.symbol != target_state {
                    continue;
                }
                let signature = program
                    .machine_parameter_contract_view(contract)
                    .expect(
                        "checked machine-parameter contract must retain a valid requirement identity",
                    )
                    .signature();
                let label = parameter.name.as_str();
                generic_targets.push(ValidatedManifestCrashTarget {
                    owner_label: label.to_owned(),
                    state_label: label.to_owned(),
                    overload_identity: program
                        .normalized_machine_parameter_overload_identity(
                            declaring_machine,
                            signature,
                        )
                        .identity(),
                    is_requirement: true,
                });
            }
        }
    }
    assert!(
        generic_targets.len() <= 1,
        "{source_kind} has duplicate exact generic requirement targets"
    );
    let owner_category_count = usize::from(!local_owners.is_empty())
        + usize::from(!generic_targets.is_empty())
        + usize::from(!trait_owners.is_empty());
    assert!(
        owner_category_count <= 1,
        "{source_kind} target owner must resolve to one retained callable category"
    );
    candidates.extend(generic_targets);

    if let Some(definition) = trait_owners.first().copied() {
        let signatures = program
            .trait_machine_signatures(definition)
            .iter()
            .filter(|signature| signature.symbol == target_state)
            .collect::<Vec<_>>();
        assert!(
            signatures.len() <= 1,
            "{source_kind} has duplicate exact trait target signatures"
        );
        if let Some(signature) = signatures.first().copied() {
            candidates.push(ValidatedManifestCrashTarget {
                owner_label: definition.name.as_str().to_owned(),
                state_label: signature.name.as_str().to_owned(),
                overload_identity: program
                    .normalized_trait_requirement_overload_identity(definition, signature)
                    .identity(),
                is_requirement: true,
            });
        }
    }

    let mut candidates = candidates.into_iter();
    let target = candidates.next().unwrap_or_else(|| {
        panic!("{source_kind} must name one exact retained callable target coordinate")
    });
    assert!(
        candidates.next().is_none(),
        "{source_kind} must name exactly one retained callable target category"
    );
    target
}

fn validated_manifest_crash_capsules(
    program: &CheckedTrees,
) -> Vec<ValidatedManifestCrashCapsule<'_>> {
    let mut coordinates = Vec::new();
    program
        .facts
        .contract_plans
        .crash_capsules
        .iter()
        .map(|capsule| {
            let coordinate = (capsule.target_machine(), capsule.target_state());
            assert!(
                !coordinates.contains(&coordinate),
                "machine contract manifest crash capsules have duplicate exact target coordinates"
            );
            coordinates.push(coordinate);
            let target = exact_manifest_crash_target(
                program,
                capsule.target_machine(),
                capsule.target_state(),
                "crash contract capsule",
            );
            assert!(
                target.is_requirement,
                "crash contract capsule target must be an exact requirement owner/signature pair"
            );
            ValidatedManifestCrashCapsule { capsule, target }
        })
        .collect()
}

fn exact_manifest_specialization_machine<'program>(
    program: &'program CheckedTrees,
    symbol: SymbolHandle,
    role: &str,
) -> &'program Machine {
    let mut matches = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol);
    let machine = matches.next().unwrap_or_else(|| {
        panic!("machine contract manifest specialization is missing its exact typed {role} machine")
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest specialization has duplicate exact typed {role} machines"
    );
    machine
}

fn validated_manifest_specializations(
    program: &CheckedTrees,
) -> Vec<ValidatedManifestSpecialization<'_>> {
    let mut instance_symbols = Vec::new();
    let mut validated = Vec::with_capacity(program.machine_specializations.len());
    for specialization in &program.machine_specializations {
        assert!(
            !instance_symbols.contains(&specialization.instance),
            "machine contract manifest specializations have duplicate exact instance rows"
        );
        instance_symbols.push(specialization.instance);
        let template =
            exact_manifest_specialization_machine(program, specialization.template, "template");
        let instance =
            exact_manifest_specialization_machine(program, specialization.instance, "instance");
        validated.push(ValidatedManifestSpecialization {
            specialization,
            template,
            instance,
            instance_contract_report_fingerprint:
                specialization_instance_contract_report_fingerprint(program, instance),
            instance_contract_commitment: exact_manifest_machine_contract(program, instance)
                .commitment,
        });
    }
    validated
}

fn exact_manifest_crash_source_state<'program>(
    program: &'program CheckedTrees,
    machine: &Machine,
    state_symbol: SymbolHandle,
    statement_ordinal: u32,
    source_kind: &str,
) -> &'program State {
    let mut states = program
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == state_symbol);
    let state = states.next().unwrap_or_else(|| {
        panic!("checked crash {source_kind} source state must belong to its exact contract machine")
    });
    assert!(
        states.next().is_none(),
        "checked crash {source_kind} source state must resolve uniquely within its exact contract machine"
    );
    let statement_index = usize::try_from(statement_ordinal)
        .expect("checked crash source statement ordinal exceeds retained index range");
    assert!(
        program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index)
            .is_some(),
        "checked crash {source_kind} statement must belong to its exact typed state"
    );
    state
}

fn exact_manifest_crash_call_source<'program>(
    program: &'program CheckedTrees,
    machine: &Machine,
    call: &psi_checked_trees::CheckedCrashCallSite,
) -> &'program State {
    let location = call.location();
    let state = exact_manifest_crash_source_state(
        program,
        machine,
        location.state(),
        location.statement_ordinal(),
        "call",
    );
    let mut flow_states = program
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, flow)| {
            flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol
        });
    let flow_state = flow_states
        .next()
        .map(|(_, flow)| flow)
        .unwrap_or_else(|| panic!("checked crash call must name one exact checked flow state"));
    assert!(
        flow_states.next().is_none(),
        "checked crash call must name exactly one checked flow state"
    );
    let calls = program
        .facts
        .flow
        .control
        .calls
        .span(flow_state.calls)
        .expect("checked crash call flow state must retain an exact valid call span");
    let statement_index = usize::try_from(location.statement_ordinal())
        .expect("checked crash call statement ordinal exceeds retained index range");
    let call_ordinal = usize::try_from(location.call_ordinal())
        .expect("checked crash call ordinal exceeds retained index range");
    let mut flow_calls = calls.iter().filter(|flow_call| {
        flow_call.statement_index == statement_index && flow_call.call_ordinal == call_ordinal
    });
    let flow_call = flow_calls
        .next()
        .unwrap_or_else(|| panic!("checked crash call must name one exact checked flow call"));
    assert!(
        flow_calls.next().is_none(),
        "checked crash call must name exactly one checked flow call"
    );
    assert_eq!(
        flow_call.target_symbol,
        call.target_state(),
        "checked crash call must retain its exact checked flow target"
    );
    state
}

pub fn machine_contract_manifest_json(program: &CheckedTrees) -> String {
    let specializations = validated_manifest_specializations(program);
    let crash_capsules = validated_manifest_crash_capsules(program);
    let mut json = String::from("{\n  \"machines\": [");
    for (index, machine) in program.machines().iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let contract = exact_manifest_machine_contract(program, machine);
        let service_reach = exact_manifest_service_reach(program, machine);
        let synchronous_invocation = exact_manifest_synchronous_invocation(program, machine);
        let suspension = exact_manifest_suspension(program, machine);
        let blocking = exact_manifest_blocking(program, machine);
        let termination = exact_manifest_termination(program, machine);
        let mutation = exact_manifest_mutation(program, machine);
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, machine.name.as_str());
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, machine.symbol).unwrap_or_else(|| {
                panic!(
                    "checked machine contract `{}` must have an exact overload identity",
                    machine.name
                )
            }),
        );

        json.push_str(",\n      \"contract\": {");
        json.push_str("\n        \"report_fingerprint\": \"0x");
        json.push_str(&format!("{:016x}", contract.report_fingerprint));
        json.push_str("\",\n        \"supply\": ");
        push_json_string(&mut json, supply_mode_name(machine.supply_mode));
        json.push_str(",\n        \"service_reach\": ");
        push_service_reach_plan_json(&mut json, program, service_reach);
        json.push_str(",\n        \"synchronous_invocation\": ");
        push_synchronous_invocation_plan_json(&mut json, synchronous_invocation, false);
        json.push_str(",\n        \"suspension\": ");
        push_suspension_plan_json(&mut json, suspension);
        json.push_str(",\n        \"blocking\": ");
        push_blocking_plan_json(&mut json, blocking);
        json.push_str(",\n        \"crashes\": ");
        push_crash_plan_json(&mut json, &contract.crash);
        json.push_str(",\n        \"termination\": ");
        push_termination_interface_json(&mut json, &termination.interface);
        json.push_str("\n      }");

        json.push_str(",\n      \"implementation\": {");
        json.push_str("\n        \"checked_may_suspend\": ");
        json.push_str(if suspension.checked_may_suspend {
            "true"
        } else {
            "false"
        });
        json.push_str(",\n        \"checked_may_block\": ");
        json.push_str(if blocking.checked_may_block {
            "true"
        } else {
            "false"
        });
        json.push_str(",\n        \"checked_service_reach\": ");
        push_service_row_json(&mut json, program, service_reach.checked_inferred);
        json.push_str(",\n        \"checked_synchronous_invocations\": ");
        push_string_array(&mut json, &synchronous_invocation.checked_inferred);
        let state_write_frames = mutation.state_write_frames.as_slice();
        json.push_str(",\n        \"inferred_write_frames\": [");
        for (frame_index, state_frame) in state_write_frames.iter().enumerate() {
            if frame_index > 0 {
                json.push(',');
            }
            let state_name = mutation_frame_state_name(program, machine, state_frame.state);
            json.push_str("\n          {\"state\": ");
            push_json_string(&mut json, state_name);
            json.push_str(", \"completeness\": ");
            push_json_string(
                &mut json,
                match state_frame.frame.completeness() {
                    psi_facts::WriteFrameCompleteness::Complete => "complete",
                    psi_facts::WriteFrameCompleteness::Opaque => "opaque",
                },
            );
            json.push_str(", \"fingerprint\": \"0x");
            json.push_str(&format!(
                "{:016x}",
                state_frame.frame.compatibility_report_fingerprint()
            ));
            json.push_str("\", \"paths\": [");
            push_json_strings(&mut json, state_frame.frame.paths());
            json.push_str("]}");
        }
        if !state_write_frames.is_empty() {
            json.push('\n');
            json.push_str("        ");
        }
        json.push(']');
        json.push_str(",\n        \"checked_crash_sites\": [");
        for (site_index, site) in contract.crash.checked_sites().iter().enumerate() {
            if site_index > 0 {
                json.push(',');
            }
            let location = site.location();
            let state_name = exact_manifest_crash_source_state(
                program,
                machine,
                location.state(),
                location.statement_ordinal(),
                "site",
            )
            .name
            .as_str();
            json.push_str("\n          {\"state\": ");
            push_json_string(&mut json, state_name);
            json.push_str(", \"statement_ordinal\": ");
            json.push_str(&location.statement_ordinal().to_string());
            json.push_str(", \"cause\": ");
            push_json_string(
                &mut json,
                match site.cause() {
                    psi_checked_trees::CrashCause::Trap => "Trap",
                    psi_checked_trees::CrashCause::Abort => "Abort",
                },
            );
            json.push_str(", \"path_guard_conjuncts\": [");
            for (guard_index, predicate) in site.path_guard_conjuncts().iter().enumerate() {
                if guard_index > 0 {
                    json.push_str(", ");
                }
                let mut identity = String::from("0x");
                for byte in predicate.canonical_bytes() {
                    identity.push_str(&format!("{byte:02x}"));
                }
                push_json_string(&mut json, &identity);
            }
            json.push(']');
            json.push_str(", \"path_guard_consequences\": [");
            for (guard_index, predicate) in site.path_guard_consequences().iter().enumerate() {
                if guard_index > 0 {
                    json.push_str(", ");
                }
                let mut identity = String::from("0x");
                for byte in predicate.canonical_bytes() {
                    identity.push_str(&format!("{byte:02x}"));
                }
                push_json_string(&mut json, &identity);
            }
            json.push(']');
            json.push_str(", \"guard_covering_buckets\": [");
            for (coverage_index, bucket) in site.guard_covering_buckets().iter().enumerate() {
                if coverage_index > 0 {
                    json.push_str(", ");
                }
                json.push_str(&bucket.get().to_string());
            }
            json.push(']');
            json.push_str(", \"covering_buckets\": [");
            for (coverage_index, (bucket, _)) in
                contract.crash.covering_buckets_for_site(site).enumerate()
            {
                if coverage_index > 0 {
                    json.push_str(", ");
                }
                json.push_str(&bucket.get().to_string());
            }
            json.push(']');
            json.push_str(", \"frontier_lower_bound\": [");
            for (claim_index, claim) in site.frontier_lower_bound().iter().enumerate() {
                if claim_index > 0 {
                    json.push_str(", ");
                }
                push_claim_identity_json(&mut json, program, *claim);
            }
            json.push(']');
            json.push('}');
        }
        if !contract.crash.checked_sites().is_empty() {
            json.push('\n');
            json.push_str("        ");
        }
        json.push(']');
        json.push_str(",\n        \"checked_crash_calls\": [");
        for (call_index, call) in contract.crash.checked_calls().iter().enumerate() {
            if call_index > 0 {
                json.push(',');
            }
            let location = call.location();
            let state_name = exact_manifest_crash_call_source(program, machine, call)
                .name
                .as_str();
            let target = exact_manifest_crash_target(
                program,
                call.target_machine(),
                call.target_state(),
                "checked crash call",
            );
            json.push_str("\n          {\"state\": ");
            push_json_string(&mut json, state_name);
            json.push_str(", \"statement_ordinal\": ");
            json.push_str(&location.statement_ordinal().to_string());
            json.push_str(", \"call_ordinal\": ");
            json.push_str(&location.call_ordinal().to_string());
            json.push_str(", \"target_machine\": ");
            push_json_string(&mut json, &target.owner_label);
            json.push_str(", \"target_callable_overload_identity\": ");
            push_json_string(&mut json, &target.overload_identity);
            json.push_str(", \"target_state\": ");
            push_json_string(&mut json, &target.state_label);
            json.push_str(", \"target_contract_report_fingerprint\": \"0x");
            json.push_str(&format!(
                "{:016x}",
                call.target_contract_report_fingerprint()
            ));
            json.push_str("\", \"path_guard_conjuncts\": [");
            for (guard_index, predicate) in call.path_guard_conjuncts().iter().enumerate() {
                if guard_index > 0 {
                    json.push_str(", ");
                }
                push_crash_predicate_identity_json(&mut json, predicate);
            }
            json.push_str("], \"path_guard_consequences\": [");
            for (guard_index, predicate) in call.path_guard_consequences().iter().enumerate() {
                if guard_index > 0 {
                    json.push_str(", ");
                }
                push_crash_predicate_identity_json(&mut json, predicate);
            }
            json.push_str("], \"surviving_buckets\": [");
            push_crash_buckets_json(&mut json, call.surviving_buckets());
            json.push_str("]}");
        }
        if !contract.crash.checked_calls().is_empty() {
            json.push('\n');
            json.push_str("        ");
        }
        json.push(']');
        json.push(',');
        json.push_str("\n        \"checked_termination\": ");
        push_termination_json(&mut json, &termination.checked_summary);
        json.push_str(",\n        \"resolved_ranking_view\": ");
        push_json_string(
            &mut json,
            termination
                .implementation_witness
                .as_ref()
                .map_or("", |witness| witness.view_path.as_str()),
        );
        if let Some(witness) = termination.implementation_witness.as_ref() {
            json.push_str(",\n        \"ranking_witness\": {\n          \"subjects\": [");
            push_json_strings(&mut json, &witness.subjects);
            json.push_str("],\n          \"view\": ");
            push_json_string(&mut json, &witness.view_path);
            json.push_str(",\n          \"view_arguments\": [");
            push_json_strings(&mut json, &witness.view_arguments);
            json.push(']');
            if let Some(range) = witness.rank_range.as_ref() {
                json.push_str(",\n          \"rank_range\": {\"floor\": ");
                push_json_string(&mut json, &range.floor);
                json.push_str(", \"ceiling\": ");
                push_json_string(&mut json, &range.ceiling);
                json.push_str(", \"ceiling_inclusive\": ");
                json.push_str(if range.ceiling_inclusive {
                    "true"
                } else {
                    "false"
                });
                json.push('}');
            }
            json.push_str("\n        }");
        }
        json.push_str("\n      }\n    }");
    }
    json.push_str("\n  ],\n  \"crash_contract_capsules\": [");
    for (index, row) in crash_capsules.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let capsule = row.capsule;
        json.push_str("\n    {\"target_machine\": ");
        push_json_string(&mut json, &row.target.owner_label);
        json.push_str(", \"target_callable_overload_identity\": ");
        push_json_string(&mut json, &row.target.overload_identity);
        json.push_str(", \"target_state\": ");
        push_json_string(&mut json, &row.target.state_label);
        json.push_str(", \"target_contract_report_fingerprint\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            capsule.target_contract_report_fingerprint()
        ));
        json.push_str("\", \"published_buckets\": [");
        push_crash_buckets_json(&mut json, capsule.published_buckets());
        json.push_str("]}");
    }
    if !crash_capsules.is_empty() {
        json.push('\n');
        json.push_str("  ");
    }
    json.push_str("],\n  \"specializations\": [");
    for (index, row) in specializations.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let specialization = row.specialization;
        json.push_str("\n    {\n      \"template\": ");
        push_json_string(&mut json, row.template.name.as_str());
        json.push_str(",\n      \"instance\": ");
        push_json_string(&mut json, row.instance.name.as_str());
        json.push_str(",\n      \"instance_report_fingerprint\": \"0x");
        json.push_str(&format!("{:016x}", specialization.report_fingerprint));
        json.push_str("\",\n      \"instance_contract_report_fingerprint\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            row.instance_contract_report_fingerprint
        ));
        json.push_str("\",\n      \"instance_contract_commitment\": \"");
        for byte in row.instance_contract_commitment.as_bytes() {
            json.push_str(&format!("{byte:02x}"));
        }
        json.push_str("\",\n      \"template_contract_report_fingerprint\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            specialization.template_contract_report_fingerprint
        ));
        json.push_str("\",\n      \"template_contract_commitment\": \"");
        for byte in specialization.template_contract_commitment.as_bytes() {
            json.push_str(&format!("{byte:02x}"));
        }
        json.push_str("\",\n      \"accepted_template_commitment\": ");
        if let Some(commitment) = specialization.accepted_template_commitment.as_deref() {
            push_json_string(&mut json, commitment);
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"type_arguments\": [");
        push_json_strings(&mut json, &specialization.type_arguments);
        json.push_str("],\n      \"const_arguments\": [");
        push_json_strings(&mut json, &specialization.const_arguments);
        json.push_str("],\n      \"type_argument_identities\": [");
        push_json_strings(&mut json, &specialization.type_argument_identities);
        json.push_str("],\n      \"const_argument_identities\": [");
        push_json_strings(&mut json, &specialization.const_argument_identities);
        json.push_str("],\n      \"machine_argument_contract_report_fingerprints\": [");
        for (identity_index, identity) in specialization
            .machine_argument_contract_report_fingerprints
            .iter()
            .enumerate()
        {
            if identity_index > 0 {
                json.push_str(", ");
            }
            push_json_string(&mut json, &format!("0x{identity:016x}"));
        }
        json.push_str("],\n      \"conformance_argument_report_fingerprints\": [");
        for (identity_index, identity) in specialization
            .conformance_argument_report_fingerprints
            .iter()
            .enumerate()
        {
            if identity_index > 0 {
                json.push_str(", ");
            }
            push_json_string(&mut json, &format!("0x{identity:016x}"));
        }
        json.push_str("],\n      \"machine_argument_contract_commitments\": [");
        for (identity_index, argument) in specialization.machine_arguments.iter().enumerate() {
            if identity_index > 0 {
                json.push_str(", ");
            }
            let owner = program
                .machines()
                .iter()
                .find(|machine| {
                    machine.symbol == *argument
                        || program
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == *argument)
                })
                .expect("specialization machine argument must retain one exact owner");
            let commitment = exact_manifest_machine_contract(program, owner).commitment;
            let text = commitment
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            push_json_string(&mut json, &text);
        }
        json.push_str("],\n      \"conformance_argument_commitments\": [");
        for (identity_index, application) in
            specialization.conformance_applications.iter().enumerate()
        {
            if identity_index > 0 {
                json.push_str(", ");
            }
            let text = application
                .commitment
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            push_json_string(&mut json, &text);
        }
        json.push_str("]\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

fn mutation_frame_state_name<'program>(
    program: &'program CheckedTrees,
    machine: &Machine,
    state_symbol: SymbolHandle,
) -> &'program str {
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .map(|state| state.name.as_str())
        .expect("checked mutation write-frame state must belong to its exact fact machine")
}

fn specialization_instance_contract_report_fingerprint(
    program: &CheckedTrees,
    instance: &Machine,
) -> u64 {
    exact_manifest_machine_contract(program, instance).report_fingerprint
}

fn supply_mode_name(mode: psi_language_semantics::MachineSupplyMode) -> &'static str {
    use psi_language_semantics::MachineSupplyMode;
    match mode {
        MachineSupplyMode::CheckedBody => "checked_body",
        MachineSupplyMode::Requirement => "requirement",
        MachineSupplyMode::TopLevelRequirement => "top_level_requirement",
        MachineSupplyMode::Boundary => "boundary",
        MachineSupplyMode::AdmissionClaim => "admission_claim",
        MachineSupplyMode::ExternalRealization { .. } => "external-realization",
    }
}

fn push_suspension_plan_json(json: &mut String, plan: psi_language_semantics::SuspensionPlan) {
    use psi_language_semantics::SuspensionInterface;
    match plan.interface {
        SuspensionInterface::InternalInferred => {
            json.push_str("{\"interface\": \"internal_inferred\"}");
        }
        SuspensionInterface::PublishedMaySuspend(value) => {
            json.push_str("{\"interface\": \"published_ceiling\", \"may_suspend\": ");
            json.push_str(if value { "true" } else { "false" });
            json.push('}');
        }
    }
}

fn push_service_reach_plan_json(
    json: &mut String,
    program: &CheckedTrees,
    plan: psi_language_semantics::ServiceReachPlan,
) {
    use psi_language_semantics::ServiceReachInterface;
    match plan.interface {
        ServiceReachInterface::InternalInferred => {
            json.push_str("{\"interface\": \"internal_inferred\"}");
        }
        ServiceReachInterface::PublishedCeiling(row) => {
            json.push_str("{\"interface\": \"published_ceiling\", \"services\": ");
            push_service_row_json(json, program, row);
            json.push('}');
        }
    }
}

fn push_synchronous_invocation_plan_json(
    json: &mut String,
    plan: &psi_language_semantics::SynchronousInvocationPlan,
    include_checked: bool,
) {
    use psi_language_semantics::SynchronousInvocationInterface;
    json.push_str("{\"interface\": ");
    push_json_string(
        json,
        match plan.interface {
            SynchronousInvocationInterface::InternalInferred => "internal_inferred",
            SynchronousInvocationInterface::PublishedCeiling => "published_ceiling",
        },
    );
    json.push_str(", \"targets\": ");
    push_string_array(json, &plan.published);
    if include_checked {
        json.push_str(", \"checked\": ");
        push_string_array(json, &plan.checked_inferred);
    }
    json.push('}');
}

fn push_string_array(json: &mut String, values: &[String]) {
    json.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(json, value);
    }
    json.push(']');
}

fn push_service_row_json(
    json: &mut String,
    program: &CheckedTrees,
    row: psi_language_semantics::ServiceReachRowId,
) {
    let reaches = &program.facts.service_reaches;
    json.push('[');
    for (index, service) in reaches.rows.services(row).iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        let name = reaches
            .services
            .definition(*service)
            .map(|definition| definition.name.as_str())
            .unwrap_or("<unknown-service>");
        push_json_string(json, name);
    }
    json.push(']');
}

fn push_blocking_plan_json(json: &mut String, plan: psi_language_semantics::BlockingPlan) {
    use psi_language_semantics::BlockingInterface;
    match plan.interface {
        BlockingInterface::InternalInferred => {
            json.push_str("{\"interface\": \"internal_inferred\"}");
        }
        BlockingInterface::PublishedMayBlock(value) => {
            json.push_str("{\"interface\": \"published_ceiling\", \"may_block\": ");
            json.push_str(if value { "true" } else { "false" });
            json.push('}');
        }
    }
}

fn push_crash_plan_json(json: &mut String, plan: &psi_checked_trees::CrashPlan) {
    json.push_str("{\"interface\": ");
    push_json_string(
        json,
        match plan.interface() {
            psi_checked_trees::CrashInterface::InternalInferred => "internal_inferred",
            psi_checked_trees::CrashInterface::PublishedCeiling => "published_ceiling",
        },
    );
    json.push_str(", \"buckets\": [");
    push_crash_buckets_json(json, plan.published());
    json.push_str("]}");
}

fn push_crash_buckets_json(json: &mut String, buckets: &[psi_checked_trees::CrashRouteBucket]) {
    for (bucket_index, bucket) in buckets.iter().enumerate() {
        if bucket_index > 0 {
            json.push_str(", ");
        }
        json.push_str("{\"cause\": ");
        push_json_string(
            json,
            match bucket.cause() {
                psi_checked_trees::CrashCause::Trap => "Trap",
                psi_checked_trees::CrashCause::Abort => "Abort",
            },
        );
        json.push_str(", \"alternative_guards\": [");
        for (guard_index, guard) in bucket.alternative_guards().iter().enumerate() {
            if guard_index > 0 {
                json.push_str(", ");
            }
            match guard {
                psi_checked_trees::CrashRouteGuard::Truth => push_json_string(json, "true"),
                psi_checked_trees::CrashRouteGuard::Predicate(predicate) => {
                    push_crash_predicate_identity_json(json, predicate);
                }
            }
        }
        json.push_str("]}");
    }
}

fn push_crash_predicate_identity_json(
    json: &mut String,
    predicate: &psi_checked_trees::CrashPredicateIdentity,
) {
    let mut identity = String::from("0x");
    for byte in predicate.canonical_bytes() {
        identity.push_str(&format!("{byte:02x}"));
    }
    push_json_string(json, &identity);
}

fn push_termination_json(
    json: &mut String,
    guarantee: &psi_language_semantics::TerminationGuarantee,
) {
    use psi_language_semantics::TerminationGuarantee;
    match guarantee {
        TerminationGuarantee::NoGuarantee => json.push_str("{\"kind\": \"no_guarantee\"}"),
        TerminationGuarantee::Terminates { premises } => {
            json.push_str("{\"kind\": \"terminates\", \"premises\": [");
            for (index, premise) in premises.iter().enumerate() {
                if index > 0 {
                    json.push_str(", ");
                }
                json.push_str("{\"profile\": ");
                json.push_str(&premise.profile.0.to_string());
                json.push_str(", \"subject_root\": ");
                json.push_str(&premise.subject.root.arena_index().to_string());
                json.push_str(", \"subject_projections\": [");
                for (projection_index, projection) in premise.subject.projections.iter().enumerate()
                {
                    if projection_index > 0 {
                        json.push_str(", ");
                    }
                    json.push_str(&projection.arena_index().to_string());
                }
                json.push_str("]}");
            }
            json.push_str("]}");
        }
    }
}

fn push_termination_interface_json(
    json: &mut String,
    interface: &psi_language_semantics::TerminationInterface,
) {
    use psi_language_semantics::TerminationInterface;
    match interface {
        TerminationInterface::InternalDerived => {
            json.push_str("{\"interface\": \"internal_derived\"}");
        }
        TerminationInterface::Published(guarantee) => {
            json.push_str("{\"interface\": \"published\", \"guarantee\": ");
            push_termination_json(json, guarantee);
            json.push('}');
        }
    }
}

fn push_json_strings(json: &mut String, values: &[String]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(json, value);
    }
}

fn state_label_from_symbol(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == symbol)
                .map(|state| format!("{}::{}", machine.name.as_str(), state.name.as_str()))
        })
        .unwrap_or_else(|| symbol_label(program, symbol))
}

fn symbol_label(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!(
            "{} (#{})",
            program.symbols.name(symbol),
            symbol.arena_index()
        )
    } else {
        "invalid".to_owned()
    }
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write;
                let _ = write!(output, "\\u{:04x}", c as u32);
            }
            c => output.push(c),
        }
    }
    output.push('"');
}

#[cfg(test)]
mod tests;
