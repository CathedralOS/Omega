use crate::phase_diagram::PhaseDiagramBuilder;
use crate::service_reach::{append_reach_and_operation_lines, service_names};
use psi_checked_trees::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowLoanFact, CheckedTrees,
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason, FlowCallFact,
    FlowInvalidationSource, FlowStateFact,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{
    StatementNode, TableTransition, TransitionTargetHandle, TransitionTargetNode,
};

mod capability_manifest;

pub use capability_manifest::{capability_manifest_html, capability_manifest_json};

pub fn checked_trees_html(program: &CheckedTrees) -> String {
    let mut diagram = PhaseDiagramBuilder::new("checked_trees");
    let mut machine_nodes = Vec::new();
    let mut state_nodes = Vec::new();

    for (machine_index, machine) in program.machines().iter().enumerate() {
        let machine_id = diagram.node(
            format!("machine_{machine_index}"),
            machine_label(program, machine),
            "machine",
            machine_index + 1,
        );
        let reach = machine_service_reach(program, machine.symbol);
        diagram.node_service_reaches(
            &machine_id,
            service_names(
                &program.facts.service_reaches.services,
                &program.facts.service_reaches.rows,
                reach.transitive,
            ),
        );
        machine_nodes.push((machine.symbol, machine_id.clone()));

        for state in program.machine_states(machine) {
            let state_id = diagram.node(
                format!("state_{machine_index}_{}", state.symbol.arena_index()),
                state_label(program, machine, state),
                "state_block",
                machine_index + 1,
            );
            if let Some(flow_state) = flow_state_for(program, machine.symbol, state.symbol) {
                diagram.node_service_reaches(
                    &state_id,
                    service_names(
                        &program.facts.service_reaches.services,
                        &program.facts.service_reaches.rows,
                        flow_state.service_reach.transitive,
                    ),
                );
            }
            diagram.containment_edge(&machine_id, &state_id);
            state_nodes.push((state.symbol, state_id));
        }
    }

    for (machine_index, machine) in program.machines().iter().enumerate() {
        for state in program.machine_states(machine) {
            let Some(source_id) = state_id_for_symbol(&state_nodes, state.symbol) else {
                continue;
            };

            append_checked_call_nodes(
                &mut diagram,
                program,
                machine_index,
                machine,
                state,
                source_id,
                &state_nodes,
            );

            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Transition(transition) = statement
                    && let Some(target_id) = transition_target_id(
                        program,
                        program.machine_states(machine),
                        &state_nodes,
                        transition,
                    )
                {
                    diagram.edge(source_id, target_id, "transition_target");
                }
            }
        }
    }

    diagram.finish()
}

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

    let mut json = format!(
        "{{\n  \"selected_provider_closure_fingerprint\": \"0x{:016x}\",\n  \"qualification_evidence\": [",
        selected_provider_plans.normalized_identity()
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
        push_json_string(
            &mut json,
            &format!("0x{:016x}", plan.identity_fingerprint()),
        );
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
        push_json_string(
            &mut json,
            &format!("0x{:016x}", plan.identity_fingerprint()),
        );
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
            .plan_by_identity(receipt_identity)
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
    use psi_typed_trees::data::TypeParameterKind;

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
    let generic_signature_matches = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_type_parameters(machine))
        .filter(|parameter| {
            matches!(
                &parameter.kind,
                TypeParameterKind::Machine { contract }
                    if parameter.symbol == source || contract.symbol == source
            )
        })
        .count();
    let matches = machine_matches
        + state_matches
        + root_operator_matches
        + domain_operator_matches
        + trait_matches
        + generic_signature_matches;
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
        ExpressionNode::Mutable(inner) => contains(*inner),
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
            plan.fingerprint,
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
        json.push_str(",\n      \"fingerprint\": ");
        push_json_string(&mut json, &format!("0x{:016x}", plan.fingerprint));
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
            row.plan.fingerprint,
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
        json.push_str("},\n      \"fingerprint\": ");
        push_json_string(&mut json, &format!("0x{:016x}", row.plan.fingerprint));
        json.push_str("\n    }");
    }
    let mut partition_compositions = program
        .facts
        .qualifications
        .content
        .partition_compositions
        .iter()
        .collect::<Vec<_>>();
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
                && row.source_fingerprint == row.source_plan.fingerprint,
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
            row.source_fingerprint,
            row.plan.fingerprint,
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
        json.push_str(",\n      \"source_fingerprint\": ");
        push_json_string(&mut json, &format!("0x{:016x}", row.source_fingerprint));
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
        json.push_str("},\n      \"fingerprint\": ");
        push_json_string(&mut json, &format!("0x{:016x}", row.plan.fingerprint));
        json.push_str("\n    }");
    }
    let mut conservation = program
        .facts
        .qualifications
        .content
        .conservation_plans
        .iter()
        .collect::<Vec<_>>();
    conservation.sort_by_key(|plan| (symbol_label(program, plan.callable), plan.fingerprint));
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
        json.push_str("},\n      \"fingerprint\": ");
        push_json_string(&mut json, &format!("0x{:016x}", plan.fingerprint));
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
    use psi_language_semantics::content::projection_fingerprint;

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
                plan.fingerprint,
                projection_fingerprint(&plan.algebra, &plan.expression),
                "content projection plan must retain its exact normalized fingerprint",
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
    use psi_language_semantics::content::{ContentConservationOwnerKind, conservation_fingerprint};

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
        plan.fingerprint,
        conservation_fingerprint(&plan.algebra, &plan.equation),
        "content conservation plan must retain its exact normalized fingerprint",
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
            projection_fingerprint,
            ..
        } => {
            let mut matches = projection_plans.iter().filter(|plan| {
                plan.domain == *domain
                    && plan.semantic_domain == *semantic_domain
                    && plan.machine == *projection_machine
                    && plan.fingerprint == *projection_fingerprint
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
                projection_fingerprint,
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
                    projection_fingerprint: *projection_fingerprint,
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
            projection_fingerprint,
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
            json.push_str(", \"projection_fingerprint\": ");
            push_json_string(json, &format!("0x{projection_fingerprint:016x}"));
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
        json.push_str(",\n      \"specialization_fingerprint\": \"0x");
        json.push_str(&format!("{:016x}", activation.specialization_fingerprint));
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
/// pin `contract.fingerprint`; proof/debug tooling may inspect
/// `implementation` without changing that identity.
pub fn machine_contract_manifest_json(program: &CheckedTrees) -> String {
    let mut json = String::from("{\n  \"machines\": [");
    for (index, machine) in program.machines().iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
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
        if let Some(contract) = program.facts.contract_plans.for_machine(machine.symbol) {
            json.push_str("\n        \"fingerprint\": \"0x");
            json.push_str(&format!("{:016x}", contract.fingerprint));
            json.push_str("\",\n        \"supply\": ");
            push_json_string(&mut json, supply_mode_name(machine.supply_mode));
            json.push_str(",\n        \"service_reach\": ");
            push_service_reach_plan_json(
                &mut json,
                program,
                independent_machine_service_reach_plan(program, machine.symbol),
            );
            json.push_str(",\n        \"synchronous_invocation\": ");
            let synchronous_invocation = program
                .facts
                .synchronous_invocations
                .for_machine(machine.symbol)
                .cloned()
                .unwrap_or_default();
            push_synchronous_invocation_plan_json(&mut json, &synchronous_invocation, false);
            json.push_str(",\n        \"suspension\": ");
            push_suspension_plan_json(
                &mut json,
                program
                    .facts
                    .suspensions
                    .for_machine(machine.symbol)
                    .unwrap_or_default(),
            );
            json.push_str(",\n        \"blocking\": ");
            push_blocking_plan_json(
                &mut json,
                program
                    .facts
                    .blocking
                    .for_machine(machine.symbol)
                    .unwrap_or_default(),
            );
            json.push_str(",\n        \"crashes\": ");
            push_crash_plan_json(&mut json, &contract.crash);
            json.push_str(",\n        \"termination\": ");
            let termination = program
                .facts
                .termination
                .for_machine(machine.symbol)
                .expect("every checked machine must publish termination facts");
            push_termination_interface_json(&mut json, &termination.interface);
            json.push_str("\n      }");
        } else {
            json.push_str("}");
        }

        json.push_str(",\n      \"implementation\": {");
        let mut has_implementation_field = false;
        if let Some(contract) = program.facts.contract_plans.for_machine(machine.symbol) {
            json.push_str("\n        \"checked_may_suspend\": ");
            json.push_str(
                if program
                    .facts
                    .suspensions
                    .for_machine(machine.symbol)
                    .is_some_and(|plan| plan.checked_may_suspend)
                {
                    "true"
                } else {
                    "false"
                },
            );
            json.push_str(",\n        \"checked_may_block\": ");
            json.push_str(
                if program
                    .facts
                    .blocking
                    .for_machine(machine.symbol)
                    .is_some_and(|plan| plan.checked_may_block)
                {
                    "true"
                } else {
                    "false"
                },
            );
            json.push_str(",\n        \"checked_service_reach\": ");
            push_service_row_json(
                &mut json,
                program,
                independent_machine_service_reach_plan(program, machine.symbol).checked_inferred,
            );
            json.push_str(",\n        \"checked_synchronous_invocations\": ");
            let checked_synchronous_invocations = program
                .facts
                .synchronous_invocations
                .for_machine(machine.symbol)
                .map(|plan| plan.checked_inferred.as_slice())
                .unwrap_or_default();
            push_string_array(&mut json, checked_synchronous_invocations);
            let state_write_frames = program
                .facts
                .mutation
                .for_machine(machine.symbol)
                .map(|fact| fact.state_write_frames.as_slice())
                .unwrap_or_default();
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
                json.push_str(&format!("{:016x}", state_frame.frame.fingerprint()));
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
                let state_name = program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == location.state())
                    .map(|state| state.name.as_str())
                    .unwrap_or("<unknown>");
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
                let state_name = program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == location.state())
                    .map(|state| state.name.as_str())
                    .unwrap_or("<unknown>");
                let target_machine = program
                    .machines()
                    .iter()
                    .find(|target| target.symbol == call.target_machine());
                let target_machine_name = target_machine
                    .map(|target| target.name.as_str())
                    .unwrap_or_else(|| program.symbols.name(call.target_machine()));
                let target_state_name = target_machine
                    .and_then(|target| {
                        program
                            .machine_states(target)
                            .iter()
                            .find(|state| state.symbol == call.target_state())
                    })
                    .map(|state| state.name.as_str())
                    .unwrap_or_else(|| program.symbols.name(call.target_state()));
                json.push_str("\n          {\"state\": ");
                push_json_string(&mut json, state_name);
                json.push_str(", \"statement_ordinal\": ");
                json.push_str(&location.statement_ordinal().to_string());
                json.push_str(", \"call_ordinal\": ");
                json.push_str(&location.call_ordinal().to_string());
                json.push_str(", \"target_machine\": ");
                push_json_string(&mut json, target_machine_name);
                json.push_str(", \"target_callable_overload_identity\": ");
                push_json_string(
                    &mut json,
                    &callable_overload_identity(
                        program,
                        call.target_machine(),
                        call.target_state(),
                    )
                    .expect("checked crash call must name an exact callable target"),
                );
                json.push_str(", \"target_state\": ");
                push_json_string(&mut json, target_state_name);
                json.push_str(", \"target_contract_fingerprint\": \"0x");
                json.push_str(&format!("{:016x}", call.target_contract_fingerprint()));
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
            has_implementation_field = true;
        }
        if program
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .is_some()
        {
            if has_implementation_field {
                json.push(',');
            }
            let termination = program
                .facts
                .termination
                .for_machine(machine.symbol)
                .expect("every checked machine must publish termination facts");
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
        }
        json.push_str("\n      }\n    }");
    }
    json.push_str("\n  ],\n  \"crash_contract_capsules\": [");
    for (index, capsule) in program
        .facts
        .contract_plans
        .crash_capsules
        .iter()
        .enumerate()
    {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\"target_machine\": ");
        push_json_string(&mut json, program.symbols.name(capsule.target_machine()));
        json.push_str(", \"target_callable_overload_identity\": ");
        push_json_string(
            &mut json,
            &callable_overload_identity(program, capsule.target_machine(), capsule.target_state())
                .expect("crash contract capsule must name an exact callable target"),
        );
        json.push_str(", \"target_state\": ");
        push_json_string(&mut json, program.symbols.name(capsule.target_state()));
        json.push_str(", \"target_contract_fingerprint\": \"0x");
        json.push_str(&format!("{:016x}", capsule.target_contract_fingerprint()));
        json.push_str("\", \"published_buckets\": [");
        push_crash_buckets_json(&mut json, capsule.published_buckets());
        json.push_str("]}");
    }
    if !program.facts.contract_plans.crash_capsules.is_empty() {
        json.push('\n');
        json.push_str("  ");
    }
    json.push_str("],\n  \"specializations\": [");
    for (index, specialization) in program.machine_specializations.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let template = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.template)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        let instance = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        let instance_contract_fingerprint =
            specialization_instance_contract_fingerprint(program, specialization.instance);
        json.push_str("\n    {\n      \"template\": ");
        push_json_string(&mut json, template);
        json.push_str(",\n      \"instance\": ");
        push_json_string(&mut json, instance);
        json.push_str(",\n      \"instance_fingerprint\": \"0x");
        json.push_str(&format!("{:016x}", specialization.fingerprint));
        json.push_str("\",\n      \"instance_contract_fingerprint\": \"0x");
        json.push_str(&format!("{instance_contract_fingerprint:016x}"));
        json.push_str("\",\n      \"template_contract_fingerprint\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            specialization.template_contract_fingerprint
        ));
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
        json.push_str("],\n      \"machine_argument_contract_fingerprints\": [");
        for (identity_index, identity) in specialization
            .machine_argument_contract_fingerprints
            .iter()
            .enumerate()
        {
            if identity_index > 0 {
                json.push_str(", ");
            }
            push_json_string(&mut json, &format!("0x{identity:016x}"));
        }
        json.push_str("],\n      \"conformance_argument_fingerprints\": [");
        for (identity_index, identity) in specialization
            .conformance_argument_fingerprints
            .iter()
            .enumerate()
        {
            if identity_index > 0 {
                json.push_str(", ");
            }
            push_json_string(&mut json, &format!("0x{identity:016x}"));
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

fn specialization_instance_contract_fingerprint(
    program: &CheckedTrees,
    instance: SymbolHandle,
) -> u64 {
    program
        .facts
        .contract_plans
        .for_machine(instance)
        .unwrap_or_else(|| {
            panic!("checked specialization instance must have an exact machine contract plan")
        })
        .fingerprint
}

fn supply_mode_name(mode: psi_language_semantics::MachineSupplyMode) -> &'static str {
    use psi_language_semantics::MachineSupplyMode;
    match mode {
        MachineSupplyMode::CheckedBody => "checked_body",
        MachineSupplyMode::Requirement => "requirement",
        MachineSupplyMode::Boundary => "boundary",
        MachineSupplyMode::Accepted => "accepted",
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

fn independent_machine_service_reach_plan(
    program: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
) -> psi_language_semantics::ServiceReachPlan {
    program
        .facts
        .service_reaches
        .plan_for_machine(machine)
        .unwrap_or_default()
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
                json.push_str(&premise.0.to_string());
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

fn machine_label(program: &CheckedTrees, machine: &Machine) -> String {
    let attached_data = machine
        .attached_data
        .as_ref()
        .map(|name| name.as_str())
        .unwrap_or("<none>");
    let mut label = format!(
        "machine {}\nattached data: {}\nmachine contracts: {}  trait satisfies: {}",
        machine.name.as_str(),
        attached_data,
        machine.contracts.len(),
        machine.satisfies.len()
    );
    append_reach_and_operation_lines(
        &mut label,
        &program.facts.service_reaches.services,
        &program.facts.service_reaches.rows,
        machine_service_reach(program, machine.symbol),
        machine_suspension_summary(program, machine.symbol),
        machine_blocking_summary(program, machine.symbol),
    );
    label
}

fn state_label(program: &CheckedTrees, machine: &Machine, state: &State) -> String {
    let borrow_state = borrow_state_for(program, machine.symbol, state.symbol);
    let flow_state = flow_state_for(program, machine.symbol, state.symbol);

    let writable_root_count = borrow_state
        .map(|borrow| borrow.writable_roots.len())
        .unwrap_or(0);
    let (invalidation_count, mutable_parameter_count, service_reach, suspension, blocking) =
        if let Some(flow) = flow_state {
            (
                flow.invalidations.len(),
                flow.mutable_parameter_count,
                flow.service_reach,
                flow.suspension,
                flow.blocking,
            )
        } else {
            (
                0,
                borrow_state
                    .map(|borrow| borrow.mutable_parameter_count)
                    .unwrap_or(0),
                Default::default(),
                Default::default(),
                Default::default(),
            )
        };

    let mut label = format!(
        "{}::{} [checked]\nparams: {}  mutable params: {}\nborrow: roots {}\ninvalidations: {}",
        machine.name.as_str(),
        state.name.as_str(),
        program.state_parameters(state).len(),
        mutable_parameter_count,
        writable_root_count,
        invalidation_count,
    );
    append_reach_and_operation_lines(
        &mut label,
        &program.facts.service_reaches.services,
        &program.facts.service_reaches.rows,
        service_reach,
        suspension,
        blocking,
    );

    if let Some(flow) = flow_state {
        append_loan_preview(&mut label, program, machine, state, flow.entry_constraints);
        append_activation_preview(&mut label, program, machine, state, flow);
        append_weakening_preview(&mut label, program, machine, state, flow);
        append_statement_preview(&mut label, program, flow);
        append_exit_preview(&mut label, program, flow);
    }

    label
}

fn append_loan_preview(
    label: &mut String,
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    constraints: psi_arena::HandleSpan<psi_checked_trees::FlowConstraintRef>,
) {
    let loans = program
        .facts
        .flow
        .borrow_loan_constraints(constraints)
        .take(3)
        .collect::<Vec<_>>();
    for loan in loans {
        label.push_str("\n  entry loan: ");
        label.push_str(&borrow_loan_label(
            program,
            machine,
            state,
            program.facts.borrow.loans.get(loan),
        ));
    }
}

fn append_activation_preview(
    label: &mut String,
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    flow: &FlowStateFact,
) {
    let activations = program
        .facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(flow.borrow_activations);
    for activation in activations.iter().take(3) {
        label.push_str("\n  activation: ");
        label.push_str(&borrow_activation_label(
            program, machine, state, activation,
        ));
    }
    if activations.len() > 3 {
        label.push_str("\n  ... ");
        label.push_str(&(activations.len() - 3).to_string());
        label.push_str(" more activations");
    }
}

fn append_weakening_preview(
    label: &mut String,
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    flow: &FlowStateFact,
) {
    let weakenings = program
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings);
    for weakening in weakenings.iter().take(3) {
        label.push_str("\n  weakening: ");
        label.push_str(&borrow_weakening_label(program, machine, state, weakening));
    }
    if weakenings.len() > 3 {
        label.push_str("\n  ... ");
        label.push_str(&(weakenings.len() - 3).to_string());
        label.push_str(" more weakenings");
    }
}

fn append_statement_preview(label: &mut String, program: &CheckedTrees, flow: &FlowStateFact) {
    let statements = program
        .facts
        .flow
        .control
        .statements
        .span_or_empty(flow.statements);
    for statement in statements.iter().take(6) {
        label.push_str("\n  stmt #");
        label.push_str(&statement.statement_index.to_string());
        label.push_str(": ctx ");
        label.push_str(&statement.entry_semantic_contexts.len().to_string());
        label.push_str(" loans ");
        label.push_str(
            &program
                .facts
                .flow
                .borrow_loan_constraints(statement.entry_constraints)
                .count()
                .to_string(),
        );
    }
    if statements.len() > 6 {
        label.push_str("\n  ... ");
        label.push_str(&(statements.len() - 6).to_string());
        label.push_str(" more statements");
    }
}

fn append_exit_preview(label: &mut String, program: &CheckedTrees, flow: &FlowStateFact) {
    let exits = program.facts.flow.control.exits.span_or_empty(flow.exits);
    for exit in exits.iter().take(3) {
        label.push_str("\n  exit #");
        label.push_str(&exit.statement_index.to_string());
        label.push_str(": ensures ");
        label.push_str(&exit.ensures.len().to_string());
        label.push_str(" ctx ");
        label.push_str(&exit.ensures_contexts.len().to_string());
    }
}

fn append_checked_call_nodes(
    diagram: &mut PhaseDiagramBuilder,
    program: &CheckedTrees,
    machine_index: usize,
    machine: &Machine,
    state: &State,
    source_id: &str,
    state_nodes: &[(SymbolHandle, String)],
) {
    let Some(flow_state) = flow_state_for(program, machine.symbol, state.symbol) else {
        return;
    };

    for call in program
        .facts
        .flow
        .control
        .calls
        .span_or_empty(flow_state.calls)
    {
        let label = checked_call_label(program, machine, state, call);
        let call_id = format!(
            "checked_call_{}_{}_{}_{}",
            machine_index,
            state.symbol.arena_index(),
            call.statement_index,
            call.call_ordinal
        );

        let rendered_id =
            if let Some(target_id) = state_id_for_symbol(state_nodes, call.target_symbol) {
                if target_id == source_id {
                    diagram.node(call_id, label, "external_call", machine_index + 1)
                } else {
                    diagram.scoped_node(
                        call_id,
                        label,
                        "external_call",
                        machine_index + 1,
                        target_id,
                    )
                }
            } else {
                diagram.node(call_id, label, "external_call", machine_index + 1)
            };

        diagram.node_service_reaches(
            &rendered_id,
            service_names(
                &program.facts.service_reaches.services,
                &program.facts.service_reaches.rows,
                call.service_reach.transitive,
            ),
        );
        diagram.edge(source_id, &rendered_id, "call");
        diagram.containment_edge(source_id, &rendered_id);
    }
}

fn checked_call_label(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    call: &FlowCallFact,
) -> String {
    let access_text = borrow_access_summary(program, machine, state, call.accesses);
    let mut label = format!(
        "call {}\nat #{}.{}\nentry: ctx {} constraints {} loans {}\ncontracts: requires {} ensures {}\nborrow: access {} invalidations {}",
        state_label_from_symbol(program, call.target_symbol),
        call.statement_index,
        call.call_ordinal,
        call.entry_semantic_contexts.len(),
        call.entry_constraints.len(),
        program
            .facts
            .flow
            .borrow_loan_constraints(call.entry_constraints)
            .count(),
        call.requires.len(),
        call.ensures.len(),
        access_text,
        call.invalidations.len(),
    );
    append_reach_and_operation_lines(
        &mut label,
        &program.facts.service_reaches.services,
        &program.facts.service_reaches.rows,
        call.service_reach,
        call.suspension,
        call.blocking,
    );
    let acknowledgement = call.operational_acknowledgement;
    let acknowledgement_text = match (
        acknowledgement.acknowledges_suspend,
        acknowledgement.acknowledges_block,
    ) {
        (false, false) => "neither",
        (true, false) => "suspend",
        (false, true) => "block",
        (true, true) => "suspend block",
    };
    let origin = match acknowledgement.origin {
        psi_language_semantics::CallOperationalAcknowledgementOrigin::Source => "source",
        psi_language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized => {
            "compiler-synthesized"
        }
    };
    label.push_str(&format!(
        "\nacknowledgement: {acknowledgement_text} ({origin})"
    ));
    label.push_str("\n\ndouble-click to scope target");
    label
}

fn borrow_access_summary(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    accesses: psi_arena::HandleSpan<BorrowArgumentAccessFact>,
) -> String {
    let access_facts = program
        .facts
        .borrow
        .argument_accesses
        .span_or_empty(accesses);
    if access_facts.is_empty() {
        return "<none>".to_owned();
    }

    access_facts
        .iter()
        .map(|access| borrow_access_label(program, machine, state, access))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn borrow_access_label(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    access: &BorrowArgumentAccessFact,
) -> String {
    let mut label = symbol_name_for_state(program, machine, state, access.root_symbol);
    for segment in program
        .facts
        .borrow
        .access_segments
        .span_or_empty(access.segments)
    {
        match segment {
            psi_facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&symbol_name_for_state(program, machine, state, *symbol));
            }
            psi_facts::PlaceSegment::Case { variant } => {
                label.push_str("::");
                label.push_str(&symbol_name_for_state(program, machine, state, *variant));
            }
            psi_facts::PlaceSegment::FixedIndex { index } => {
                label.push('[');
                label.push_str(&index.to_string());
                label.push(']');
            }
            psi_facts::PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&program.expression_table.display_name(*expression));
                label.push(']');
            }
        }
    }
    label.push_str(": ");
    label.push_str(match access.kind {
        BorrowAccessKind::Read => "read",
        BorrowAccessKind::Mutable => "mutable",
    });
    label
}

