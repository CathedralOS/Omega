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

    let mut json = String::from("{\n  \"qualification_evidence\": [");
    for (index, (fact, domain_label)) in rows.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"subject\": ");
        push_json_string(&mut json, &qualification_subject(program, fact));
        json.push_str(",\n      \"domain\": ");
        push_json_string(&mut json, domain_label);
        json.push_str(",\n      \"origin\": ");
        push_json_string(&mut json, fact.evidence.origin.as_str());
        json.push_str(",\n      \"program_point\": ");
        push_json_string(&mut json, program_point_name(fact.point));
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
        if fact.evidence.requirement_symbol.is_valid() {
            push_json_string(
                &mut json,
                &qualification_symbol_label(program, fact.evidence.requirement_symbol),
            );
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
        json.push_str(",\n      \"parameter_index\": null");
        json.push_str(",\n      \"subject_type\": ");
        if let Some(subject_type) = &method.result_type_identity {
            push_json_string(&mut json, subject_type);
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"domain\": ");
        push_json_string(&mut json, &claim.domain);
        json.push_str(",\n      \"effective_carry\": ");
        push_carry_policy_json(&mut json, claim.effective_carry);
        json.push_str(",\n      \"provider_plan\": ");
        push_json_string(&mut json, &plan.name);
        json.push_str(",\n      \"receipt_identity\": ");
        push_json_string(
            &mut json,
            &format!("0x{:016x}", plan.identity_fingerprint()),
        );
        json.push_str("\n    }");
    }

    json.push_str("\n  ],\n  \"vacuous_qualification_uses\": [");
    for (index, use_fact) in program.facts.qualifications.vacuous_uses.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, use_fact.machine),
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
        push_json_string(
            &mut json,
            program
                .semantic_domains
                .name(use_fact.semantic_domain)
                .unwrap_or("<unknown>"),
        );
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

