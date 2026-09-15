//! Qualification evidence, receipts, and exact program-point validation.

#[cfg(test)]
mod tests;

use crate::manifest_coordinates::{
    exact_program_point_label, machine_overload_identity, program_point_name,
    qualification_symbol_label,
};
use crate::manifest_values::{push_carry_policy_json, push_json_string};
use checked_trees::CheckedTrees;

/// Public checked qualification-evidence surface. The fact's program point and
/// its establishment origin remain independent, and admitted rows retain their
/// normalized receipt identity when provider admission supplied one.
pub fn qualification_evidence_manifest_json(
    program: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> String {
    use facts::FactPayload;
    use language_semantics::QualificationEvidenceOrigin;

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
    &typed_trees::machine::Machine,
    Vec<(language_semantics::SemanticDomainId, &str)>,
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
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    origin: language_semantics::QualificationEvidenceOrigin,
    receipt_identity: u64,
) {
    if receipt_identity != 0 {
        assert_eq!(
            origin,
            language_semantics::QualificationEvidenceOrigin::AdmittedReceipt,
            "nonzero qualification evidence receipt must use admitted-receipt origin",
        );
        selected_provider_plans
            .plan_by_report_fingerprint(receipt_identity)
            .expect(
                "qualification evidence receipt must name an exact retained selected provider plan",
            );
    }
}

fn validate_qualification_source(program: &CheckedTrees, evidence: &facts::QualificationEvidence) {
    use language_semantics::QualificationEvidenceOrigin;
    use typed_trees::data::{MachineParameterContract, TypeParameterKind};

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

fn validate_qualification_program_point(program: &CheckedTrees, point: facts::ProgramPoint) {
    use facts::ProgramPoint;

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
        }
        | ProgramPoint::TransitionArm {
            machine_symbol,
            state_symbol,
            statement_index,
            ..
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
            transition_target,
        } => {
            assert!(
                program.facts.proof.contract_exits.iter().any(|(_, exit)| {
                    exit.machine_symbol == machine_symbol
                        && exit.state_symbol == state_symbol
                        && exit.statement_index == statement_index
                        && exit.transition_target == transition_target
                }),
                "qualification evidence exit point must name an exact checked exit"
            );
            // Implicit Unit returns sit after the final statement, including
            // statement zero for an empty body. Their exit join is the bound.
            (machine_symbol, state_symbol, None, None)
        }
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
    if let ProgramPoint::TransitionArm {
        statement_index,
        transition_target,
        ..
    } = point
    {
        let typed_trees::statement::StatementNode::Transition(transition) =
            &program.statement_table.statements(state.statement_nodes)[statement_index]
        else {
            panic!("qualification evidence transition point must name a transition");
        };
        assert!(
            !transition_target.is_valid()
                || transition_target == transition.target
                || transition_target == transition.continuation,
            "qualification evidence transition target must belong to its exact statement",
        );
    }
}

fn validate_vacuous_qualification_use<'program>(
    program: &'program CheckedTrees,
    use_fact: &checked_trees::VacuousQualificationUse,
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
            typed_trees::expression::ExpressionNode::Cast(_)
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
    statement: &typed_trees::statement::StatementNode,
    target: typed_trees::expression::ExpressionHandle,
    visited: &mut Vec<typed_trees::expression::ExpressionHandle>,
) -> bool {
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

    let mut contains =
        |expression| qualification_expression_contains(program, expression, target, visited);
    match statement {
        StatementNode::RootBinding(_) | StatementNode::AssemblyFact(_) => false,
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
    expression: typed_trees::expression::ExpressionHandle,
    target: typed_trees::expression::ExpressionHandle,
    visited: &mut Vec<typed_trees::expression::ExpressionHandle>,
) -> bool {
    use typed_trees::expression::ExpressionNode;

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
        ExpressionNode::Match(dispatch) => {
            contains(dispatch.subject)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        let pattern_contains = match arm.pattern {
                            typed_trees::expression::MatchPattern::Value(pattern) => {
                                contains(pattern)
                            }
                            typed_trees::expression::MatchPattern::Wildcard => false,
                        };
                        pattern_contains || contains(arm.value)
                    })
        }
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
    plan: &effects::provider_plan::ProviderPlan,
    method: &effects::provider_plan::ServiceMethod,
) -> String {
    let owner = if method.requirement_owner.is_empty() {
        &plan.schema.trait_name
    } else {
        &method.requirement_owner
    };
    format!("{owner}::{}", method.name)
}

fn qualification_subject(program: &CheckedTrees, fact: &facts::Fact) -> String {
    use facts::{FactPlace, PlaceRoot, PlaceSegment};

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

fn qualification_requirement_identity(
    program: &CheckedTrees,
    evidence: &facts::QualificationEvidence,
) -> Option<String> {
    if evidence.origin != language_semantics::QualificationEvidenceOrigin::AdmittedReceipt {
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
