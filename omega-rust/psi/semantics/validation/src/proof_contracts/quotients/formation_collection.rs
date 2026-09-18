//! Collecting validated quotient formations and rejecting operation
//! requests.

use crate::proof_contracts::quotients::ValidatedQuotientFormation;
use crate::proof_contracts::quotients::carrier_fence::{
    CarrierFenceViolation, first_forbidden_carrier_content,
};
use crate::proof_contracts::quotients::equivalence_selection::{
    base_data_symbol, validate_equivalence_selection,
};
use crate::proof_contracts::quotients::relation_plan;
use diagnostics::Diagnostic;
use std::collections::HashSet;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::proof_only::ProofOnlyClassification;

pub(crate) fn collect_validated_quotient_formations(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedQuotientFormation> {
    let mut formations = Vec::new();
    for definition in program.data_definitions() {
        let Some(quotient) = &definition.quotient else {
            continue;
        };
        let diagnostics_before = diagnostics.len();
        let relation_name = quotient
            .relation
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");

        let Some(carrier_symbol) = base_data_symbol(program, quotient.carrier) else {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` carrier `{}` must name a data type or generic data family",
                definition.name,
                program.display_type_reference_with_constraints(quotient.carrier),
            )));
            continue;
        };
        let Some(carrier) = program
            .data_definitions()
            .iter()
            .find(|candidate| candidate.symbol == carrier_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` has unknown carrier `{}`",
                definition.name,
                program.display_type_reference_with_constraints(quotient.carrier),
            )));
            continue;
        };
        if !proof_only.is_proof_only(carrier.symbol) {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` carrier `{}` has a runtime layout; quotient carriers must be proof-only",
                definition.name, carrier.name,
            )));
        }
        if let Some(forbidden) = first_forbidden_carrier_content(
            program,
            proof_only,
            quotient.carrier,
            &mut HashSet::new(),
        ) {
            diagnostics.push(Diagnostic::error(match forbidden {
                CarrierFenceViolation::NonCopyType(forbidden) => format!(
                    "quotient data `{}` carrier `{}` contains non-copy Type content `{forbidden}`; the initial quotient surface cannot identify affine or linear occurrences",
                    definition.name, carrier.name,
                ),
                CarrierFenceViolation::RoutedQualification(forbidden) => format!(
                    "quotient data `{}` carrier `{}` contains routed qualification `{forbidden}`; the initial quotient surface cannot identify exact custody occurrences",
                    definition.name, carrier.name,
                ),
            }));
        }

        let Some(relation) = program
            .propositions()
            .iter()
            .find(|relation| relation.symbol == quotient.relation_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` relation `{relation_name}` must resolve to one exact proposition family",
                definition.name,
            )));
            continue;
        };
        let parameters = program.proposition_parameters(relation);
        let signature_matches = parameters.len() == 2
            && parameters.iter().all(|parameter| {
                base_data_symbol(program, parameter.type_reference) == Some(carrier_symbol)
            });
        if !signature_matches {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` relation `{relation_name}` must be one proposition family over exactly two `{}` carrier values",
                definition.name, carrier.name,
            )));
            continue;
        }

        validate_equivalence_selection(
            program,
            definition,
            quotient,
            relation,
            carrier_symbol,
            diagnostics,
        );
        if diagnostics.len() == diagnostics_before {
            formations.push(ValidatedQuotientFormation {
                data_symbol: definition.symbol,
                carrier: quotient.carrier,
                relation_symbol: relation.symbol,
            });
        }
    }
    formations
}

/// Reject every retained sealed request while deriving the exact relation plan
/// for the one shape whose owner/result context is already unambiguous: a
/// direct state-terminal request. This is a non-authoritative prerequisite;
/// deriving `RA`/`RR` here does not admit execution or create a checked-tree
/// operation.
pub(crate) fn reject_quotient_operation_requests(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !program
        .expression_table
        .iter_expressions()
        .any(|(_, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.quotient_operation.is_some())
        })
    {
        return;
    }
    // Reuse the shared whole-call-graph inference. Quotient requests still
    // reject before checked lowering, so this is the one authoritative effect
    // computation on that path rather than a local expression walk.
    let operational = crate::infer_operational_may(program);
    let service_reaches = crate::infer_service_reaches(program, &operational);
    let mut planned_requests = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let Some(result_root) = relation_plan::fallthrough_result_root(program, state) else {
                continue;
            };
            let ExpressionNode::Call(call) = program
                .expression_table
                .expression(result_root.request_expression)
            else {
                continue;
            };
            let Some(request) = &call.quotient_operation else {
                continue;
            };
            planned_requests.push(result_root.request_expression);
            let operation = operation_name(request.kind);
            match relation_plan::derive_direct_terminal_plan(program, machine, state, call, request)
            {
                Ok(plan) => {
                    let congruence = &plan.theorem_evidence[0];
                    let representative_purity = relation_plan::pure_representative_effect(
                        &plan.representative,
                        &operational,
                        &service_reaches,
                    );
                    let complete_result_flow = relation_plan::complete_single_state_result_flow(
                        program,
                        machine,
                        state,
                        result_root,
                    );
                    let complete_forwarded_result_flow = complete_result_flow
                        .is_none()
                        .then(|| {
                            relation_plan::complete_state_forwarding_result_flow(
                                program,
                                machine,
                                state,
                                result_root,
                            )
                        })
                        .flatten();
                    // Compose the exact canonical Terminal row this plan's own
                    // certificate licenses. Ordinary validation rederives the
                    // complete congruence/transport join here rather than
                    // listing it as an assumed outstanding obligation; the
                    // reconstruction stays diagnostic and grants no execution
                    // authority.
                    let canonical_correspondence = complete_result_flow
                        .zip(representative_purity)
                        .map(|(result_flow, purity)| {
                            relation_plan::canonical_direct_correspondence(
                                program,
                                machine,
                                state,
                                result_root.request_expression,
                                &plan,
                                purity,
                                result_flow,
                            )
                        });
                    let canonical_join = match &canonical_correspondence {
                        Some(Ok(_)) => {
                            " plus rederived canonical Terminal correspondence".to_owned()
                        }
                        Some(Err(reason)) => {
                            format!(" (canonical Terminal correspondence unavailable: {reason})")
                        }
                        None => String::new(),
                    };
                    let correspondence = plan
                        .render_define_correspondence()
                        .or_else(|| plan.render_direct_lift_correspondence())
                        .map(|value| format!(" plus exact {value}"))
                        .unwrap_or_default();
                    let precondition = plan
                        .render_representative_precondition()
                        .map(|value| format!(" plus exact {value}"))
                        .unwrap_or_default();
                    let public_precondition = plan
                        .render_public_precondition()
                        .map(|value| format!(" plus exact {value}"))
                        .unwrap_or_default();
                    let precondition_correspondence = plan
                        .render_define_precondition_correspondence()
                        .or_else(|| plan.render_direct_lift_precondition_implication())
                        .map(|value| format!(" plus exact {value}"))
                        .unwrap_or_default();
                    let fixed_call_preconditions = plan
                        .render_fixed_representative_call_preconditions()
                        .map(|value| format!(" plus discharged {value}"))
                        .unwrap_or_default();
                    let correspondence_certificate = plan
                        .render_correspondence_certificate()
                        .map(|value| format!(" plus composed non-executable {value}"))
                        .unwrap_or_default();
                    let termination = plan
                        .render_representative_termination()
                        .map(|value| format!(" plus checked {value}"))
                        .unwrap_or_default();
                    let purity = representative_purity
                        .map(|_| " plus checked pure representative effect summary")
                        .unwrap_or_default();
                    let theorem = format!(" plus exact {}", plan.render_selected_theorem(program));
                    let transport = plan
                        .render_selected_transport(program)
                        .map(|value| format!(" plus exact {value}"))
                        .unwrap_or_default();
                    let theorem_schema = match &plan.theorem_schema_verification {
                        Ok(_) => format!(
                            " plus verified exact {}",
                            plan.render_expected_theorem_schema()
                        ),
                        Err(reason) => format!(
                            " plus expected exact {} (verification failed: {reason})",
                            plan.render_expected_theorem_schema()
                        ),
                    };
                    let theorem_termination = congruence
                        .termination
                        .map(|_| " plus checked theorem termination summary")
                        .unwrap_or_default();
                    let theorem_purity = congruence
                        .purity
                        .map(|_| " plus checked pure theorem effect summary")
                        .unwrap_or_default();
                    let theorem_crash = if congruence.crash_free {
                        " plus checked crash-free theorem routes"
                    } else {
                        Default::default()
                    };
                    let transport_schema = plan
                        .render_transport_schema_verification()
                        .map(|value| format!(" plus {value}"))
                        .unwrap_or_default();
                    let selected_transport = plan.theorem_evidence.get(1);
                    let transport_termination = selected_transport
                        .and_then(|transport| transport.termination)
                        .map(|_| " plus checked forward-transport termination summary")
                        .unwrap_or_default();
                    let transport_purity = selected_transport
                        .and_then(|transport| transport.purity)
                        .map(|_| " plus checked pure forward-transport effect summary")
                        .unwrap_or_default();
                    let transport_crash =
                        if selected_transport.is_some_and(|transport| transport.crash_free) {
                            " plus checked crash-free forward-transport routes"
                        } else {
                            Default::default()
                        };
                    let result_path = if result_root.alias_count == 0 {
                        "the exact result root".to_owned()
                    } else {
                        format!(
                            "{} exact immutable result alias{}",
                            result_root.alias_count,
                            if result_root.alias_count == 1 {
                                ""
                            } else {
                                "es"
                            },
                        )
                    };
                    let result_flow = if complete_result_flow.is_some() {
                        format!(
                            "complete transition-free single-state normal-result coverage through {result_path}"
                        )
                    } else if complete_forwarded_result_flow.is_some() {
                        format!(
                            "complete finite state-forwarded normal-result coverage through {result_path}"
                        )
                    } else {
                        format!("one unchanged state-fallthrough result edge through {result_path}")
                    };
                    let plan_kind = if result_root.alias_count == 0 {
                        "direct-terminal"
                    } else {
                        "immutable-alias fallthrough"
                    };
                    let mut remaining = Vec::new();
                    if plan.correspondence_certificate.is_none() {
                        remaining.push("complete operation/static correspondence".to_owned());
                    }
                    if plan.theorem_schema_verification.is_err() {
                        remaining.push("exact selected theorem schema verification".to_owned());
                    }
                    if representative_purity.is_none() {
                        remaining.push("the effect fence".to_owned());
                    }
                    if termination.is_empty() {
                        remaining.push("the termination fence".to_owned());
                    }
                    if congruence.termination.is_none() {
                        remaining.push("the selected theorem termination fence".to_owned());
                    }
                    if congruence.purity.is_none() {
                        remaining.push("the selected theorem effect fence".to_owned());
                    }
                    if !congruence.crash_free {
                        remaining.push("the selected theorem crash fence".to_owned());
                    }
                    if let Some(transport) = selected_transport {
                        if transport.termination.is_none() {
                            remaining.push(
                                "the selected forward-transport termination fence".to_owned(),
                            );
                        }
                        if transport.purity.is_none() {
                            remaining
                                .push("the selected forward-transport effect fence".to_owned());
                        }
                        if !transport.crash_free {
                            remaining.push("the selected forward-transport crash fence".to_owned());
                        }
                        if plan
                            .transport_schema_verification
                            .as_ref()
                            .is_none_or(|verification| verification.is_err())
                        {
                            remaining.push(
                                "exact selected forward-transport schema verification".to_owned(),
                            );
                        }
                    }
                    if complete_result_flow.is_none() && complete_forwarded_result_flow.is_none() {
                        remaining.push("all normalized result exits".to_owned());
                    }
                    // A composed certificate already carries the whole Q => P
                    // lane: the automatic exact/arithmetic rung for
                    // `lift<F, Congruence>` and the selected theorem for
                    // `lift<F, Congruence, Transport>`. Only an absent
                    // certificate leaves the general implication and adapted
                    // argument judgments outstanding.
                    if request.kind == typed_trees::expression::QuotientOperationKind::Lift
                        && plan.correspondence_certificate.is_none()
                    {
                        remaining.push(
                            "general precondition implication and adapted lift arguments"
                                .to_owned(),
                        );
                    }
                    if plan.has_undischarged_fixed_representative_preconditions() {
                        remaining.push("fixed representative call obligations".to_owned());
                    }
                    if !matches!(canonical_correspondence, Some(Ok(_))) {
                        remaining.push("canonical Terminal correspondence".to_owned());
                    }
                    let fence = if remaining.is_empty() {
                        "but executable quotient operations are not admitted until executable quotient lowering exists".to_owned()
                    } else {
                        format!(
                            "but executable quotient operations are not admitted until {} are independently checked",
                            remaining.join(", "),
                        )
                    };
                    diagnostics.push(Diagnostic::error(format!(
                        "`Quotient::{operation}` has compiler-derived {plan_kind} relations {} and {} plus exact representative telescope {}{termination}{purity}{theorem}{theorem_schema}{theorem_termination}{theorem_purity}{theorem_crash}{transport}{transport_schema}{transport_termination}{transport_purity}{transport_crash}{correspondence}{public_precondition}{precondition}{precondition_correspondence}{fixed_call_preconditions}{correspondence_certificate}{canonical_join} and {result_flow}, {fence}",
                        plan.render_ra(program),
                        plan.render_rr(program),
                        plan.render_representative_telescope(program),
                    )))
                }
                Err(reason) => {
                    let implication = relation_plan::render_failed_builtin_implication(
                        program, machine, state, call, request, reason,
                    )
                    .map(|context| format!("; {context}"))
                    .unwrap_or_default();
                    diagnostics.push(Diagnostic::error(format!(
                        "`Quotient::{operation}` retains its exact representative operation and selected theorem machine application, but its direct-terminal relation plan is unresolved ({reason}){implication}; executable quotient operations are not admitted",
                    )));
                }
            }
        }
    }

    // Preserve the global fail-closed fence for nested, contract-only, or
    // otherwise ownerless table expressions. Their context is deliberately not
    // guessed from arena proximity.
    for (handle, expression) in program.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = expression else {
            continue;
        };
        let Some(request) = &call.quotient_operation else {
            continue;
        };
        if !planned_requests.contains(&handle) {
            let operation = operation_name(request.kind);
            diagnostics.push(Diagnostic::error(format!(
                "`Quotient::{operation}` retains its exact representative operation and selected theorem machine application, but executable quotient operations are not admitted until quotient formation, theorem-schema, correspondence, and result-flow obligations are independently checked",
            )));
        }
    }
}

fn operation_name(kind: typed_trees::expression::QuotientOperationKind) -> &'static str {
    match kind {
        typed_trees::expression::QuotientOperationKind::Lift => "lift",
        typed_trees::expression::QuotientOperationKind::Define => "define",
    }
}