/// Render the authored owner of an exact inherited requirement. The selected
/// schema is the deployment boundary and may be a descendant that only refines
/// calling policy, so reconstructing `Schema::method` would misattribute the
/// semantic requirement. Legacy singleton schemas have no exact identity and
/// retain the compatibility spelling.
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
    for (map_index, (_, map)) in ownership.claim_outcome_maps.iter().enumerate() {
        if map_index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, &symbol_label(program, map.machine_symbol));
        json.push_str(",\n      \"state\": ");
        push_json_string(
            &mut json,
            &state_label_from_symbol(program, map.state_symbol),
        );
        json.push_str(",\n      \"entries\": [");
        for (entry_index, entry) in ownership
            .claim_outcome_entries
            .span_or_empty(map.entries)
            .iter()
            .enumerate()
        {
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
    let mut content_projections = program
        .facts
        .qualifications
        .content
        .plans
        .iter()
        .collect::<Vec<_>>();
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
    for (index, row) in identity_reshuffles.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, &symbol_label(program, row.machine_symbol));
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
            ownership.segments.span_or_empty(row.input_segments),
        );
        json.push_str("},\n      \"output_path\": ");
        push_claim_path_json(
            &mut json,
            program,
            ownership.segments.span_or_empty(row.output_segments),
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
    for (index, row) in partition_compositions.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, &symbol_label(program, row.machine_symbol));
        json.push_str(",\n      \"state\": ");
        push_json_string(
            &mut json,
            &state_label_from_symbol(program, row.state_symbol),
        );
        json.push_str(",\n      \"source_callable\": ");
        push_json_string(&mut json, &symbol_label(program, row.source_callable));
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
    for (index, plan) in conservation.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
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
            FactPlace::Unknown | FactPlace::Place(_) => "<unknown>".to_owned(),
        };
    };
    let place = program.facts.semantic.places.get(place);
    let mut subject = match place.root {
        PlaceRoot::Unknown => "<unknown>".to_owned(),
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

        json.push_str(",\n      \"contract\": {");
        if let Some(contract) = program.facts.contract_plans.for_machine(machine.symbol) {
            json.push_str("\n        \"fingerprint\": \"0x");
            json.push_str(&format!("{:016x}", contract.fingerprint));
            json.push_str("\",\n        \"supply\": ");
            push_json_string(&mut json, supply_mode_name(contract.supply_mode));
            json.push_str(",\n        \"service_reach\": ");
            push_service_reach_plan_json(&mut json, program, contract.service_reach);
            json.push_str(",\n        \"synchronous_invocation\": ");
            push_synchronous_invocation_plan_json(
                &mut json,
                &contract.synchronous_invocation,
                false,
            );
            json.push_str(",\n        \"suspension\": ");
            push_suspension_plan_json(&mut json, contract.suspension);
            json.push_str(",\n        \"blocking\": ");
            push_blocking_plan_json(&mut json, contract.blocking);
            json.push_str(",\n        \"crashes\": ");
            push_crash_plan_json(&mut json, &contract.crash);
            json.push_str(",\n        \"termination\": ");
            push_termination_interface_json(&mut json, &contract.termination);
            json.push_str("\n      }");
        } else {
            json.push_str("}");
        }

        json.push_str(",\n      \"implementation\": {");
        let mut has_implementation_field = false;
        if let Some(contract) = program.facts.contract_plans.for_machine(machine.symbol) {
            json.push_str("\n        \"checked_may_suspend\": ");
            json.push_str(if contract.suspension.checked_may_suspend {
                "true"
            } else {
                "false"
            });
            json.push_str(",\n        \"checked_may_block\": ");
            json.push_str(if contract.blocking.checked_may_block {
                "true"
            } else {
                "false"
            });
            json.push_str(",\n        \"checked_service_reach\": ");
            push_service_row_json(&mut json, program, contract.service_reach.checked_inferred);
            json.push_str(",\n        \"checked_synchronous_invocations\": ");
            push_string_array(&mut json, &contract.synchronous_invocation.checked_inferred);
            json.push_str(",\n        \"inferred_write_frames\": [");
            for (frame_index, state_frame) in contract.inferred_write_frames.iter().enumerate() {
                if frame_index > 0 {
                    json.push(',');
                }
                let state_name = program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == state_frame.state)
                    .map(|state| state.name.as_str())
                    .unwrap_or("<unknown>");
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
            if !contract.inferred_write_frames.is_empty() {
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
                json.push_str(", \"guard_covering_buckets\": [");
                for (coverage_index, bucket) in site.guard_covering_buckets().iter().enumerate() {
                    if coverage_index > 0 {
                        json.push_str(", ");
                    }
                    json.push_str(&bucket.get().to_string());
                }
                json.push(']');
                json.push('}');
            }
            if !contract.crash.checked_sites().is_empty() {
                json.push('\n');
                json.push_str("        ");
            }
            json.push(']');
            has_implementation_field = true;
        }
        if let Some(fact) = program.facts.termination.for_machine(machine.symbol) {
            if has_implementation_field {
                json.push(',');
            }
            json.push_str("\n        \"checked_termination\": ");
            push_termination_json(&mut json, &fact.checked_summary);
            json.push_str(",\n        \"resolved_ranking_view\": ");
            push_json_string(&mut json, &fact.resolved_view_path);
            has_implementation_field = true;
        }
        if let Some(witness) = machine.termination_plan.implementation_witness.as_ref() {
            if has_implementation_field {
                json.push(',');
            }
            json.push_str("\n        \"ranking_witness\": {\n          \"subjects\": [");
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
    json.push_str("\n  ],\n  \"specializations\": [");
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
        json.push_str("\n    {\n      \"template\": ");
        push_json_string(&mut json, template);
        json.push_str(",\n      \"instance\": ");
        push_json_string(&mut json, instance);
        json.push_str(",\n      \"instance_fingerprint\": \"0x");
        json.push_str(&format!("{:016x}", specialization.fingerprint));
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
        json.push_str("]\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
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
    for (bucket_index, bucket) in plan.published().iter().enumerate() {
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
        json.push_str(", \"containment_demand\": ");
        push_json_string(json, bucket.containment_demand());
        json.push_str(", \"alternative_guards\": [");
        for (guard_index, guard) in bucket.alternative_guards().iter().enumerate() {
            if guard_index > 0 {
                json.push_str(", ");
            }
            match guard {
                psi_checked_trees::CrashRouteGuard::Truth => push_json_string(json, "true"),
                psi_checked_trees::CrashRouteGuard::Predicate(predicate) => {
                    let mut identity = String::from("0x");
                    for byte in predicate.canonical_bytes() {
                        identity.push_str(&format!("{byte:02x}"));
                    }
                    push_json_string(json, &identity);
                }
            }
        }
        json.push_str("]}");
    }
    json.push_str("]}");
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
        machine_operational_summary(program, machine.symbol),
    );
    label
}

fn state_label(program: &CheckedTrees, machine: &Machine, state: &State) -> String {
    let borrow_state = borrow_state_for(program, machine.symbol, state.symbol);
    let flow_state = flow_state_for(program, machine.symbol, state.symbol);

    let writable_root_count = borrow_state
        .map(|borrow| borrow.writable_roots.len())
        .unwrap_or(0);
    let (invalidation_count, mutable_parameter_count, service_reach, operational) =
        if let Some(flow) = flow_state {
            (
                flow.invalidations.len(),
                flow.mutable_parameter_count,
                flow.service_reach,
                flow.operational,
            )
        } else {
            (
                0,
                borrow_state
                    .map(|borrow| borrow.mutable_parameter_count)
                    .unwrap_or(0),
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
        operational,
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
        call.operational,
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

fn machine_operational_summary(
    program: &CheckedTrees,
    symbol: SymbolHandle,
) -> psi_language_semantics::OperationalMaySummary {
    let mut summary = psi_language_semantics::OperationalMaySummary::default();
    for flow in program
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| state.machine_symbol == symbol)
    {
        summary.direct_may_suspend |= flow.operational.direct_may_suspend;
        summary.direct_may_block |= flow.operational.direct_may_block;
    }
    if let Some(machine) = program
        .facts
        .operational
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
    {
        summary.transitive_may_suspend = machine.transitive_may_suspend;
        summary.transitive_may_block = machine.transitive_may_block;
    }
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
        carry_manifest_json, claim_outcome_manifest_json, machine_contract_manifest_json,
        push_termination_interface_json, qualification_evidence_manifest_json,
    };
    use psi_checked_trees::{
        CheckedTrees, ClaimCarryPolicyFact, ContentIdentityReshuffleFact,
        ContentPartitionCompositionFact, ContentPartitionPlaceSubstitution,
        ContentPartitionResultRewrite, DataCarryFact, FlowClaimOutcomeEntryFact,
        FlowClaimOutcomeMapFact, FlowClaimOutcomeSource, MachineActivationCarryFact,
        MachineContractPlan, MachineTerminationFact,
    };
    use psi_facts::{
        Fact, FactOrigin, FactPayload, FactPlace, ProgramPoint, QualificationEvidence,
    };
    use psi_language_semantics::content::{
        ContentAlgebraIdentity, ContentArithmeticOperator, ContentConservationEquation,
        ContentConservationOwnerKind, ContentConservationPlan, ContentConservationTerm,
        ContentFieldSegment, ContentPlaceRoot, ContentPlaceVersion, ContentProjectionExpression,
        ContentProjectionPlan, ContentScalarExpression, ContentStructuralPlace,
        conservation_fingerprint,
    };
    use psi_language_semantics::{
        BlockingInterface, BlockingPlan, CarryAddress, CarryCpu, CarryHostThread, CarryPolicy,
        CarrySuspension, MachineSupplyMode, MachineTerminationPlan, QualificationEvidenceOrigin,
        RankingViewId, RankingWitness, SemanticDomainId, SuspensionInterface, SuspensionPlan,
        TerminationGuarantee, TerminationInterface,
    };
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::state::State;
    use psi_typed_trees::typed_trees::MachineSpecialization;

    #[test]
    fn claim_outcome_manifest_keeps_paths_and_source_kinds_structured() {
        let mut program = CheckedTrees::default();
        let output_segments = program.facts.flow.ownership.segments.insert_many([
            psi_facts::PlaceSegment::Case {
                variant: SymbolHandle::invalid(),
            },
            psi_facts::PlaceSegment::Field {
                symbol: SymbolHandle::invalid(),
            },
        ]);
        let entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .insert_many([
                FlowClaimOutcomeEntryFact {
                    output_segments,
                    source: FlowClaimOutcomeSource::Input {
                        parameter_symbol: SymbolHandle::invalid(),
                        segments: Default::default(),
                    },
                },
                FlowClaimOutcomeEntryFact {
                    output_segments: Default::default(),
                    source: FlowClaimOutcomeSource::Established {
                        claim_identity:
                            psi_language_semantics::PermissionClaimIdentity::Established {
                                machine_symbol: SymbolHandle::invalid(),
                                state_symbol: SymbolHandle::invalid(),
                                source: psi_language_semantics::PermissionEventSource::Statement {
                                    statement_index: 2,
                                },
                                ordinal: 7,
                            },
                        provenance: psi_language_semantics::PermissionProvenance::Established {
                            machine_symbol: SymbolHandle::invalid(),
                            state_symbol: SymbolHandle::invalid(),
                            source: psi_language_semantics::PermissionEventSource::StateEntry,
                        },
                    },
                },
            ]);
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .insert(FlowClaimOutcomeMapFact {
                machine_symbol: SymbolHandle::invalid(),
                state_symbol: SymbolHandle::invalid(),
                entries,
            });
        program
            .facts
            .qualifications
            .content
            .plans
            .push(ContentProjectionPlan {
                domain: SymbolHandle::invalid(),
                semantic_domain: SemanticDomainId(41),
                carrier_identity: "named(name(Region))".to_owned(),
                machine: SymbolHandle::invalid(),
                algebra: ContentAlgebraIdentity::CountedQuantity {
                    unit: "named(name(ByteUnit))".to_owned(),
                },
                expression: ContentProjectionExpression::CountedQuantity {
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
                },
                fingerprint: 0x0123_4567_89ab_cdef,
            });
        let input = ContentConservationTerm::Projection {
            domain: SymbolHandle::invalid(),
            semantic_domain: SemanticDomainId(41),
            projection_machine: SymbolHandle::invalid(),
            projection_fingerprint: 0x0123_4567_89ab_cdef,
            subject: ContentStructuralPlace {
                version: ContentPlaceVersion::Entry,
                root: ContentPlaceRoot::Parameter {
                    position: 0,
                    symbol: SymbolHandle::invalid(),
                    name: "region".to_owned(),
                    is_self: false,
                },
                segments: Vec::new(),
            },
        };
        let output = ContentConservationTerm::Projection {
            domain: SymbolHandle::invalid(),
            semantic_domain: SemanticDomainId(41),
            projection_machine: SymbolHandle::invalid(),
            projection_fingerprint: 0x0123_4567_89ab_cdef,
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
                target: output_subject.clone(),
            },
        ];
        let result_rewrite = ContentPartitionResultRewrite {
            claim_identity: psi_language_semantics::PermissionClaimIdentity::Established {
                machine_symbol: SymbolHandle::invalid(),
                state_symbol: SymbolHandle::invalid(),
                source: psi_language_semantics::PermissionEventSource::Statement {
                    statement_index: 4,
                },
                ordinal: 12,
            },
            source: substitutions[1].source.clone(),
            target: substitutions[1].target.clone(),
        };
        let partition_entry_place = input_subject.clone();
        let equation = ContentConservationEquation::new(input, output);
        let fingerprint = conservation_fingerprint(&algebra, &equation);
        let plan = ContentConservationPlan {
            owner_kind: ContentConservationOwnerKind::Machine,
            owner: SymbolHandle::invalid(),
            callable: SymbolHandle::invalid(),
            algebra,
            equation,
            fingerprint,
        };
        program
            .facts
            .qualifications
            .content
            .identity_reshuffles
            .push(ContentIdentityReshuffleFact {
                machine_symbol: SymbolHandle::invalid(),
                state_symbol: SymbolHandle::invalid(),
                claim_identity: psi_language_semantics::PermissionClaimIdentity::Established {
                    machine_symbol: SymbolHandle::invalid(),
                    state_symbol: SymbolHandle::invalid(),
                    source: psi_language_semantics::PermissionEventSource::StateEntry,
                    ordinal: 9,
                },
                input_parameter_symbol: SymbolHandle::invalid(),
                input_segments: Default::default(),
                output_segments: Default::default(),
                plan: plan.clone(),
            });
        program
            .facts
            .qualifications
            .content
            .partition_compositions
            .push(ContentPartitionCompositionFact {
                machine_symbol: SymbolHandle::invalid(),
                state_symbol: SymbolHandle::invalid(),
                source_callable: SymbolHandle::invalid(),
                source_fingerprint: plan.fingerprint,
                source_derivation_depth: 0,
                source_plan: plan.clone(),
                statement_index: 4,
                call_ordinal: 2,
                input_claim_identities: vec![
                    psi_language_semantics::PermissionClaimIdentity::Established {
                        machine_symbol: SymbolHandle::invalid(),
                        state_symbol: SymbolHandle::invalid(),
                        source: psi_language_semantics::PermissionEventSource::StateEntry,
                        ordinal: 11,
                    },
                ],
                input_claim_bindings: vec![psi_checked_trees::ContentPartitionInputClaimBinding {
                    claim_identity: psi_language_semantics::PermissionClaimIdentity::Established {
                        machine_symbol: SymbolHandle::invalid(),
                        state_symbol: SymbolHandle::invalid(),
                        source: psi_language_semantics::PermissionEventSource::StateEntry,
                        ordinal: 11,
                    },
                    entry_place: partition_entry_place,
                }],
                result_rewrites: vec![result_rewrite],
                substitutions,
                plan,
            });

        let json = claim_outcome_manifest_json(&program);

        assert!(json.contains("\"claim_outcome_maps\""));
        assert!(
            json.contains("\"output_path\": [{\"case\": \"invalid\"}, {\"field\": \"invalid\"}]")
        );
        assert!(json.contains("\"kind\": \"input\""));
        assert!(json.contains("\"kind\": \"established\""));
        assert!(json.contains("\"statement_index\": 2"));
        assert!(json.contains("\"ordinal\": 7"));
        assert!(json.contains("\"kind\": \"state_entry\""));
        assert!(json.contains("\"content_projections\""));
        assert!(json.contains("\"content_identity_reshuffles\": [\n    {"));
        assert!(json.contains("\"content_partition_compositions\": [\n    {"));
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
        assert!(json.contains("\"ordinal\": 11"));
        assert!(json.contains("\"ordinal\": 12"));
        assert!(json.contains("\"input\": {\"parameter\": \"invalid\", \"path\": []}"));
        assert!(json.contains("\"ordinal\": 9"));
        assert!(json.contains("\"semantic_domain_id\": 41"));
        assert!(json.contains("\"kind\": \"counted_quantity\""));
        assert!(json.contains("\"unit\": \"named(name(ByteUnit))\""));
        assert!(json.contains("\"kind\": \"runtime_scalar_embedding\""));
        assert!(json.contains("\"path\": [\"length\"]"));
        assert!(json.contains("\"operator\": \"add\""));
        assert!(json.contains("\"fingerprint\": \"0x0123456789abcdef\""));
    }

    #[test]
    fn qualification_evidence_manifest_separates_origin_point_and_receipt() {
        let subject = SymbolHandle::from_arena_index(4);
        let domain = SymbolHandle::from_arena_index(5);
        let provider = SymbolHandle::from_arena_index(6);
        let mut program = CheckedTrees::default();
        let place = program.facts.semantic.append_symbol_place(subject);
        program.facts.semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point: ProgramPoint::CallEnsures {
                machine_symbol: subject,
                state_symbol: subject,
                statement_index: 2,
                call_ordinal: 1,
            },
            origin: FactOrigin::CallEnsures,
            evidence: QualificationEvidence {
                origin: QualificationEvidenceOrigin::AdmittedReceipt,
                source_symbol: provider,
                requirement_symbol: SymbolHandle::invalid(),
                receipt_identity: 0x1234,
            },
            payload: FactPayload::DomainMembership {
                value: Default::default(),
                domain: Default::default(),
                domain_symbol: domain,
            },
        });

        let json = qualification_evidence_manifest_json(
            &program,
            &omega_effects::SelectedProviderPlanFacts::default(),
        );

        assert!(json.contains("\"subject\": \"#4\""));
        assert!(json.contains("\"domain\": \"#5\""));
        assert!(json.contains("\"origin\": \"admitted_receipt\""));
        assert!(json.contains("\"program_point\": \"call_ensures\""));
        assert!(json.contains("\"source\": \"#6\""));
        assert!(json.contains("\"requirement\": null"));
        assert!(json.contains("\"receipt_identity\": \"0x0000000000001234\""));
    }

    #[test]
    fn carry_manifest_keeps_authored_and_effective_policies_separate() {
        let symbol = SymbolHandle::from_arena_index(7);
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
        program.typed.push_machine(Machine {
            symbol: machine,
            name: Identifier::generated("Worker::run"),
            ..Default::default()
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
        assert!(json.contains("\"analysis_complete\": true"));
        assert!(json.contains("\"subtree_machine_count\": 1"));
        assert!(json.contains("\"unnamed_strict_values\": 1"));
        assert!(json.contains("\"claim_policies\": ["));
        assert!(json.contains("\"claim_identity\": {\"kind\": \"unknown\"}"));
        assert!(json.contains("\"contributing_origins\": 2"));
    }

    #[test]
    fn machine_contract_manifest_keeps_interface_and_witness_separate() {
        let symbol = SymbolHandle::from_arena_index(2);
        let state_symbol = SymbolHandle::from_arena_index(3);
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
                "ExecutionDomain",
            ),
        ]);
        let abort_bucket = crash
            .published_with_ids()
            .next()
            .map(|(id, _)| id)
            .expect("published abort bucket");
        let crash = crash
            .with_checked_sites(vec![psi_checked_trees::CheckedCrashSite::new(
                psi_checked_trees::CrashSiteLocation::new(state_symbol, 4),
                psi_checked_trees::CrashCause::Abort,
                vec![abort_bucket],
            )])
            .expect("one crash site per source location");
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
        program
            .facts
            .contract_plans
            .machines
            .push(MachineContractPlan {
                machine: symbol,
                supply_mode: MachineSupplyMode::CheckedBody,
                service_reach: psi_language_semantics::ServiceReachPlan {
                    interface: psi_language_semantics::ServiceReachInterface::PublishedCeiling(
                        service_row,
                    ),
                    checked_inferred: service_row,
                },
                synchronous_invocation: psi_language_semantics::SynchronousInvocationPlan {
                    interface:
                        psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling,
                    published: vec!["parameter:0".to_owned()],
                    checked_inferred: vec!["parameter:0".to_owned()],
                },
                suspension: SuspensionPlan {
                    interface: SuspensionInterface::PublishedMaySuspend(false),
                    checked_may_suspend: false,
                },
                blocking: BlockingPlan {
                    interface: BlockingInterface::PublishedMayBlock(true),
                    checked_may_block: true,
                },
                crash,
                termination: psi_language_semantics::TerminationInterface::Published(
                    TerminationGuarantee::NoGuarantee,
                ),
                inferred_write_frames: Vec::new(),
                fingerprint: 0x1234,
            });
        program
            .facts
            .termination
            .machines
            .push(MachineTerminationFact {
                machine: symbol,
                checked_summary: TerminationGuarantee::Terminates {
                    premises: Vec::new(),
                },
                resolved_view_path: "Nat::Descending".to_string(),
            });

        let json = machine_contract_manifest_json(&program);
        let contract_start = json.find("\"contract\"").expect("contract object");
        let implementation_start = json
            .find("\"implementation\"")
            .expect("implementation object");
        let contract = &json[contract_start..implementation_start];

        assert!(contract.contains("\"fingerprint\": \"0x0000000000001234\""));
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
            "\"crashes\": {\"interface\": \"published_ceiling\", \"buckets\": [{\"cause\": \"Abort\", \"containment_demand\": \"ExecutionDomain\", \"alternative_guards\": [\"true\"]}]}"
        ));
        assert!(contract.contains(
            "\"termination\": {\"interface\": \"published\", \"guarantee\": {\"kind\": \"no_guarantee\"}}"
        ));
        assert!(!contract.contains("inferred_write_frames"));
        assert!(!contract.contains("remaining"));
        assert!(json[implementation_start..].contains("\"inferred_write_frames\": []"));
        assert!(json[implementation_start..].contains(
            "\"checked_crash_sites\": [\n          {\"state\": \"entry\", \"statement_ordinal\": 4, \"cause\": \"Abort\", \"guard_covering_buckets\": [1]}"
        ));
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
        let mut program = CheckedTrees::default();
        program.typed.push_machine(Machine {
            symbol,
            name: Identifier::generated("accepted_map"),
            supply_mode: MachineSupplyMode::Accepted,
            ..Default::default()
        });
        program
            .typed
            .machine_specializations
            .push(MachineSpecialization {
                template: symbol,
                instance: symbol,
                type_arguments: vec!["Card".to_owned()],
                const_arguments: Vec::new(),
                machine_arguments: vec![SymbolHandle::from_arena_index(8)],
                template_contract_fingerprint: 0x1111,
                accepted_template_commitment: Some("accepted_map".to_owned()),
                machine_argument_contract_fingerprints: vec![0x2222],
                fingerprint: 0x3333,
            });

        let json = machine_contract_manifest_json(&program);
        assert!(json.contains("\"template\": \"accepted_map\""));
        assert!(json.contains("\"accepted_template_commitment\": \"accepted_map\""));
        assert!(json.contains("\"template_contract_fingerprint\": \"0x0000000000001111\""));
        assert!(
            json.contains("\"machine_argument_contract_fingerprints\": [\"0x0000000000002222\"]")
        );
        assert!(json.contains("\"instance_fingerprint\": \"0x0000000000003333\""));
    }
}
