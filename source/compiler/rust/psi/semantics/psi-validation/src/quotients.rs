//! N6 proof quotient migration boundary.
//!
//! Quotient formation selects one exact proposition relation and one exact
//! named `Equivalence` conformance from the declaration's static `where`
//! surface. Structural law-machine discovery is not an authority path.
//!
//! Sealed `Quotient::lift`/`define` requests retain their exact source-selected
//! operation and conformance identities, but this module rejects executable
//! admission until formation, correspondence, and contract obligations are
//! checked. Bare representative calls never discover structural respect proof
//! machines and never acquire lift authority.

use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionNode;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::proof_only::ProofOnlyClassification;
use psi_typed_trees::state::State;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use std::collections::HashSet;

mod carrier_fence;
mod relation_plan;

use carrier_fence::{CarrierFenceViolation, first_forbidden_carrier_content};

pub(crate) fn type_has_forbidden_denotational_content(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    type_reference: TypeReferenceHandle,
) -> bool {
    first_forbidden_carrier_content(program, proof_only, type_reference, &mut HashSet::new())
        .is_some()
}

const CORE_EQUIVALENCE_SOURCE: &str = "relation.omg";

pub(crate) fn validate_quotients(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
) {
    reject_quotient_operation_requests(program, diagnostics);

    for definition in program.data_definitions() {
        let Some(quotient) = &definition.quotient else {
            continue;
        };
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
    }
}