fn borrow_loan_label(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    loan: &BorrowLoanFact,
) -> String {
    let mut place = symbol_name_for_state(program, machine, state, loan.root_symbol);
    for segment in program
        .facts
        .borrow
        .access_segments
        .span_or_empty(loan.segments)
    {
        match segment {
            psi_facts::PlaceSegment::Field { symbol } => {
                place.push('.');
                place.push_str(&symbol_name_for_state(program, machine, state, *symbol));
            }
            psi_facts::PlaceSegment::Case { variant } => {
                place.push_str("::");
                place.push_str(&symbol_name_for_state(program, machine, state, *variant));
            }
            psi_facts::PlaceSegment::FixedIndex { index } => {
                place.push('[');
                place.push_str(&index.to_string());
                place.push(']');
            }
            psi_facts::PlaceSegment::Index { expression } => {
                place.push('[');
                place.push_str(&program.expression_table.display_name(*expression));
                place.push(']');
            }
        }
    }

    format!(
        "{} -> {} [created {}, last use {}]",
        symbol_name_for_state(program, machine, state, loan.owner_symbol),
        place,
        loan.statement_index,
        loan.last_use_statement_index
    )
}

fn borrow_activation_label(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    activation: &FlowBorrowActivationFact,
) -> String {
    format!(
        "{} -> {}",
        borrow_event_source_label(program, activation.source),
        borrow_loan_label(
            program,
            machine,
            state,
            program.facts.borrow.loans.get(activation.loan),
        ),
    )
}

fn borrow_weakening_label(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    weakening: &FlowBorrowWeakeningFact,
) -> String {
    format!(
        "{} -> {} ({})",
        borrow_event_source_label(program, weakening.source),
        borrow_loan_label(
            program,
            machine,
            state,
            program.facts.borrow.loans.get(weakening.loan),
        ),
        borrow_weakening_reason_label(weakening.reason),
    )
}

fn borrow_event_source_label(program: &CheckedTrees, source: FlowInvalidationSource) -> String {
    match source {
        FlowInvalidationSource::Statement { statement_index } => {
            format!("statement {statement_index}")
        }
        FlowInvalidationSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => format!(
            "call #{}.{} -> {}",
            statement_index,
            call_ordinal,
            state_label_from_symbol(program, target_symbol)
        ),
    }
}

fn borrow_weakening_reason_label(reason: FlowBorrowWeakeningReason) -> &'static str {
    match reason {
        FlowBorrowWeakeningReason::LastUseExpired => "after last use",
        FlowBorrowWeakeningReason::StateExit => "at state exit",
        FlowBorrowWeakeningReason::LocalReassigned => "after local reassignment",
    }
}

fn symbol_name_for_state(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> String {
    if symbol == machine.symbol {
        return "self".to_owned();
    }

    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
    {
        return parameter.name.as_str().to_owned();
    }

    if let Some(owned) = program
        .machine_owned_data(machine)
        .iter()
        .find(|owned| owned.symbol == symbol)
    {
        return owned.name.as_str().to_owned();
    }

    semantic_symbol_name(program, symbol)
}

fn flow_state_for(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&FlowStateFact> {
    program
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
                .then_some(state)
        })
}

fn borrow_state_for(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&psi_checked_trees::StateBorrowFact> {
    program.facts.borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
    })
}

fn machine_service_reach(
    program: &CheckedTrees,
    symbol: SymbolHandle,
) -> psi_language_semantics::ServiceReachSummary {
    program
        .facts
        .service_reaches
        .for_machine(symbol)
        .map(|reach| psi_language_semantics::ServiceReachSummary {
            direct: reach.inferred_direct,
            transitive: reach.inferred_transitive,
        })
        .unwrap_or_default()
}

fn machine_suspension_summary(
    program: &CheckedTrees,
    symbol: SymbolHandle,
) -> psi_language_semantics::SuspensionSummary {
    let mut summary = psi_language_semantics::SuspensionSummary::default();
    for flow in program
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| state.machine_symbol == symbol)
    {
        summary.direct_may_suspend |= flow.suspension.direct_may_suspend;
    }
    summary.transitive_may_suspend = program
        .facts
        .suspensions
        .for_machine(symbol)
        .is_some_and(|plan| plan.checked_may_suspend);
    summary
}

fn machine_blocking_summary(
    program: &CheckedTrees,
    symbol: SymbolHandle,
) -> psi_language_semantics::BlockingSummary {
    let mut summary = psi_language_semantics::BlockingSummary::default();
    for flow in program
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| state.machine_symbol == symbol)
    {
        summary.direct_may_block |= flow.blocking.direct_may_block;
    }
    summary.transitive_may_block = program
        .facts
        .blocking
        .for_machine(symbol)
        .is_some_and(|plan| plan.checked_may_block);
    summary
}

fn state_id_for_symbol(
    state_nodes: &[(SymbolHandle, String)],
    symbol: SymbolHandle,
) -> Option<&str> {
    state_nodes
        .iter()
        .find(|(candidate, _)| *candidate == symbol)
        .map(|(_, id)| id.as_str())
}

fn transition_target_id<'states>(
    program: &CheckedTrees,
    states: &'states [State],
    state_nodes: &'states [(SymbolHandle, String)],
    transition: &TableTransition,
) -> Option<&'states str> {
    transition_target_symbol_in_states(program, states, transition.target)
        .and_then(|symbol| state_id_for_symbol(state_nodes, symbol))
}

fn transition_target_symbol_in_states(
    program: &CheckedTrees,
    states: &[State],
    target: TransitionTargetHandle,
) -> Option<SymbolHandle> {
    if !target.is_valid() {
        return None;
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { path, .. } => states
            .iter()
            .find(|state| state.symbol == path.symbol)
            .map(|state| state.symbol),
        TransitionTargetNode::Value(_)
        | TransitionTargetNode::SelfTarget
        | TransitionTargetNode::Terminal => None,
    }
}

fn semantic_symbol_name(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    for machine in program.machines() {
        if machine.symbol == symbol {
            return machine.name.as_str().to_owned();
        }
        for state in program.machine_states(machine) {
            if state.symbol == symbol {
                return state.name.as_str().to_owned();
            }
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol {
                    return parameter.name.as_str().to_owned();
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.symbol == symbol {
                return owned.name.as_str().to_owned();
            }
        }
    }
    for data in program.data_definitions() {
        if data.symbol == symbol {
            return data.name.as_str().to_owned();
        }
        for member in program.data_members(data) {
            match member {
                psi_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return field.name.as_str().to_owned();
                }
                psi_typed_trees::data::DataMember::Variant(variant) if variant.symbol == symbol => {
                    return variant.name.as_str().to_owned();
                }
                _ => {}
            }
        }
    }
    if let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == symbol)
    {
        return domain.name.to_string();
    }
    if let Some(invariant) = program
        .invariant_definitions()
        .iter()
        .find(|invariant| invariant.symbol == symbol)
    {
        return invariant.name.to_string();
    }
    if let Some(trait_definition) = program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == symbol)
    {
        return trait_definition.name.as_str().to_owned();
    }
    program.symbols.name(symbol).to_string()
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
mod tests {
    use super::{
        carry_manifest_json, claim_outcome_manifest_json, machine_blocking_summary,
        machine_contract_manifest_json, machine_suspension_summary, mutation_frame_state_name,
        push_termination_interface_json, qualification_evidence_manifest_json,
        qualification_requirement_identity, qualification_subject,
        specialization_instance_contract_fingerprint, symbol_label, task_activation_manifest_json,
        validate_content_conservation_plan, validate_content_identity_reshuffle,
        validate_content_partition_input_custody, validate_content_partition_result_rewrites,
        validate_content_partition_substitution_replay, validate_qualification_program_point,
        validate_qualification_receipt, validate_qualification_source,
        validate_vacuous_qualification_use, validated_content_projection_plans,
        validated_machine_semantic_domain_commitments,
    };
    use psi_checked_trees::{
        CheckedTrees, ClaimCarryPolicyFact, ContentIdentityReshuffleFact,
        ContentPartitionCompositionFact, ContentPartitionPlaceSubstitution,
        ContentPartitionResultRewrite, DataCarryFact, FlowCallFact, FlowClaimOutcomeEntryFact,
        FlowClaimOutcomeMapFact, FlowClaimOutcomeSource, FlowPermissionEventFact, FlowStateFact,
        MachineActivationCarryFact, MachineContractPlan, MachineMutationFact,
        MachineQualifications, MachineServiceReachRows, StateWriteFramePlan,
        SuspensionCrossingCarryFact, VacuousQualificationUse,
    };
    use psi_facts::{
        Fact, FactOrigin, FactPayload, FactPlace, Place, PlaceRoot, ProgramPoint,
        QualificationEvidence,
    };
    use psi_language_semantics::content::{
        ContentAlgebraIdentity, ContentArithmeticOperator, ContentConservationEquation,
        ContentConservationOwnerKind, ContentConservationPlan, ContentConservationTerm,
        ContentFieldSegment, ContentPlaceRoot, ContentPlaceVersion, ContentProjectionExpression,
        ContentProjectionPlan, ContentScalarExpression, ContentStructuralPlace,
        conservation_fingerprint, projection_fingerprint,
    };
    use psi_language_semantics::{
        BlockingInterface, BlockingPlan, BlockingSummary, CarryAddress, CarryCpu, CarryHostThread,
        CarryPolicy, CarrySuspension, MachineSupplyMode, MachineTerminationPlan, PermissionAccess,
        PermissionClaimIdentity, PermissionEventKind, PermissionEventSource, PermissionProvenance,
        QualificationEvidenceOrigin, RankingViewId, RankingWitness, SemanticDomainId,
        SuspensionInterface, SuspensionPlan, SuspensionSummary, TerminationGuarantee,
        TerminationInterface,
    };
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::data::{TypeParameter, TypeParameterKind};
    use psi_typed_trees::domain::DomainDefinition;
    use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::operator::OperatorDefinition;
    use psi_typed_trees::signature::{StateParameter, StateSignature};
    use psi_typed_trees::state::State;
    use psi_typed_trees::statement::StatementNode;
    use psi_typed_trees::trait_definition::TraitDefinition;
    use psi_typed_trees::typed_trees::MachineSpecialization;
    use psi_typed_trees::types::TypeReferenceNode;