/// Reject every retained sealed request while deriving the exact relation plan
/// for the one shape whose owner/result context is already unambiguous: a
/// direct state-terminal request. This is a non-authoritative prerequisite;
/// deriving `RA`/`RR` here does not admit execution or create a checked-tree
/// operation.
fn reject_quotient_operation_requests(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
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
    let operational = psi_effects::infer_operational_may(program);
    let service_reaches = psi_effects::infer_service_reaches(program, &operational);
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
            match relation_plan::derive_direct_terminal_plan(
                program, machine, state, call, request,
            ) {
                Ok(plan) => {
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
                    let complete_forwarded_result_flow = complete_result_flow.is_none().then(|| {
                        relation_plan::complete_state_forwarding_result_flow(
                            program,
                            machine,
                            state,
                            result_root,
                        )
                    }).flatten();
                    let correspondence = plan
                        .render_define_correspondence()
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
                        .map(|value| format!(" plus exact {value}"))
                        .unwrap_or_default();
                    let termination = plan
                        .render_representative_termination()
                        .map(|value| format!(" plus checked {value}"))
                        .unwrap_or_default();
                    let purity = representative_purity
                        .map(|_| " plus checked pure representative effect summary")
                        .unwrap_or_default();
                    let theorem = format!(
                        " plus exact {}",
                        plan.render_selected_theorem(program)
                    );
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
                    let theorem_termination = plan
                        .selected_theorem_termination
                        .map(|_| " plus checked theorem termination summary")
                        .unwrap_or_default();
                    let theorem_purity = plan
                        .selected_theorem_purity
                        .map(|_| " plus checked pure theorem effect summary")
                        .unwrap_or_default();
                    let theorem_crash = plan
                        .selected_theorem_crash_free
                        .then_some(" plus checked crash-free theorem routes")
                        .unwrap_or_default();
                    let result_path = if result_root.alias_count == 0 {
                        "the exact result root".to_owned()
                    } else {
                        format!(
                            "{} exact immutable result alias{}",
                            result_root.alias_count,
                            if result_root.alias_count == 1 { "" } else { "es" },
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
                    let mut remaining = vec!["complete operation/static correspondence".to_owned()];
                    if plan.theorem_schema_verification.is_err() {
                        remaining.push("exact selected theorem schema verification".to_owned());
                    }
                    if representative_purity.is_none() {
                        remaining.push("the effect fence".to_owned());
                    }
                    if termination.is_empty() {
                        remaining.push("the termination fence".to_owned());
                    }
                    if plan.selected_theorem_termination.is_none() {
                        remaining.push("the selected theorem termination fence".to_owned());
                    }
                    if plan.selected_theorem_purity.is_none() {
                        remaining.push("the selected theorem effect fence".to_owned());
                    }
                    if !plan.selected_theorem_crash_free {
                        remaining.push("the selected theorem crash fence".to_owned());
                    }
                    if complete_result_flow.is_none() && complete_forwarded_result_flow.is_none() {
                        remaining.push("all normalized result exits".to_owned());
                    }
                    diagnostics.push(Diagnostic::error(format!(
                        "`Quotient::{operation}` has compiler-derived {plan_kind} relations {} and {} plus exact representative telescope {}{termination}{purity}{theorem}{theorem_schema}{theorem_termination}{theorem_purity}{theorem_crash}{correspondence}{public_precondition}{precondition}{precondition_correspondence} and {result_flow}, but executable quotient operations are not admitted until {} are independently checked",
                        plan.render_ra(program),
                        plan.render_rr(program),
                        plan.render_representative_telescope(program),
                        remaining.join(", "),
                    )))
                }
                Err(reason) => diagnostics.push(Diagnostic::error(format!(
                    "`Quotient::{operation}` retains its exact representative operation and selected theorem machine application, but its direct-terminal relation plan is unresolved ({reason}); executable quotient operations are not admitted",
                ))),
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

fn operation_name(kind: psi_typed_trees::expression::QuotientOperationKind) -> &'static str {
    match kind {
        psi_typed_trees::expression::QuotientOperationKind::Lift => "lift",
        psi_typed_trees::expression::QuotientOperationKind::Define => "define",
    }
}

fn validate_equivalence_selection(
    program: &TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
    quotient: &psi_typed_trees::data::QuotientDefinition,
    relation: &psi_typed_trees::proposition::PropositionDefinition,
    carrier_symbol: SymbolHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(selection) = &quotient.equivalence else {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` requires one exact named `Equivalence` conformance in its static `where R satisfies Equivalence<C, R> as Name` surface",
            definition.name,
        )));
        return;
    };
    if selection.relation_symbol != quotient.relation_symbol
        || selection.relation != quotient.relation
    {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` selects equivalence evidence for `{}` instead of its exact relation `{}`",
            definition.name,
            selection
                .relation
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::"),
            quotient
                .relation
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::"),
        )));
    }
    if !is_sealed_core_equivalence(program, selection.trait_symbol, &selection.trait_name) {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` static selection must resolve the sealed toolchain `Equivalence` declaration from `{CORE_EQUIVALENCE_SOURCE}`; an authored lookalike is not equivalence authority",
            definition.name
        )));
        return;
    }
    let Some(trait_definition) = program
        .traits()
        .iter()
        .find(|candidate| candidate.symbol == selection.trait_symbol)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` static selection retains no exact `Equivalence` trait identity",
            definition.name,
        )));
        return;
    };
    if trait_definition.is_boundary {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` cannot use boundary trait `{}` as equivalence authority",
            definition.name, trait_definition.name,
        )));
    }

    let selection_arguments = program
        .type_reference_table
        .type_reference_handles(selection.trait_arguments);
    let [selected_carrier, selected_relation] = selection_arguments else {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` must select `Equivalence<C, R>` with exactly its carrier and relation arguments",
            definition.name,
        )));
        return;
    };
    if program.normalized_type_identity(*selected_carrier)
        != program.normalized_type_identity(quotient.carrier)
        || type_reference_symbol(program, *selected_relation) != Some(relation.symbol)
    {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` selected `Equivalence` arguments do not exactly match carrier `{}` and relation `{}`",
            definition.name,
            program.display_type_reference(quotient.carrier),
            relation.name,
        )));
    }

    let Some(conformance) = program
        .conformances()
        .iter()
        .find(|candidate| candidate.symbol == selection.conformance_symbol)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` names unresolved equivalence conformance `{}`",
            definition.name, selection.conformance_name,
        )));
        return;
    };
    if conformance.alias.as_ref() != Some(&selection.conformance_name)
        || conformance.trait_name != selection.trait_name
        || !conformance.lifetime_parameters.is_empty()
        || !program.conformance_type_parameters(conformance).is_empty()
    {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` must select one exact closed nongeneric conformance named `{}` for `Equivalence`",
            definition.name, selection.conformance_name,
        )));
        return;
    }
    if !matches!(
        conformance.subject,
        psi_typed_trees::trait_definition::ConformanceSubject::Subjectless
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` conformance `{}` must be carrierless proof evidence for its exact relation `{}`",
            definition.name, selection.conformance_name, relation.name
        )));
    }
    let conformance_arguments = program
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    if conformance_arguments.len() != selection_arguments.len()
        || conformance_arguments
            .iter()
            .zip(selection_arguments)
            .any(|(actual, selected)| {
                program.normalized_type_identity(*actual)
                    != program.normalized_type_identity(*selected)
            })
    {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` conformance `{}` does not implement the exact selected `Equivalence<C, R>` application",
            definition.name, selection.conformance_name,
        )));
    }
    let Some(rows) = program.closed_conformance_rows(conformance) else {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` conformance `{}` must be one closed implementation; attached structural satisfier discovery is not permitted",
            definition.name, selection.conformance_name,
        )));
        return;
    };
    for row in rows {
        let Some(declaring_trait) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == row.declaring_trait)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` conformance `{}` row `{}::{}` has no exact declaring trait",
                definition.name,
                selection.conformance_name,
                row.declaring_trait_name,
                row.requirement_name,
            )));
            continue;
        };
        let Some(requirement) = program
            .trait_machine_signatures(declaring_trait)
            .iter()
            .find(|candidate| candidate.symbol == row.requirement)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` conformance `{}` row `{}::{}` has no exact inherited requirement",
                definition.name,
                selection.conformance_name,
                row.declaring_trait_name,
                row.requirement_name,
            )));
            continue;
        };
        let Some(declaring_arguments) = crate::traits::arguments_for_declaring_trait(
            program,
            trait_definition,
            selection_arguments,
            row.declaring_trait,
            &mut Vec::new(),
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` conformance `{}` cannot instantiate inherited row `{}::{}`",
                definition.name,
                selection.conformance_name,
                row.declaring_trait_name,
                row.requirement_name,
            )));
            continue;
        };
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == row.realization_machine)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` conformance `{}` row `{}::{}` has no exact realization",
                definition.name,
                selection.conformance_name,
                row.declaring_trait_name,
                row.requirement_name,
            )));
            continue;
        };
        crate::contract_entailment::check_law_conformance(
            program,
            machine,
            Some(selection.conformance_name.as_str()),
            declaring_trait,
            requirement,
            &declaring_arguments,
            diagnostics,
        );
        if !row_has_exact_equivalence_premises(
            program,
            machine,
            relation,
            row.requirement_name.as_str(),
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` conformance `{}` row `{}::{}` strengthens or changes the sealed equivalence law premises; quotient formation requires the exact inherited contract",
                definition.name,
                selection.conformance_name,
                row.declaring_trait_name,
                row.requirement_name,
            )));
        }
        let mut visited = HashSet::new();
        if let Err(admitted) = checked_proof_dependency(program, machine, &mut visited) {
            diagnostics.push(Diagnostic::error(format!(
                "quotient data `{}` conformance `{}` depends on admitted or boundary proof machine `{admitted}` through row `{}::{}`; admitted evidence cannot license `%`",
                definition.name,
                selection.conformance_name,
                row.declaring_trait_name,
                row.requirement_name,
            )));
        }
    }

    if base_data_symbol(program, quotient.carrier) != Some(carrier_symbol) {
        diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` changed carrier identity while checking equivalence selection",
            definition.name,
        )));
    }
}

fn row_has_exact_equivalence_premises(
    program: &TypedTrees,
    machine: &Machine,
    relation: &psi_typed_trees::proposition::PropositionDefinition,
    requirement: &str,
) -> bool {
    let expected: &[(usize, usize)] = match requirement {
        "reflexive" => &[],
        "symmetric" => &[(0, 1)],
        "transitive" => &[(0, 1), (1, 2)],
        _ => return false,
    };
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let parameters = program.state_parameters(entry);
    let facts = program
        .machine_contracts(machine)
        .iter()
        .filter(|contract| {
            contract.kind == psi_typed_trees::signature::SignatureContractKind::Requires
        })
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .collect::<Vec<_>>();
    facts.len() == expected.len()
        && facts.iter().zip(expected).all(|(fact, (left, right))| {
            let (Some(left), Some(right)) = (parameters.get(*left), parameters.get(*right)) else {
                return false;
            };
            fact_is_exact_relation_pair(program, fact, relation, left, right, parameters)
        })
}

fn fact_is_exact_relation_pair(
    program: &TypedTrees,
    fact: &psi_typed_trees::domain::ProofFact,
    relation: &psi_typed_trees::proposition::PropositionDefinition,
    left: &psi_typed_trees::signature::StateParameter,
    right: &psi_typed_trees::signature::StateParameter,
    parameters: &[psi_typed_trees::signature::StateParameter],
) -> bool {
    match fact {
        psi_typed_trees::domain::ProofFact::Proposition(application) => {
            exact_relation_application_matches(
                program,
                application,
                relation.symbol,
                left.symbol,
                right.symbol,
                left.type_reference,
                right.type_reference,
            )
        }
        psi_typed_trees::domain::ProofFact::Expression(actual) => {
            let psi_typed_trees::proposition::PropositionBody::Transparent {
                proposition:
                    psi_typed_trees::proposition::PropositionFormula::BooleanExpression(expected),
            } = relation.body
            else {
                return false;
            };
            let relation_parameters = program.proposition_parameters(relation);
            let [relation_left, relation_right] = relation_parameters else {
                return false;
            };
            let expected = program.render_proof_expression_with_parameters(
                expected,
                &[
                    (
                        relation_left.symbol,
                        relation_left.name.as_str().to_owned(),
                        "$left".to_owned(),
                    ),
                    (
                        relation_right.symbol,
                        relation_right.name.as_str().to_owned(),
                        "$right".to_owned(),
                    ),
                ],
            );
            let substitutions = parameters
                .iter()
                .map(|parameter| {
                    let replacement = if parameter.symbol == left.symbol {
                        "$left"
                    } else if parameter.symbol == right.symbol {
                        "$right"
                    } else {
                        "$other"
                    };
                    (
                        parameter.symbol,
                        parameter.name.as_str().to_owned(),
                        replacement.to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            expected == program.render_proof_expression_with_parameters(*actual, &substitutions)
        }
        psi_typed_trees::domain::ProofFact::Membership(_) => false,
    }
}

pub(crate) fn exact_relation_application_matches(
    program: &TypedTrees,
    application: &psi_typed_trees::proposition::PropositionApplication,
    relation_symbol: SymbolHandle,
    left_symbol: SymbolHandle,
    right_symbol: SymbolHandle,
    left_type: TypeReferenceHandle,
    right_type: TypeReferenceHandle,
) -> bool {
    let Some(relation) = program
        .propositions()
        .iter()
        .find(|relation| relation.symbol == relation_symbol)
    else {
        return false;
    };
    if application.proposition != relation.symbol
        || !matches!(
            program.expression_table.expression_handles(application.arguments),
            [left_expression, right_expression]
                if expression_is_symbol(program, *left_expression, left_symbol)
                    && expression_is_symbol(program, *right_expression, right_symbol)
        )
    {
        return false;
    }
    let Some(mut expected_binders) = exact_generic_argument_symbols(program, left_type) else {
        return false;
    };
    let Some(right_binders) = exact_generic_argument_symbols(program, right_type) else {
        return false;
    };
    expected_binders.extend(right_binders);
    let declared_binders = program.proposition_binders(relation);
    application.binder_arguments.len() == declared_binders.len()
        && expected_binders.len() == declared_binders.len()
        && application
            .binder_arguments
            .iter()
            .zip(declared_binders)
            .zip(expected_binders)
            .all(|((actual, declared), expected)| {
                actual.symbol == expected
                    && matches!(
                        (actual.kind, &declared.kind),
                        (
                            psi_typed_trees::proposition::PropositionBinderArgumentKind::Type,
                            psi_typed_trees::proposition::PropositionBinderKind::Type,
                        ) | (
                            psi_typed_trees::proposition::PropositionBinderArgumentKind::Const,
                            psi_typed_trees::proposition::PropositionBinderKind::Const { .. },
                        ) | (
                            psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine,
                            psi_typed_trees::proposition::PropositionBinderKind::Machine,
                        )
                    )
            })
}

fn exact_generic_argument_symbols(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<Vec<SymbolHandle>> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .map(|argument| type_reference_symbol(program, *argument))
            .collect(),
        TypeReferenceNode::Reference { referee, .. } => {
            exact_generic_argument_symbols(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            exact_generic_argument_symbols(program, *base_type)
        }
        TypeReferenceNode::Named { .. } => Some(Vec::new()),
        _ => None,
    }
}

fn expression_is_symbol(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(path) if symbol.is_valid() && path.symbol == symbol
    )
}

fn is_sealed_core_equivalence(
    program: &TypedTrees,
    symbol: SymbolHandle,
    name: &psi_typed_trees::name::Identifier,
) -> bool {
    if !symbol.is_valid() || name.as_str() != "Equivalence" {
        return false;
    }
    let Some(span) = program.symbols.symbol_source_span(symbol) else {
        return false;
    };
    let Some(source) = program.symbols.source_file(span) else {
        return false;
    };
    let source_matches = source.origin == psi_source::SourceOrigin::Toolchain
        && source
            .path
            .strip_prefix(&source.package_root)
            .ok()
            .is_some_and(|relative| relative == std::path::Path::new(CORE_EQUIVALENCE_SOURCE));
    let Some(definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == symbol)
    else {
        return false;
    };
    let parameters = program.trait_type_parameters(definition);
    let parameter_shape = matches!(
        parameters,
        [carrier, relation]
            if matches!(carrier.kind, psi_typed_trees::data::TypeParameterKind::Type)
                && matches!(relation.kind, psi_typed_trees::data::TypeParameterKind::Proposition { .. })
    );
    let mut parent_names = program
        .trait_requirements(definition)
        .iter()
        .filter_map(|parent| {
            program
                .traits()
                .iter()
                .find(|candidate| candidate.symbol == parent.symbol)
                .map(|parent| parent.name.as_str())
        })
        .collect::<Vec<_>>();
    parent_names.sort_unstable();
    source_matches && parameter_shape && parent_names == ["Reflexive", "Symmetric", "Transitive"]
}

fn type_reference_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
        TypeReferenceNode::Reference { referee, .. } => type_reference_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_symbol(program, *base_type)
        }
        _ => None,
    }
}