    use omega_effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceEntryAuthorityFlow,
        ServiceEntryClaim, ServiceMethod, ServiceResultClaim, ServiceSchema,
    };

    fn push_behavior_contract(
        program: &mut CheckedTrees,
        machine: SymbolHandle,
        checked_may_suspend: bool,
        checked_may_block: bool,
    ) {
        program
            .facts
            .suspensions
            .machines
            .push(psi_checked_trees::MachineSuspensionFact {
                machine,
                plan: SuspensionPlan {
                    checked_may_suspend,
                    ..Default::default()
                },
            });
        program
            .facts
            .blocking
            .machines
            .push(psi_checked_trees::MachineBlockingFact {
                machine,
                plan: BlockingPlan {
                    checked_may_block,
                    ..Default::default()
                },
            });
        program
            .facts
            .termination
            .machines
            .push(psi_checked_trees::MachineTerminationFact {
                machine,
                plan: Default::default(),
            });
        program
            .facts
            .contract_plans
            .machines
            .push(MachineContractPlan {
                machine,
                closed_scalar_values: Default::default(),
                crash: Default::default(),
                fingerprint: 0,
            });
    }

    fn push_behavior_flow_state(
        program: &mut CheckedTrees,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        suspension: SuspensionSummary,
        blocking: BlockingSummary,
    ) {
        program.facts.flow.control.states.insert(FlowStateFact {
            machine_symbol,
            state_symbol,
            suspension,
            blocking,
            ..Default::default()
        });
    }

    fn mutation_state_owner_fixture() -> (CheckedTrees, SymbolHandle, SymbolHandle, SymbolHandle) {
        let owner = SymbolHandle::from_arena_index(50);
        let owner_state = SymbolHandle::from_arena_index(51);
        let other = SymbolHandle::from_arena_index(52);
        let other_state = SymbolHandle::from_arena_index(53);
        let mut program = CheckedTrees::default();
        for (machine_symbol, state_symbol, machine_name, state_name) in [
            (owner, owner_state, "Owner::write", "entry"),
            (other, other_state, "Other::write", "other_entry"),
        ] {
            let mut machine = Machine {
                symbol: machine_symbol,
                name: Identifier::generated(machine_name),
                ..Default::default()
            };
            program.typed.push_machine_state(
                &mut machine,
                State {
                    symbol: state_symbol,
                    name: Identifier::generated(state_name),
                    ..Default::default()
                },
            );
            program.typed.push_machine(machine);
        }
        (program, owner, owner_state, other_state)
    }

    fn vacuous_qualification_fixture() -> (
        CheckedTrees,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
        SemanticDomainId,
        ExpressionHandle,
        ExpressionHandle,
    ) {
        let machine_symbol = SymbolHandle::from_arena_index(60);
        let state_symbol = SymbolHandle::from_arena_index(61);
        let other_state_symbol = SymbolHandle::from_arena_index(63);
        let domain_symbol = SymbolHandle::from_arena_index(64);
        let mut program = CheckedTrees::default();
        let semantic_domain = program.typed.semantic_domains.intern("i64::Distance<1000>");
        let declaration_domain = program.typed.semantic_domains.intern("i64::Distance");
        program.typed.push_domain_definition(DomainDefinition {
            symbol: domain_symbol,
            name: Identifier::generated("i64::Distance"),
            semantic_id: declaration_domain,
            ..Default::default()
        });
        let cast_value = program
            .typed
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let cast_expression =
            program
                .typed
                .expression_table
                .insert(ExpressionNode::Cast(TableCastExpression {
                    value: cast_value,
                    target_type: Default::default(),
                    target_label: Default::default(),
                    domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
                    semantic_domain: Default::default(),
                    semantic_domain_arguments: Default::default(),
                    semantic_domain_symbol: domain_symbol,
                    semantic_domain_id: semantic_domain,
                    form: Default::default(),
                }));
        let statement_expression = program
            .typed
            .expression_table
            .insert(ExpressionNode::Mutable(cast_expression));
        for (machine, state, machine_name, state_name) in [
            (machine_symbol, state_symbol, "Main::main", "main"),
            (
                SymbolHandle::from_arena_index(62),
                other_state_symbol,
                "Other::run",
                "run",
            ),
        ] {
            let mut definition = Machine {
                symbol: machine,
                name: Identifier::generated(machine_name),
                ..Default::default()
            };
            let mut state_definition = State {
                symbol: state,
                name: Identifier::generated(state_name),
                ..Default::default()
            };
            if machine == machine_symbol {
                for _ in 0..3 {
                    program
                        .typed
                        .statement_table
                        .push_statement(&mut state_definition.statement_nodes, Default::default());
                }
                program.typed.statement_table.push_statement(
                    &mut state_definition.statement_nodes,
                    StatementNode::Expression(statement_expression),
                );
            }
            program
                .typed
                .push_machine_state(&mut definition, state_definition);
            program.typed.push_machine(definition);
        }
        (
            program,
            machine_symbol,
            state_symbol,
            other_state_symbol,
            domain_symbol,
            semantic_domain,
            cast_expression,
            statement_expression,
        )
    }

    fn selected_storage_plan() -> ProviderPlan {
        ProviderPlan {
            name: "selected::Storage".to_owned(),
            provider_type: "StorageProvider".to_owned(),
            target: String::new(),
            schema: ServiceSchema {
                trait_name: "StorageRoot".to_owned(),
                methods: vec![ServiceMethod {
                    name: "transfer".to_owned(),
                    requirement_owner: "StorageBase".to_owned(),
                    requirement_identity: "StorageBase::transfer".to_owned(),
                    parameter_count: 1,
                    parameter_type_identities: vec!["Token".to_owned()],
                    entry_claims: vec![ServiceEntryClaim {
                        parameter_index: 0,
                        domain: "Token::Granted".to_owned(),
                        predicate_body: psi_language_semantics::DomainPredicateBody::Bodyless,
                        effective_carry: CarryPolicy::STRICT,
                        authority_flow: ServiceEntryAuthorityFlow::Accepts,
                    }],
                    has_result: true,
                    result_type_identity: Some("Token".to_owned()),
                    result_claims: vec![ServiceResultClaim {
                        domain: "Token::Issued".to_owned(),
                        effective_carry: CarryPolicy::STRICT,
                    }],
                    service_reach: vec!["StorageRoot".to_owned()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    calling_plan_fingerprint: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "transfer".to_owned(),
                requirement_identity: "StorageBase::transfer".to_owned(),
                binding: ProviderBinding::CheckedAdapter {
                    machine: "StorageProvider::transfer".to_owned(),
                },
            }],
            origin_package: "omega::providers::storage".to_owned(),
        }
    }

    fn push_qualification_requirement(
        program: &mut CheckedTrees,
        is_boundary: bool,
        owner_index: u32,
        requirement_index: u32,
        owner_name: &str,
    ) -> (SymbolHandle, SymbolHandle) {
        let mut owner = TraitDefinition {
            symbol: SymbolHandle::from_arena_index(owner_index),
            is_boundary,
            name: Identifier::generated(owner_name),
            ..Default::default()
        };
        let requirement = SymbolHandle::from_arena_index(requirement_index);
        program.typed.push_trait_machine_signature(
            &mut owner,
            StateSignature {
                symbol: requirement,
                name: Identifier::generated("transfer"),
                ..Default::default()
            },
        );
        let owner_symbol = owner.symbol;
        program.typed.push_trait_definition(owner);
        (owner_symbol, requirement)
    }

    #[test]
    fn checked_behavior_summaries_keep_operational_axes_independent() {
        let suspending_machine = SymbolHandle::from_arena_index(1);
        let blocking_machine = SymbolHandle::from_arena_index(2);
        let unknown_machine = SymbolHandle::from_arena_index(3);
        let mut program = CheckedTrees::default();

        push_behavior_contract(&mut program, suspending_machine, true, false);
        push_behavior_flow_state(
            &mut program,
            suspending_machine,
            SymbolHandle::from_arena_index(11),
            SuspensionSummary {
                direct_may_suspend: true,
                transitive_may_suspend: false,
            },
            BlockingSummary::default(),
        );
        push_behavior_contract(&mut program, blocking_machine, false, true);
        push_behavior_flow_state(
            &mut program,
            blocking_machine,
            SymbolHandle::from_arena_index(12),
            SuspensionSummary::default(),
            BlockingSummary {
                direct_may_block: true,
                transitive_may_block: false,
            },
        );

        assert_eq!(
            machine_suspension_summary(&program, suspending_machine),
            SuspensionSummary {
                direct_may_suspend: true,
                transitive_may_suspend: true,
            }
        );
        assert_eq!(
            machine_blocking_summary(&program, suspending_machine),
            BlockingSummary::default()
        );
        assert_eq!(
            machine_suspension_summary(&program, blocking_machine),
            SuspensionSummary::default()
        );
        assert_eq!(
            machine_blocking_summary(&program, blocking_machine),
            BlockingSummary {
                direct_may_block: true,
                transitive_may_block: true,
            }
        );
        assert_eq!(
            machine_suspension_summary(&program, unknown_machine),
            SuspensionSummary::default()
        );
        assert_eq!(
            machine_blocking_summary(&program, unknown_machine),
            BlockingSummary::default()
        );
    }

    fn claim_outcome_validation_fixture() -> (
        CheckedTrees,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
    ) {
        let machine_symbol = SymbolHandle::from_arena_index(100);
        let state_symbol = SymbolHandle::from_arena_index(101);
        let parameter_symbol = SymbolHandle::from_arena_index(102);
        let other_machine_symbol = SymbolHandle::from_arena_index(103);
        let other_state_symbol = SymbolHandle::from_arena_index(104);
        let mut program = CheckedTrees::default();
        for (machine, state, machine_name, state_name) in [
            (machine_symbol, state_symbol, "Claims::map", "map"),
            (
                other_machine_symbol,
                other_state_symbol,
                "OtherClaims::map",
                "map",
            ),
        ] {
            let mut machine_definition = Machine {
                symbol: machine,
                name: Identifier::generated(machine_name),
                ..Default::default()
            };
            let mut state_definition = State {
                symbol: state,
                name: Identifier::generated(state_name),
                ..Default::default()
            };
            if machine == machine_symbol {
                program.typed.push_state_parameter(
                    &mut state_definition,
                    StateParameter {
                        symbol: parameter_symbol,
                        name: Identifier::generated("resource"),
                        ..Default::default()
                    },
                );
                for _ in 0..5 {
                    program
                        .typed
                        .statement_table
                        .push_statement(&mut state_definition.statement_nodes, Default::default());
                }
            }
            program
                .typed
                .push_machine_state(&mut machine_definition, state_definition);
            program.typed.push_machine(machine_definition);
        }
        let input_identity = PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::StateEntry,
            ordinal: 0,
        };
        let input_provenance = PermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::StateEntry,
        };
        program
            .facts
            .flow
            .ownership
            .permissions
            .insert(FlowPermissionEventFact {
                machine_symbol,
                state_symbol,
                source: PermissionEventSource::StateEntry,
                kind: PermissionEventKind::Establish,
                access: PermissionAccess::Owned,
                claim_identity: input_identity,
                provenance: input_provenance,
                root: PlaceRoot::Symbol(parameter_symbol),
                obligation_live: true,
                ..Default::default()
            });
        let established_identity = PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Statement { statement_index: 0 },
            ordinal: 1,
        };
        let established_provenance = PermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Statement { statement_index: 0 },
        };
        program
            .facts
            .flow
            .ownership
            .permissions
            .insert(FlowPermissionEventFact {
                machine_symbol,
                state_symbol,
                source: PermissionEventSource::Statement { statement_index: 0 },
                kind: PermissionEventKind::Transfer,
                access: PermissionAccess::Owned,
                claim_identity: established_identity,
                provenance: established_provenance,
                root: PlaceRoot::Symbol(state_symbol),
                obligation_live: true,
                ..Default::default()
            });
        let established_output = program
            .facts
            .flow
            .ownership
            .segments
            .insert_many([psi_facts::PlaceSegment::FixedIndex { index: 1 }]);
        let entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .insert_many([
                FlowClaimOutcomeEntryFact {
                    output_segments: Default::default(),
                    source: FlowClaimOutcomeSource::Input {
                        parameter_symbol,
                        segments: Default::default(),
                    },
                },
                FlowClaimOutcomeEntryFact {
                    output_segments: established_output,
                    source: FlowClaimOutcomeSource::Established {
                        claim_identity: established_identity,
                        provenance: established_provenance,
                    },
                },
            ]);
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .insert(FlowClaimOutcomeMapFact {
                machine_symbol,
                state_symbol,
                entries,
            });
        (
            program,
            machine_symbol,
            state_symbol,
            parameter_symbol,
            other_machine_symbol,
            other_state_symbol,
        )
    }

    fn first_claim_outcome_entries_mut(
        program: &mut CheckedTrees,
    ) -> &mut [FlowClaimOutcomeEntryFact] {
        let entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .iter()
            .next()
            .expect("fixture map")
            .1
            .entries;
        program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .span_mut(entries)
            .expect("fixture entries")
    }

    #[test]
    fn claim_outcome_manifest_accepts_exact_sources_and_explicit_empty_map() {
        let (mut program, _, _, _, other_machine, other_state) = claim_outcome_validation_fixture();
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .insert(FlowClaimOutcomeMapFact {
                machine_symbol: other_machine,
                state_symbol: other_state,
                entries: Default::default(),
            });

        let json = claim_outcome_manifest_json(&program);
        assert!(json.contains("\"kind\": \"input\""));
        assert!(json.contains("\"kind\": \"established\""));
        assert_eq!(json.matches("\"entries\": [").count(), 2);
        assert!(json.contains("\"entries\": [\n      ]"));
    }

    #[test]
    #[should_panic(expected = "state must belong to its exact typed machine")]
    fn claim_outcome_manifest_rejects_cross_machine_state() {
        let (mut program, _, _, _, _, other_state) = claim_outcome_validation_fixture();
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .for_each_mut(|_, map| map.state_symbol = other_state);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "one row per exact machine and state")]
    fn claim_outcome_manifest_rejects_duplicate_map_coordinate() {
        let (mut program, ..) = claim_outcome_validation_fixture();
        let duplicate = program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .iter()
            .next()
            .expect("fixture map")
            .1
            .clone();
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .insert(duplicate);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "exact valid entry span")]
    fn claim_outcome_manifest_rejects_invalid_entry_span() {
        let (mut program, ..) = claim_outcome_validation_fixture();
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .for_each_mut(|_, map| {
                map.entries =
                    psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
            });
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "exact valid output path span")]
    fn claim_outcome_manifest_rejects_invalid_output_path_span() {
        let (mut program, ..) = claim_outcome_validation_fixture();
        first_claim_outcome_entries_mut(&mut program)[0].output_segments =
            psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "one entry per exact output path")]
    fn claim_outcome_manifest_rejects_duplicate_output_path() {
        let (mut program, ..) = claim_outcome_validation_fixture();
        let entries = first_claim_outcome_entries_mut(&mut program);
        entries[1].output_segments = entries[0].output_segments;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain an exact known source")]
    fn claim_outcome_manifest_rejects_unknown_source() {
        let (mut program, ..) = claim_outcome_validation_fixture();
        first_claim_outcome_entries_mut(&mut program)[0].source = FlowClaimOutcomeSource::Unknown;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "exact parameter owned by its state")]
    fn claim_outcome_manifest_rejects_missing_input_parameter() {
        let (mut program, ..) = claim_outcome_validation_fixture();
        first_claim_outcome_entries_mut(&mut program)[0].source = FlowClaimOutcomeSource::Input {
            parameter_symbol: SymbolHandle::from_arena_index(999),
            segments: Default::default(),
        };
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "input source must retain an exact valid path span")]
    fn claim_outcome_manifest_rejects_invalid_input_path_span() {
        let (mut program, _, _, parameter, ..) = claim_outcome_validation_fixture();
        first_claim_outcome_entries_mut(&mut program)[0].source = FlowClaimOutcomeSource::Input {
            parameter_symbol: parameter,
            segments: psi_arena::HandleSpan::from_parts(
                psi_arena::Handle::from_arena_index(999),
                1,
            ),
        };
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "one distinct live retained permission origin")]
    fn claim_outcome_manifest_rejects_absent_input_origin() {
        let (mut program, ..) = claim_outcome_validation_fixture();
        program.facts.flow.ownership.permissions = Default::default();
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "one distinct live retained permission origin")]
    fn claim_outcome_manifest_rejects_ambiguous_input_origin() {
        let (mut program, machine, state, parameter, ..) = claim_outcome_validation_fixture();
        program
            .facts
            .flow
            .ownership
            .permissions
            .insert(FlowPermissionEventFact {
                machine_symbol: machine,
                state_symbol: state,
                source: PermissionEventSource::StateEntry,
                kind: PermissionEventKind::Establish,
                access: PermissionAccess::Owned,
                claim_identity: PermissionClaimIdentity::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: PermissionEventSource::StateEntry,
                    ordinal: 9,
                },
                provenance: PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: PermissionEventSource::StateEntry,
                },
                root: PlaceRoot::Symbol(parameter),
                obligation_live: true,
                ..Default::default()
            });
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "non-unknown claim identity")]
    fn claim_outcome_manifest_rejects_unknown_established_identity() {
        let (mut program, machine, state, ..) = claim_outcome_validation_fixture();
        first_claim_outcome_entries_mut(&mut program)[1].source =
            FlowClaimOutcomeSource::Established {
                claim_identity: PermissionClaimIdentity::Unknown,
                provenance: PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: PermissionEventSource::Statement { statement_index: 0 },
                },
            };
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain non-unknown provenance")]
    fn claim_outcome_manifest_rejects_unknown_established_provenance() {
        let (mut program, machine, state, ..) = claim_outcome_validation_fixture();
        first_claim_outcome_entries_mut(&mut program)[1].source =
            FlowClaimOutcomeSource::Established {
                claim_identity: PermissionClaimIdentity::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: PermissionEventSource::Statement { statement_index: 0 },
                    ordinal: 1,
                },
                provenance: PermissionProvenance::Unknown,
            };
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must match one retained permission event")]
    fn claim_outcome_manifest_rejects_detached_established_pair() {
        let (mut program, machine, state, ..) = claim_outcome_validation_fixture();
        first_claim_outcome_entries_mut(&mut program)[1].source =
            FlowClaimOutcomeSource::Established {
                claim_identity: PermissionClaimIdentity::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: PermissionEventSource::Statement { statement_index: 0 },
                    ordinal: 99,
                },
                provenance: PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: PermissionEventSource::Statement { statement_index: 0 },
                },
            };
        claim_outcome_manifest_json(&program);
    }

    fn content_identity_reshuffle_validation_fixture() -> CheckedTrees {
        let (mut program, machine, state, parameter, ..) = claim_outcome_validation_fixture();
        let domain = SymbolHandle::from_arena_index(105);
        let carrier_symbol = SymbolHandle::from_arena_index(106);
        let projection_machine_symbol = SymbolHandle::from_arena_index(107);
        let projection_state_symbol = SymbolHandle::from_arena_index(108);
        let carrier = program
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: carrier_symbol,
                name: Identifier::generated("Resource"),
            });
        let semantic_domain = program.typed.semantic_domains.intern("Resource::Counted");
        program.typed.push_domain_definition(DomainDefinition {
            symbol: domain,
            name: Identifier::generated("Resource::Counted"),
            target_type: carrier,
            semantic_id: semantic_domain,
            ..Default::default()
        });
        let mut projection_machine = Machine {
            symbol: projection_machine_symbol,
            name: Identifier::generated("Resource::content"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut projection_machine,
            State {
                symbol: projection_state_symbol,
                name: Identifier::generated("entry"),
                ..Default::default()
            },
        );
        program.typed.push_machine(projection_machine);
        let algebra = ContentAlgebraIdentity::CountedQuantity {
            unit: "named(name(Unit))".to_owned(),
        };
        let expression = ContentProjectionExpression::CountedQuantity {
            magnitude: ContentScalarExpression::Natural("1".to_owned()),
        };
        let projection_identity = projection_fingerprint(&algebra, &expression);
        program
            .facts
            .qualifications
            .content
            .plans
            .push(ContentProjectionPlan {
                domain,
                semantic_domain,
                carrier_identity: program
                    .typed
                    .normalized_type_identity(carrier)
                    .into_string(),
                machine: projection_machine_symbol,
                algebra: algebra.clone(),
                expression,
                fingerprint: projection_identity,
            });
        let input = ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine: projection_machine_symbol,
            projection_fingerprint: projection_identity,
            subject: ContentStructuralPlace {
                version: ContentPlaceVersion::Entry,
                root: ContentPlaceRoot::Parameter {
                    position: 0,
                    symbol: parameter,
                    name: "resource".to_owned(),
                    is_self: false,
                },
                segments: Vec::new(),
            },
        };
        let output = ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine: projection_machine_symbol,
            projection_fingerprint: projection_identity,
            subject: ContentStructuralPlace {
                version: ContentPlaceVersion::Current,
                root: ContentPlaceRoot::Result,
                segments: Vec::new(),
            },
        };
        let equation = ContentConservationEquation::new(input, output);
        let fingerprint = conservation_fingerprint(&algebra, &equation);
        let claim_identity = program
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .find_map(|(_, event)| {
                (event.source == PermissionEventSource::StateEntry).then_some(event.claim_identity)
            })
            .expect("fixture entry identity");
        program
            .facts
            .qualifications
            .content
            .identity_reshuffles
            .push(ContentIdentityReshuffleFact {
                machine_symbol: machine,
                state_symbol: state,
                claim_identity,
                input_parameter_symbol: parameter,
                input_segments: Default::default(),
                output_segments: Default::default(),
                plan: ContentConservationPlan {
                    owner_kind: ContentConservationOwnerKind::Machine,
                    owner: machine,
                    callable: state,
                    algebra,
                    equation,
                    fingerprint,
                },
            });
        program
    }

    #[test]
    fn content_identity_reshuffle_manifest_accepts_exact_witness_custody() {
        let program = content_identity_reshuffle_validation_fixture();
        let json = claim_outcome_manifest_json(&program);
        assert!(json.contains("\"content_identity_reshuffles\": [\n    {"));
        assert!(json.contains("\"input\": {\"parameter\":"));
        assert!(json.contains("\"output_path\": []"));
    }

    #[test]
    #[should_panic(expected = "input must retain an exact valid path span")]
    fn content_identity_reshuffle_manifest_rejects_invalid_input_span() {
        let mut program = content_identity_reshuffle_validation_fixture();
        program.facts.qualifications.content.identity_reshuffles[0].input_segments =
            psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "output must retain an exact valid path span")]
    fn content_identity_reshuffle_manifest_rejects_invalid_output_span() {
        let mut program = content_identity_reshuffle_validation_fixture();
        program.facts.qualifications.content.identity_reshuffles[0].output_segments =
            psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "exact parameter owned by its state")]
    fn content_identity_reshuffle_manifest_rejects_missing_parameter() {
        let mut program = content_identity_reshuffle_validation_fixture();
        program.facts.qualifications.content.identity_reshuffles[0].input_parameter_symbol =
            SymbolHandle::from_arena_index(999);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain a non-unknown claim identity")]
    fn content_identity_reshuffle_manifest_rejects_unknown_claim_identity() {
        let mut program = content_identity_reshuffle_validation_fixture();
        program.facts.qualifications.content.identity_reshuffles[0].claim_identity =
            PermissionClaimIdentity::Unknown;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain its exact input permission identity")]
    fn content_identity_reshuffle_manifest_rejects_wrong_claim_identity() {
        let mut program = content_identity_reshuffle_validation_fixture();
        let row = &mut program.facts.qualifications.content.identity_reshuffles[0];
        row.claim_identity = PermissionClaimIdentity::Established {
            machine_symbol: row.machine_symbol,
            state_symbol: row.state_symbol,
            source: PermissionEventSource::StateEntry,
            ordinal: 99,
        };
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "one distinct live retained permission identity")]
    fn content_identity_reshuffle_manifest_rejects_ambiguous_entry_identity() {
        let mut program = content_identity_reshuffle_validation_fixture();
        let row = program.facts.qualifications.content.identity_reshuffles[0].clone();
        program
            .facts
            .flow
            .ownership
            .permissions
            .insert(FlowPermissionEventFact {
                machine_symbol: row.machine_symbol,
                state_symbol: row.state_symbol,
                source: PermissionEventSource::StateEntry,
                kind: PermissionEventKind::Establish,
                access: PermissionAccess::Owned,
                claim_identity: PermissionClaimIdentity::Established {
                    machine_symbol: row.machine_symbol,
                    state_symbol: row.state_symbol,
                    source: PermissionEventSource::StateEntry,
                    ordinal: 99,
                },
                provenance: PermissionProvenance::Established {
                    machine_symbol: row.machine_symbol,
                    state_symbol: row.state_symbol,
                    source: PermissionEventSource::StateEntry,
                },
                root: PlaceRoot::Symbol(row.input_parameter_symbol),
                obligation_live: true,
                ..Default::default()
            });
        validate_content_identity_reshuffle(&program, &row);
    }

    #[test]
    #[should_panic(expected = "one exact input-relative claim outcome")]
    fn content_identity_reshuffle_manifest_rejects_absent_input_outcome() {
        let mut program = content_identity_reshuffle_validation_fixture();
        let unmatched_output = program
            .facts
            .flow
            .ownership
            .segments
            .insert_many([psi_facts::PlaceSegment::FixedIndex { index: 9 }]);
        program.facts.qualifications.content.identity_reshuffles[0].output_segments =
            unmatched_output;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "paths must not retain a runtime index")]
    fn content_identity_reshuffle_manifest_rejects_runtime_index_path() {
        let mut program = content_identity_reshuffle_validation_fixture();
        let runtime_output =
            program
                .facts
                .flow
                .ownership
                .segments
                .insert_many([psi_facts::PlaceSegment::Index {
                    expression: ExpressionHandle::invalid(),
                }]);
        program.facts.qualifications.content.identity_reshuffles[0].output_segments =
            runtime_output;
        first_claim_outcome_entries_mut(&mut program)[0].output_segments = runtime_output;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "field path must name an exact typed field")]
    fn content_identity_reshuffle_manifest_rejects_missing_typed_segment() {
        let mut program = content_identity_reshuffle_validation_fixture();
        let missing_output =
            program
                .facts
                .flow
                .ownership
                .segments
                .insert_many([psi_facts::PlaceSegment::Field {
                    symbol: SymbolHandle::from_arena_index(999),
                }]);
        program.facts.qualifications.content.identity_reshuffles[0].output_segments =
            missing_output;
        first_claim_outcome_entries_mut(&mut program)[0].output_segments = missing_output;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(
        expected = "equation must retain its exact input and output projection subjects"
    )]
    fn content_identity_reshuffle_manifest_rejects_subject_drift() {
        let mut program = content_identity_reshuffle_validation_fixture();
        let row = &mut program.facts.qualifications.content.identity_reshuffles[0];
        let mutate_subject = |term: &ContentConservationTerm| match term {
            ContentConservationTerm::Projection {
                domain,
                semantic_domain,
                projection_machine,
                projection_fingerprint,
                subject,
            } if subject.version == ContentPlaceVersion::Entry => {
                let mut subject = subject.clone();
                let ContentPlaceRoot::Parameter { position, .. } = &mut subject.root else {
                    unreachable!("fixture entry subject is a parameter")
                };
                *position = 1;
                ContentConservationTerm::Projection {
                    domain: *domain,
                    semantic_domain: *semantic_domain,
                    projection_machine: *projection_machine,
                    projection_fingerprint: *projection_fingerprint,
                    subject,
                }
            }
            other => other.clone(),
        };
        let left = mutate_subject(row.plan.equation.left());
        let right = mutate_subject(row.plan.equation.right());
        row.plan.equation = ContentConservationEquation::new(left, right);
        row.plan.fingerprint = conservation_fingerprint(&row.plan.algebra, &row.plan.equation);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain one exact witness row per plan")]
    fn content_identity_reshuffle_manifest_rejects_duplicate_row() {
        let mut program = content_identity_reshuffle_validation_fixture();
        let duplicate = program.facts.qualifications.content.identity_reshuffles[0].clone();
        program
            .facts
            .qualifications
            .content
            .identity_reshuffles
            .push(duplicate);
        claim_outcome_manifest_json(&program);
    }

    fn content_partition_input_validation_fixture() -> CheckedTrees {
        let mut program = content_identity_reshuffle_validation_fixture();
        let reshuffle = program.facts.qualifications.content.identity_reshuffles[0].clone();
        let entry_place = [
            reshuffle.plan.equation.left(),
            reshuffle.plan.equation.right(),
        ]
        .into_iter()
        .find_map(|term| match term {
            ContentConservationTerm::Projection { subject, .. }
                if subject.version == ContentPlaceVersion::Entry =>
            {
                Some(subject.clone())
            }
            _ => None,
        })
        .expect("fixture entry projection subject");
        let source_left = reshuffle.plan.equation.left().clone();
        let source_right = reshuffle.plan.equation.right().clone();
        let source_equation = ContentConservationEquation::new(
            ContentConservationTerm::Separate(vec![source_left.clone(), source_right.clone()]),
            ContentConservationTerm::Separate(vec![source_left, source_right]),
        );
        let source_plan = ContentConservationPlan {
            owner_kind: reshuffle.plan.owner_kind,
            owner: reshuffle.plan.owner,
            callable: reshuffle.plan.callable,
            algebra: reshuffle.plan.algebra.clone(),
            fingerprint: conservation_fingerprint(&reshuffle.plan.algebra, &source_equation),
            equation: source_equation,
        };
        let mut substitution_subjects = Vec::new();
        for term in [source_plan.equation.left(), source_plan.equation.right()] {
            let ContentConservationTerm::Separate(children) = term else {
                unreachable!("fixture source equation is separated")
            };
            for child in children {
                let ContentConservationTerm::Projection { subject, .. } = child else {
                    unreachable!("fixture separated children are projections")
                };
                if !substitution_subjects.contains(subject) {
                    substitution_subjects.push(subject.clone());
                }
            }
        }
        let substitutions = substitution_subjects
            .into_iter()
            .map(|subject| ContentPartitionPlaceSubstitution {
                source: subject.clone(),
                target: subject,
            })
            .collect();
        let calls = program.facts.flow.control.calls.insert_many([FlowCallFact {
            statement_index: 4,
            call_ordinal: 2,
            target_symbol: reshuffle.state_symbol,
            ..Default::default()
        }]);
        program.facts.flow.control.states.insert(FlowStateFact {
            machine_symbol: reshuffle.machine_symbol,
            state_symbol: reshuffle.state_symbol,
            calls,
            ..Default::default()
        });
        program
            .facts
            .qualifications
            .content
            .partition_compositions
            .push(ContentPartitionCompositionFact {
                machine_symbol: reshuffle.machine_symbol,
                state_symbol: reshuffle.state_symbol,
                source_callable: source_plan.callable,
                source_fingerprint: source_plan.fingerprint,
                source_derivation_depth: 0,
                source_plan: source_plan.clone(),
                statement_index: 4,
                call_ordinal: 2,
                input_claim_identities: vec![reshuffle.claim_identity],
                input_claim_bindings: vec![psi_checked_trees::ContentPartitionInputClaimBinding {
                    claim_identity: reshuffle.claim_identity,
                    entry_place,
                }],
                result_rewrites: Vec::new(),
                substitutions,
                plan: source_plan,
            });
        program
    }

    #[test]
    fn content_partition_input_manifest_accepts_exact_call_and_input_custody() {
        let program = content_partition_input_validation_fixture();
        let json = claim_outcome_manifest_json(&program);

        assert!(json.contains("\"call\": {\"statement_index\": 4, \"call_ordinal\": 2}"));
        assert!(json.contains("\"input_claim_identities\": [{\"kind\": \"established\""));
        assert!(json.contains("\"entry_place\": {\"version\": \"entry\""));
    }

    #[test]
    #[should_panic(expected = "statement index must be within its exact state")]
    fn content_partition_input_manifest_rejects_out_of_range_statement() {
        let mut program = content_partition_input_validation_fixture();
        program.facts.qualifications.content.partition_compositions[0].statement_index = 5;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must name one exact checked flow state")]
    fn content_partition_input_manifest_rejects_missing_flow_state() {
        let mut program = content_partition_input_validation_fixture();
        program.facts.flow.control.states.clear();
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must name exactly one checked flow state")]
    fn content_partition_input_manifest_rejects_duplicate_flow_state() {
        let mut program = content_partition_input_validation_fixture();
        let duplicate = program
            .facts
            .flow
            .control
            .states
            .iter()
            .next()
            .expect("fixture flow state")
            .1
            .clone();
        program.facts.flow.control.states.insert(duplicate);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "flow state must retain an exact valid call span")]
    fn content_partition_input_manifest_rejects_invalid_call_span() {
        let mut program = content_partition_input_validation_fixture();
        program.facts.flow.control.states.for_each_mut(|_, state| {
            state.calls =
                psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
        });
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain one exact checked call coordinate")]
    fn content_partition_input_manifest_rejects_wrong_call_ordinal() {
        let mut program = content_partition_input_validation_fixture();
        let calls = program
            .facts
            .flow
            .control
            .states
            .iter()
            .next()
            .expect("fixture flow state")
            .1
            .calls;
        program
            .facts
            .flow
            .control
            .calls
            .span_mut(calls)
            .expect("fixture calls")[0]
            .call_ordinal = 3;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain its exact checked source target")]
    fn content_partition_input_manifest_rejects_wrong_call_target() {
        let mut program = content_partition_input_validation_fixture();
        let calls = program
            .facts
            .flow
            .control
            .states
            .iter()
            .next()
            .expect("fixture flow state")
            .1
            .calls;
        program
            .facts
            .flow
            .control
            .calls
            .span_mut(calls)
            .expect("fixture calls")[0]
            .target_symbol = SymbolHandle::from_arena_index(999);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain exactly one checked call coordinate")]
    fn content_partition_input_manifest_rejects_duplicate_call_coordinate() {
        let mut program = content_partition_input_validation_fixture();
        let old_calls = program
            .facts
            .flow
            .control
            .states
            .iter()
            .next()
            .expect("fixture flow state")
            .1
            .calls;
        let call = program
            .facts
            .flow
            .control
            .calls
            .span(old_calls)
            .expect("fixture calls")[0]
            .clone();
        let calls = program
            .facts
            .flow
            .control
            .calls
            .insert_many([call.clone(), call]);
        program
            .facts
            .flow
            .control
            .states
            .for_each_mut(|_, state| state.calls = calls);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain at least one input claim identity")]
    fn content_partition_input_manifest_rejects_empty_inputs() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.input_claim_identities.clear();
        row.input_claim_bindings.clear();
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "input claim identities must be non-unknown")]
    fn content_partition_input_manifest_rejects_unknown_identity() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.input_claim_identities[0] = PermissionClaimIdentity::Unknown;
        row.input_claim_bindings[0].claim_identity = PermissionClaimIdentity::Unknown;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "input claim identities must be unique")]
    fn content_partition_input_manifest_rejects_duplicate_identity() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.input_claim_identities
            .push(row.input_claim_identities[0]);
        row.input_claim_bindings
            .push(row.input_claim_bindings[0].clone());
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "input identities must exactly match ordered bindings")]
    fn content_partition_input_manifest_rejects_binding_identity_drift() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.input_claim_bindings[0].claim_identity = PermissionClaimIdentity::Established {
            machine_symbol: row.machine_symbol,
            state_symbol: row.state_symbol,
            source: PermissionEventSource::StateEntry,
            ordinal: 99,
        };
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must name its exact caller parameter")]
    fn content_partition_input_manifest_rejects_wrong_entry_parameter() {
        let mut program = content_partition_input_validation_fixture();
        let ContentPlaceRoot::Parameter { symbol, .. } =
            &mut program.facts.qualifications.content.partition_compositions[0]
                .input_claim_bindings[0]
                .entry_place
                .root
        else {
            unreachable!("fixture binding is an entry parameter")
        };
        *symbol = SymbolHandle::from_arena_index(999);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must match one live retained permission event")]
    fn content_partition_input_manifest_rejects_binding_path_drift() {
        let mut program = content_partition_input_validation_fixture();
        program.facts.qualifications.content.partition_compositions[0].input_claim_bindings[0]
            .entry_place
            .segments
            .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(9));
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "paths must not retain a runtime index")]
    fn content_partition_input_manifest_rejects_runtime_permission_path() {
        let mut program = content_partition_input_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        let runtime_path =
            program
                .facts
                .flow
                .ownership
                .segments
                .insert_many([psi_facts::PlaceSegment::Index {
                    expression: ExpressionHandle::invalid(),
                }]);
        program
            .facts
            .flow
            .ownership
            .permissions
            .for_each_mut(|_, event| {
                if event.source == PermissionEventSource::StateEntry {
                    event.segments = runtime_path;
                }
            });
        validate_content_partition_input_custody(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must match one live retained permission event")]
    fn content_partition_input_manifest_rejects_missing_permission_event() {
        let mut program = content_partition_input_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        program.facts.flow.ownership.permissions.clear();
        validate_content_partition_input_custody(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must match one live retained permission event")]
    fn content_partition_input_manifest_rejects_ambiguous_permission_event() {
        let mut program = content_partition_input_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        let duplicate = program
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .find(|(_, event)| event.source == PermissionEventSource::StateEntry)
            .expect("fixture entry permission")
            .1
            .clone();
        program.facts.flow.ownership.permissions.insert(duplicate);
        validate_content_partition_input_custody(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must retain one exact row per call and plan")]
    fn content_partition_input_manifest_rejects_duplicate_partition_row() {
        let mut program = content_partition_input_validation_fixture();
        let duplicate = program.facts.qualifications.content.partition_compositions[0].clone();
        program
            .facts
            .qualifications
            .content
            .partition_compositions
            .push(duplicate);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    fn content_partition_substitution_manifest_accepts_exact_closed_replay() {
        let program = content_partition_input_validation_fixture();
        let json = claim_outcome_manifest_json(&program);

        assert!(json.contains("\"substitutions\": [{\"source\": {\"version\": \"entry\""));
        assert!(json.contains("\"kind\": \"separate\""));
    }

    #[test]
    #[should_panic(expected = "source equation must retain an authored partition")]
    fn content_partition_substitution_manifest_rejects_nonpartition_source() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        let ContentConservationTerm::Separate(left) = row.source_plan.equation.left() else {
            unreachable!("fixture left is separated")
        };
        let left = left.clone();
        row.source_plan.equation =
            ContentConservationEquation::new(left[0].clone(), left[1].clone());
        validate_content_partition_substitution_replay(row);
    }

    #[test]
    #[should_panic(expected = "must retain a nonempty exact substitution map")]
    fn content_partition_substitution_manifest_rejects_empty_map() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.substitutions.clear();
        validate_content_partition_substitution_replay(row);
    }

    #[test]
    #[should_panic(expected = "substitution sources must be unique")]
    fn content_partition_substitution_manifest_rejects_duplicate_source() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.substitutions.push(row.substitutions[0].clone());
        validate_content_partition_substitution_replay(row);
    }

    #[test]
    #[should_panic(expected = "substitution targets must be unique")]
    fn content_partition_substitution_manifest_rejects_duplicate_target() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.substitutions[1].target = row.substitutions[0].target.clone();
        validate_content_partition_substitution_replay(row);
    }

    #[test]
    #[should_panic(expected = "substitution source must occur in the source equation")]
    fn content_partition_substitution_manifest_rejects_extra_source() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.substitutions.push(ContentPartitionPlaceSubstitution {
            source: ContentStructuralPlace {
                version: ContentPlaceVersion::Current,
                root: ContentPlaceRoot::Result,
                segments: vec![
                    psi_language_semantics::content::ContentPlaceSegment::FixedIndex(98),
                ],
            },
            target: ContentStructuralPlace {
                version: ContentPlaceVersion::Current,
                root: ContentPlaceRoot::Result,
                segments: vec![
                    psi_language_semantics::content::ContentPlaceSegment::FixedIndex(99),
                ],
            },
        });
        validate_content_partition_substitution_replay(row);
    }

    #[test]
    #[should_panic(expected = "must cover every source subject exactly once")]
    fn content_partition_substitution_manifest_rejects_missing_subject() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.substitutions.pop();
        validate_content_partition_substitution_replay(row);
    }

    #[test]
    #[should_panic(expected = "derived equation must equal exact substitution replay")]
    fn content_partition_substitution_manifest_rejects_target_drift() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.substitutions[0]
            .target
            .segments
            .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(9));
        validate_content_partition_substitution_replay(row);
    }

    #[test]
    #[should_panic(expected = "derived equation must equal exact substitution replay")]
    fn content_partition_substitution_manifest_rejects_projection_tuple_drift() {
        fn drift_first_projection(term: &ContentConservationTerm) -> ContentConservationTerm {
            match term {
                ContentConservationTerm::Projection {
                    domain,
                    semantic_domain,
                    projection_machine,
                    projection_fingerprint,
                    subject,
                } => ContentConservationTerm::Projection {
                    domain: SymbolHandle::from_arena_index(domain.arena_index() + 1000),
                    semantic_domain: *semantic_domain,
                    projection_machine: *projection_machine,
                    projection_fingerprint: *projection_fingerprint,
                    subject: subject.clone(),
                },
                ContentConservationTerm::Separate(terms) => {
                    let mut terms = terms.clone();
                    terms[0] = drift_first_projection(&terms[0]);
                    ContentConservationTerm::Separate(terms)
                }
            }
        }

        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.plan.equation = ContentConservationEquation::new(
            drift_first_projection(row.plan.equation.left()),
            row.plan.equation.right().clone(),
        );
        validate_content_partition_substitution_replay(row);
    }

    #[test]
    #[should_panic(expected = "replay must preserve the exact source algebra")]
    fn content_partition_substitution_manifest_rejects_algebra_drift() {
        let mut program = content_partition_input_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.plan.algebra = ContentAlgebraIdentity::CountedQuantity {
            unit: "named(name(OtherUnit))".to_owned(),
        };
        validate_content_partition_substitution_replay(row);
    }

    fn content_partition_result_rewrite_validation_fixture() -> CheckedTrees {
        fn rewrite_result_subject(
            term: &ContentConservationTerm,
            source: &ContentStructuralPlace,
            target: &ContentStructuralPlace,
        ) -> ContentConservationTerm {
            match term {
                ContentConservationTerm::Projection {
                    domain,
                    semantic_domain,
                    projection_machine,
                    projection_fingerprint,
                    subject,
                } => ContentConservationTerm::Projection {
                    domain: *domain,
                    semantic_domain: *semantic_domain,
                    projection_machine: *projection_machine,
                    projection_fingerprint: *projection_fingerprint,
                    subject: if subject == source {
                        target.clone()
                    } else {
                        subject.clone()
                    },
                },
                ContentConservationTerm::Separate(terms) => ContentConservationTerm::Separate(
                    terms
                        .iter()
                        .map(|term| rewrite_result_subject(term, source, target))
                        .collect(),
                ),
            }
        }

        let mut program = content_partition_input_validation_fixture();
        let local_symbol = SymbolHandle::from_arena_index(109);
        let (machine_symbol, state_symbol, statement_index, call_ordinal, source_callable) = {
            let row = &program.facts.qualifications.content.partition_compositions[0];
            (
                row.machine_symbol,
                row.state_symbol,
                row.statement_index,
                row.call_ordinal,
                row.source_callable,
            )
        };
        let statement_nodes = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
            .and_then(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == state_symbol)
            })
            .expect("fixture caller state")
            .statement_nodes;
        program
            .typed
            .statement_table
            .statements_mut(statement_nodes)[statement_index] =
            StatementNode::LocalData(psi_typed_trees::statement::TableLocalData {
                symbol: local_symbol,
                name: Identifier::generated("staged"),
                ..Default::default()
            });

        let result_identity = PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Call {
                statement_index,
                call_ordinal,
                target_symbol: source_callable,
            },
            ordinal: 12,
        };
        let result_provenance = PermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Call {
                statement_index,
                call_ordinal,
                target_symbol: source_callable,
            },
        };
        program
            .facts
            .flow
            .ownership
            .permissions
            .insert(FlowPermissionEventFact {
                machine_symbol,
                state_symbol,
                source: PermissionEventSource::Call {
                    statement_index,
                    call_ordinal,
                    target_symbol: source_callable,
                },
                kind: PermissionEventKind::Establish,
                access: PermissionAccess::Owned,
                claim_identity: result_identity,
                provenance: result_provenance,
                root: PlaceRoot::Symbol(local_symbol),
                obligation_live: true,
                ..Default::default()
            });

        let (source, target) = {
            let row = &mut program.facts.qualifications.content.partition_compositions[0];
            let substitution = row
                .substitutions
                .iter_mut()
                .find(|substitution| substitution.source.root == ContentPlaceRoot::Result)
                .expect("fixture result substitution");
            substitution
                .target
                .segments
                .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(2));
            let source = substitution.source.clone();
            let target = substitution.target.clone();
            row.plan.equation = ContentConservationEquation::new(
                rewrite_result_subject(row.source_plan.equation.left(), &source, &target),
                rewrite_result_subject(row.source_plan.equation.right(), &source, &target),
            );
            row.plan.fingerprint = conservation_fingerprint(&row.plan.algebra, &row.plan.equation);
            row.result_rewrites = vec![ContentPartitionResultRewrite {
                claim_identity: result_identity,
                source: source.clone(),
                target: target.clone(),
            }];
            (source, target)
        };
        let output_segments = program
            .facts
            .flow
            .ownership
            .segments
            .insert_many([psi_facts::PlaceSegment::FixedIndex { index: 2 }]);
        assert!(source.segments.is_empty());
        assert_eq!(
            target.segments,
            vec![psi_language_semantics::content::ContentPlaceSegment::FixedIndex(2)]
        );
        let old_entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .iter()
            .next()
            .expect("fixture outcome map")
            .1
            .entries;
        let mut entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .span(old_entries)
            .expect("fixture outcome entries")
            .to_vec();
        entries.push(FlowClaimOutcomeEntryFact {
            output_segments,
            source: FlowClaimOutcomeSource::Established {
                claim_identity: result_identity,
                provenance: result_provenance,
            },
        });
        let entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .insert_many(entries);
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .for_each_mut(|_, map| map.entries = entries);
        program
    }

    fn append_result_outcome_entry(program: &mut CheckedTrees, entry: FlowClaimOutcomeEntryFact) {
        let old_entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .iter()
            .next()
            .expect("fixture outcome map")
            .1
            .entries;
        let mut entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .span(old_entries)
            .expect("fixture outcome entries")
            .to_vec();
        entries.push(entry);
        let entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .insert_many(entries);
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .for_each_mut(|_, map| map.entries = entries);
    }

    #[test]
    fn content_partition_result_rewrite_manifest_accepts_exact_staged_custody() {
        let program = content_partition_result_rewrite_validation_fixture();
        let json = claim_outcome_manifest_json(&program);

        assert!(
            json.contains("\"result_rewrites\": [{\"claim_identity\": {\"kind\": \"established\"")
        );
        assert!(json.contains("\"target\": {\"version\": \"current\""));
        assert!(json.contains("\"fixed_index\": 2"));
    }

    #[test]
    fn content_partition_result_rewrite_manifest_accepts_explicit_empty() {
        let program = content_partition_input_validation_fixture();
        let json = claim_outcome_manifest_json(&program);
        assert!(json.contains("\"result_rewrites\": []"));
    }

    #[test]
    #[should_panic(expected = "must retain a non-unknown claim identity")]
    fn content_partition_result_rewrite_manifest_rejects_unknown_identity() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        program.facts.qualifications.content.partition_compositions[0].result_rewrites[0]
            .claim_identity = PermissionClaimIdentity::Unknown;
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "claim identities must be unique")]
    fn content_partition_result_rewrite_manifest_rejects_duplicate_identity() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.result_rewrites.push(row.result_rewrites[0].clone());
        let row = row.clone();
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "result rewrite sources must be unique")]
    fn content_partition_result_rewrite_manifest_rejects_duplicate_source() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        let mut duplicate = row.result_rewrites[0].clone();
        duplicate.claim_identity = PermissionClaimIdentity::Established {
            machine_symbol: row.machine_symbol,
            state_symbol: row.state_symbol,
            source: PermissionEventSource::Statement {
                statement_index: row.statement_index,
            },
            ordinal: 99,
        };
        duplicate
            .target
            .segments
            .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(3));
        row.result_rewrites.push(duplicate);
        let row = row.clone();
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "result rewrite targets must be unique")]
    fn content_partition_result_rewrite_manifest_rejects_duplicate_target() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        let mut duplicate = row.result_rewrites[0].clone();
        duplicate.claim_identity = PermissionClaimIdentity::Established {
            machine_symbol: row.machine_symbol,
            state_symbol: row.state_symbol,
            source: PermissionEventSource::Statement {
                statement_index: row.statement_index,
            },
            ordinal: 99,
        };
        duplicate
            .source
            .segments
            .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(3));
        row.result_rewrites.push(duplicate);
        let row = row.clone();
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "source must be an exact current result place")]
    fn content_partition_result_rewrite_manifest_rejects_wrong_source_root() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        program.facts.qualifications.content.partition_compositions[0].result_rewrites[0]
            .source
            .version = ContentPlaceVersion::Entry;
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "target must be an exact current result place")]
    fn content_partition_result_rewrite_manifest_rejects_wrong_target_root() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.result_rewrites[0].target.root = ContentPlaceRoot::Parameter {
            position: 0,
            symbol: SymbolHandle::from_arena_index(102),
            name: "resource".to_owned(),
            is_self: false,
        };
        let row = row.clone();
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must retain one exact substitution pair")]
    fn content_partition_result_rewrite_manifest_rejects_missing_substitution_pair() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        row.result_rewrites[0]
            .target
            .segments
            .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(9));
        let row = row.clone();
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must belong to its exact staged local")]
    fn content_partition_result_rewrite_manifest_rejects_nonlocal_statement() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        let statement_nodes = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == row.machine_symbol)
            .and_then(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == row.state_symbol)
            })
            .expect("fixture state")
            .statement_nodes;
        program
            .typed
            .statement_table
            .statements_mut(statement_nodes)[row.statement_index] = StatementNode::default();
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must match one live staged-local permission event")]
    fn content_partition_result_rewrite_manifest_rejects_wrong_local_root() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        program
            .facts
            .flow
            .ownership
            .permissions
            .for_each_mut(|_, event| {
                if event.claim_identity == row.result_rewrites[0].claim_identity {
                    event.root = PlaceRoot::Symbol(SymbolHandle::from_arena_index(999));
                }
            });
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must match one live staged-local permission event")]
    fn content_partition_result_rewrite_manifest_rejects_missing_event() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        program
            .facts
            .flow
            .ownership
            .permissions
            .for_each_mut(|_, event| {
                if event.claim_identity == row.result_rewrites[0].claim_identity {
                    event.obligation_live = false;
                }
            });
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must match one live staged-local permission event")]
    fn content_partition_result_rewrite_manifest_rejects_ambiguous_event() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        let duplicate = program
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .find(|(_, event)| event.claim_identity == row.result_rewrites[0].claim_identity)
            .expect("fixture result event")
            .1
            .clone();
        program.facts.flow.ownership.permissions.insert(duplicate);
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "result event must retain an exact valid path")]
    fn content_partition_result_rewrite_manifest_rejects_invalid_event_path() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        program
            .facts
            .flow
            .ownership
            .permissions
            .for_each_mut(|_, event| {
                if event.claim_identity == row.result_rewrites[0].claim_identity {
                    event.segments = psi_arena::HandleSpan::from_parts(
                        psi_arena::Handle::from_arena_index(999),
                        1,
                    );
                }
            });
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "paths must not retain a runtime index")]
    fn content_partition_result_rewrite_manifest_rejects_runtime_event_path() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        let runtime_path =
            program
                .facts
                .flow
                .ownership
                .segments
                .insert_many([psi_facts::PlaceSegment::Index {
                    expression: ExpressionHandle::invalid(),
                }]);
        program
            .facts
            .flow
            .ownership
            .permissions
            .for_each_mut(|_, event| {
                if event.claim_identity == row.result_rewrites[0].claim_identity {
                    event.segments = runtime_path;
                }
            });
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must name one exact checked outcome map")]
    fn content_partition_result_rewrite_manifest_rejects_missing_outcome_map() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        program.facts.flow.ownership.claim_outcome_maps.clear();
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must name exactly one checked outcome map")]
    fn content_partition_result_rewrite_manifest_rejects_ambiguous_outcome_map() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        let duplicate = program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .iter()
            .next()
            .expect("fixture outcome map")
            .1
            .clone();
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .insert(duplicate);
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "outcome map must retain an exact valid entry span")]
    fn content_partition_result_rewrite_manifest_rejects_invalid_outcome_span() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .for_each_mut(|_, map| {
                map.entries =
                    psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
            });
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must match one exact established outcome")]
    fn content_partition_result_rewrite_manifest_rejects_output_path_mismatch() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        let mismatched = program
            .facts
            .flow
            .ownership
            .segments
            .insert_many([psi_facts::PlaceSegment::FixedIndex { index: 3 }]);
        let entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .iter()
            .next()
            .expect("fixture outcome map")
            .1
            .entries;
        program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .span_mut(entries)
            .expect("fixture outcome entries")
            .iter_mut()
            .filter(|entry| {
                matches!(
                    entry.source,
                    FlowClaimOutcomeSource::Established { claim_identity, .. }
                        if claim_identity == row.result_rewrites[0].claim_identity
                )
            })
            .for_each(|entry| entry.output_segments = mismatched);
        validate_content_partition_result_rewrites(&program, &row);
    }

    #[test]
    #[should_panic(expected = "must match one exact established outcome")]
    fn content_partition_result_rewrite_manifest_rejects_ambiguous_outcome_entry() {
        let mut program = content_partition_result_rewrite_validation_fixture();
        let row = program.facts.qualifications.content.partition_compositions[0].clone();
        let entry = {
            let entries = program
                .facts
                .flow
                .ownership
                .claim_outcome_maps
                .iter()
                .next()
                .expect("fixture outcome map")
                .1
                .entries;
            program
                .facts
                .flow
                .ownership
                .claim_outcome_entries
                .span(entries)
                .expect("fixture outcome entries")
                .iter()
                .find(|entry| {
                    matches!(
                        entry.source,
                        FlowClaimOutcomeSource::Established { claim_identity, .. }
                            if claim_identity == row.result_rewrites[0].claim_identity
                    )
                })
                .expect("fixture result outcome")
                .clone()
        };
        append_result_outcome_entry(&mut program, entry);
        validate_content_partition_result_rewrites(&program, &row);
    }

    fn content_projection_validation_fixture() -> CheckedTrees {
        let domain_symbol = SymbolHandle::from_arena_index(90);
        let carrier_symbol = SymbolHandle::from_arena_index(91);
        let machine_symbol = SymbolHandle::from_arena_index(92);
        let mut program = CheckedTrees::default();
        let carrier = program
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: carrier_symbol,
                name: Identifier::generated("Resource"),
            });
        let semantic_domain = program.typed.semantic_domains.intern("Resource::Counted");
        program.typed.push_domain_definition(DomainDefinition {
            symbol: domain_symbol,
            name: Identifier::generated("Resource::Counted"),
            target_type: carrier,
            semantic_id: semantic_domain,
            ..Default::default()
        });
        let mut projection_machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated("Resource::content"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut projection_machine,
            State {
                symbol: SymbolHandle::from_arena_index(99),
                name: Identifier::generated("entry"),
                ..Default::default()
            },
        );
        program.typed.push_machine(projection_machine);
        let algebra = ContentAlgebraIdentity::CountedQuantity {
            unit: "named(name(Unit))".to_owned(),
        };
        let expression = ContentProjectionExpression::CountedQuantity {
            magnitude: ContentScalarExpression::Natural("1".to_owned()),
        };
        let fingerprint = projection_fingerprint(&algebra, &expression);
        program
            .facts
            .qualifications
            .content
            .plans
            .push(ContentProjectionPlan {
                domain: domain_symbol,
                semantic_domain,
                carrier_identity: program
                    .typed
                    .normalized_type_identity(carrier)
                    .into_string(),
                machine: machine_symbol,
                algebra,
                expression,
                fingerprint,
            });
        program
    }

    fn content_conservation_plan(
        projection: &ContentProjectionPlan,
        owner_kind: ContentConservationOwnerKind,
        owner: SymbolHandle,
        callable: SymbolHandle,
    ) -> ContentConservationPlan {
        let left = ContentConservationTerm::Projection {
            domain: projection.domain,
            semantic_domain: projection.semantic_domain,
            projection_machine: projection.machine,
            projection_fingerprint: projection.fingerprint,
            subject: ContentStructuralPlace {
                version: ContentPlaceVersion::Entry,
                root: ContentPlaceRoot::Parameter {
                    position: 0,
                    symbol: SymbolHandle::invalid(),
                    name: "resource".to_owned(),
                    is_self: false,
                },
                segments: Vec::new(),
            },
        };
        let right = ContentConservationTerm::Projection {
            domain: projection.domain,
            semantic_domain: projection.semantic_domain,
            projection_machine: projection.machine,
            projection_fingerprint: projection.fingerprint,
            subject: ContentStructuralPlace {
                version: ContentPlaceVersion::Current,
                root: ContentPlaceRoot::Result,
                segments: Vec::new(),
            },
        };
        let algebra = projection.algebra.clone();
        let equation = ContentConservationEquation::new(left, right);
        let fingerprint = conservation_fingerprint(&algebra, &equation);
        ContentConservationPlan {
            owner_kind,
            owner,
            callable,
            algebra,
            equation,
            fingerprint,
        }
    }

    fn content_conservation_validation_fixture() -> (
        CheckedTrees,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
    ) {
        let machine_symbol = SymbolHandle::from_arena_index(93);
        let state_symbol = SymbolHandle::from_arena_index(94);
        let other_machine_symbol = SymbolHandle::from_arena_index(95);
        let other_state_symbol = SymbolHandle::from_arena_index(96);
        let trait_symbol = SymbolHandle::from_arena_index(97);
        let requirement_symbol = SymbolHandle::from_arena_index(98);
        let mut program = content_projection_validation_fixture();
        for (machine, state, machine_name, state_name) in [
            (
                machine_symbol,
                state_symbol,
                "Resource::transfer",
                "transfer",
            ),
            (
                other_machine_symbol,
                other_state_symbol,
                "OtherResource::transfer",
                "transfer",
            ),
        ] {
            let mut definition = Machine {
                symbol: machine,
                name: Identifier::generated(machine_name),
                ..Default::default()
            };
            program.typed.push_machine_state(
                &mut definition,
                State {
                    symbol: state,
                    name: Identifier::generated(state_name),
                    ..Default::default()
                },
            );
            program.typed.push_machine(definition);
        }
        let mut trait_definition = TraitDefinition {
            symbol: trait_symbol,
            name: Identifier::generated("ResourceContract"),
            ..Default::default()
        };
        program.typed.push_trait_machine_signature(
            &mut trait_definition,
            StateSignature {
                symbol: requirement_symbol,
                name: Identifier::generated("transfer"),
                ..Default::default()
            },
        );
        program.typed.push_trait_definition(trait_definition);
        let projection = program.facts.qualifications.content.plans[0].clone();
        program
            .facts
            .qualifications
            .content
            .conservation_plans
            .extend([
                content_conservation_plan(
                    &projection,
                    ContentConservationOwnerKind::Machine,
                    machine_symbol,
                    state_symbol,
                ),
                content_conservation_plan(
                    &projection,
                    ContentConservationOwnerKind::TraitRequirement,
                    trait_symbol,
                    requirement_symbol,
                ),
            ]);
        (
            program,
            machine_symbol,
            state_symbol,
            other_machine_symbol,
            other_state_symbol,
        )
    }

    fn push_content_partition_row(
        program: &mut CheckedTrees,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        source_plan: ContentConservationPlan,
        plan: ContentConservationPlan,
    ) {
        program
            .facts
            .qualifications
            .content
            .partition_compositions
            .push(ContentPartitionCompositionFact {
                machine_symbol,
                state_symbol,
                source_callable: source_plan.callable,
                source_fingerprint: source_plan.fingerprint,
                source_derivation_depth: 0,
                source_plan,
                statement_index: 0,
                call_ordinal: 0,
                input_claim_identities: Vec::new(),
                input_claim_bindings: Vec::new(),
                result_rewrites: Vec::new(),
                substitutions: Vec::new(),
                plan,
            });
    }

    #[test]
    fn content_conservation_manifest_accepts_exact_machine_and_trait_custody() {
        let (program, ..) = content_conservation_validation_fixture();
        let projections = validated_content_projection_plans(&program);

        for plan in &program.facts.qualifications.content.conservation_plans {
            validate_content_conservation_plan(&program, &projections, plan);
        }
        let json = claim_outcome_manifest_json(&program);
        assert_eq!(json.matches("\"owner_kind\": \"machine\"").count(), 1);
        assert_eq!(
            json.matches("\"owner_kind\": \"trait_requirement\"")
                .count(),
            1
        );
    }

    #[test]
    #[should_panic(expected = "trait owner must name an exact typed trait definition")]
    fn content_conservation_manifest_rejects_wrong_owner_kind() {
        let (mut program, ..) = content_conservation_validation_fixture();
        program.facts.qualifications.content.conservation_plans[0].owner_kind =
            ContentConservationOwnerKind::TraitRequirement;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "machine callable must be a state owned by its exact machine")]
    fn content_conservation_manifest_rejects_cross_owner_callable() {
        let (mut program, _, _, _, other_state) = content_conservation_validation_fixture();
        program.facts.qualifications.content.conservation_plans[0].callable = other_state;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain its exact normalized fingerprint")]
    fn content_conservation_manifest_rejects_fingerprint_drift() {
        let (mut program, ..) = content_conservation_validation_fixture();
        program.facts.qualifications.content.conservation_plans[0].fingerprint ^= 1;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must join one exact retained projection plan")]
    fn content_conservation_manifest_rejects_projection_tuple_drift() {
        let (mut program, ..) = content_conservation_validation_fixture();
        let plan = &mut program.facts.qualifications.content.conservation_plans[0];
        let mut left = plan.equation.left().clone();
        let ContentConservationTerm::Projection {
            projection_fingerprint,
            ..
        } = &mut left
        else {
            unreachable!("fixture term is a projection")
        };
        *projection_fingerprint ^= 1;
        plan.equation = ContentConservationEquation::new(left, plan.equation.right().clone());
        plan.fingerprint = conservation_fingerprint(&plan.algebra, &plan.equation);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "projection term must retain the plan's exact algebra")]
    fn content_conservation_manifest_rejects_projection_algebra_drift() {
        let (mut program, ..) = content_conservation_validation_fixture();
        let plan = &mut program.facts.qualifications.content.conservation_plans[0];
        plan.algebra = ContentAlgebraIdentity::CountedQuantity {
            unit: "named(name(UnrelatedUnit))".to_owned(),
        };
        plan.fingerprint = conservation_fingerprint(&plan.algebra, &plan.equation);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(
        expected = "must retain one authored row per exact owner, callable, and algebra"
    )]
    fn content_conservation_manifest_rejects_duplicate_authored_key() {
        let (mut program, ..) = content_conservation_validation_fixture();
        let duplicate = program.facts.qualifications.content.conservation_plans[0].clone();
        program
            .facts
            .qualifications
            .content
            .conservation_plans
            .push(duplicate);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "identity reshuffle must retain its exact plan owner and callable")]
    fn content_conservation_manifest_rejects_reshuffle_outer_coordinate_drift() {
        let (mut program, _, state, other_machine, _) = content_conservation_validation_fixture();
        let plan = program.facts.qualifications.content.conservation_plans[0].clone();
        program
            .facts
            .qualifications
            .content
            .identity_reshuffles
            .push(ContentIdentityReshuffleFact {
                machine_symbol: other_machine,
                state_symbol: state,
                claim_identity: Default::default(),
                input_parameter_symbol: SymbolHandle::invalid(),
                input_segments: Default::default(),
                output_segments: Default::default(),
                plan,
            });
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain its exact derived-plan owner and callable")]
    fn content_conservation_manifest_rejects_partition_outer_coordinate_drift() {
        let (mut program, _, state, other_machine, _) = content_conservation_validation_fixture();
        let plan = program.facts.qualifications.content.conservation_plans[0].clone();
        push_content_partition_row(&mut program, other_machine, state, plan.clone(), plan);
        claim_outcome_manifest_json(&program);
    }

    #[test]
    #[should_panic(expected = "must retain its exact source-plan coordinates")]
    fn content_conservation_manifest_rejects_partition_source_coordinate_drift() {
        let (mut program, machine, state, ..) = content_conservation_validation_fixture();
        let plan = program.facts.qualifications.content.conservation_plans[0].clone();
        push_content_partition_row(&mut program, machine, state, plan.clone(), plan);
        program.facts.qualifications.content.partition_compositions[0].source_fingerprint ^= 1;
        claim_outcome_manifest_json(&program);
    }

    #[test]
    fn content_projection_manifest_accepts_exact_normalized_plan_custody() {
        let program = content_projection_validation_fixture();
        let plans = validated_content_projection_plans(&program);

        assert_eq!(
            plans,
            program
                .facts
                .qualifications
                .content
                .plans
                .iter()
                .collect::<Vec<_>>()
        );
    }

    #[test]
    #[should_panic(expected = "must name a nonempty exact declared domain")]
    fn content_projection_manifest_rejects_missing_domain() {
        let mut program = content_projection_validation_fixture();
        program.facts.qualifications.content.plans[0].domain = SymbolHandle::from_arena_index(99);
        validated_content_projection_plans(&program);
    }

    #[test]
    #[should_panic(expected = "must retain its exact registered semantic domain")]
    fn content_projection_manifest_rejects_unregistered_semantic_domain() {
        let mut program = content_projection_validation_fixture();
        program.facts.qualifications.content.plans[0].semantic_domain = SemanticDomainId(u32::MAX);
        validated_content_projection_plans(&program);
    }

    #[test]
    #[should_panic(expected = "must retain its exact normalized carrier identity")]
    fn content_projection_manifest_rejects_carrier_identity_drift() {
        let mut program = content_projection_validation_fixture();
        program.facts.qualifications.content.plans[0].carrier_identity =
            "named(name(Unrelated))".to_owned();
        validated_content_projection_plans(&program);
    }

    #[test]
    #[should_panic(expected = "must name an exact typed projection machine")]
    fn content_projection_manifest_rejects_missing_projection_machine() {
        let mut program = content_projection_validation_fixture();
        program.facts.qualifications.content.plans[0].machine = SymbolHandle::from_arena_index(99);
        validated_content_projection_plans(&program);
    }

    #[test]
    #[should_panic(expected = "must retain its exact normalized fingerprint")]
    fn content_projection_manifest_rejects_fingerprint_drift() {
        let mut program = content_projection_validation_fixture();
        program.facts.qualifications.content.plans[0].fingerprint ^= 1;
        validated_content_projection_plans(&program);
    }

    #[test]
    #[should_panic(expected = "must retain one row per exact domain")]
    fn content_projection_manifest_rejects_duplicate_domain() {
        let mut program = content_projection_validation_fixture();
        let duplicate = program.facts.qualifications.content.plans[0].clone();
        program.facts.qualifications.content.plans.push(duplicate);
        validated_content_projection_plans(&program);
    }

    #[test]
    #[should_panic(expected = "must retain one row per exact semantic domain")]
    fn content_projection_manifest_rejects_duplicate_semantic_domain() {
        let mut program = content_projection_validation_fixture();
        let first = program.facts.qualifications.content.plans[0].clone();
        let second_domain_symbol = SymbolHandle::from_arena_index(93);
        let second_carrier = program
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::from_arena_index(94),
                name: Identifier::generated("OtherResource"),
            });
        program.typed.push_domain_definition(DomainDefinition {
            symbol: second_domain_symbol,
            name: Identifier::generated("OtherResource::Counted"),
            target_type: second_carrier,
            semantic_id: first.semantic_domain,
            ..Default::default()
        });
        let mut duplicate_semantic = first;
        duplicate_semantic.domain = second_domain_symbol;
        duplicate_semantic.carrier_identity = program
            .typed
            .normalized_type_identity(second_carrier)
            .into_string();
        program
            .facts
            .qualifications
            .content
            .plans
            .push(duplicate_semantic);
        validated_content_projection_plans(&program);
    }

    #[test]
    fn claim_outcome_manifest_keeps_paths_and_source_kinds_structured() {
        let machine_symbol = SymbolHandle::from_arena_index(20);
        let state_symbol = SymbolHandle::from_arena_index(21);
        let projection_machine_symbol = SymbolHandle::from_arena_index(22);
        let projection_state_symbol = SymbolHandle::from_arena_index(23);
        let domain_symbol = SymbolHandle::from_arena_index(24);
        let carrier_symbol = SymbolHandle::from_arena_index(25);
        let parameter_symbol = SymbolHandle::from_arena_index(26);
        let local_symbol = SymbolHandle::from_arena_index(27);
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated("Region::partition"),
            ..Default::default()
        };
        let mut state = State {
            symbol: state_symbol,
            name: Identifier::generated("entry"),
            ..Default::default()
        };
        program.typed.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated("region"),
                ..Default::default()
            },
        );
        for _ in 0..4 {
            program
                .typed
                .statement_table
                .push_statement(&mut state.statement_nodes, Default::default());
        }
        program.typed.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::LocalData(psi_typed_trees::statement::TableLocalData {
                symbol: local_symbol,
                name: Identifier::generated("partitioned"),
                ..Default::default()
            }),
        );
        program.typed.push_machine_state(&mut machine, state);
        program.typed.push_machine(machine);
        let mut projection_machine = Machine {
            symbol: projection_machine_symbol,
            name: Identifier::generated("Region::content"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut projection_machine,
            State {
                symbol: projection_state_symbol,
                name: Identifier::generated("entry"),
                ..Default::default()
            },
        );
        program.typed.push_machine(projection_machine);
        let carrier = program
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: carrier_symbol,
                name: Identifier::generated("Region"),
            });
        let semantic_domain = program
            .typed
            .semantic_domains
            .intern("Region::PartitionedContent");
        program.typed.push_domain_definition(DomainDefinition {
            symbol: domain_symbol,
            name: Identifier::generated("Region::PartitionedContent"),
            target_type: carrier,
            semantic_id: semantic_domain,
            ..Default::default()
        });
        let output_segments = program.facts.flow.ownership.segments.insert_many([
            psi_facts::PlaceSegment::Case {
                variant: SymbolHandle::invalid(),
            },
            psi_facts::PlaceSegment::Field {
                symbol: SymbolHandle::invalid(),
            },
        ]);
        let input_identity = PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::StateEntry,
            ordinal: 6,
        };
        let input_provenance = PermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::StateEntry,
        };
        program
            .facts
            .flow
            .ownership
            .permissions
            .insert(FlowPermissionEventFact {
                machine_symbol,
                state_symbol,
                source: PermissionEventSource::StateEntry,
                kind: PermissionEventKind::Establish,
                access: PermissionAccess::Owned,
                claim_identity: input_identity,
                provenance: input_provenance,
                root: PlaceRoot::Symbol(parameter_symbol),
                obligation_live: true,
                ..Default::default()
            });
        let established_identity = PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Statement { statement_index: 2 },
            ordinal: 7,
        };
        let established_provenance = PermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::StateEntry,
        };
        program
            .facts
            .flow
            .ownership
            .permissions
            .insert(FlowPermissionEventFact {
                machine_symbol,
                state_symbol,
                source: PermissionEventSource::Statement { statement_index: 2 },
                kind: PermissionEventKind::Transfer,
                access: PermissionAccess::Owned,
                claim_identity: established_identity,
                provenance: established_provenance,
                root: PlaceRoot::Symbol(state_symbol),
                obligation_live: true,
                ..Default::default()
            });
        let result_identity = PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Call {
                statement_index: 4,
                call_ordinal: 2,
                target_symbol: state_symbol,
            },
            ordinal: 12,
        };
        let result_provenance = PermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Call {
                statement_index: 4,
                call_ordinal: 2,
                target_symbol: state_symbol,
            },
        };
        program
            .facts
            .flow
            .ownership
            .permissions
            .insert(FlowPermissionEventFact {
                machine_symbol,
                state_symbol,
                source: PermissionEventSource::Call {
                    statement_index: 4,
                    call_ordinal: 2,
                    target_symbol: state_symbol,
                },
                kind: PermissionEventKind::Establish,
                access: PermissionAccess::Owned,
                claim_identity: result_identity,
                provenance: result_provenance,
                root: PlaceRoot::Symbol(local_symbol),
                obligation_live: true,
                ..Default::default()
            });
        let result_output_segments = program
            .facts
            .flow
            .ownership
            .segments
            .insert_many([psi_facts::PlaceSegment::FixedIndex { index: 2 }]);
        let entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .insert_many([
                FlowClaimOutcomeEntryFact {
                    output_segments: Default::default(),
                    source: FlowClaimOutcomeSource::Input {
                        parameter_symbol,
                        segments: Default::default(),
                    },
                },
                FlowClaimOutcomeEntryFact {
                    output_segments,
                    source: FlowClaimOutcomeSource::Established {
                        claim_identity: established_identity,
                        provenance: established_provenance,
                    },
                },
                FlowClaimOutcomeEntryFact {
                    output_segments: result_output_segments,
                    source: FlowClaimOutcomeSource::Established {
                        claim_identity: result_identity,
                        provenance: result_provenance,
                    },
                },
            ]);
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .insert(FlowClaimOutcomeMapFact {
                machine_symbol,
                state_symbol,
                entries,
            });
        let projection_algebra = ContentAlgebraIdentity::CountedQuantity {
            unit: "named(name(ByteUnit))".to_owned(),
        };
        let projection_expression = ContentProjectionExpression::CountedQuantity {
            magnitude: ContentScalarExpression::Arithmetic {
                operator: ContentArithmeticOperator::Add,
                left: Box::new(ContentScalarExpression::RuntimeScalarEmbedding(vec![
                    ContentFieldSegment {
                        symbol: SymbolHandle::invalid(),
                        name: "length".to_owned(),
                    },
                ])),
                right: Box::new(ContentScalarExpression::Natural("1".to_owned())),
            },
        };
        let projection_identity =
            projection_fingerprint(&projection_algebra, &projection_expression);
        program
            .facts
            .qualifications
            .content
            .plans
            .push(ContentProjectionPlan {
                domain: domain_symbol,
                semantic_domain,
                carrier_identity: program
                    .typed
                    .normalized_type_identity(carrier)
                    .into_string(),
                machine: projection_machine_symbol,
                algebra: projection_algebra,
                expression: projection_expression,
                fingerprint: projection_identity,
            });
        let input = ContentConservationTerm::Projection {
            domain: domain_symbol,
            semantic_domain,
            projection_machine: projection_machine_symbol,
            projection_fingerprint: projection_identity,
            subject: ContentStructuralPlace {
                version: ContentPlaceVersion::Entry,
                root: ContentPlaceRoot::Parameter {
                    position: 0,
                    symbol: parameter_symbol,
                    name: "region".to_owned(),
                    is_self: false,
                },
                segments: Vec::new(),
            },
        };
        let output = ContentConservationTerm::Projection {
            domain: domain_symbol,
            semantic_domain,
            projection_machine: projection_machine_symbol,
            projection_fingerprint: projection_identity,
            subject: ContentStructuralPlace {
                version: ContentPlaceVersion::Current,
                root: ContentPlaceRoot::Result,
                segments: Vec::new(),
            },
        };
        let algebra = ContentAlgebraIdentity::CountedQuantity {
            unit: "named(name(ByteUnit))".to_owned(),
        };
        let ContentConservationTerm::Projection {
            subject: input_subject,
            ..
        } = &input
        else {
            unreachable!("fixture input is a projection")
        };
        let ContentConservationTerm::Projection {
            subject: output_subject,
            ..
        } = &output
        else {
            unreachable!("fixture output is a projection")
        };
        let substitutions = vec![
            ContentPartitionPlaceSubstitution {
                source: input_subject.clone(),
                target: input_subject.clone(),
            },
            ContentPartitionPlaceSubstitution {
                source: output_subject.clone(),
                target: ContentStructuralPlace {
                    segments: vec![
                        psi_language_semantics::content::ContentPlaceSegment::FixedIndex(2),
                    ],
                    ..output_subject.clone()
                },
            },
        ];
        let result_rewrite = ContentPartitionResultRewrite {
            claim_identity: result_identity,
            source: substitutions[1].source.clone(),
            target: substitutions[1].target.clone(),
        };
        let partition_entry_place = input_subject.clone();
        let equation = ContentConservationEquation::new(input.clone(), output.clone());
        let fingerprint = conservation_fingerprint(&algebra, &equation);
        let plan = ContentConservationPlan {
            owner_kind: ContentConservationOwnerKind::Machine,
            owner: machine_symbol,
            callable: state_symbol,
            algebra,
            equation,
            fingerprint,
        };
        let source_partition_equation = ContentConservationEquation::new(
            ContentConservationTerm::Separate(vec![input.clone(), output.clone()]),
            ContentConservationTerm::Separate(vec![input.clone(), output.clone()]),
        );
        let source_partition_plan = ContentConservationPlan {
            owner_kind: ContentConservationOwnerKind::Machine,
            owner: machine_symbol,
            callable: state_symbol,
            algebra: plan.algebra.clone(),
            fingerprint: conservation_fingerprint(&plan.algebra, &source_partition_equation),
            equation: source_partition_equation,
        };
        let derived_output = ContentConservationTerm::Projection {
            domain: domain_symbol,
            semantic_domain,
            projection_machine: projection_machine_symbol,
            projection_fingerprint: projection_identity,
            subject: substitutions[1].target.clone(),
        };
        let derived_partition_equation = ContentConservationEquation::new(
            ContentConservationTerm::Separate(vec![input.clone(), derived_output.clone()]),
            ContentConservationTerm::Separate(vec![input, derived_output]),
        );
        let derived_partition_plan = ContentConservationPlan {
            owner_kind: ContentConservationOwnerKind::Machine,
            owner: machine_symbol,
            callable: state_symbol,
            algebra: plan.algebra.clone(),
            fingerprint: conservation_fingerprint(&plan.algebra, &derived_partition_equation),
            equation: derived_partition_equation,
        };
        program
            .facts
            .qualifications
            .content
            .identity_reshuffles
            .push(ContentIdentityReshuffleFact {
                machine_symbol,
                state_symbol,
                claim_identity: input_identity,
                input_parameter_symbol: parameter_symbol,
                input_segments: Default::default(),
                output_segments: Default::default(),
                plan: plan.clone(),
            });
        program
            .facts
            .qualifications
            .content
            .conservation_plans
            .push(plan.clone());
        let calls = program.facts.flow.control.calls.insert_many([FlowCallFact {
            statement_index: 4,
            call_ordinal: 2,
            target_symbol: state_symbol,
            ..Default::default()
        }]);
        program.facts.flow.control.states.insert(FlowStateFact {
            machine_symbol,
            state_symbol,
            calls,
            ..Default::default()
        });
        program
            .facts
            .qualifications
            .content
            .partition_compositions
            .push(ContentPartitionCompositionFact {
                machine_symbol,
                state_symbol,
                source_callable: state_symbol,
                source_fingerprint: source_partition_plan.fingerprint,
                source_derivation_depth: 0,
                source_plan: source_partition_plan,
                statement_index: 4,
                call_ordinal: 2,
                input_claim_identities: vec![input_identity],
                input_claim_bindings: vec![psi_checked_trees::ContentPartitionInputClaimBinding {
                    claim_identity: input_identity,
                    entry_place: partition_entry_place,
                }],
                result_rewrites: vec![result_rewrite],
                substitutions,
                plan: derived_partition_plan,
            });

        let json = claim_outcome_manifest_json(&program);
        let claim_maps = &json[..json
            .find("\"content_projections\"")
            .expect("content projection section")];
        let projections_start = claim_maps.len();
        let projections_end = json
            .find("\"content_identity_reshuffles\"")
            .expect("identity reshuffle section");
        let projections = &json[projections_start..projections_end];
        let reshuffles_start = json
            .find("\"content_identity_reshuffles\"")
            .expect("identity reshuffle section");
        let reshuffles_end = json
            .find("\"content_partition_compositions\"")
            .expect("partition composition section");
        let reshuffles = &json[reshuffles_start..reshuffles_end];
        let compositions_start = reshuffles_end;
        let compositions_end = json
            .find("\"content_conservation\"")
            .expect("content conservation section");
        let compositions = &json[compositions_start..compositions_end];
        let conservation = &json[compositions_end..];

        assert!(json.contains("\"claim_outcome_maps\""));
        assert!(
            claim_maps.contains(
                "\"machine_overload_identity\": \"named-callable(path(Region::partition)"
            )
        );
        assert!(
            json.contains("\"output_path\": [{\"case\": \"invalid\"}, {\"field\": \"invalid\"}]")
        );
        assert!(json.contains("\"kind\": \"input\""));
        assert!(json.contains("\"kind\": \"established\""));
        assert!(json.contains("\"statement_index\": 2"));
        assert!(json.contains("\"ordinal\": 7"));
        assert!(json.contains("\"kind\": \"state_entry\""));
        assert!(json.contains("\"content_projections\""));
        assert!(projections.contains(
            "\"projection_machine_overload_identity\": \"named-callable(path(Region::content)"
        ));
        assert!(json.contains("\"content_identity_reshuffles\": [\n    {"));
        assert!(
            reshuffles.contains(
                "\"machine_overload_identity\": \"named-callable(path(Region::partition)"
            )
        );
        assert!(json.contains("\"content_partition_compositions\": [\n    {"));
        assert!(
            compositions.contains(
                "\"machine_overload_identity\": \"named-callable(path(Region::partition)"
            )
        );
        assert!(compositions.contains(
            "\"source_callable_overload_identity\": \"named-callable(path(Region::partition)"
        ));
        assert!(json.contains("\"source_derivation_depth\": 0"));
        assert!(json.contains("\"source_equation\": {\"left\":"));
        assert!(json.contains("\"substitutions\": [{\"source\": {\"version\": \"entry\""));
        assert!(json.contains("\"call\": {\"statement_index\": 4, \"call_ordinal\": 2}"));
        assert!(json.contains("\"input_claim_identities\": [{\"kind\": \"established\""));
        assert!(json.contains(
            "\"input_claim_bindings\": [{\"claim_identity\": {\"kind\": \"established\""
        ));
        assert!(json.contains("\"entry_place\": {\"version\": \"entry\""));
        assert!(
            json.contains("\"result_rewrites\": [{\"claim_identity\": {\"kind\": \"established\"")
        );
        assert!(json.contains("\"source\": {\"version\": \"current\""));
        assert!(json.contains("\"target\": {\"version\": \"current\""));
        assert!(json.contains("\"ordinal\": 12"));
        assert!(json.contains(&format!(
            "\"input\": {{\"parameter\": \"{}\", \"path\": []}}",
            symbol_label(&program, parameter_symbol)
        )));
        assert!(json.contains("\"ordinal\": 6"));
        assert!(json.contains(&format!("\"semantic_domain_id\": {}", semantic_domain.0)));
        assert!(json.contains("\"kind\": \"counted_quantity\""));
        assert!(json.contains("\"unit\": \"named(name(ByteUnit))\""));
        assert!(json.contains("\"kind\": \"runtime_scalar_embedding\""));
        assert!(json.contains("\"path\": [\"length\"]"));
        assert!(json.contains("\"operator\": \"add\""));
        assert!(json.contains(&format!(
            "\"fingerprint\": \"0x{projection_identity:016x}\""
        )));
        assert!(
            conservation.contains(
                "\"callable_overload_identity\": \"named-callable(path(Region::partition)"
            )
        );
    }

    fn semantic_domain_commitment_fixture() -> (
        CheckedTrees,
        SymbolHandle,
        SymbolHandle,
        SemanticDomainId,
        SemanticDomainId,
    ) {
        let first_machine = SymbolHandle::from_arena_index(80);
        let second_machine = SymbolHandle::from_arena_index(81);
        let mut program = CheckedTrees::default();
        for (symbol, state_symbol, name) in [
            (
                first_machine,
                SymbolHandle::from_arena_index(82),
                "DistanceWorker::run",
            ),
            (
                second_machine,
                SymbolHandle::from_arena_index(83),
                "AuditWorker::run",
            ),
        ] {
            let mut machine = Machine {
                symbol,
                name: Identifier::generated(name),
                ..Default::default()
            };
            program.typed.push_machine_state(
                &mut machine,
                State {
                    symbol: state_symbol,
                    name: Identifier::generated("entry"),
                    ..Default::default()
                },
            );
            program.typed.push_machine(machine);
        }
        let distance = program.typed.semantic_domains.intern("i64::Distance<1000>");
        let wrapping = program.typed.semantic_domains.intern("i64::Wrapping");
        (program, first_machine, second_machine, distance, wrapping)
    }

    #[test]
    fn qualification_manifest_publishes_ordered_exact_machine_domain_commitments() {
        let (mut program, first_machine, second_machine, distance, wrapping) =
            semantic_domain_commitment_fixture();
        program
            .facts
            .qualifications
            .machines
            .push(MachineQualifications {
                machine: first_machine,
                body_committed: vec![distance, wrapping],
            });
        program
            .facts
            .qualifications
            .machines
            .push(MachineQualifications {
                machine: second_machine,
                body_committed: vec![distance],
            });

        let json = qualification_evidence_manifest_json(
            &program,
            &omega_effects::SelectedProviderPlanFacts::default(),
        );

        let commitments = json
            .split_once("\"machine_semantic_domain_commitments\": [")
            .expect("qualification artifact publishes implementation commitments")
            .1
            .split_once("\"vacuous_qualification_uses\": [")
            .expect("commitments remain independent from vacuous evidence")
            .0;
        assert!(commitments.contains("\"machine\": \"#80\""));
        assert!(commitments.contains("\"machine\": \"#81\""));
        assert!(commitments.contains("\"machine_overload_identity\":"));
        assert_eq!(
            commitments
                .matches(&format!("\"semantic_domain_id\": {}", distance.0))
                .count(),
            2,
            "the same normalized domain may be committed independently by two machines"
        );
        let first_distance = commitments
            .find(&format!("\"semantic_domain_id\": {}", distance.0))
            .expect("first ordered domain");
        let first_wrapping = commitments
            .find(&format!("\"semantic_domain_id\": {}", wrapping.0))
            .expect("second ordered domain");
        assert!(first_distance < first_wrapping);
        assert!(commitments.contains("\"semantic_domain\": \"i64::Distance<1000>\""));
        assert!(commitments.contains("\"semantic_domain\": \"i64::Wrapping\""));
    }

    #[test]
    #[should_panic(expected = "must name an exact owning machine")]
    fn qualification_manifest_rejects_missing_commitment_machine() {
        let (mut program, _, _, distance, _) = semantic_domain_commitment_fixture();
        program
            .facts
            .qualifications
            .machines
            .push(MachineQualifications {
                machine: SymbolHandle::from_arena_index(99),
                body_committed: vec![distance],
            });
        validated_machine_semantic_domain_commitments(&program);
    }

    #[test]
    #[should_panic(expected = "one row per exact machine")]
    fn qualification_manifest_rejects_duplicate_commitment_machine() {
        let (mut program, machine, _, distance, wrapping) = semantic_domain_commitment_fixture();
        for domain in [distance, wrapping] {
            program
                .facts
                .qualifications
                .machines
                .push(MachineQualifications {
                    machine,
                    body_committed: vec![domain],
                });
        }
        validated_machine_semantic_domain_commitments(&program);
    }

    #[test]
    #[should_panic(expected = "must retain at least one domain")]
    fn qualification_manifest_rejects_empty_commitment_domains() {
        let (mut program, machine, _, _, _) = semantic_domain_commitment_fixture();
        program
            .facts
            .qualifications
            .machines
            .push(MachineQualifications {
                machine,
                body_committed: Vec::new(),
            });
        validated_machine_semantic_domain_commitments(&program);
    }

    #[test]
    #[should_panic(expected = "must be strictly increasing")]
    fn qualification_manifest_rejects_duplicate_commitment_domains() {
        let (mut program, machine, _, distance, _) = semantic_domain_commitment_fixture();
        program
            .facts
            .qualifications
            .machines
            .push(MachineQualifications {
                machine,
                body_committed: vec![distance, distance],
            });
        validated_machine_semantic_domain_commitments(&program);
    }

    #[test]
    #[should_panic(expected = "must be strictly increasing")]
    fn qualification_manifest_rejects_out_of_order_commitment_domains() {
        let (mut program, machine, _, distance, wrapping) = semantic_domain_commitment_fixture();
        let (higher, lower) = if distance.0 > wrapping.0 {
            (distance, wrapping)
        } else {
            (wrapping, distance)
        };
        program
            .facts
            .qualifications
            .machines
            .push(MachineQualifications {
                machine,
                body_committed: vec![higher, lower],
            });
        validated_machine_semantic_domain_commitments(&program);
    }

    #[test]
    #[should_panic(expected = "must name a registered domain")]
    fn qualification_manifest_rejects_unknown_commitment_domain() {
        let (mut program, machine, _, _, _) = semantic_domain_commitment_fixture();
        program
            .facts
            .qualifications
            .machines
            .push(MachineQualifications {
                machine,
                body_committed: vec![SemanticDomainId(u32::MAX)],
            });
        validated_machine_semantic_domain_commitments(&program);
    }

    #[test]
    fn qualification_evidence_manifest_separates_origin_point_and_receipt() {
        let subject = SymbolHandle::from_arena_index(4);
        let domain = SymbolHandle::from_arena_index(5);
        let machine_symbol = SymbolHandle::from_arena_index(80);
        let state_symbol = SymbolHandle::from_arena_index(81);
        let plan = selected_storage_plan();
        assert_ne!(plan.schema.trait_name, "StorageBase");
        let receipt_identity = plan.identity_fingerprint();
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&plan),
            std::slice::from_ref(&plan.name),
        )
        .expect("complete selected provider plan");
        let mut program = CheckedTrees::default();
        program.typed.push_domain_definition(DomainDefinition {
            symbol: domain,
            name: Identifier::generated("Storage::Qualified"),
            ..Default::default()
        });
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated("StorageCaller::run"),
            ..Default::default()
        };
        let mut state = State {
            symbol: state_symbol,
            name: Identifier::generated("run"),
            ..Default::default()
        };
        for _ in 0..3 {
            program
                .typed
                .statement_table
                .push_statement(&mut state.statement_nodes, Default::default());
        }
        program.typed.push_machine_state(&mut machine, state);
        program.typed.push_machine(machine);
        let mut calls = Default::default();
        program.facts.flow.control.calls.append_to_span(
            &mut calls,
            FlowCallFact {
                statement_index: 2,
                call_ordinal: 1,
                ..Default::default()
            },
        );
        program.facts.flow.control.states.append(FlowStateFact {
            machine_symbol,
            state_symbol,
            calls,
            ..Default::default()
        });
        let (requirement_owner, requirement) =
            push_qualification_requirement(&mut program, true, 70, 71, "StorageBase");
        let place = program.facts.semantic.append_symbol_place(subject);
        program.facts.semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point: ProgramPoint::CallEnsures {
                machine_symbol,
                state_symbol,
                statement_index: 2,
                call_ordinal: 1,
            },
            origin: FactOrigin::CallEnsures,
            evidence: QualificationEvidence {
                origin: QualificationEvidenceOrigin::AdmittedReceipt,
                source_symbol: requirement_owner,
                requirement_symbol: requirement,
                receipt_identity,
            },
            payload: FactPayload::DomainMembership {
                value: Default::default(),
                domain: Default::default(),
                domain_symbol: domain,
            },
        });
        let unstamped_subject = SymbolHandle::from_arena_index(7);
        let unstamped_place = program
            .facts
            .semantic
            .append_symbol_place(unstamped_subject);
        program.facts.semantic.append_fact(Fact {
            place: FactPlace::Place(unstamped_place),
            point: ProgramPoint::Global,
            origin: FactOrigin::CallEnsures,
            evidence: QualificationEvidence {
                origin: QualificationEvidenceOrigin::AdmittedReceipt,
                source_symbol: requirement_owner,
                requirement_symbol: requirement,
                receipt_identity: 0,
            },
            payload: FactPayload::DomainMembership {
                value: Default::default(),
                domain: Default::default(),
                domain_symbol: domain,
            },
        });

        let json = qualification_evidence_manifest_json(&program, &selected);

        assert!(json.contains(&format!(
            "\"selected_provider_closure_fingerprint\": \"0x{:016x}\"",
            selected.normalized_identity()
        )));
        assert!(json.contains("\"subject\": \"#4\""));
        assert!(json.contains("\"domain\": \"#5\""));
        assert!(json.contains("\"origin\": \"admitted_receipt\""));
        assert!(json.contains("\"program_point\": \"call_ensures\""));
        assert!(json.contains("\"program_point_identity\": \"#81:call-ensures-2-1\""));
        assert!(json.contains("\"program_point_identity\": \"global\""));
        assert!(json.contains("\"source\": \"#70\""));
        assert!(json.contains("\"requirement\": \"#71\""));
        assert!(
            json.contains("\"requirement_identity\": \"named-callable(path(StorageBase::transfer)")
        );
        assert!(json.contains(&format!(
            "\"receipt_identity\": \"0x{receipt_identity:016x}\""
        )));
        assert!(json.contains("\"receipt_identity\": null"));
    }

    #[test]
    #[should_panic(expected = "qualification evidence must name an exact declared domain")]
    fn qualification_manifest_rejects_missing_declared_domain() {
        let subject = SymbolHandle::from_arena_index(4);
        let domain = SymbolHandle::from_arena_index(5);
        let mut program = CheckedTrees::default();
        let place = program.facts.semantic.append_symbol_place(subject);
        program.facts.semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point: ProgramPoint::Global,
            origin: FactOrigin::MachineFieldDomain {
                machine_symbol: subject,
            },
            evidence: QualificationEvidence::from_origin(
                QualificationEvidenceOrigin::CheckedValidation,
                subject,
            ),
            payload: FactPayload::DomainMembership {
                value: Default::default(),
                domain: Default::default(),
                domain_symbol: domain,
            },
        });

        qualification_evidence_manifest_json(
            &program,
            &omega_effects::SelectedProviderPlanFacts::default(),
        );
    }

    #[test]
    #[should_panic(expected = "must retain a semantic subject position")]
    fn qualification_manifest_rejects_unknown_subject() {
        qualification_subject(
            &CheckedTrees::default(),
            &Fact {
                place: FactPlace::Unknown,
                ..Default::default()
            },
        );
    }

    #[test]
    #[should_panic(expected = "must retain a semantic subject position")]
    fn qualification_manifest_rejects_unknown_place_root() {
        let mut program = CheckedTrees::default();
        let place = program.facts.semantic.append_place(Place {
            root: PlaceRoot::Unknown,
            segments: Default::default(),
        });
        qualification_subject(
            &program,
            &Fact {
                place: FactPlace::Place(place),
                ..Default::default()
            },
        );
    }

    #[test]
    #[should_panic(expected = "program point must name an exact typed machine")]
    fn qualification_manifest_rejects_missing_program_point_machine() {
        validate_qualification_program_point(
            &CheckedTrees::default(),
            ProgramPoint::Machine {
                machine_symbol: SymbolHandle::from_arena_index(80),
            },
        );
    }

    #[test]
    #[should_panic(expected = "program point state must belong to its exact typed machine")]
    fn qualification_manifest_rejects_cross_machine_program_point_state() {
        let machine_symbol = SymbolHandle::from_arena_index(80);
        let other_machine_symbol = SymbolHandle::from_arena_index(82);
        let other_state_symbol = SymbolHandle::from_arena_index(83);
        let mut program = CheckedTrees::default();
        for (symbol, state) in [
            (machine_symbol, SymbolHandle::from_arena_index(81)),
            (other_machine_symbol, other_state_symbol),
        ] {
            let mut machine = Machine {
                symbol,
                name: Identifier::generated("Worker::run"),
                ..Default::default()
            };
            program.typed.push_machine_state(
                &mut machine,
                State {
                    symbol: state,
                    name: Identifier::generated("run"),
                    ..Default::default()
                },
            );
            program.typed.push_machine(machine);
        }

        validate_qualification_program_point(
            &program,
            ProgramPoint::State {
                machine_symbol,
                state_symbol: other_state_symbol,
            },
        );
    }

    #[test]
    #[should_panic(expected = "statement index must be within its exact typed state")]
    fn qualification_manifest_rejects_out_of_range_program_point_statement() {
        let machine_symbol = SymbolHandle::from_arena_index(80);
        let state_symbol = SymbolHandle::from_arena_index(81);
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated("Worker::run"),
            ..Default::default()
        };
        let mut state = State {
            symbol: state_symbol,
            name: Identifier::generated("run"),
            ..Default::default()
        };
        program
            .typed
            .statement_table
            .push_statement(&mut state.statement_nodes, Default::default());
        program.typed.push_machine_state(&mut machine, state);
        program.typed.push_machine(machine);

        validate_qualification_program_point(
            &program,
            ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index: 1,
            },
        );
    }

    #[test]
    #[should_panic(expected = "call point must name an exact checked flow state")]
    fn qualification_manifest_rejects_call_without_flow_state() {
        let machine_symbol = SymbolHandle::from_arena_index(80);
        let state_symbol = SymbolHandle::from_arena_index(81);
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated("Worker::run"),
            ..Default::default()
        };
        let mut state = State {
            symbol: state_symbol,
            name: Identifier::generated("run"),
            ..Default::default()
        };
        program
            .typed
            .statement_table
            .push_statement(&mut state.statement_nodes, Default::default());
        program.typed.push_machine_state(&mut machine, state);
        program.typed.push_machine(machine);

        validate_qualification_program_point(
            &program,
            ProgramPoint::Call {
                machine_symbol,
                state_symbol,
                statement_index: 0,
                call_ordinal: 0,
            },
        );
    }

    #[test]
    #[should_panic(expected = "call point must name an exact owned checked flow call")]
    fn qualification_manifest_rejects_wrong_call_ordinal() {
        let machine_symbol = SymbolHandle::from_arena_index(80);
        let state_symbol = SymbolHandle::from_arena_index(81);
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated("Worker::run"),
            ..Default::default()
        };
        let mut state = State {
            symbol: state_symbol,
            name: Identifier::generated("run"),
            ..Default::default()
        };
        program
            .typed
            .statement_table
            .push_statement(&mut state.statement_nodes, Default::default());
        program.typed.push_machine_state(&mut machine, state);
        program.typed.push_machine(machine);
        let mut calls = Default::default();
        program.facts.flow.control.calls.append_to_span(
            &mut calls,
            FlowCallFact {
                statement_index: 0,
                call_ordinal: 1,
                ..Default::default()
            },
        );
        program.facts.flow.control.states.append(FlowStateFact {
            machine_symbol,
            state_symbol,
            calls,
            ..Default::default()
        });

        validate_qualification_program_point(
            &program,
            ProgramPoint::CallEnsures {
                machine_symbol,
                state_symbol,
                statement_index: 0,
                call_ordinal: 2,
            },
        );
    }

    #[test]
    fn qualification_manifest_accepts_exact_independent_semantic_sources() {
        let machine_symbol = SymbolHandle::from_arena_index(80);
        let state_symbol = SymbolHandle::from_arena_index(81);
        let root_operator_symbol = SymbolHandle::from_arena_index(82);
        let domain_symbol = SymbolHandle::from_arena_index(83);
        let domain_operator_symbol = SymbolHandle::from_arena_index(84);
        let trait_symbol = SymbolHandle::from_arena_index(85);
        let parameter_symbol = SymbolHandle::from_arena_index(86);
        let parameter_signature_symbol = SymbolHandle::from_arena_index(87);
        let mut program = CheckedTrees::default();

        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated("Worker::run"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                name: Identifier::generated("run"),
                ..Default::default()
            },
        );
        program.typed.push_machine_type_parameter(
            &mut machine,
            TypeParameter {
                symbol: parameter_symbol,
                name: Identifier::generated("Dependency"),
                kind: TypeParameterKind::Machine {
                    contract: StateSignature {
                        symbol: parameter_signature_symbol,
                        name: Identifier::generated("invoke"),
                        ..Default::default()
                    },
                },
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
        program.typed.push_operator(OperatorDefinition {
            symbol: root_operator_symbol,
            ..Default::default()
        });
        let mut domain = DomainDefinition {
            symbol: domain_symbol,
            name: Identifier::generated("Validated"),
            ..Default::default()
        };
        program.typed.push_domain_operator(
            &mut domain,
            OperatorDefinition {
                symbol: domain_operator_symbol,
                ..Default::default()
            },
        );
        program.typed.push_domain_definition(domain);
        program.typed.push_trait_definition(TraitDefinition {
            symbol: trait_symbol,
            name: Identifier::generated("Transform"),
            ..Default::default()
        });

        for (source_symbol, origin) in [
            (
                machine_symbol,
                QualificationEvidenceOrigin::CheckedValidation,
            ),
            (state_symbol, QualificationEvidenceOrigin::Prover),
            (
                root_operator_symbol,
                QualificationEvidenceOrigin::CheckedTransformation,
            ),
            (
                domain_operator_symbol,
                QualificationEvidenceOrigin::Propagated,
            ),
            (
                trait_symbol,
                QualificationEvidenceOrigin::AuthorizedRouteEstablishment,
            ),
            (
                parameter_symbol,
                QualificationEvidenceOrigin::VacuousQualification,
            ),
            (
                parameter_signature_symbol,
                QualificationEvidenceOrigin::Prover,
            ),
        ] {
            validate_qualification_source(
                &program,
                &QualificationEvidence::from_origin(origin, source_symbol),
            );
        }
    }

    #[test]
    fn qualification_manifest_keeps_admitted_source_on_requirement_pair_rule() {
        let mut program = CheckedTrees::default();
        let (requirement_owner, requirement) =
            push_qualification_requirement(&mut program, true, 70, 71, "StorageBase");

        validate_qualification_source(
            &program,
            &QualificationEvidence {
                origin: QualificationEvidenceOrigin::AdmittedReceipt,
                source_symbol: requirement_owner,
                requirement_symbol: requirement,
                ..Default::default()
            },
        );
    }

    #[test]
    #[should_panic(expected = "must retain a nonempty exact source symbol")]
    fn qualification_manifest_rejects_empty_non_admitted_source() {
        validate_qualification_source(
            &CheckedTrees::default(),
            &QualificationEvidence {
                origin: QualificationEvidenceOrigin::Prover,
                ..Default::default()
            },
        );
    }

    #[test]
    #[should_panic(expected = "must resolve to exactly one retained typed semantic declaration")]
    fn qualification_manifest_rejects_absent_non_admitted_source() {
        validate_qualification_source(
            &CheckedTrees::default(),
            &QualificationEvidence::from_origin(
                QualificationEvidenceOrigin::Prover,
                SymbolHandle::from_arena_index(80),
            ),
        );
    }

    #[test]
    #[should_panic(expected = "must resolve to exactly one retained typed semantic declaration")]
    fn qualification_manifest_rejects_ambiguous_non_admitted_source() {
        let source = SymbolHandle::from_arena_index(80);
        let mut program = CheckedTrees::default();
        program.typed.push_machine(Machine {
            symbol: source,
            name: Identifier::generated("Worker::run"),
            ..Default::default()
        });
        program.typed.push_trait_definition(TraitDefinition {
            symbol: source,
            name: Identifier::generated("Worker"),
            ..Default::default()
        });

        validate_qualification_source(
            &program,
            &QualificationEvidence::from_origin(QualificationEvidenceOrigin::Prover, source),
        );
    }

    #[test]
    #[should_panic(expected = "must name an exact retained selected provider plan")]
    fn qualification_manifest_rejects_unselected_nonzero_receipt() {
        validate_qualification_receipt(
            &omega_effects::SelectedProviderPlanFacts::default(),
            QualificationEvidenceOrigin::AdmittedReceipt,
            selected_storage_plan().identity_fingerprint(),
        );
    }

    #[test]
    #[should_panic(expected = "must use admitted-receipt origin")]
    fn qualification_manifest_rejects_selected_receipt_on_non_admitted_origin() {
        let plan = selected_storage_plan();
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&plan),
            std::slice::from_ref(&plan.name),
        )
        .expect("complete selected provider plan");
        validate_qualification_receipt(
            &selected,
            QualificationEvidenceOrigin::Prover,
            plan.identity_fingerprint(),
        );
    }

    #[test]
    #[should_panic(expected = "must name an exact boundary requirement")]
    fn qualification_manifest_rejects_admitted_evidence_without_requirement() {
        qualification_requirement_identity(
            &CheckedTrees::default(),
            &QualificationEvidence {
                origin: QualificationEvidenceOrigin::AdmittedReceipt,
                ..Default::default()
            },
        );
    }

    #[test]
    #[should_panic(expected = "must name an exact boundary requirement")]
    fn qualification_manifest_rejects_admitted_ordinary_trait_requirement() {
        let mut program = CheckedTrees::default();
        let (requirement_owner, requirement) =
            push_qualification_requirement(&mut program, false, 70, 71, "StorageBase");
        qualification_requirement_identity(
            &program,
            &QualificationEvidence {
                origin: QualificationEvidenceOrigin::AdmittedReceipt,
                source_symbol: requirement_owner,
                requirement_symbol: requirement,
                ..Default::default()
            },
        );
    }

    #[test]
    #[should_panic(expected = "must name an exact boundary requirement owner/signature pair")]
    fn qualification_manifest_rejects_admitted_requirement_without_owner() {
        let mut program = CheckedTrees::default();
        let (_, requirement) =
            push_qualification_requirement(&mut program, true, 70, 71, "StorageBase");
        qualification_requirement_identity(
            &program,
            &QualificationEvidence {
                origin: QualificationEvidenceOrigin::AdmittedReceipt,
                requirement_symbol: requirement,
                ..Default::default()
            },
        );
    }

    #[test]
    #[should_panic(expected = "must name an exact boundary requirement owner/signature pair")]
    fn qualification_manifest_rejects_cross_owner_requirement() {
        let mut program = CheckedTrees::default();
        let (_, requirement) =
            push_qualification_requirement(&mut program, true, 70, 71, "StorageBase");
        let (unrelated_owner, _) =
            push_qualification_requirement(&mut program, true, 72, 73, "AuditBase");
        qualification_requirement_identity(
            &program,
            &QualificationEvidence {
                origin: QualificationEvidenceOrigin::AdmittedReceipt,
                source_symbol: unrelated_owner,
                requirement_symbol: requirement,
                ..Default::default()
            },
        );
    }

    #[test]
    #[should_panic(expected = "non-admitted qualification evidence must not name")]
    fn qualification_manifest_rejects_requirement_on_non_admitted_evidence() {
        let mut program = CheckedTrees::default();
        let (_, requirement) =
            push_qualification_requirement(&mut program, true, 70, 71, "StorageBase");
        qualification_requirement_identity(
            &program,
            &QualificationEvidence {
                origin: QualificationEvidenceOrigin::Prover,
                requirement_symbol: requirement,
                ..Default::default()
            },
        );
    }

    #[test]
    fn qualification_manifest_retains_provider_origin_outside_plan_identity() {
        let plan = selected_storage_plan();
        let plan_identity = plan.identity_fingerprint();
        let mut relocated = plan.clone();
        relocated.origin_package = "omega::providers::relocated".to_owned();
        assert_eq!(
            relocated.identity_fingerprint(),
            plan_identity,
            "provider origin is provenance beside, not part of, plan identity"
        );
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&plan),
            std::slice::from_ref(&plan.name),
        )
        .expect("complete selected provider plan");
        let selected_closure_identity = selected.normalized_identity();

        let json = qualification_evidence_manifest_json(&CheckedTrees::default(), &selected);
        assert!(json.contains(&format!(
            "\"selected_provider_closure_fingerprint\": \"0x{selected_closure_identity:016x}\""
        )));
        assert_eq!(json.matches("\"provider_origin_package\"").count(), 2);
        assert_eq!(
            json.matches("\"provider_origin_package\": \"omega::providers::storage\"")
                .count(),
            2
        );
        assert!(json.contains("\"flow\": \"accepts\""));
        assert!(json.contains("\"flow\": \"returns\""));
        assert_eq!(json.matches("\"boundary\": \"StorageRoot\"").count(), 2);
        assert_eq!(
            json.matches("\"requirement\": \"StorageBase::transfer\"")
                .count(),
            2
        );
        assert_eq!(
            json.matches("\"requirement_owner\": \"StorageBase\"")
                .count(),
            2
        );
        assert_eq!(
            json.matches("\"requirement_identity\": \"StorageBase::transfer\"")
                .count(),
            2
        );
        assert!(json.contains(&format!("\"receipt_identity\": \"0x{plan_identity:016x}\"")));

        let mut absent = plan.clone();
        absent.origin_package.clear();
        let selected_absent = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&absent),
            std::slice::from_ref(&absent.name),
        )
        .expect("selected provider with explicitly absent origin");
        let absent_json =
            qualification_evidence_manifest_json(&CheckedTrees::default(), &selected_absent);
        assert_eq!(
            absent_json
                .matches("\"provider_origin_package\": null")
                .count(),
            2
        );
    }

    #[test]
    fn qualification_manifest_retains_vacuous_use_owner_overload_identity() {
        let (
            mut program,
            machine_symbol,
            state_symbol,
            _,
            domain_symbol,
            semantic_domain,
            cast_expression,
            _,
        ) = vacuous_qualification_fixture();
        assert_ne!(
            program
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == domain_symbol)
                .expect("declared domain")
                .semantic_id,
            semantic_domain,
            "the declared family and selected indexed instance remain independent",
        );
        program
            .facts
            .qualifications
            .vacuous_uses
            .push(VacuousQualificationUse {
                machine: machine_symbol,
                state: state_symbol,
                statement_index: 3,
                expression: cast_expression,
                domain: domain_symbol,
                semantic_domain,
            });

        let json = qualification_evidence_manifest_json(
            &program,
            &omega_effects::SelectedProviderPlanFacts::default(),
        );

        assert!(json.contains("\"machine\": \"#60\""));
        assert!(json.contains("\"machine_overload_identity\": \"named-callable(path(Main::main)"));
        assert!(json.contains("\"statement_index\": 3"));
        assert!(json.contains(&format!("\"semantic_domain_id\": {}", semantic_domain.0)));
        assert!(json.contains("\"semantic_domain\": \"i64::Distance<1000>\""));
    }

    #[test]
    #[should_panic(expected = "statement index must be within its exact state")]
    fn qualification_manifest_rejects_out_of_range_vacuous_statement() {
        let (program, machine, state, _, domain, semantic_domain, cast_expression, _) =
            vacuous_qualification_fixture();
        validate_vacuous_qualification_use(
            &program,
            &VacuousQualificationUse {
                machine,
                state,
                statement_index: 4,
                expression: cast_expression,
                domain,
                semantic_domain,
            },
        );
    }

    #[test]
    #[should_panic(expected = "must name a valid retained expression")]
    fn qualification_manifest_rejects_invalid_vacuous_expression() {
        let (program, machine, state, _, domain, semantic_domain, _, _) =
            vacuous_qualification_fixture();
        validate_vacuous_qualification_use(
            &program,
            &VacuousQualificationUse {
                machine,
                state,
                statement_index: 3,
                expression: ExpressionHandle::invalid(),
                domain,
                semantic_domain,
            },
        );
    }

    #[test]
    #[should_panic(expected = "must name its exact retained cast")]
    fn qualification_manifest_rejects_reachable_non_cast_vacuous_expression() {
        let (program, machine, state, _, domain, semantic_domain, _, statement_expression) =
            vacuous_qualification_fixture();
        validate_vacuous_qualification_use(
            &program,
            &VacuousQualificationUse {
                machine,
                state,
                statement_index: 3,
                expression: statement_expression,
                domain,
                semantic_domain,
            },
        );
    }

    #[test]
    #[should_panic(expected = "cast must belong to its exact statement")]
    fn qualification_manifest_rejects_cross_statement_vacuous_cast() {
        let (program, machine, state, _, domain, semantic_domain, cast_expression, _) =
            vacuous_qualification_fixture();
        validate_vacuous_qualification_use(
            &program,
            &VacuousQualificationUse {
                machine,
                state,
                statement_index: 2,
                expression: cast_expression,
                domain,
                semantic_domain,
            },
        );
    }

    #[test]
    #[should_panic(expected = "state must belong to its exact owning machine")]
    fn qualification_manifest_rejects_cross_machine_vacuous_state() {
        let (program, machine, _, other_state, domain, semantic_domain, cast_expression, _) =
            vacuous_qualification_fixture();
        validate_vacuous_qualification_use(
            &program,
            &VacuousQualificationUse {
                machine,
                state: other_state,
                statement_index: 0,
                expression: cast_expression,
                domain,
                semantic_domain,
            },
        );
    }

    #[test]
    #[should_panic(expected = "must name an exact declared domain")]
    fn qualification_manifest_rejects_missing_vacuous_domain() {
        let (program, machine, state, _, _, semantic_domain, cast_expression, _) =
            vacuous_qualification_fixture();
        validate_vacuous_qualification_use(
            &program,
            &VacuousQualificationUse {
                machine,
                state,
                statement_index: 3,
                expression: cast_expression,
                domain: SymbolHandle::from_arena_index(99),
                semantic_domain,
            },
        );
    }

    #[test]
    #[should_panic(expected = "must name a registered semantic-domain instance")]
    fn qualification_manifest_rejects_unknown_vacuous_semantic_domain() {
        let (program, machine, state, _, domain, _, cast_expression, _) =
            vacuous_qualification_fixture();
        validate_vacuous_qualification_use(
            &program,
            &VacuousQualificationUse {
                machine,
                state,
                statement_index: 3,
                expression: cast_expression,
                domain,
                semantic_domain: SemanticDomainId(99),
            },
        );
    }

    #[test]
    fn carry_manifest_keeps_authored_and_effective_policies_separate() {
        let symbol = SymbolHandle::from_arena_index(7);
        let state_symbol = SymbolHandle::from_arena_index(9);
        let declared = CarryPolicy {
            suspension: CarrySuspension::Forbidden,
            cpu: CarryCpu::Origin,
            host_thread: CarryHostThread::Any,
            address: CarryAddress::Stable,
        };
        let mut program = CheckedTrees::default();
        program
            .typed
            .push_data_definition(psi_typed_trees::data::DataDefinition {
                symbol,
                name: Identifier::generated("PerCpuLease"),
                ..Default::default()
            });
        program.facts.carry.data.push(DataCarryFact {
            data: symbol,
            declared: Some(declared),
            effective: CarryPolicy::PERMISSIVE,
        });
        let machine = SymbolHandle::from_arena_index(8);
        let mut machine_definition = Machine {
            symbol: machine,
            name: Identifier::generated("Worker::run"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine_definition,
            State {
                symbol: state_symbol,
                name: Identifier::generated("entry"),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine_definition);
        program
            .facts
            .carry
            .suspension_crossings
            .push(SuspensionCrossingCarryFact {
                machine,
                state: state_symbol,
                statement_index: 3,
                call_ordinal: 1,
                target: machine,
                effective: CarryPolicy::STRICT,
                live_values: Vec::new(),
            });
        program
            .facts
            .carry
            .activation_wide_carry
            .push(MachineActivationCarryFact {
                machine,
                effective: CarryPolicy::STRICT,
                analysis_complete: true,
                contributing_types: Vec::new(),
                unnamed_strict_values: 1,
            });
        program
            .facts
            .carry
            .claim_policies
            .push(ClaimCarryPolicyFact {
                claim_identity: psi_language_semantics::PermissionClaimIdentity::Unknown,
                effective: CarryPolicy::STRICT,
                contributing_origins: 2,
            });

        let json = carry_manifest_json(&program);

        assert!(json.contains("\"type\": \"PerCpuLease\""));
        assert!(json.contains(
            "\"declared\": {\"suspension\": \"forbidden\", \"cpu\": \"same\", \"thread\": \"any\", \"address\": \"stable\"}"
        ));
        assert!(json.contains(
            "\"effective\": {\"suspension\": \"allowed\", \"cpu\": \"any\", \"thread\": \"any\", \"address\": \"movable\"}"
        ));
        assert!(json.contains("\"machine\": \"Worker::run\""));
        assert!(json.contains("\"machine_overload_identity\": \"named-callable(path(Worker::run)"));
        assert!(json.contains(
            "\"safe_point_crossings\": [\n    {\n      \"machine\": \"Worker::run\",\n      \"machine_overload_identity\": \"named-callable(path(Worker::run)"
        ));
        assert!(json.contains("\"analysis_complete\": true"));
        assert!(json.contains("\"subtree_machine_count\": 1"));
        assert!(json.contains("\"unnamed_strict_values\": 1"));
        assert!(json.contains("\"claim_policies\": ["));
        assert!(json.contains("\"claim_identity\": {\"kind\": \"unknown\"}"));
        assert!(json.contains("\"contributing_origins\": 2"));
    }

    #[test]
    fn task_activation_manifest_retains_exact_target_overload_identity() {
        let machine_symbol = SymbolHandle::from_arena_index(8);
        let state_symbol = SymbolHandle::from_arena_index(9);
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated("Worker::run"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                name: Identifier::generated("entry"),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);

        let normalized_id = |identity| {
            omega_task_plans::MachineContractId::from_normalized_identity(identity)
                .expect("nonzero normalized identity")
        };
        let plan =
            omega_task_plans::validate_activation_plan(omega_task_plans::ActivationPlanCandidate {
                machine_contract: normalized_id(1),
                entry: omega_task_plans::MachineEntryId::from_normalized_identity(2)
                    .expect("nonzero entry identity"),
                argument_layout: omega_task_plans::ValueLayoutId::from_normalized_identity(3)
                    .expect("nonzero argument layout identity"),
                terminal_outcome_layout: omega_task_plans::ValueLayoutId::from_normalized_identity(
                    4,
                )
                .expect("nonzero result layout identity"),
                calling_plan: omega_task_plans::CallingPlanId::from_normalized_identity(5)
                    .expect("nonzero calling-plan identity"),
                stack_plan: omega_task_plans::StackPlan {
                    bytes: 4096,
                    alignment: 16,
                    representation:
                        omega_task_plans::StackRepresentationId::from_normalized_identity(6)
                            .expect("nonzero stack representation identity"),
                },
                may_suspend: false,
                may_block: false,
                canonical_suspension_crossings: Vec::new(),
                carry_obligations: omega_task_plans::ActivationCarryObligations::none(),
                cancellation_required: false,
            })
            .expect("valid non-suspending activation plan");
        let activations = omega_task_plans::TaskActivationPlanSet {
            activations: vec![omega_task_plans::TaskActivationPlanFact {
                start_requirement: SymbolHandle::invalid(),
                target_machine: machine_symbol,
                target_entry: state_symbol,
                specialization_fingerprint: 0x1234,
                operation: omega_task_plans::TaskStartOperation::Start,
                selected_runtime: omega_task_plans::SelectedTaskRuntimeProviderFact {
                    runtime: omega_task_plans::TaskRuntimeId::from_normalized_identity(7)
                        .expect("nonzero runtime identity"),
                    provider_plan_name: "Runtime::selected".to_owned(),
                    requirement_identity: "TaskRuntime::start#exact".to_owned(),
                },
                plan,
            }],
        };

        let json = task_activation_manifest_json(&program, &activations);

        assert!(json.contains("\"target_machine\": \"Worker::run\""));
        assert!(
            json.contains(
                "\"target_machine_overload_identity\": \"named-callable(path(Worker::run)"
            )
        );
        assert!(json.contains("\"activation_plan_id\": \"0x"));
    }

    #[test]
    fn machine_contract_manifest_reads_independent_mutation_facts() {
        let (mut program, machine_symbol, state_symbol, _) = mutation_state_owner_fixture();
        push_behavior_contract(&mut program, machine_symbol, false, false);

        let without_mutation = machine_contract_manifest_json(&program);
        assert!(without_mutation.contains("\"inferred_write_frames\": []"));

        program.facts.mutation.machines.push(MachineMutationFact {
            machine: machine_symbol,
            state_write_frames: vec![StateWriteFramePlan {
                state: state_symbol,
                frame: psi_facts::NormalizedWriteFrame::complete(vec!["self.value".to_owned()]),
            }],
        });
        let with_mutation = machine_contract_manifest_json(&program);
        let contract_start = with_mutation.find("\"contract\"").expect("contract object");
        let implementation_start = with_mutation
            .find("\"implementation\"")
            .expect("implementation object");
        assert!(
            !with_mutation[contract_start..implementation_start].contains("inferred_write_frames")
        );
        assert!(with_mutation[implementation_start..].contains(
            "\"inferred_write_frames\": [\n          {\"state\": \"entry\", \"completeness\": \"complete\""
        ));
        assert!(with_mutation[implementation_start..].contains("\"paths\": [\"self.value\"]"));
    }

    #[test]
    #[should_panic(expected = "write-frame state must belong to its exact fact machine")]
    fn machine_contract_manifest_rejects_cross_machine_mutation_frame_state() {
        let (program, owner, _, other_state) = mutation_state_owner_fixture();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == owner)
            .expect("owner machine");

        mutation_frame_state_name(&program, machine, other_state);
    }

    #[test]
    #[should_panic(expected = "write-frame state must belong to its exact fact machine")]
    fn machine_contract_manifest_rejects_missing_mutation_frame_state() {
        let (program, owner, _, _) = mutation_state_owner_fixture();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == owner)
            .expect("owner machine");

        mutation_frame_state_name(&program, machine, SymbolHandle::from_arena_index(99));
    }

    #[test]
    fn machine_contract_manifest_distinguishes_published_empty_from_internal_empty_reach() {
        let public_symbol = SymbolHandle::from_arena_index(20);
        let private_symbol = SymbolHandle::from_arena_index(21);
        let mut program = CheckedTrees::default();
        for (machine_symbol, state_symbol, name) in [
            (
                public_symbol,
                SymbolHandle::from_arena_index(22),
                "Public::run",
            ),
            (
                private_symbol,
                SymbolHandle::from_arena_index(23),
                "Private::run",
            ),
        ] {
            let mut machine = Machine {
                symbol: machine_symbol,
                name: Identifier::generated(name),
                ..Default::default()
            };
            program.typed.push_machine_state(
                &mut machine,
                State {
                    symbol: state_symbol,
                    name: Identifier::generated("entry"),
                    ..Default::default()
                },
            );
            program.typed.push_machine(machine);
            push_behavior_contract(&mut program, machine_symbol, false, false);
        }

        let empty = psi_language_semantics::ServiceReachRowTable::EMPTY_ROW;
        for (machine, interface) in [
            (
                public_symbol,
                psi_language_semantics::ServiceReachInterface::PublishedCeiling(empty),
            ),
            (
                private_symbol,
                psi_language_semantics::ServiceReachInterface::InternalInferred,
            ),
        ] {
            program.facts.service_reaches.machines.append_to_span(
                &mut program.facts.service_reaches.root_machines,
                MachineServiceReachRows {
                    machine,
                    interface,
                    published_ceiling: empty,
                    inferred_direct: empty,
                    inferred_transitive: empty,
                    effective: empty,
                    states: Default::default(),
                },
            );
        }

        let json = machine_contract_manifest_json(&program);
        let public_start = json
            .find("\"machine\": \"Public::run\"")
            .expect("public row");
        let private_start = json
            .find("\"machine\": \"Private::run\"")
            .expect("private row");
        assert!(json[public_start..private_start].contains(
            "\"service_reach\": {\"interface\": \"published_ceiling\", \"services\": []}"
        ));
        assert!(
            json[private_start..]
                .contains("\"service_reach\": {\"interface\": \"internal_inferred\"}")
        );
    }

    #[test]
    fn machine_contract_manifest_keeps_interface_and_witness_separate() {
        let symbol = SymbolHandle::from_arena_index(2);
        let state_symbol = SymbolHandle::from_arena_index(3);
        let capsule_machine_symbol = SymbolHandle::from_arena_index(4);
        let capsule_state_symbol = SymbolHandle::from_arena_index(5);
        let service_symbol = SymbolHandle::from_arena_index(1);
        let mut program = CheckedTrees::default();
        let service = program
            .facts
            .service_reaches
            .services
            .intern(service_symbol, "Readable");
        let service_row = program.facts.service_reaches.rows.intern(vec![service]);
        let crash = psi_checked_trees::CrashPlan::published_ceiling(vec![
            psi_checked_trees::CrashRouteBucket::unconditional(
                psi_checked_trees::CrashCause::Abort,
            ),
        ]);
        let abort_bucket = crash
            .published_with_ids()
            .next()
            .map(|(id, _)| id)
            .expect("published abort bucket");
        let abandoned_claim = psi_language_semantics::PermissionClaimIdentity::Established {
            machine_symbol: symbol,
            state_symbol,
            source: psi_language_semantics::PermissionEventSource::StateEntry,
            ordinal: 0,
        };
        let crash = crash
            .with_checked_sites(vec![
                psi_checked_trees::CheckedCrashSite::new(
                    psi_checked_trees::CrashSiteLocation::new(state_symbol, 4),
                    psi_checked_trees::CrashCause::Abort,
                    vec![abort_bucket],
                    vec![abandoned_claim],
                )
                .with_path_guard_conjuncts(vec![
                    psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(vec![
                        1, 9, 0, 0, 0, 0,
                    ]),
                ])
                .with_path_guard_consequences(vec![
                    psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(vec![1, 4, 1]),
                ]),
            ])
            .expect("one crash site per source location")
            .with_checked_calls(vec![
                psi_checked_trees::CheckedCrashCallSite::new(
                    psi_checked_trees::CrashCallSiteLocation::new(state_symbol, 7, 2),
                    symbol,
                    state_symbol,
                    0x1234,
                    vec![psi_checked_trees::CrashRouteBucket::unconditional(
                        psi_checked_trees::CrashCause::Trap,
                    )],
                )
                .with_path_guard_conjuncts(vec![
                    psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(vec![1, 4, 1]),
                ]),
                psi_checked_trees::CheckedCrashCallSite::new(
                    psi_checked_trees::CrashCallSiteLocation::new(state_symbol, 8, 0),
                    capsule_machine_symbol,
                    capsule_state_symbol,
                    0x5678,
                    vec![psi_checked_trees::CrashRouteBucket::unconditional(
                        psi_checked_trees::CrashCause::Trap,
                    )],
                ),
            ])
            .expect("one crash call per invocation coordinate");
        let mut machine = Machine {
            symbol,
            name: Identifier::generated("Worker::run"),
            termination_plan: MachineTerminationPlan {
                implementation_witness: Some(RankingWitness {
                    subjects: vec!["remaining".to_string()],
                    ranking_view: RankingViewId::NAT_DESCENDING,
                    view_path: "Nat::Descending".to_string(),
                    view_arguments: Vec::new(),
                    rank_range: None,
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                name: Identifier::generated("entry"),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
        let mut capsule_trait = TraitDefinition {
            symbol: capsule_machine_symbol,
            name: Identifier::generated("Firmware"),
            ..Default::default()
        };
        program.typed.push_trait_machine_signature(
            &mut capsule_trait,
            StateSignature {
                symbol: capsule_state_symbol,
                name: Identifier::generated("read"),
                ..Default::default()
            },
        );
        program.typed.push_trait_definition(capsule_trait);
        program.facts.service_reaches.machines.append_to_span(
            &mut program.facts.service_reaches.root_machines,
            MachineServiceReachRows {
                machine: symbol,
                interface: psi_language_semantics::ServiceReachInterface::PublishedCeiling(
                    service_row,
                ),
                published_ceiling: service_row,
                inferred_direct: service_row,
                inferred_transitive: service_row,
                effective: service_row,
                states: Default::default(),
            },
        );
        program.facts.synchronous_invocations.machines.push(
            psi_checked_trees::MachineSynchronousInvocationFact {
                machine: symbol,
                plan: psi_language_semantics::SynchronousInvocationPlan {
                    interface:
                        psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling,
                    published: vec!["parameter:0".to_owned()],
                    checked_inferred: vec!["parameter:0".to_owned()],
                },
            },
        );
        program
            .facts
            .suspensions
            .machines
            .push(psi_checked_trees::MachineSuspensionFact {
                machine: symbol,
                plan: SuspensionPlan {
                    interface: SuspensionInterface::PublishedMaySuspend(false),
                    checked_may_suspend: false,
                },
            });
        program
            .facts
            .blocking
            .machines
            .push(psi_checked_trees::MachineBlockingFact {
                machine: symbol,
                plan: BlockingPlan {
                    interface: BlockingInterface::PublishedMayBlock(true),
                    checked_may_block: true,
                },
            });
        program
            .facts
            .termination
            .machines
            .push(psi_checked_trees::MachineTerminationFact {
                machine: symbol,
                plan: MachineTerminationPlan {
                    interface: psi_language_semantics::TerminationInterface::Published(
                        TerminationGuarantee::NoGuarantee,
                    ),
                    checked_summary: TerminationGuarantee::Terminates {
                        premises: Vec::new(),
                    },
                    implementation_witness: Some(RankingWitness {
                        subjects: vec!["remaining".to_string()],
                        ranking_view: RankingViewId::NAT_DESCENDING,
                        view_path: "Nat::Descending".to_string(),
                        view_arguments: Vec::new(),
                        rank_range: None,
                    }),
                },
            });
        program
            .facts
            .contract_plans
            .machines
            .push(MachineContractPlan {
                machine: symbol,
                closed_scalar_values: Default::default(),
                crash,
                fingerprint: 0x1234,
            });
        program.facts.contract_plans.crash_capsules.push(
            psi_checked_trees::CrashContractCapsule::new(
                capsule_machine_symbol,
                capsule_state_symbol,
                0x5678,
                vec![psi_checked_trees::CrashRouteBucket::unconditional(
                    psi_checked_trees::CrashCause::Trap,
                )],
            ),
        );
        let json = machine_contract_manifest_json(&program);
        let contract_start = json.find("\"contract\"").expect("contract object");
        let implementation_start = json
            .find("\"implementation\"")
            .expect("implementation object");
        let contract = &json[contract_start..implementation_start];

        assert!(contract.contains("\"fingerprint\": \"0x0000000000001234\""));
        assert!(contract.contains("\"supply\": \"checked_body\""));
        assert!(!contract.contains("\"supply\": \"accepted\""));
        assert!(json.contains("\"machine_overload_identity\": \"named-callable(path(Worker::run)"));
        assert!(contract.contains(
            "\"service_reach\": {\"interface\": \"published_ceiling\", \"services\": [\"Readable\"]}"
        ));
        assert!(contract.contains(
            "\"synchronous_invocation\": {\"interface\": \"published_ceiling\", \"targets\": [\"parameter:0\"]}"
        ));
        assert!(contract.contains(
            "\"suspension\": {\"interface\": \"published_ceiling\", \"may_suspend\": false}"
        ));
        assert!(
            contract.contains(
                "\"blocking\": {\"interface\": \"published_ceiling\", \"may_block\": true}"
            )
        );
        assert!(contract.contains(
            "\"crashes\": {\"interface\": \"published_ceiling\", \"buckets\": [{\"cause\": \"Abort\", \"alternative_guards\": [\"true\"]}]}"
        ));
        assert!(contract.contains(
            "\"termination\": {\"interface\": \"published\", \"guarantee\": {\"kind\": \"no_guarantee\"}}"
        ));
        assert!(!contract.contains("inferred_write_frames"));
        assert!(!contract.contains("remaining"));
        assert!(json[implementation_start..].contains("\"inferred_write_frames\": []"));
        assert!(json[implementation_start..].contains(
            "\"checked_crash_sites\": [\n          {\"state\": \"entry\", \"statement_ordinal\": 4, \"cause\": \"Abort\", \"path_guard_conjuncts\": [\"0x010900000000\"], \"path_guard_consequences\": [\"0x010401\"], \"guard_covering_buckets\": [1], \"covering_buckets\": [1], \"frontier_lower_bound\": [{\"kind\": \"established\""
        ));
        assert!(json[implementation_start..].contains(
            "\"checked_crash_calls\": [\n          {\"state\": \"entry\", \"statement_ordinal\": 7, \"call_ordinal\": 2, \"target_machine\": \"Worker::run\", \"target_callable_overload_identity\": \"named-callable(path(Worker::run)"
        ));
        assert!(json[implementation_start..].contains(
            "\"target_state\": \"entry\", \"target_contract_fingerprint\": \"0x0000000000001234\", \"path_guard_conjuncts\": [\"0x010401\"], \"path_guard_consequences\": [], \"surviving_buckets\": [{\"cause\": \"Trap\", \"alternative_guards\": [\"true\"]}]"
        ));
        assert!(
            json[implementation_start..].contains("\"statement_ordinal\": 8, \"call_ordinal\": 0")
        );
        assert!(json[implementation_start..].contains(
            "\"target_callable_overload_identity\": \"named-callable(path(Firmware::read)"
        ));
        assert!(json.contains("\"crash_contract_capsules\": [\n    {\"target_machine\":"));
        assert!(json.contains(
            "\"target_callable_overload_identity\": \"named-callable(path(Firmware::read)"
        ));
        assert!(json.contains("\"target_contract_fingerprint\": \"0x0000000000005678\""));
        assert!(
            json[implementation_start..]
                .contains("\"source\": {\"kind\": \"state_entry\"}, \"ordinal\": 0}]")
        );
        assert!(json[implementation_start..].contains("\"checked_may_suspend\": false"));
        assert!(json[implementation_start..].contains("\"checked_may_block\": true"));
        assert!(json[implementation_start..].contains("\"checked_service_reach\": [\"Readable\"]"));
        assert!(
            json[implementation_start..]
                .contains("\"checked_synchronous_invocations\": [\"parameter:0\"]")
        );
        assert!(json[implementation_start..].contains("\"kind\": \"terminates\""));
        assert!(json[implementation_start..].contains("\"subjects\": [\"remaining\"]"));
        assert!(json[implementation_start..].contains("\"view\": \"Nat::Descending\""));
    }

    #[test]
    fn termination_manifest_distinguishes_private_derivation_from_public_omission() {
        let mut internal = String::new();
        push_termination_interface_json(&mut internal, &TerminationInterface::InternalDerived);
        assert_eq!(internal, "{\"interface\": \"internal_derived\"}");

        let mut omitted = String::new();
        push_termination_interface_json(
            &mut omitted,
            &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
        );
        assert_eq!(
            omitted,
            "{\"interface\": \"published\", \"guarantee\": {\"kind\": \"no_guarantee\"}}"
        );
    }

    #[test]
    fn machine_contract_manifest_records_specialization_trust_and_contract_ids() {
        let symbol = SymbolHandle::from_arena_index(3);
        let state_symbol = SymbolHandle::from_arena_index(4);
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol,
            name: Identifier::generated("accepted_map"),
            supply_mode: MachineSupplyMode::Accepted,
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                name: Identifier::generated("entry"),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
        program
            .typed
            .machine_specializations
            .push(MachineSpecialization {
                template: symbol,
                instance: symbol,
                type_arguments: vec!["Card".to_owned()],
                const_arguments: vec!["1".to_owned()],
                type_argument_identities: vec!["named(name(Card))".to_owned()],
                const_argument_identities: vec!["named(name(1))".to_owned()],
                machine_arguments: vec![SymbolHandle::from_arena_index(8)],
                conformance_arguments: Vec::new(),
                template_contract_fingerprint: 0x1111,
                accepted_template_commitment: Some("accepted_map".to_owned()),
                machine_argument_contract_fingerprints: vec![0x2222],
                conformance_argument_fingerprints: vec![0x4444, 0x5555],
                fingerprint: 0x3333,
            });
        push_behavior_contract(&mut program, symbol, false, false);
        program
            .facts
            .contract_plans
            .machines
            .last_mut()
            .expect("specialization contract fixture")
            .fingerprint = 0xaaaa;

        let json = machine_contract_manifest_json(&program);
        assert!(json.contains("\"template\": \"accepted_map\""));
        assert!(json.contains("\"accepted_template_commitment\": \"accepted_map\""));
        assert!(json.contains("\"template_contract_fingerprint\": \"0x0000000000001111\""));
        assert!(json.contains("\"type_arguments\": [\"Card\"]"));
        assert!(json.contains("\"const_arguments\": [\"1\"]"));
        assert!(json.contains("\"type_argument_identities\": [\"named(name(Card))\"]"));
        assert!(json.contains("\"const_argument_identities\": [\"named(name(1))\"]"));
        assert!(
            json.contains("\"machine_argument_contract_fingerprints\": [\"0x0000000000002222\"]")
        );
        assert!(json.contains(
            "\"conformance_argument_fingerprints\": [\"0x0000000000004444\", \"0x0000000000005555\"]"
        ));
        assert!(json.contains("\"instance_fingerprint\": \"0x0000000000003333\""));
        assert!(json.contains("\"instance_contract_fingerprint\": \"0x000000000000aaaa\""));
    }

    #[test]
    #[should_panic(expected = "must have an exact machine contract plan")]
    fn specialization_manifest_fails_closed_without_exact_instance_contract() {
        specialization_instance_contract_fingerprint(
            &CheckedTrees::default(),
            SymbolHandle::from_arena_index(1),
        );
    }
}