fn checked_proof_dependency<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    visited: &mut HashSet<u32>,
) -> Result<(), &'program str> {
    if !machine.supply_mode.is_checked_body() {
        return Err(machine.name.as_str());
    }
    if !visited.insert(machine.symbol.arena_index()) {
        return Ok(());
    }
    for dependency in crate::call_cycles::machine_call_dependency_symbols(program, machine) {
        let Some(callee) = program.machines().iter().find(|candidate| {
            candidate.symbol == dependency
                || program
                    .machine_states(candidate)
                    .iter()
                    .any(|state| state.symbol == dependency)
        }) else {
            continue;
        };
        if callee.symbol != machine.symbol {
            checked_proof_dependency(program, callee, visited)?;
        }
    }
    Ok(())
}

fn base_data_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
        TypeReferenceNode::Reference { referee, .. } => base_data_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => base_data_symbol(program, *base_type),
        _ => None,
    }
}

/// A bare representative call whose operands happen to be quotient values.
/// This shape is retained only to replace generic nominal mismatch cascades
/// with the settled explicit-wrapper diagnostic. It carries no admission.
pub(crate) struct LegacyQuotientCallCandidate<'program> {
    pub(crate) quotient: &'program psi_typed_trees::data::DataDefinition,
    pub(crate) operation: &'program Machine,
}

pub(crate) fn legacy_quotient_call_candidate<'program>(
    program: &'program TypedTrees,
    receiver_type: Option<TypeReferenceHandle>,
    argument_types: &[Option<TypeReferenceHandle>],
    state: &'program State,
) -> Option<LegacyQuotientCallCandidate<'program>> {
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != argument_types.len() {
        return None;
    }

    let operation = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
    })?;
    let is_attached = operation.attached_data.is_some();
    if is_attached != receiver_type.is_some() {
        return None;
    }

    let first_operand = receiver_type.or_else(|| argument_types.first().copied().flatten())?;
    let quotient = quotient_for_type(program, first_operand)?;
    let quotient_metadata = quotient.quotient.as_ref()?;
    let carrier = base_data_symbol(program, quotient_metadata.carrier)?;
    if base_data_symbol(program, state.return_type) != Some(carrier) {
        return None;
    }
    if let Some(receiver_type) = receiver_type {
        if quotient_for_type(program, receiver_type)?.symbol != quotient.symbol {
            return None;
        }
        let attached_carrier = operation.attached_data.as_ref().and_then(|attached| {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == attached.as_str())
        })?;
        if attached_carrier.symbol != carrier {
            return None;
        }
    }
    for (parameter, argument_type) in parameters.iter().zip(argument_types) {
        if base_data_symbol(program, parameter.type_reference) != Some(carrier) {
            return None;
        }
        let argument_quotient = quotient_for_type(program, (*argument_type)?)?;
        if argument_quotient.symbol != quotient.symbol {
            return None;
        }
    }

    Some(LegacyQuotientCallCandidate {
        quotient,
        operation,
    })
}

/// Identify a bare attached representative call solely for a precise
/// migration diagnostic. This does not resolve the call, validate arguments,
/// inspect proof machines, or grant any lift authority.
pub(crate) fn legacy_attached_quotient_call_candidate<'program>(
    program: &'program TypedTrees,
    receiver_type: TypeReferenceHandle,
    target: &str,
) -> Option<LegacyQuotientCallCandidate<'program>> {
    let quotient = quotient_for_type(program, receiver_type)?;
    let carrier_symbol = base_data_symbol(program, quotient.quotient.as_ref()?.carrier)?;
    let carrier = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == carrier_symbol)?;
    let operation = program.machines().iter().find(|machine| {
        machine
            .attached_data
            .as_ref()
            .is_some_and(|attached| attached.as_str() == carrier.name.as_str())
            && program
                .machine_states(machine)
                .iter()
                .any(|state| state.name.as_str() == target)
    })?;
    Some(LegacyQuotientCallCandidate {
        quotient,
        operation,
    })
}

fn quotient_for_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&psi_typed_trees::data::DataDefinition> {
    let symbol = base_data_symbol(program, type_reference)?;
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol && definition.quotient.is_some())
}

#[cfg(test)]
mod tests {
    use super::{
        CarrierFenceViolation, exact_relation_application_matches, first_forbidden_carrier_content,
        validate_quotients,
    };
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;
    use psi_typed_trees::data::{DataDefinition, DataField, DataMember};
    use psi_typed_trees::domain::{
        DomainAliasConstituent, DomainAliasDefinition, DomainDefinition,
    };
    use psi_typed_trees::expression::{
        ExpressionHandle, ExpressionNode, QuotientOperationKind, QuotientOperationRequest,
        StaticMachineArgument, TableCallExpression, TableNamePath,
    };
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::proposition::{
        PropositionApplication, PropositionBinder, PropositionBinderArgument,
        PropositionBinderArgumentKind, PropositionBinderKind, PropositionDefinition,
    };
    use psi_typed_trees::types::{
        DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };

    fn static_argument(name: &'static str) -> StaticMachineArgument {
        StaticMachineArgument {
            path: vec![Identifier::generated_static(name)].into_boxed_slice(),
            application: None,
            const_literal: None,
            evidence_projection: None,
            symbol: SymbolHandle::invalid(),
        }
    }

    fn recursive_proof_carrier_with(
        contained: Option<(
            SymbolHandle,
            &'static str,
            psi_language_semantics::Multiplicity,
        )>,
    ) -> (TypedTrees, TypeReferenceHandle) {
        let mut program = TypedTrees::default();
        let carrier_symbol = SymbolHandle::from_arena_index(20);
        let carrier_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: carrier_symbol,
                name: Identifier::generated_static("Carrier"),
            });
        let mut carrier = DataDefinition {
            symbol: carrier_symbol,
            name: Identifier::generated_static("Carrier"),
            ..Default::default()
        };
        program.push_data_member(
            &mut carrier,
            DataMember::Field(DataField {
                symbol: SymbolHandle::from_arena_index(21),
                name: Identifier::generated_static("next"),
                type_reference: carrier_type,
                ..Default::default()
            }),
        );
        if let Some((symbol, name, multiplicity)) = contained {
            let contained_type = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated_static(name),
                });
            program.push_data_member(
                &mut carrier,
                DataMember::Field(DataField {
                    symbol: SymbolHandle::from_arena_index(22),
                    name: Identifier::generated_static("payload"),
                    type_reference: contained_type,
                    ..Default::default()
                }),
            );
            program.push_data_definition(DataDefinition {
                symbol,
                name: Identifier::generated_static(name),
                properties: psi_typed_trees::data::DataProperties {
                    multiplicity,
                    carry: None,
                },
                ..Default::default()
            });
        }
        program.push_data_definition(carrier);
        (program, carrier_type)
    }

    fn constrained_copy_type(
        program: &mut TypedTrees,
        domain: DomainConstraint,
    ) -> TypeReferenceHandle {
        let copy_symbol = SymbolHandle::from_arena_index(40);
        let copy_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: copy_symbol,
                name: Identifier::generated_static("CopyValue"),
            });
        program.push_data_definition(DataDefinition {
            symbol: copy_symbol,
            name: Identifier::generated_static("CopyValue"),
            properties: psi_typed_trees::data::DataProperties {
                multiplicity: psi_language_semantics::Multiplicity::Unrestricted,
                carry: None,
            },
            ..Default::default()
        });
        let constraints = program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::Domain(domain)]);
        program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: copy_type,
                constraints,
            })
    }

    fn checked_route() -> psi_language_semantics::DomainEstablishmentRoute {
        psi_language_semantics::DomainEstablishmentRoute::CheckedRequirement {
            trait_definition: SymbolHandle::from_arena_index(90),
            requirement: SymbolHandle::from_arena_index(91),
        }
    }

    #[test]
    fn recursive_proof_carrier_without_runtime_content_passes_noncopy_fence() {
        let (program, carrier) = recursive_proof_carrier_with(None);
        let proof_only = psi_typed_trees::proof_only::classify(&program);

        assert_eq!(
            first_forbidden_carrier_content(
                &program,
                &proof_only,
                carrier,
                &mut std::collections::HashSet::new(),
            ),
            None,
        );
    }

    #[test]
    fn recursive_proof_carrier_rejects_contained_affine_runtime_type() {
        let token_symbol = SymbolHandle::from_arena_index(30);
        let (program, carrier) = recursive_proof_carrier_with(Some((
            token_symbol,
            "Token",
            psi_language_semantics::Multiplicity::Affine,
        )));
        let proof_only = psi_typed_trees::proof_only::classify(&program);

        assert_eq!(
            first_forbidden_carrier_content(
                &program,
                &proof_only,
                carrier,
                &mut std::collections::HashSet::new(),
            ),
            Some(CarrierFenceViolation::NonCopyType("Token".to_owned())),
        );
    }

    #[test]
    fn recursive_proof_carrier_accepts_contained_copy_runtime_type() {
        let token_symbol = SymbolHandle::from_arena_index(31);
        let (program, carrier) = recursive_proof_carrier_with(Some((
            token_symbol,
            "CopyToken",
            psi_language_semantics::Multiplicity::Unrestricted,
        )));
        let proof_only = psi_typed_trees::proof_only::classify(&program);

        assert_eq!(
            first_forbidden_carrier_content(
                &program,
                &proof_only,
                carrier,
                &mut std::collections::HashSet::new(),
            ),
            None,
        );
    }

    #[test]
    fn routed_qualification_on_copy_content_rejects() {
        let mut program = TypedTrees::default();
        let routed = constrained_copy_type(
            &mut program,
            DomainConstraint {
                name: Identifier::generated_static("Issued"),
                establishment_routes: vec![checked_route()],
                ..Default::default()
            },
        );
        let proof_only = psi_typed_trees::proof_only::classify(&program);

        assert_eq!(
            first_forbidden_carrier_content(
                &program,
                &proof_only,
                routed,
                &mut std::collections::HashSet::new(),
            ),
            Some(CarrierFenceViolation::RoutedQualification(
                "Issued".to_owned()
            )),
        );
    }

    #[test]
    fn predicate_only_qualification_on_copy_content_passes() {
        let mut program = TypedTrees::default();
        let predicate_only = constrained_copy_type(
            &mut program,
            DomainConstraint {
                name: Identifier::generated_static("NonZero"),
                predicate_body: psi_language_semantics::DomainPredicateBody::Present,
                ..Default::default()
            },
        );
        let proof_only = psi_typed_trees::proof_only::classify(&program);

        assert_eq!(
            first_forbidden_carrier_content(
                &program,
                &proof_only,
                predicate_only,
                &mut std::collections::HashSet::new(),
            ),
            None,
        );
    }

    #[test]
    fn transparent_alias_cannot_hide_routed_qualification() {
        let mut program = TypedTrees::default();
        let routed_symbol = SymbolHandle::from_arena_index(50);
        let alias_symbol = SymbolHandle::from_arena_index(51);
        program.push_domain_definition(DomainDefinition {
            symbol: routed_symbol,
            name: Identifier::generated_static("Issued"),
            establishment_routes: vec![checked_route()],
            ..Default::default()
        });
        program.push_domain_definition(DomainDefinition {
            symbol: alias_symbol,
            name: Identifier::generated_static("Usable"),
            alias: Some(DomainAliasDefinition {
                constituents: vec![DomainAliasConstituent {
                    domain_symbol: routed_symbol,
                    ..Default::default()
                }],
            }),
            ..Default::default()
        });
        let aliased = constrained_copy_type(
            &mut program,
            DomainConstraint {
                name: Identifier::generated_static("Usable"),
                symbol: alias_symbol,
                ..Default::default()
            },
        );
        let proof_only = psi_typed_trees::proof_only::classify(&program);

        assert_eq!(
            first_forbidden_carrier_content(
                &program,
                &proof_only,
                aliased,
                &mut std::collections::HashSet::new(),
            ),
            Some(CarrierFenceViolation::RoutedQualification(
                "Issued".to_owned()
            )),
        );
    }

    #[test]
    fn retained_sealed_request_is_not_executable_admission() {
        let mut program = TypedTrees::default();
        let arguments = program
            .expression_table
            .insert_expression_handles(std::iter::empty());
        program
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                receiver: ExpressionHandle::invalid(),
                target_symbol: SymbolHandle::invalid(),
                target: Identifier::generated_static("lift"),
                machine_arguments: Box::default(),
                quotient_operation: Some(QuotientOperationRequest {
                    kind: QuotientOperationKind::Lift,
                    representative_operation: static_argument("representative"),
                    selected_theorem: static_argument("ExactRespect"),
                }),
                private_layout_operation: None,
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: Default::default(),
            }));
        let proof_only = psi_typed_trees::proof_only::classify(&program);
        let mut diagnostics = Vec::new();

        validate_quotients(&program, &proof_only, &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("executable quotient operations are not admitted")
        );
    }

    #[test]
    fn quotient_relation_application_rejects_swapped_generic_binders() {
        let mut program = TypedTrees::default();
        let relation_symbol = SymbolHandle::from_arena_index(1);
        let left_symbol = SymbolHandle::from_arena_index(2);
        let right_symbol = SymbolHandle::from_arena_index(3);
        let left_binder = SymbolHandle::from_arena_index(4);
        let right_binder = SymbolHandle::from_arena_index(5);
        let family_symbol = SymbolHandle::from_arena_index(6);

        let mut relation = PropositionDefinition {
            symbol: relation_symbol,
            name: Identifier::generated_static("Related"),
            ..Default::default()
        };
        for (symbol, name) in [(left_binder, "L"), (right_binder, "R")] {
            program.push_proposition_binder(
                &mut relation,
                PropositionBinder {
                    symbol,
                    name: Identifier::generated_static(name),
                    kind: PropositionBinderKind::Machine,
                    ..Default::default()
                },
            );
        }
        program.push_proposition(relation);

        let left_argument = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: left_binder,
                name: Identifier::generated_static("L"),
            });
        let right_argument = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: right_binder,
                name: Identifier::generated_static("R"),
            });
        let left_arguments = program
            .type_reference_table
            .insert_type_reference_handles([left_argument]);
        let right_arguments = program
            .type_reference_table
            .insert_type_reference_handles([right_argument]);
        let left_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: family_symbol,
                base_name: Identifier::generated_static("Carrier"),
                lifetime_arguments: Vec::new(),
                arguments: left_arguments,
            });
        let right_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: family_symbol,
                base_name: Identifier::generated_static("Carrier"),
                lifetime_arguments: Vec::new(),
                arguments: right_arguments,
            });

        let left = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                head_symbol: left_symbol,
                symbol: left_symbol,
                ..Default::default()
            }));
        let right = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                head_symbol: right_symbol,
                symbol: right_symbol,
                ..Default::default()
            }));
        let arguments = program
            .expression_table
            .insert_expression_handles([left, right]);
        let binder_argument = |symbol| PropositionBinderArgument {
            kind: PropositionBinderArgumentKind::Machine,
            path: Box::default(),
            const_literal: None,
            evidence_projection: None,
            symbol,
        };
        let application = |binders: [SymbolHandle; 2]| PropositionApplication {
            proposition: relation_symbol,
            name: Identifier::generated_static("Related"),
            binder_arguments: binders.map(binder_argument).into(),
            arguments,
        };

        assert!(exact_relation_application_matches(
            &program,
            &application([left_binder, right_binder]),
            relation_symbol,
            left_symbol,
            right_symbol,
            left_type,
            right_type,
        ));
        assert!(!exact_relation_application_matches(
            &program,
            &application([right_binder, left_binder]),
            relation_symbol,
            left_symbol,
            right_symbol,
            left_type,
            right_type,
        ));
    }
}
