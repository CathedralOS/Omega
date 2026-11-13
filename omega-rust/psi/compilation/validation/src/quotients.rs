/*
Quotient formation, equivalence relation plans, and correspondence request
admission (`proof_contracts::quotients`).
*/
pub(crate) mod equivalence_selection {
    use crate::proof_contracts::quotients::CORE_EQUIVALENCE_SOURCE;
    use diagnostics::Diagnostic;
    use std::collections::HashSet;
    use symbols::JoinedPath;
    use symbols::SymbolHandle;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::machine::Machine;
    use typed_trees::proposition::ProofSubstitutions;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn validate_equivalence_selection(
        program: &dyn crate::CheckingInput,
        definition: &typed_trees::data::DataDefinition,
        quotient: &typed_trees::data::QuotientDefinition,
        relation: &typed_trees::proposition::PropositionDefinition,
        carrier_symbol: SymbolHandle,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(selection) = &quotient.equivalence else {
            diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` requires one exact named `Equivalence` conformance in its static `requires R satisfies Equivalence<C, R> as Name` surface",
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
                .joined_path(),
            quotient
                .relation
                .joined_path(),
        )));
        }
        if !is_sealed_core_equivalence(program, selection.trait_symbol, &selection.trait_name) {
            diagnostics.push(Diagnostic::error(format!(
            "quotient data `{}` static selection must resolve the sealed toolchain `Equivalence` declaration from `{CORE_EQUIVALENCE_SOURCE}`; an authored lookalike is not equivalence authority",
            definition.name
        )));
            return;
        }
        let Some(trait_definition) = program.trait_definition_by_symbol(selection.trait_symbol)
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

        let Some(conformance) = program.conformance_by_symbol(selection.conformance_symbol) else {
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
        for row in &conformance.rows {
            let Some(declaring_trait) = program.trait_definition_by_symbol(row.declaring_trait)
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
            let Some(requirement) =
                program.trait_machine_signature_by_symbol(declaring_trait, row.requirement)
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
            let Some(application) = typed_trees::trait_definition::AppliedTraitScope::new(
                program.typed_program(),
                trait_definition,
                typed_trees::trait_definition::TraitApplicationArguments {
                    lexical_owner: definition.symbol,
                    enclosing_data_applications: &[],
                    arguments: selection_arguments,
                    lifetimes: &[],
                },
            )
            .and_then(|root| root.declaring_scope(program.typed_program(), row.declaring_trait)) else {
                diagnostics.push(Diagnostic::error(format!(
                    "quotient data `{}` conformance `{}` cannot instantiate inherited row `{}::{}`",
                    definition.name,
                    selection.conformance_name,
                    row.declaring_trait_name,
                    row.requirement_name,
                )));
                continue;
            };
            let Some(machine) = program.machine(row.realization_machine) else {
                diagnostics.push(Diagnostic::error(format!(
                    "quotient data `{}` conformance `{}` row `{}::{}` has no exact realization",
                    definition.name,
                    selection.conformance_name,
                    row.declaring_trait_name,
                    row.requirement_name,
                )));
                continue;
            };
            crate::proof_contracts::contract_entailment::check_law_conformance(
                program,
                machine,
                Some(selection.conformance_name.as_str()),
                &application,
                requirement,
                diagnostics,
                &mut Vec::new(),
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
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        relation: &typed_trees::proposition::PropositionDefinition,
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
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .collect::<Vec<_>>();
        facts.len() == expected.len()
            && facts.iter().zip(expected).all(|(fact, (left, right))| {
                let (Some(left), Some(right)) = (parameters.get(*left), parameters.get(*right))
                else {
                    return false;
                };
                fact_is_exact_relation_pair(program, fact, relation, left, right, parameters)
            })
    }

    pub(crate) fn fact_is_exact_relation_pair(
        program: &dyn crate::CheckingInput,
        fact: &ProofFact,
        relation: &typed_trees::proposition::PropositionDefinition,
        left: &typed_trees::signature::StateParameter,
        right: &typed_trees::signature::StateParameter,
        parameters: &[typed_trees::signature::StateParameter],
    ) -> bool {
        match fact {
            ProofFact::Proposition(application) => exact_relation_application_matches(
                program,
                application,
                relation.symbol,
                left.symbol,
                right.symbol,
                left.type_reference,
                right.type_reference,
            ),
            ProofFact::Expression(actual) => {
                let typed_trees::proposition::PropositionBody::Transparent {
                    proposition:
                        typed_trees::proposition::PropositionFormula::BooleanExpression(expected),
                } = relation.body
                else {
                    return false;
                };
                let relation_parameters = program.proposition_parameters(relation);
                let [relation_left, relation_right] = relation_parameters else {
                    return false;
                };
                let expected = program.render_proof_expression(
                    expected,
                    ProofSubstitutions::ByParameter(&[
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
                    ]),
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
                expected
                    == program.render_proof_expression(
                        *actual,
                        ProofSubstitutions::ByParameter(&substitutions),
                    )
            }
            ProofFact::Membership(_) => false,
        }
    }

    pub(crate) fn exact_relation_application_matches(
        program: &dyn crate::CheckingInput,
        application: &typed_trees::proposition::PropositionApplication,
        relation_symbol: SymbolHandle,
        left_symbol: SymbolHandle,
        right_symbol: SymbolHandle,
        left_type: TypeReferenceHandle,
        right_type: TypeReferenceHandle,
    ) -> bool {
        let Some(relation) = program.proposition_definition_by_symbol(relation_symbol) else {
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
                                typed_trees::proposition::PropositionBinderArgumentKind::Type,
                                typed_trees::proposition::PropositionBinderKind::Type,
                            ) | (
                                typed_trees::proposition::PropositionBinderArgumentKind::Const,
                                typed_trees::proposition::PropositionBinderKind::Const { .. },
                            ) | (
                                typed_trees::proposition::PropositionBinderArgumentKind::Machine,
                                typed_trees::proposition::PropositionBinderKind::Machine,
                            )
                        )
                })
    }

    fn exact_generic_argument_symbols(
        program: &dyn crate::CheckingInput,
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
        program: &dyn crate::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
        symbol: SymbolHandle,
    ) -> bool {
        matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Name(path) if symbol.is_valid() && path.symbol == symbol
        )
    }

    fn is_sealed_core_equivalence(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
        name: &typed_trees::name::Identifier,
    ) -> bool {
        if !symbol.is_valid() || name.as_str() != "Equivalence" {
            return false;
        }
        let source_matches = program.symbols.core_source(symbol)
            == Some(language_core::core_vocabulary::CoreSource::Relation);
        let Some(definition) = program.trait_definition_by_symbol(symbol) else {
            return false;
        };
        let parameters = program.trait_type_parameters(definition);
        let parameter_shape = matches!(
            parameters,
            [carrier, relation]
                if matches!(carrier.kind, typed_trees::data::TypeParameterKind::Type)
                    && matches!(relation.kind, typed_trees::data::TypeParameterKind::Proposition { .. })
        );
        let mut parent_names = program
            .trait_requirements(definition)
            .iter()
            .filter_map(|parent| {
                program
                    .trait_definition_by_symbol(parent.symbol)
                    .map(|parent| parent.name.as_str())
            })
            .collect::<Vec<_>>();
        parent_names.sort_unstable();
        source_matches
            && parameter_shape
            && parent_names == ["Reflexive", "Symmetric", "Transitive"]
    }

    fn type_reference_symbol(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<SymbolHandle> {
        program.type_reference_table.type_head_symbol(
            program
                .type_reference_table
                .peeled_type_reference(type_reference),
        )
    }

    pub(crate) fn checked_proof_dependency<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program Machine,
        visited: &mut HashSet<u32>,
    ) -> Result<(), &'program str> {
        if !machine.supply_mode.is_checked_body() {
            return Err(machine.name.as_str());
        }
        if !visited.insert(machine.symbol.arena_index()) {
            return Ok(());
        }
        for dependency in
            crate::machine_calls::call_cycles::machine_call_dependency_symbols(program, machine)
        {
            let Some(callee) = program.machine_by_own_or_state_symbol(dependency) else {
                continue;
            };
            if callee.symbol != machine.symbol {
                checked_proof_dependency(program, callee, visited)?;
            }
        }
        Ok(())
    }

    pub(crate) fn base_data_symbol(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<SymbolHandle> {
        if !type_reference.is_valid() {
            return None;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
            TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
            TypeReferenceNode::Reference { referee, .. } => base_data_symbol(program, *referee),
            TypeReferenceNode::Constrained { base_type, .. } => {
                base_data_symbol(program, *base_type)
            }
            _ => None,
        }
    }

    pub(crate) fn quotient_for_type(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        let symbol = base_data_symbol(program, type_reference)?;
        program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == symbol && definition.quotient.is_some())
    }
}
pub(crate) mod formation_collection {
    use crate::frozen_program::expression_table;
    use crate::proof_contracts::carrier_contents::{
        CarrierFenceViolation, first_forbidden_quotient_content,
    };
    use crate::proof_contracts::quotients::ValidatedQuotientFormation;
    use crate::proof_contracts::quotients::equivalence_selection::{
        base_data_symbol, validate_equivalence_selection,
    };
    use crate::proof_contracts::quotients::relation_plan;
    use diagnostics::Diagnostic;
    use std::collections::HashSet;
    use symbols::JoinedPath;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::proof_only::ProofOnlyClassification;

    pub(crate) fn collect_validated_quotient_formations(
        program: &dyn crate::CheckingInput,
        proof_only: &ProofOnlyClassification,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<ValidatedQuotientFormation> {
        let mut formations = Vec::new();
        for definition in program.data_definitions() {
            let Some(quotient) = &definition.quotient else {
                continue;
            };
            let diagnostics_before = diagnostics.len();
            let relation_name = quotient.relation.joined_path();

            let Some(carrier_symbol) = base_data_symbol(program, quotient.carrier) else {
                diagnostics.push(Diagnostic::error(format!(
                    "quotient data `{}` carrier `{}` must name a data type or generic data family",
                    definition.name,
                    program.display_type_reference_with_constraints(quotient.carrier),
                )));
                continue;
            };
            let Some(carrier) = crate::declarations::data_queries::data_definition_by_symbol(
                program,
                carrier_symbol,
            ) else {
                diagnostics.push(Diagnostic::error(format!(
                    "quotient data `{}` has unknown carrier `{}`",
                    definition.name,
                    program.display_type_reference_with_constraints(quotient.carrier),
                )));
                continue;
            };
            if let Some(forbidden) = first_forbidden_quotient_content(
                program,
                proof_only,
                quotient.carrier,
                &mut HashSet::new(),
            ) {
                diagnostics.push(Diagnostic::error(match forbidden {
                CarrierFenceViolation::UnshareableType(forbidden) => format!(
                    "quotient data `{}` carrier `{}` contains non-copy Type content `{forbidden}`; the initial quotient surface cannot identify affine or linear occurrences",
                    definition.name, carrier.name,
                ),
                CarrierFenceViolation::RoutedQualification(forbidden) => format!(
                    "quotient data `{}` carrier `{}` contains routed qualification `{forbidden}`; the initial quotient surface cannot identify exact custody occurrences",
                    definition.name, carrier.name,
                ),
            }));
            }

            let Some(relation) = program.proposition_definition_by_symbol(quotient.relation_symbol)
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

    pub(crate) fn reject_quotient_operation_requests(
        program: &dyn crate::CheckingInput,
        termination: &dyn super::CheckedTerminationOracle,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !expression_table(program).iter_expressions().any(|(_, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.quotient_operation.is_some())
        })
    {
        return;
    }
        let mut planned_requests = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let Some(result_root) = relation_plan::fallthrough_result_root(program, state)
                else {
                    continue;
                };
                let ExpressionNode::Call(call) =
                    expression_table(program).expression(result_root.request_expression)
                else {
                    continue;
                };
                let Some(request) = &call.quotient_operation else {
                    continue;
                };
                planned_requests.push(result_root.request_expression);
                let operation = operation_name(request.kind);
                match relation_plan::derive_direct_terminal_plan(
                    program,
                    termination,
                    machine,
                    state,
                    call,
                    request,
                ) {
                    Ok(plan) => {
                        let congruence = &plan.theorem_evidence[0];
                        let representative_purity = plan.representative_purity;
                        let complete_result_flow = relation_plan::complete_result_flow(
                            program,
                            machine,
                            state,
                            result_root,
                        );
                        let canonical_correspondence = complete_result_flow
                            .clone()
                            .zip(representative_purity)
                            .map(|(result_flow, purity)| {
                                relation_plan::normalized_direct_correspondence(
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
                                " plus rederived normalized source correspondence".to_owned()
                            }
                            Some(Err(reason)) => {
                                format!(" (normalized source correspondence unavailable: {reason})")
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
                            .render_correspondence_certificate(program)
                            .map(|value| format!(" plus composed non-executable {value}"))
                            .unwrap_or_default();
                        let termination = plan
                            .render_representative_termination(program)
                            .map(|value| format!(" plus checked {value}"))
                            .unwrap_or_default();
                        let purity = representative_purity
                            .map(|_| " plus checked pure representative effect summary")
                            .unwrap_or_default();
                        let theorem =
                            format!(" plus exact {}", plan.render_selected_theorem(program));
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
                        let result_flow = match &complete_result_flow {
                            Some(relation_plan::CompleteResultFlow::Single(_)) => format!(
                                "complete transition-free single-state normal-result coverage through {result_path}"
                            ),
                            Some(relation_plan::CompleteResultFlow::Forwarded(_)) => format!(
                                "complete finite state-forwarded normal-result coverage through {result_path}"
                            ),
                            None => format!(
                                "one unchanged state-fallthrough result edge through {result_path}"
                            ),
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
                                remaining
                                    .push("the selected forward-transport crash fence".to_owned());
                            }
                            if plan
                                .transport_schema_verification
                                .as_ref()
                                .is_none_or(|verification| verification.is_err())
                            {
                                remaining.push(
                                    "exact selected forward-transport schema verification"
                                        .to_owned(),
                                );
                            }
                        }
                        if complete_result_flow.is_none() {
                            remaining.push("all normalized result exits".to_owned());
                        }
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
                            remaining.push("normalized source correspondence".to_owned());
                        }
                        let fence = if remaining.is_empty() {
                            "but the complete checked quotient request batch was not admitted"
                                .to_owned()
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

        for (handle, expression) in expression_table(program).iter_expressions() {
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
}
pub(crate) mod relation_plan {
    use crate::frozen_program::expression_table;
    use std::fmt;
    use symbols::SymbolHandle;
    use typed_trees::expression::{
        QuotientOperationKind, QuotientOperationRequest, QuotientTheoremRole,
        StaticMachineArgument, TableCallExpression,
    };
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::state::State;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) mod correspondence_certificate {
        use super::precondition::{
            ContractFactLocation, ContractOwner, DefinePreconditionCorrespondence,
            RepresentativePreconditionPartition, precondition_fact_at,
        };
        use super::proof_fact_identity::{
            ProofFactIdentityContext, ProofValueSubstitution, proof_facts_match,
        };
        use super::representative::RepresentativeRuntimeParameter;
        use super::runtime_correspondence::{
            DefineRuntimeCorrespondence, DirectLiftArgumentSource, DirectLiftRuntimeCorrespondence,
            DirectLiftRuntimePosition, landed_primitive,
        };
        use super::theorem_schema::{ExpectedTheoremSchema, TheoremApplicationSide};
        use super::theorem_schema_verification::VerifiedTheoremSchema;
        use super::transport_schema::VerifiedForwardPreconditionTransportSchema;
        use super::{PlannedQuotientTheoremEvidence, RelationPlanError, RepresentativeTelescope};
        use crate::{
            StrictArithmeticBindingValue, StrictArithmeticImplicationJudgment,
            StrictArithmeticSymbolBinding, strict_arithmetic_expression_implication,
        };
        use numerics::arithmetic::ArithmeticDomain;
        use symbols::SymbolHandle;
        use typed_trees::domain::ProofFact;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum DirectLiftPreconditionProof {
            ExactMatch { public: ContractFactLocation },
            ArithmeticEntailment { premises: Vec<ContractFactLocation> },
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct DirectLiftPreconditionFactRow {
            pub(crate) application: TheoremApplicationSide,
            pub(crate) proof: DirectLiftPreconditionProof,
            pub(crate) representative: ContractFactLocation,
            pub(crate) theorem: ContractFactLocation,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct DirectLiftPreconditionImplication {
            pub(crate) rows: Vec<DirectLiftPreconditionFactRow>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum FixedRepresentativeCallProof {
            ExactMatch { public: ContractFactLocation },
            ArithmeticEntailment { premises: Vec<ContractFactLocation> },
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct FixedRepresentativeCallFactRow {
            pub(crate) proof: FixedRepresentativeCallProof,
            pub(crate) representative: ContractFactLocation,
            pub(crate) theorem_left: ContractFactLocation,
            pub(crate) theorem_right: ContractFactLocation,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct FixedRepresentativeCallPreconditions {
            pub(crate) rows: Vec<FixedRepresentativeCallFactRow>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum QuotientCorrespondenceEvidence {
            DirectLift {
                runtime: DirectLiftRuntimeCorrespondence,
                precondition: DirectLiftPreconditionImplication,
                fixed: FixedRepresentativeCallPreconditions,
            },
            DirectLiftWithTransport {
                runtime: DirectLiftRuntimeCorrespondence,
                transport: VerifiedForwardPreconditionTransportSchema,
            },
            Define {
                runtime: DefineRuntimeCorrespondence,
                precondition: DefinePreconditionCorrespondence,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct QuotientCorrespondenceCertificate {
            pub(crate) theorem: VerifiedTheoremSchema,
            pub(crate) evidence: QuotientCorrespondenceEvidence,
        }

        #[allow(clippy::too_many_arguments)]
        fn exact_public_precondition<'program>(
            program: &'program dyn crate::CheckingInput,
            public_machine: &'program Machine,
            public_state: &'program State,
            public_values: &[ProofValueSubstitution],
            representative: &RepresentativeTelescope,
            representative_values: &[ProofValueSubstitution],
            representative_fact: &ProofFact,
            candidates: &[ContractFactLocation],
        ) -> Option<ContractFactLocation> {
            candidates.iter().copied().find(|location| {
                precondition_fact_at(
                    program,
                    public_machine.contracts,
                    public_state.contracts,
                    *location,
                )
                .is_some_and(|public_fact| {
                    proof_facts_match(
                        program,
                        public_fact,
                        representative_fact,
                        ProofFactIdentityContext {
                            values: public_values,
                            static_bindings: &[],
                        },
                        ProofFactIdentityContext {
                            values: representative_values,
                            static_bindings: &representative.static_application.bindings,
                        },
                    )
                })
            })
        }

        pub(crate) fn derive_direct_lift_precondition_implication(
            program: &dyn crate::CheckingInput,
            public_machine: &Machine,
            public_state: &State,
            representative: &RepresentativeTelescope,
            public: &RepresentativePreconditionPartition,
            representative_partition: &RepresentativePreconditionPartition,
            runtime: &DirectLiftRuntimeCorrespondence,
            expected_theorem: &ExpectedTheoremSchema,
            verified_theorem: &VerifiedTheoremSchema,
        ) -> Result<DirectLiftPreconditionImplication, RelationPlanError> {
            let mut rows = Vec::with_capacity(representative_partition.dependent.len() * 2);
            for application in [TheoremApplicationSide::Left, TheoremApplicationSide::Right] {
                let application_schema = match application {
                    TheoremApplicationSide::Left => &expected_theorem.left_application,
                    TheoremApplicationSide::Right => &expected_theorem.right_application,
                };
                if runtime.positions.len() != application_schema.arguments.len() {
                    return Err(RelationPlanError::DirectLiftRuntimeArityMismatch);
                }
                let (public_values, representative_values) =
                    proof_value_substitutions(runtime, |position| {
                        format!(
                            "$theorem_parameter_{}",
                            application_schema.arguments[position]
                        )
                    });
                let arithmetic_bindings = arithmetic_bindings_for_application(
                    program,
                    representative,
                    runtime,
                    expected_theorem,
                    &application_schema.arguments,
                );
                let public_expression_hypotheses = public_expression_hypotheses(
                    program,
                    public_machine,
                    public_state,
                    &public.dependent,
                );

                for (representative_position, representative_location) in
                    representative_partition.dependent.iter().enumerate()
                {
                    let representative_fact = precondition_fact_at(
                        program,
                        representative.machine_contracts,
                        representative.state_contracts,
                        *representative_location,
                    )
                    .ok_or_else(|| implication_error(application, representative_position))?;
                    let exact_public = exact_public_precondition(
                        program,
                        public_machine,
                        public_state,
                        &public_values,
                        representative,
                        &representative_values,
                        representative_fact,
                        &public.dependent,
                    );
                    let proof = if let Some(public) = exact_public {
                        DirectLiftPreconditionProof::ExactMatch { public }
                    } else {
                        let (Some(hypotheses), ProofFact::Expression(goal)) =
                            (public_expression_hypotheses.as_deref(), representative_fact)
                        else {
                            return Err(implication_error(application, representative_position));
                        };
                        if strict_arithmetic_expression_implication(
                            program,
                            public_machine,
                            hypotheses,
                            *goal,
                            &arithmetic_bindings,
                            &[],
                        ) != StrictArithmeticImplicationJudgment::Proven
                        {
                            return Err(implication_error(application, representative_position));
                        }
                        DirectLiftPreconditionProof::ArithmeticEntailment {
                            premises: public.dependent.clone(),
                        }
                    };
                    let theorem = verified_legality_coordinate(
                        expected_theorem,
                        verified_theorem,
                        application,
                        *representative_location,
                    )
                    .ok_or(RelationPlanError::DirectLiftTheoremLegalityMismatch)?;
                    rows.push(DirectLiftPreconditionFactRow {
                        application,
                        proof,
                        representative: *representative_location,
                        theorem,
                    });
                }
            }
            Ok(DirectLiftPreconditionImplication { rows })
        }

        fn public_expression_hypotheses(
            program: &dyn crate::CheckingInput,
            public_machine: &Machine,
            public_state: &State,
            locations: &[ContractFactLocation],
        ) -> Option<Vec<typed_trees::expression::ExpressionHandle>> {
            locations
                .iter()
                .map(|location| {
                    let expression = precondition_fact_at(
                        program,
                        public_machine.contracts,
                        public_state.contracts,
                        *location,
                    )?
                    .as_expression()?;
                    Some(*expression)
                })
                .collect::<Option<Vec<_>>>()
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn derive_fixed_representative_call_preconditions(
            program: &dyn crate::CheckingInput,
            public_machine: &Machine,
            public_state: &State,
            representative: &RepresentativeTelescope,
            public: &RepresentativePreconditionPartition,
            representative_partition: &RepresentativePreconditionPartition,
            runtime: &DirectLiftRuntimeCorrespondence,
            expected_theorem: &ExpectedTheoremSchema,
            verified_theorem: &VerifiedTheoremSchema,
        ) -> Result<FixedRepresentativeCallPreconditions, RelationPlanError> {
            let (public_values, representative_values) =
                proof_value_substitutions(runtime, |position| {
                    format!("$runtime_parameter_{position}")
                });
            let arithmetic_bindings =
                arithmetic_bindings_for_runtime(program, representative, runtime);
            let public_expression_hypotheses =
                public_expression_hypotheses(program, public_machine, public_state, &public.fixed);
            let mut rows = Vec::with_capacity(representative_partition.fixed.len());
            for (representative_position, representative_location) in
                representative_partition.fixed.iter().enumerate()
            {
                let representative_fact = precondition_fact_at(
                    program,
                    representative.machine_contracts,
                    representative.state_contracts,
                    *representative_location,
                )
                .ok_or(
                    RelationPlanError::DirectLiftFixedPreconditionNotImplied(
                        representative_position,
                    ),
                )?;
                let exact_public = exact_public_precondition(
                    program,
                    public_machine,
                    public_state,
                    &public_values,
                    representative,
                    &representative_values,
                    representative_fact,
                    &public.fixed,
                );
                let proof = if let Some(public) = exact_public {
                    FixedRepresentativeCallProof::ExactMatch { public }
                } else {
                    let (Some(hypotheses), ProofFact::Expression(goal)) =
                        (public_expression_hypotheses.as_deref(), representative_fact)
                    else {
                        return Err(RelationPlanError::DirectLiftFixedPreconditionNotImplied(
                            representative_position,
                        ));
                    };
                    if strict_arithmetic_expression_implication(
                        program,
                        public_machine,
                        hypotheses,
                        *goal,
                        &arithmetic_bindings,
                        &[],
                    ) != StrictArithmeticImplicationJudgment::Proven
                    {
                        return Err(RelationPlanError::DirectLiftFixedPreconditionNotImplied(
                            representative_position,
                        ));
                    }
                    FixedRepresentativeCallProof::ArithmeticEntailment {
                        premises: public.fixed.clone(),
                    }
                };
                let theorem_left = verified_legality_coordinate(
                    expected_theorem,
                    verified_theorem,
                    TheoremApplicationSide::Left,
                    *representative_location,
                )
                .ok_or(RelationPlanError::DirectLiftFixedTheoremLegalityMismatch)?;
                let theorem_right = verified_legality_coordinate(
                    expected_theorem,
                    verified_theorem,
                    TheoremApplicationSide::Right,
                    *representative_location,
                )
                .ok_or(RelationPlanError::DirectLiftFixedTheoremLegalityMismatch)?;
                rows.push(FixedRepresentativeCallFactRow {
                    proof,
                    representative: *representative_location,
                    theorem_left,
                    theorem_right,
                });
            }
            Ok(FixedRepresentativeCallPreconditions { rows })
        }

        pub(crate) fn proof_value_substitutions(
            runtime: &DirectLiftRuntimeCorrespondence,
            public_identity: impl Fn(usize) -> String,
        ) -> (Vec<ProofValueSubstitution>, Vec<ProofValueSubstitution>) {
            let mut public_values = Vec::new();
            let mut representative_values = Vec::with_capacity(runtime.positions.len());
            for (position_index, position) in runtime.positions.iter().enumerate() {
                let value = match &position.source {
                    DirectLiftArgumentSource::PublicParameter(public_parameter) => {
                        let value = public_values
                            .iter()
                            .find_map(|value: &ProofValueSubstitution| {
                                (value.symbol == *public_parameter).then(|| value.clone())
                            })
                            .unwrap_or_else(|| {
                                let value = ProofValueSubstitution::symbolic(
                                    *public_parameter,
                                    public_identity(position_index),
                                );
                                public_values.push(value.clone());
                                value
                            });
                        value.rebound(position.representative_parameter)
                    }
                    DirectLiftArgumentSource::Literal(literal) => {
                        proof_value_for_literal(position.representative_parameter, literal)
                    }
                };
                representative_values.push(value);
            }
            (public_values, representative_values)
        }

        pub(crate) fn proof_value_for_literal(
            symbol: SymbolHandle,
            literal: &super::runtime_correspondence::ClosedLiftLiteral,
        ) -> ProofValueSubstitution {
            use super::runtime_correspondence::ClosedLiftLiteral;

            match literal {
                ClosedLiftLiteral::Boolean(value) => {
                    ProofValueSubstitution::boolean(symbol, *value)
                }
                ClosedLiftLiteral::Integer { spelling, landing } => {
                    ProofValueSubstitution::integer(symbol, spelling, *landing)
                }
                ClosedLiftLiteral::Float { spelling, landing } => {
                    ProofValueSubstitution::float(symbol, spelling, *landing)
                }
                ClosedLiftLiteral::ByteString { bytes, .. } => {
                    ProofValueSubstitution::byte_string(symbol, bytes)
                }
                ClosedLiftLiteral::FixedByteArray { bytes, .. } => {
                    ProofValueSubstitution::fixed_byte_array(symbol, bytes)
                }
                ClosedLiftLiteral::BooleanArray { values, .. } => {
                    ProofValueSubstitution::boolean_array(symbol, values)
                }
                ClosedLiftLiteral::NestedFixedByteArray { rows, .. } => {
                    ProofValueSubstitution::nested_fixed_byte_array(symbol, rows)
                }
                ClosedLiftLiteral::NestedBooleanArray { rows, .. } => {
                    ProofValueSubstitution::nested_boolean_array(symbol, rows)
                }
                ClosedLiftLiteral::BooleanTensor3 { planes, .. } => {
                    ProofValueSubstitution::boolean_tensor3(symbol, planes)
                }
                ClosedLiftLiteral::RecursivePrimitiveArray { elements, .. } => {
                    ProofValueSubstitution::recursive_primitive_array(symbol, elements)
                }
                ClosedLiftLiteral::IntegerArray { elements, .. } => {
                    ProofValueSubstitution::integer_array(
                        symbol,
                        elements
                            .iter()
                            .map(|element| (element.spelling.clone(), element.landing)),
                    )
                }
                ClosedLiftLiteral::NestedIntegerArray { rows, .. } => {
                    ProofValueSubstitution::nested_integer_array(symbol, rows)
                }
                ClosedLiftLiteral::FloatArray { elements, .. } => {
                    ProofValueSubstitution::float_array(
                        symbol,
                        elements
                            .iter()
                            .map(|element| (element.spelling.clone(), element.landing)),
                    )
                }
                ClosedLiftLiteral::NestedFloatArray { rows, .. } => {
                    ProofValueSubstitution::nested_float_array(symbol, rows)
                }
            }
        }

        fn arithmetic_bindings_for_runtime(
            program: &dyn crate::CheckingInput,
            representative: &RepresentativeTelescope,
            runtime: &DirectLiftRuntimeCorrespondence,
        ) -> Vec<StrictArithmeticSymbolBinding> {
            let mut bindings = Vec::new();
            for (position_index, (position, representative_parameter)) in runtime
                .positions
                .iter()
                .zip(&representative.parameters)
                .enumerate()
            {
                push_exact_position_arithmetic_bindings(
                    program,
                    &mut bindings,
                    position,
                    representative_parameter,
                    representative_parameter.type_reference,
                    || format!("$runtime_parameter_{position_index}"),
                );
            }
            append_static_integer_bindings(&mut bindings, representative);
            bindings
        }

        fn arithmetic_bindings_for_application(
            program: &dyn crate::CheckingInput,
            representative: &RepresentativeTelescope,
            runtime: &DirectLiftRuntimeCorrespondence,
            expected: &ExpectedTheoremSchema,
            theorem_arguments: &[usize],
        ) -> Vec<StrictArithmeticSymbolBinding> {
            let mut bindings = Vec::new();
            for ((position, representative_parameter), theorem_position) in runtime
                .positions
                .iter()
                .zip(&representative.parameters)
                .zip(theorem_arguments)
            {
                let Some(parameter) = expected.parameters.get(*theorem_position) else {
                    continue;
                };
                push_exact_position_arithmetic_bindings(
                    program,
                    &mut bindings,
                    position,
                    representative_parameter,
                    parameter.type_reference,
                    || format!("$theorem_parameter_{theorem_position}"),
                );
            }
            append_static_integer_bindings(&mut bindings, representative);
            bindings
        }

        fn append_static_integer_bindings(
            bindings: &mut Vec<StrictArithmeticSymbolBinding>,
            representative: &RepresentativeTelescope,
        ) {
            for binding in &representative.static_application.bindings {
                if binding.kind != super::RepresentativeStaticBindingKind::Const {
                    continue;
                }
                let Some(value) = binding
                    .argument
                    .const_literal
                    .as_ref()
                    .and_then(|literal| literal.value_bignum())
                else {
                    continue;
                };
                push_arithmetic_binding(
                    bindings,
                    binding.parameter,
                    StrictArithmeticBindingValue::Integer(value),
                );
            }
        }

        fn push_exact_position_arithmetic_bindings(
            program: &dyn crate::CheckingInput,
            bindings: &mut Vec<StrictArithmeticSymbolBinding>,
            position: &DirectLiftRuntimePosition,
            representative_parameter: &RepresentativeRuntimeParameter,
            type_reference: TypeReferenceHandle,
            atom_identity: impl FnOnce() -> String,
        ) {
            let Some((primitive, unsigned)) = exact_integer_parameter(program, type_reference)
            else {
                return;
            };
            match &position.source {
                DirectLiftArgumentSource::PublicParameter(_)
                | DirectLiftArgumentSource::Literal(_) => push_position_arithmetic_bindings(
                    bindings,
                    position,
                    representative_parameter,
                    primitive,
                    unsigned,
                    atom_identity,
                ),
            }
        }

        fn push_position_arithmetic_bindings(
            bindings: &mut Vec<StrictArithmeticSymbolBinding>,
            position: &DirectLiftRuntimePosition,
            representative_parameter: &RepresentativeRuntimeParameter,
            primitive: PrimitiveType,
            unsigned: bool,
            atom_identity: impl FnOnce() -> String,
        ) {
            match &position.source {
                DirectLiftArgumentSource::PublicParameter(public_symbol) => {
                    let value = bindings
                        .iter()
                        .find_map(|binding: &StrictArithmeticSymbolBinding| {
                            (binding.symbol == *public_symbol).then(|| binding.value.clone())
                        })
                        .unwrap_or_else(|| StrictArithmeticBindingValue::Atom {
                            identity: atom_identity(),
                            unsigned,
                        });
                    push_arithmetic_binding(bindings, *public_symbol, value.clone());
                    push_arithmetic_binding(bindings, representative_parameter.symbol, value);
                }
                DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::Integer { spelling, landing },
                ) if landing.domain == ArithmeticDomain::Exact
                    && landed_primitive(landing.landed_type) == primitive =>
                {
                    if let Some(value) = integer_spelling_value(spelling) {
                        push_arithmetic_binding(
                            bindings,
                            representative_parameter.symbol,
                            StrictArithmeticBindingValue::Integer(value),
                        );
                    }
                }
                _ => {}
            }
        }

        fn push_arithmetic_binding(
            bindings: &mut Vec<StrictArithmeticSymbolBinding>,
            symbol: SymbolHandle,
            value: StrictArithmeticBindingValue,
        ) {
            if !bindings.iter().any(|binding| binding.symbol == symbol) {
                bindings.push(StrictArithmeticSymbolBinding { symbol, value });
            }
        }

        fn exact_integer_parameter(
            program: &dyn crate::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> Option<(PrimitiveType, bool)> {
            crate::value_custody::type_shapes::exact_integer_primitive(
                program,
                type_reference,
                true,
            )
            .map(|primitive| (primitive, !primitive.is_signed_integer()))
        }

        fn integer_spelling_value(spelling: &str) -> Option<numerics::bignum::BigInt> {
            use numerics::literals::{IntegerLiteral, IntegerRadix};

            let (negative, unsigned) = spelling
                .strip_prefix('-')
                .map_or((false, spelling), |unsigned| (true, unsigned));
            let (radix, digits) = if let Some(digits) = unsigned.strip_prefix("0b") {
                (IntegerRadix::Binary, digits)
            } else if let Some(digits) = unsigned.strip_prefix("0o") {
                (IntegerRadix::Octal, digits)
            } else if let Some(digits) = unsigned.strip_prefix("0x") {
                (IntegerRadix::Hexadecimal, digits)
            } else {
                (IntegerRadix::Decimal, unsigned)
            };
            IntegerLiteral::from_parts(negative, radix, digits)
                .ok()?
                .value_bignum()
        }

        fn implication_error(
            application: TheoremApplicationSide,
            representative_position: usize,
        ) -> RelationPlanError {
            match application {
                TheoremApplicationSide::Left => {
                    RelationPlanError::DirectLiftLeftPreconditionNotImplied(representative_position)
                }
                TheoremApplicationSide::Right => {
                    RelationPlanError::DirectLiftRightPreconditionNotImplied(
                        representative_position,
                    )
                }
            }
        }

        fn verified_legality_coordinate(
            expected: &ExpectedTheoremSchema,
            verified: &VerifiedTheoremSchema,
            application: TheoremApplicationSide,
            representative: ContractFactLocation,
        ) -> Option<ContractFactLocation> {
            let expected_position = expected.legality_premises.iter().position(|premise| {
                premise.application == application
                    && premise.fact == theorem_location(representative)
            })?;
            verified.legality_premises.iter().find_map(|fact| {
                (fact.expected_position == expected_position).then_some(fact.actual)
            })
        }

        fn theorem_location(location: ContractFactLocation) -> ContractFactLocation {
            ContractFactLocation {
                owner: match location.owner {
                    ContractOwner::Machine => ContractOwner::Machine,
                    ContractOwner::State => ContractOwner::State,
                },
                contract_position: location.contract_position,
                fact_position: location.fact_position,
            }
        }

        pub(crate) fn compose_lift_correspondence_certificate(
            theorem: &Result<VerifiedTheoremSchema, RelationPlanError>,
            runtime: &DirectLiftRuntimeCorrespondence,
            precondition: &DirectLiftPreconditionImplication,
            fixed: &FixedRepresentativeCallPreconditions,
            representative_partition: &RepresentativePreconditionPartition,
        ) -> Option<QuotientCorrespondenceCertificate> {
            if fixed
                .rows
                .iter()
                .map(|row| row.representative)
                .ne(representative_partition.fixed.iter().copied())
            {
                return None;
            }
            Some(QuotientCorrespondenceCertificate {
                theorem: theorem.as_ref().ok()?.clone(),
                evidence: QuotientCorrespondenceEvidence::DirectLift {
                    runtime: runtime.clone(),
                    precondition: precondition.clone(),
                    fixed: fixed.clone(),
                },
            })
        }

        pub(crate) fn compose_lift_transport_correspondence_certificate(
            congruence: &Result<VerifiedTheoremSchema, RelationPlanError>,
            congruence_evidence: &PlannedQuotientTheoremEvidence,
            transport: &Result<VerifiedForwardPreconditionTransportSchema, RelationPlanError>,
            transport_evidence: &PlannedQuotientTheoremEvidence,
            runtime: &DirectLiftRuntimeCorrespondence,
        ) -> Option<QuotientCorrespondenceCertificate> {
            let verified_congruence = congruence.as_ref().ok()?;
            let verified_transport = transport.as_ref().ok()?;
            if !theorem_evidence_is_eligible(
                congruence_evidence,
                typed_trees::expression::QuotientTheoremRole::Congruence,
                verified_congruence.theorem_machine_symbol,
                verified_congruence.theorem_state_symbol,
            ) || !theorem_evidence_is_eligible(
                transport_evidence,
                verified_transport.role,
                verified_transport.theorem_machine_symbol,
                verified_transport.theorem_state_symbol,
            ) || transport_evidence.selected_application
                != verified_transport.selected_application
            {
                return None;
            }
            Some(QuotientCorrespondenceCertificate {
                theorem: verified_congruence.clone(),
                evidence: QuotientCorrespondenceEvidence::DirectLiftWithTransport {
                    runtime: runtime.clone(),
                    transport: verified_transport.clone(),
                },
            })
        }

        fn theorem_evidence_is_eligible(
            evidence: &PlannedQuotientTheoremEvidence,
            role: typed_trees::expression::QuotientTheoremRole,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
        ) -> bool {
            evidence.role == role
                && evidence.selected_application.machine_symbol == machine_symbol
                && evidence.selected_application.state_symbol == state_symbol
                && evidence.termination.is_some_and(|termination| {
                    termination.machine_symbol == machine_symbol
                        && termination.state_symbol == state_symbol
                })
                && evidence.purity.is_some_and(|purity| {
                    purity.machine_symbol == machine_symbol && purity.state_symbol == state_symbol
                })
                && evidence.crash_free
        }

        pub(crate) fn compose_define_correspondence_certificate(
            theorem: &Result<VerifiedTheoremSchema, RelationPlanError>,
            runtime: &DefineRuntimeCorrespondence,
            precondition: &DefinePreconditionCorrespondence,
            representative_partition: &RepresentativePreconditionPartition,
        ) -> Option<QuotientCorrespondenceCertificate> {
            if precondition
                .fixed
                .iter()
                .map(|pair| pair.representative)
                .ne(representative_partition.fixed.iter().copied())
            {
                return None;
            }
            Some(QuotientCorrespondenceCertificate {
                theorem: theorem.as_ref().ok()?.clone(),
                evidence: QuotientCorrespondenceEvidence::Define {
                    runtime: runtime.clone(),
                    precondition: precondition.clone(),
                },
            })
        }
    }
    pub(crate) mod normalized_judgment {
        use language_semantics::quotient_correspondence::{
            QuotientCallableIdentity, QuotientCongruenceCorrespondence,
            QuotientContractFactCoordinate, QuotientContractOwner, QuotientCorrespondenceJudgment,
            QuotientCorrespondenceOperationKind, QuotientCrashCertificate,
            QuotientForwardPreconditionTransportCorrespondence,
            QuotientForwardPreconditionTransportFact, QuotientMachineApplication,
            QuotientPositionalRelation, QuotientPurityCertificate, QuotientRelationIdentity,
            QuotientRepresentativeApplication, QuotientRepresentativeEligibility,
            QuotientResultFlow, QuotientResultRelation, QuotientRuntimeArgument,
            QuotientStateForwarding, QuotientStaticApplication, QuotientTerminationCertificate,
            QuotientTheoremConclusion, QuotientTheoremCorrespondence, QuotientTheoremEligibility,
            QuotientTheoremEvidence, QuotientTheoremParameter, QuotientTheoremParameterRole,
            QuotientTheoremRelationPremise, QuotientTheoremRole,
        };
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::StatementNode;

        use super::correspondence_certificate::{
            QuotientCorrespondenceCertificate, QuotientCorrespondenceEvidence,
        };
        use super::precondition::{ContractFactLocation, ContractOwner};
        use super::representative::MachineStateCoordinate;
        use super::result_flow::{CompleteResultFlow, CompleteStateForwardingResultFlow};
        use super::runtime_correspondence::{
            DirectLiftArgumentSource, direct_public_parameter_symbol,
        };
        use super::theorem_schema::TheoremParameterRole;
        use super::transport_schema::VerifiedForwardPreconditionTransportSchema;
        use super::{DirectTerminalRelationPlan, ExactQuotientRelation, PositionalRelation};

        pub(crate) fn normalized_direct_correspondence(
            program: &dyn crate::CheckingInput,
            public_machine: &Machine,
            public_state: &State,
            request_expression: ExpressionHandle,
            plan: &DirectTerminalRelationPlan,
            representative_purity: MachineStateCoordinate,
            result_flow: CompleteResultFlow,
        ) -> Result<QuotientCorrespondenceJudgment, String> {
            let transport_lift = match plan
                .correspondence_certificate
                .as_ref()
                .map(|certificate| &certificate.evidence)
            {
                Some(QuotientCorrespondenceEvidence::Define { .. }) => false,
                Some(QuotientCorrespondenceEvidence::DirectLiftWithTransport { .. }) => true,
                Some(QuotientCorrespondenceEvidence::DirectLift { .. }) => false,
                None => return Err("the complete correspondence certificate is absent".to_owned()),
            };
            normalized_correspondence(
                program,
                public_machine,
                public_state,
                request_expression,
                plan,
                representative_purity,
                result_flow,
                transport_lift,
            )
        }

        #[allow(clippy::too_many_arguments)]
        fn normalized_correspondence(
            program: &dyn crate::CheckingInput,
            public_machine: &Machine,
            public_state: &State,
            request_expression: ExpressionHandle,
            plan: &DirectTerminalRelationPlan,
            representative_purity: MachineStateCoordinate,
            result_flow: CompleteResultFlow,
            transport_lift: bool,
        ) -> Result<QuotientCorrespondenceJudgment, String> {
            let planned_theorem = plan.theorem_evidence.first().ok_or_else(|| {
                "quotient correspondence requires a Congruence theorem entry".to_owned()
            })?;
            let expected_roles = if transport_lift { 2 } else { 1 };
            if plan.theorem_evidence.len() != expected_roles {
                return Err("quotient correspondence theorem role roster drifted".to_owned());
            }
            if planned_theorem.role != typed_trees::expression::QuotientTheoremRole::Congruence {
                return Err(
                    "quotient correspondence theorem evidence has a noncanonical role".to_owned(),
                );
            }
            let selected_theorem = &planned_theorem.selected_application;
            match &result_flow {
                CompleteResultFlow::Single(_) => {
                    require_single_entry(program, public_machine, public_state, "public operation")?
                }
                CompleteResultFlow::Forwarded(_) => {}
            }
            require_empty_owner_telescope(program, public_machine, "public operation")?;

            let (representative_machine, representative_state) = exact_machine_state(
                program,
                plan.representative.machine_symbol,
                plan.representative.state_symbol,
                "representative",
            )?;
            require_single_entry(
                program,
                representative_machine,
                representative_state,
                "representative",
            )?;
            let (theorem_machine, theorem_state) = exact_machine_state(
                program,
                selected_theorem.machine_symbol,
                selected_theorem.state_symbol,
                "selected theorem",
            )?;
            require_single_entry(program, theorem_machine, theorem_state, "selected theorem")?;

            if !plan.representative.static_application.bindings.is_empty()
                || !plan
                    .representative
                    .static_application
                    .lifetime_arguments
                    .is_empty()
                || plan.theorem_evidence.iter().any(|evidence| {
                    !evidence
                        .selected_application
                        .static_application
                        .bindings
                        .is_empty()
                        || !evidence
                            .selected_application
                            .static_application
                            .lifetime_arguments
                            .is_empty()
                })
            {
                return Err(
                    "the proof-only bridge requires empty closed static applications".to_owned(),
                );
            }
            require_empty_owner_telescope(program, representative_machine, "representative")?;
            for evidence in &plan.theorem_evidence {
                let (machine, state) = exact_machine_state(
                    program,
                    evidence.selected_application.machine_symbol,
                    evidence.selected_application.state_symbol,
                    "selected theorem",
                )?;
                require_single_entry(program, machine, state, "selected theorem")?;
                require_empty_owner_telescope(program, machine, "selected theorem")?;
                require_plain_immutable_parameters(
                    evidence
                        .selected_application
                        .parameters
                        .iter()
                        .map(|parameter| parameter.is_mutable),
                    "selected theorem",
                )?;
            }

            let public_parameters = program
                .public_state_parameters(public_state)
                .collect::<Vec<_>>();
            require_plain_immutable_parameters(
                public_parameters
                    .iter()
                    .map(|parameter| parameter.is_mutable),
                "public operation",
            )?;
            require_plain_immutable_parameters(
                plan.representative
                    .parameters
                    .iter()
                    .map(|parameter| parameter.is_mutable),
                "representative",
            )?;
            require_plain_immutable_parameters(
                selected_theorem
                    .parameters
                    .iter()
                    .map(|parameter| parameter.is_mutable),
                "selected theorem",
            )?;

            let certificate = plan
                .correspondence_certificate
                .as_ref()
                .ok_or_else(|| "the complete correspondence certificate is absent".to_owned())?;
            let (operation_kind, runtime_arguments, transport) = match &certificate.evidence {
                QuotientCorrespondenceEvidence::Define {
                    runtime,
                    precondition,
                } if !transport_lift => {
                    if !precondition.dependent.is_empty() || !precondition.fixed.is_empty() {
                        return Err(
                    "the proof-only bridge admits no public or representative preconditions"
                        .to_owned(),
                );
                    }
                    let positions = canonical_define_runtime_arguments(
                        &public_parameters,
                        &plan.representative.parameters,
                        &runtime.positions,
                    )?;
                    (QuotientCorrespondenceOperationKind::Define, positions, None)
                }
                QuotientCorrespondenceEvidence::DirectLiftWithTransport { runtime, transport }
                    if transport_lift =>
                {
                    let positions = canonical_lift_runtime_arguments(
                        program,
                        &public_parameters,
                        &plan.representative.parameters,
                        &runtime.positions,
                    )?;
                    (
                        QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport,
                        positions,
                        Some(transport),
                    )
                }
                QuotientCorrespondenceEvidence::DirectLift { runtime, .. } if !transport_lift => (
                    QuotientCorrespondenceOperationKind::Lift,
                    canonical_lift_runtime_arguments(
                        program,
                        &public_parameters,
                        &plan.representative.parameters,
                        &runtime.positions,
                    )?,
                    None,
                ),
                _ => {
                    return Err(
                        "the proof-only bridge operation/certificate kind drifted".to_owned()
                    );
                }
            };
            if operation_kind == QuotientCorrespondenceOperationKind::Define {
                require_empty_partition(plan.public_precondition.as_ref(), "public")?;
                require_empty_partition(
                    plan.representative_precondition.as_ref(),
                    "representative",
                )?;
                if !plan.expected_theorem_schema.legality_premises.is_empty()
                    || !certificate.theorem.legality_premises.is_empty()
                {
                    return Err(
                        "the proof-only bridge admits no theorem legality premises".to_owned()
                    );
                }
            }

            if plan.representative_termination.is_none()
                || representative_purity.machine_symbol != plan.representative.machine_symbol
                || representative_purity.state_symbol != plan.representative.state_symbol
                || plan.theorem_evidence.iter().any(|evidence| {
                    evidence.termination.is_none()
                        || evidence.purity.is_none()
                        || !evidence.crash_free
                })
            {
                return Err(
                    "purity, termination, or theorem crash eligibility is incomplete".to_owned(),
                );
            }
            let statement_position = program
                .statement_table
                .statements(public_state.statement_nodes)
                .len()
                .checked_sub(1)
                .ok_or_else(|| "direct result state has no terminal statement".to_owned())?;
            let result_flow = match result_flow {
                CompleteResultFlow::Single(flow) => {
                    if flow.root.request_expression != request_expression
                        || flow.machine_symbol != public_machine.symbol
                        || flow.state_symbol != public_state.symbol
                    {
                        return Err("direct result-flow certificate identity drifted".to_owned());
                    }
                    QuotientResultFlow::Direct {
                        statement_position: to_u32(
                            statement_position,
                            "result statement position",
                        )?,
                        immutable_alias_count: to_u32(
                            flow.root.alias_count,
                            "immutable alias count",
                        )?,
                    }
                }
                CompleteResultFlow::Forwarded(flow) => canonical_forwarded_result_flow(
                    program,
                    public_machine,
                    public_state,
                    request_expression,
                    &flow,
                    statement_position,
                )?,
            };

            if operation_kind != QuotientCorrespondenceOperationKind::Define {
                if runtime_arguments.len() != plan.representative.parameters.len()
                    || plan.input_relations.len() != plan.representative.parameters.len()
                {
                    return Err("transport lift runtime correspondence arity drifted".to_owned());
                }
            } else if public_parameters.len() != plan.representative.parameters.len()
                || runtime_arguments.len() != public_parameters.len()
                || plan.input_relations.len() != public_parameters.len()
            {
                return Err("faithful runtime correspondence arity drifted".to_owned());
            }

            let input_relations = plan
                .input_relations
                .iter()
                .enumerate()
                .map(|(position, relation)| match relation {
                    PositionalRelation::Quotient(relation) => {
                        Ok(QuotientPositionalRelation::Quotient(relation_identity(
                            program, *relation,
                        )?))
                    }
                    PositionalRelation::ExactEquality(public_type) => {
                        let public_type = canonical_type(program, *public_type)?;
                        let representative_type = canonical_type(
                            program,
                            plan.representative.parameters[position].type_reference,
                        )?;
                        if public_type != representative_type {
                            return Err("ordinary faithful input type identity drifted".to_owned());
                        }
                        Ok(QuotientPositionalRelation::ExactEquality {
                            public_type,
                            representative_type,
                        })
                    }
                })
                .collect::<Result<Vec<_>, String>>()?;
            let result_relation = match plan.result_relation {
                PositionalRelation::Quotient(relation) => {
                    QuotientPositionalRelation::Quotient(relation_identity(program, relation)?)
                }
                PositionalRelation::ExactEquality(public_type) => {
                    QuotientPositionalRelation::ExactEquality {
                        public_type: canonical_type(program, public_type)?,
                        representative_type: canonical_type(
                            program,
                            plan.representative.return_type,
                        )?,
                    }
                }
            };
            let theorem = theorem_correspondence(program, plan, certificate, true)?;

            Ok(QuotientCorrespondenceJudgment {
                operation_kind,
                public_operation: callable_identity(program, public_machine)?,
                representative: machine_application(program, representative_machine)?,
                input_relations,
                result_relation,
                runtime_arguments,
                theorem_evidence: {
                    let mut evidence = vec![QuotientTheoremEvidence {
                        role: QuotientTheoremRole::Congruence,
                        selected_application: machine_application(program, theorem_machine)?,
                        correspondence: QuotientTheoremCorrespondence::Congruence(theorem),
                        eligibility: QuotientTheoremEligibility {
                            purity: QuotientPurityCertificate::PureClosure,
                            termination: QuotientTerminationCertificate::Unconditional,
                            crash: QuotientCrashCertificate::CrashFree,
                        },
                    }];
                    if let Some(transport) = transport {
                        let selected = &plan.theorem_evidence[1].selected_application;
                        let (machine, _) = exact_machine_state(
                            program,
                            selected.machine_symbol,
                            selected.state_symbol,
                            "selected transport theorem",
                        )?;
                        evidence.push(QuotientTheoremEvidence {
                            role: QuotientTheoremRole::ForwardPreconditionTransport,
                            selected_application: machine_application(program, machine)?,
                            correspondence:
                                QuotientTheoremCorrespondence::ForwardPreconditionTransport(
                                    transport_correspondence(transport)?,
                                ),
                            eligibility: QuotientTheoremEligibility {
                                purity: QuotientPurityCertificate::PureClosure,
                                termination: QuotientTerminationCertificate::Unconditional,
                                crash: QuotientCrashCertificate::CrashFree,
                            },
                        });
                    }
                    evidence
                },
                representative_eligibility: QuotientRepresentativeEligibility {
                    purity: QuotientPurityCertificate::PureClosure,
                    termination: QuotientTerminationCertificate::Unconditional,
                },
                result_flow,
            })
        }

        fn canonical_forwarded_result_flow(
            program: &dyn crate::CheckingInput,
            public_machine: &Machine,
            public_state: &State,
            request_expression: ExpressionHandle,
            flow: &CompleteStateForwardingResultFlow,
            statement_position: usize,
        ) -> Result<QuotientResultFlow, String> {
            if flow.root.request_expression != request_expression
                || flow.machine_symbol != public_machine.symbol
                || flow.result_state_symbol != public_state.symbol
            {
                return Err("forwarded result-flow certificate identity drifted".to_owned());
            }
            let states = program.machine_states(public_machine);
            let state_position = |symbol: symbols::SymbolHandle| {
                states
                    .iter()
                    .position(|state| state.symbol == symbol)
                    .and_then(|position| u32::try_from(position).ok())
            };
            let Some(result_state_position) = state_position(flow.result_state_symbol) else {
                return Err("forwarded result flow lost its result state".to_owned());
            };
            let mut forwarding = Vec::with_capacity(flow.forwarding_edges.len());
            for edge in &flow.forwarding_edges {
                let (Some(source_position), Some(target_position)) = (
                    state_position(edge.source_state_symbol),
                    state_position(edge.target_state_symbol),
                ) else {
                    return Err("forwarded result flow lost a state position".to_owned());
                };
                require_positional_state_forwarding(
                    program,
                    &states[source_position as usize],
                    &states[target_position as usize],
                )?;
                forwarding.push(QuotientStateForwarding {
                    source_position,
                    target_position,
                });
            }
            Ok(QuotientResultFlow::Forwarded {
                machine_state_count: to_u32(states.len(), "machine state count")?,
                result_state_position,
                statement_position: to_u32(statement_position, "result statement position")?,
                immutable_alias_count: to_u32(flow.root.alias_count, "immutable alias count")?,
                forwarding,
            })
        }

        fn require_positional_state_forwarding(
            program: &dyn crate::CheckingInput,
            source: &State,
            target: &State,
        ) -> Result<(), String> {
            let [StatementNode::Transition(transition)] =
                program.statement_table.statements(source.statement_nodes)
            else {
                return Err("forwarding state lost its single transition".to_owned());
            };
            let typed_trees::statement::TransitionTargetNode::Named {
                path, arguments, ..
            } = program.statement_table.transition_target(transition.target)
            else {
                return Err("forwarding state lost its named target".to_owned());
            };
            if path.symbol != target.symbol {
                return Err("forwarding edge target drifted".to_owned());
            }
            let source_parameters = program.state_parameters(source);
            let target_parameters = program.state_parameters(target);
            let arguments = program.statement_table.expression_handles(*arguments);
            if source_parameters.len() != target_parameters.len()
                || arguments.len() != target_parameters.len()
            {
                return Err(
                    "forwarded result flow does not preserve the public telescope".to_owned(),
                );
            }
            for (position, (source_parameter, target_parameter)) in source_parameters
                .iter()
                .zip(target_parameters.iter())
                .enumerate()
            {
                if source_parameter.is_const != target_parameter.is_const
                    || source_parameter.is_mutable != target_parameter.is_mutable
                    || program.normalized_type_identity(source_parameter.type_reference)
                        != program.normalized_type_identity(target_parameter.type_reference)
                    || direct_public_parameter_symbol(program, arguments[position])
                        != Some(source_parameter.symbol)
                {
                    return Err(
                        "forwarded result flow does not preserve the public telescope".to_owned(),
                    );
                }
            }
            Ok(())
        }

        fn theorem_correspondence(
            program: &dyn crate::CheckingInput,
            plan: &DirectTerminalRelationPlan,
            certificate: &QuotientCorrespondenceCertificate,
            retain_legality: bool,
        ) -> Result<QuotientCongruenceCorrespondence, String> {
            let expected = &plan.expected_theorem_schema;
            let verified = &certificate.theorem;
            let selected_theorem = &plan.theorem_evidence[0].selected_application;
            if verified.theorem_machine_symbol != selected_theorem.machine_symbol
                || verified.theorem_state_symbol != selected_theorem.state_symbol
                || verified.parameters.len() != expected.parameters.len()
            {
                return Err("verified theorem identity or parameter roster drifted".to_owned());
            }
            let parameters = expected
                .parameters
                .iter()
                .enumerate()
                .map(|(position, parameter)| {
                    let verified_parameter = verified
                        .parameters
                        .iter()
                        .find(|parameter| parameter.expected_position == position)
                        .ok_or_else(|| {
                            "verified theorem parameter coordinate is absent".to_owned()
                        })?;
                    if verified_parameter.theorem_symbol
                        != selected_theorem.parameters[position].symbol
                    {
                        return Err("verified theorem parameter symbol drifted".to_owned());
                    }
                    let input_position =
                        to_u32(parameter.representative_position, "theorem input position")?;
                    let role = match parameter.role {
                        TheoremParameterRole::QuotientLeft => {
                            QuotientTheoremParameterRole::QuotientLeft { input_position }
                        }
                        TheoremParameterRole::QuotientRight => {
                            QuotientTheoremParameterRole::QuotientRight { input_position }
                        }
                        TheoremParameterRole::Shared => {
                            QuotientTheoremParameterRole::Shared { input_position }
                        }
                    };
                    Ok(QuotientTheoremParameter {
                        theorem_position: to_u32(position, "theorem parameter position")?,
                        role,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;

            let relation_premises = expected
                .relation_premises
                .iter()
                .enumerate()
                .map(|(position, premise)| {
                    let verified_premise = verified
                        .relation_premises
                        .iter()
                        .find(|premise| premise.expected_position == position)
                        .ok_or_else(|| "verified theorem relation premise is absent".to_owned())?;
                    Ok(QuotientTheoremRelationPremise {
                        expected_position: to_u32(position, "relation premise position")?,
                        actual: contract_coordinate(verified_premise.actual)?,
                        relation: canonical_symbol(program, premise.relation.relation_symbol)?,
                        left_parameter: to_u32(premise.left_parameter, "left theorem parameter")?,
                        right_parameter: to_u32(
                            premise.right_parameter,
                            "right theorem parameter",
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;

            Ok(QuotientCongruenceCorrespondence {
                parameters,
                relation_premises,
                legality_premises: if retain_legality {
                    expected
                        .legality_premises
                        .iter()
                        .enumerate()
                        .map(|(position, expected)| {
                            let verified = verified
                                .legality_premises
                                .iter()
                                .find(|premise| premise.expected_position == position)
                                .ok_or_else(|| {
                                    "verified theorem legality premise is absent".to_owned()
                                })?;
                            Ok(QuotientForwardPreconditionTransportFact {
                                application: expected.application,
                                source: contract_coordinate(expected.fact)?,
                                actual: contract_coordinate(verified.actual)?,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?
                } else {
                    Vec::new()
                },
                conclusion: QuotientTheoremConclusion {
                    actual: contract_coordinate(verified.conclusion)?,
                    relation: match expected.result_relation {
                        PositionalRelation::Quotient(relation) => QuotientResultRelation::Quotient(
                            canonical_symbol(program, relation.relation_symbol)?,
                        ),
                        PositionalRelation::ExactEquality(reference) => {
                            QuotientResultRelation::ExactEquality(canonical_type(
                                program, reference,
                            )?)
                        }
                    },
                    left: QuotientRepresentativeApplication {
                        arguments: expected
                            .left_application
                            .arguments
                            .iter()
                            .map(|position| to_u32(*position, "left application argument"))
                            .collect::<Result<Vec<_>, _>>()?,
                    },
                    right: QuotientRepresentativeApplication {
                        arguments: expected
                            .right_application
                            .arguments
                            .iter()
                            .map(|position| to_u32(*position, "right application argument"))
                            .collect::<Result<Vec<_>, _>>()?,
                    },
                },
            })
        }

        fn canonical_define_runtime_arguments(
            public: &[&typed_trees::signature::StateParameter],
            representative: &[super::representative::RepresentativeRuntimeParameter],
            runtime: &[super::runtime_correspondence::DefineRuntimePosition],
        ) -> Result<Vec<QuotientRuntimeArgument>, String> {
            if runtime.len() != public.len() || representative.len() != public.len() {
                return Err("faithful runtime correspondence arity drifted".to_owned());
            }
            runtime
                .iter()
                .enumerate()
                .map(|(position, runtime)| {
                    if runtime.public_parameter != public[position].symbol
                        || runtime.representative_parameter != representative[position].symbol
                    {
                        return Err(
                            "faithful runtime correspondence is not position preserving".to_owned()
                        );
                    }
                    canonical_runtime_position(position)
                })
                .collect()
        }

        fn canonical_lift_runtime_arguments(
            program: &dyn crate::CheckingInput,
            public: &[&typed_trees::signature::StateParameter],
            representative: &[super::representative::RepresentativeRuntimeParameter],
            runtime: &[super::runtime_correspondence::DirectLiftRuntimePosition],
        ) -> Result<Vec<QuotientRuntimeArgument>, String> {
            if runtime.len() != representative.len() {
                return Err("lift runtime correspondence arity drifted".to_owned());
            }
            runtime
                .iter()
                .zip(representative)
                .map(|(runtime, parameter)| {
                    if runtime.representative_parameter != parameter.symbol {
                        return Err(
                            "lift runtime correspondence lost representative order".to_owned()
                        );
                    }
                    match &runtime.source {
                        DirectLiftArgumentSource::PublicParameter(symbol) => {
                            let position = public
                                .iter()
                                .position(|parameter| parameter.symbol == *symbol)
                                .ok_or_else(|| {
                                    "lift runtime source is not a public parameter".to_owned()
                                })?;
                            canonical_runtime_position(position)
                        }
                        DirectLiftArgumentSource::Literal(literal) => {
                            Ok(QuotientRuntimeArgument::ClosedLiteral {
                                value_identity:
                                    super::correspondence_certificate::proof_value_for_literal(
                                        parameter.symbol,
                                        literal,
                                    )
                                    .canonical_identity()
                                    .to_owned(),
                                representative_type: canonical_type(
                                    program,
                                    parameter.type_reference,
                                )?,
                            })
                        }
                    }
                })
                .collect()
        }

        fn canonical_runtime_position(position: usize) -> Result<QuotientRuntimeArgument, String> {
            Ok(QuotientRuntimeArgument::PublicParameter {
                public_position: to_u32(position, "public parameter position")?,
            })
        }

        fn transport_correspondence(
            transport: &VerifiedForwardPreconditionTransportSchema,
        ) -> Result<QuotientForwardPreconditionTransportCorrespondence, String> {
            fn row(
                fact: &super::transport_schema::VerifiedTransportFact,
            ) -> Result<QuotientForwardPreconditionTransportFact, String> {
                Ok(QuotientForwardPreconditionTransportFact {
                    application: fact.application,
                    source: fact_coordinate(
                        fact.source,
                        "source contract position",
                        "source contract fact position",
                    )?,
                    actual: contract_coordinate(fact.actual)?,
                })
            }
            Ok(QuotientForwardPreconditionTransportCorrespondence {
                public_premises: transport
                    .public_premises
                    .iter()
                    .map(row)
                    .collect::<Result<Vec<_>, _>>()?,
                representative_conclusions: transport
                    .representative_conclusions
                    .iter()
                    .map(row)
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }

        pub(crate) fn relation_identity(
            program: &dyn crate::CheckingInput,
            relation: ExactQuotientRelation,
        ) -> Result<QuotientRelationIdentity, String> {
            let quotient = crate::declarations::data_queries::data_definition_by_symbol(
                program,
                relation.quotient_symbol,
            )
            .ok_or_else(|| "quotient relation lost its declaration".to_owned())?;
            if !program.data_type_parameters(quotient).is_empty() {
                return Err(
                    "the proof-only bridge excludes generic quotient declarations".to_owned(),
                );
            }
            let formation = quotient
                .quotient
                .as_ref()
                .ok_or_else(|| "quotient relation lost its formation".to_owned())?;
            if formation.relation_symbol != relation.relation_symbol {
                return Err("quotient relation identity drifted from formation".to_owned());
            }
            Ok(QuotientRelationIdentity {
                quotient_declaration: canonical_symbol(program, relation.quotient_symbol)?,
                quotient_type: canonical_type(program, relation.quotient_type)?,
                carrier_type: canonical_type(program, formation.carrier)?,
                relation: canonical_symbol(program, relation.relation_symbol)?,
            })
        }

        fn require_empty_partition(
            partition: Option<&super::precondition::RepresentativePreconditionPartition>,
            side: &str,
        ) -> Result<(), String> {
            let partition =
                partition.ok_or_else(|| format!("{side} precondition partition is absent"))?;
            if !partition.dependent.is_empty() || !partition.fixed.is_empty() {
                return Err(format!(
                    "the proof-only bridge admits no {side} preconditions"
                ));
            }
            Ok(())
        }

        fn exact_machine_state<'a>(
            program: &'a dyn crate::CheckingInput,
            machine_symbol: symbols::SymbolHandle,
            state_symbol: symbols::SymbolHandle,
            label: &str,
        ) -> Result<(&'a Machine, &'a State), String> {
            let matches = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == machine_symbol)
                .flat_map(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .filter(move |state| state.symbol == state_symbol)
                        .map(move |state| (machine, state))
                })
                .collect::<Vec<_>>();
            let [found] = matches.as_slice() else {
                return Err(format!("{label} does not resolve exactly"));
            };
            Ok(*found)
        }

        fn require_single_entry(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            label: &str,
        ) -> Result<(), String> {
            let [entry] = program.machine_states(machine) else {
                return Err(format!("{label} must have exactly one state"));
            };
            if entry.symbol != state.symbol {
                return Err(format!("{label} retained a non-entry state"));
            }
            Ok(())
        }

        fn require_empty_owner_telescope(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            label: &str,
        ) -> Result<(), String> {
            if !machine.lifetime_parameters.is_empty()
                || !program.machine_type_parameters(machine).is_empty()
            {
                return Err(format!("{label} must have an empty static telescope"));
            }
            Ok(())
        }

        fn require_plain_immutable_parameters(
            parameters: impl IntoIterator<Item = bool>,
            label: &str,
        ) -> Result<(), String> {
            if parameters.into_iter().any(|is_mutable| is_mutable) {
                return Err(format!("{label} parameters must be immutable"));
            }
            Ok(())
        }

        fn machine_application(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
        ) -> Result<QuotientMachineApplication, String> {
            Ok(QuotientMachineApplication {
                callable: callable_identity(program, machine)?,
                static_application: QuotientStaticApplication {
                    bindings: Vec::new(),
                },
            })
        }

        fn callable_identity(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
        ) -> Result<QuotientCallableIdentity, String> {
            Ok(QuotientCallableIdentity {
                declaration: canonical_symbol(program, machine.symbol)?,
                overload: program
                    .machine_callable_identity_text(machine)
                    .ok_or_else(|| "callable has no canonical overload identity".to_owned())?,
            })
        }

        fn canonical_symbol(
            program: &dyn crate::CheckingInput,
            symbol: symbols::SymbolHandle,
        ) -> Result<String, String> {
            program.normalized_hermetic_symbol_identity(symbol)
        }

        fn canonical_type(
            program: &dyn crate::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> Result<String, String> {
            let identity = program.package_qualified_type_identity_text(type_reference);
            if identity.is_empty() || identity.contains("unresolved-owner") {
                return Err("type identity is not hermetic".to_owned());
            }
            Ok(identity)
        }

        fn fact_coordinate(
            location: ContractFactLocation,
            position_label: &str,
            fact_label: &str,
        ) -> Result<QuotientContractFactCoordinate, String> {
            Ok(QuotientContractFactCoordinate {
                owner: match location.owner {
                    ContractOwner::Machine => QuotientContractOwner::Machine,
                    ContractOwner::State => QuotientContractOwner::State,
                },
                contract_position: to_u32(location.contract_position, position_label)?,
                fact_position: to_u32(location.fact_position, fact_label)?,
            })
        }

        fn contract_coordinate(
            location: ContractFactLocation,
        ) -> Result<QuotientContractFactCoordinate, String> {
            fact_coordinate(location, "contract position", "contract fact position")
        }

        fn to_u32(value: usize, label: &str) -> Result<u32, String> {
            u32::try_from(value).map_err(|_| format!("{label} does not fit canonical identity"))
        }
    }
    pub(crate) mod precondition {
        use super::proof_fact_identity::{
            ProofFactIdentityContext, ProofValueSubstitution, proof_facts_match,
        };
        use super::runtime_correspondence::DirectLiftArgumentSource;
        use super::{
            DefineRuntimeCorrespondence, DefineRuntimePosition, DirectLiftRuntimeCorrespondence,
            PositionalRelation, RelationPlanError, RepresentativeStaticBinding,
            RepresentativeTelescope,
        };
        use crate::frozen_program::expression_table;
        use arena::HandleSpan;
        use symbols::SymbolHandle;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::signature::{SignatureContract, SignatureContractKind};
        use typed_trees::state::State;

        pub(crate) use super::{ContractFactLocation, ContractOwner};

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct RepresentativePreconditionPartition {
            pub(crate) dependent: Vec<ContractFactLocation>,
            pub(crate) fixed: Vec<ContractFactLocation>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct DefinePreconditionFactPair {
            pub(crate) public: ContractFactLocation,
            pub(crate) representative: ContractFactLocation,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct DefinePreconditionCorrespondence {
            pub(crate) dependent: Vec<DefinePreconditionFactPair>,
            pub(crate) fixed: Vec<DefinePreconditionFactPair>,
        }

        pub(crate) fn derive_public_precondition_partition(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            input_relations: &[PositionalRelation],
            runtime_positions: &[DefineRuntimePosition],
        ) -> Result<RepresentativePreconditionPartition, RelationPlanError> {
            derive_public_precondition_partition_from_positions(
                program,
                machine,
                state,
                input_relations,
                &runtime_positions
                    .iter()
                    .map(|position| Some(position.public_parameter))
                    .collect::<Vec<_>>(),
                RelationPlanError::DefineRuntimeArityMismatch,
            )
        }

        pub(crate) fn derive_direct_lift_public_precondition_partition(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            input_relations: &[PositionalRelation],
            runtime: &DirectLiftRuntimeCorrespondence,
        ) -> Result<RepresentativePreconditionPartition, RelationPlanError> {
            derive_public_precondition_partition_from_positions(
                program,
                machine,
                state,
                input_relations,
                &runtime
                    .positions
                    .iter()
                    .map(|position| match &position.source {
                        DirectLiftArgumentSource::PublicParameter(symbol) => Some(*symbol),
                        DirectLiftArgumentSource::Literal(_) => None,
                    })
                    .collect::<Vec<_>>(),
                RelationPlanError::DirectLiftRuntimeArityMismatch,
            )
        }

        fn derive_public_precondition_partition_from_positions(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            input_relations: &[PositionalRelation],
            public_positions: &[Option<SymbolHandle>],
            arity_error: RelationPlanError,
        ) -> Result<RepresentativePreconditionPartition, RelationPlanError> {
            let public_parameters = program.public_state_parameters(state).collect::<Vec<_>>();
            if public_positions.len() != input_relations.len() {
                return Err(arity_error);
            }
            let mut varying_parameters = input_relations
                .iter()
                .zip(public_positions)
                .filter_map(|(relation, position)| {
                    matches!(relation, PositionalRelation::Quotient(_))
                        .then_some(*position)
                        .flatten()
                })
                .collect::<Vec<_>>();
            for parameter in public_parameters {
                if !public_positions.contains(&Some(parameter.symbol))
                    && crate::proof_contracts::quotients::equivalence_selection::quotient_for_type(
                        program,
                        parameter.type_reference,
                    )
                    .is_some()
                {
                    varying_parameters.push(parameter.symbol);
                }
            }
            derive_precondition_partition(
                program,
                machine.contracts,
                state.contracts,
                &varying_parameters,
            )
        }

        pub(crate) fn derive_representative_precondition_partition(
            program: &dyn crate::CheckingInput,
            input_relations: &[PositionalRelation],
            representative: &RepresentativeTelescope,
        ) -> Result<RepresentativePreconditionPartition, RelationPlanError> {
            let varying_parameters = input_relations
                .iter()
                .zip(&representative.parameters)
                .filter_map(|(relation, parameter)| {
                    matches!(relation, PositionalRelation::Quotient(_)).then_some(parameter.symbol)
                })
                .collect::<Vec<_>>();
            derive_precondition_partition(
                program,
                representative.machine_contracts,
                representative.state_contracts,
                &varying_parameters,
            )
        }

        fn derive_precondition_partition(
            program: &dyn crate::CheckingInput,
            machine_contracts: HandleSpan<SignatureContract>,
            state_contracts: HandleSpan<SignatureContract>,
            varying_parameters: &[SymbolHandle],
        ) -> Result<RepresentativePreconditionPartition, RelationPlanError> {
            let mut partition = RepresentativePreconditionPartition {
                dependent: Vec::new(),
                fixed: Vec::new(),
            };
            for (owner, contracts) in [
                (
                    ContractOwner::Machine,
                    program.signature_contracts.span_or_empty(machine_contracts),
                ),
                (
                    ContractOwner::State,
                    program.signature_contracts.span_or_empty(state_contracts),
                ),
            ] {
                for (contract_position, contract) in contracts.iter().enumerate() {
                    if contract.kind != SignatureContractKind::Requires {
                        continue;
                    }
                    for (fact_position, fact) in program
                        .proof_facts
                        .span_or_empty(contract.facts)
                        .iter()
                        .enumerate()
                    {
                        let location = ContractFactLocation {
                            owner,
                            contract_position,
                            fact_position,
                        };
                        if proof_fact_depends_on_any(program, fact, varying_parameters)? {
                            partition.dependent.push(location);
                        } else {
                            partition.fixed.push(location);
                        }
                    }
                }
            }
            Ok(partition)
        }

        pub(crate) fn derive_define_precondition_correspondence(
            program: &dyn crate::CheckingInput,
            public_machine: &Machine,
            public_state: &State,
            representative: &RepresentativeTelescope,
            public: &RepresentativePreconditionPartition,
            representative_partition: &RepresentativePreconditionPartition,
            runtime: &DefineRuntimeCorrespondence,
        ) -> Result<DefinePreconditionCorrespondence, RelationPlanError> {
            let public_substitutions = runtime
                .positions
                .iter()
                .enumerate()
                .map(|(position, binding)| {
                    ProofValueSubstitution::symbolic(
                        binding.public_parameter,
                        format!("${position}"),
                    )
                })
                .collect::<Vec<_>>();
            let representative_substitutions = runtime
                .positions
                .iter()
                .enumerate()
                .map(|(position, binding)| {
                    ProofValueSubstitution::symbolic(
                        binding.representative_parameter,
                        format!("${position}"),
                    )
                })
                .collect::<Vec<_>>();
            let dependent = pair_precondition_facts(
                program,
                public_machine.contracts,
                public_state.contracts,
                representative.machine_contracts,
                representative.state_contracts,
                &public.dependent,
                &representative_partition.dependent,
                &public_substitutions,
                &representative_substitutions,
                &representative.static_application.bindings,
            )?;
            let fixed = pair_precondition_facts(
                program,
                public_machine.contracts,
                public_state.contracts,
                representative.machine_contracts,
                representative.state_contracts,
                &public.fixed,
                &representative_partition.fixed,
                &public_substitutions,
                &representative_substitutions,
                &representative.static_application.bindings,
            )?;
            Ok(DefinePreconditionCorrespondence { dependent, fixed })
        }

        #[allow(clippy::too_many_arguments)]
        fn pair_precondition_facts(
            program: &dyn crate::CheckingInput,
            public_machine_contracts: HandleSpan<SignatureContract>,
            public_state_contracts: HandleSpan<SignatureContract>,
            representative_machine_contracts: HandleSpan<SignatureContract>,
            representative_state_contracts: HandleSpan<SignatureContract>,
            public_locations: &[ContractFactLocation],
            representative_locations: &[ContractFactLocation],
            public_substitutions: &[ProofValueSubstitution],
            representative_substitutions: &[ProofValueSubstitution],
            representative_static: &[RepresentativeStaticBinding],
        ) -> Result<Vec<DefinePreconditionFactPair>, RelationPlanError> {
            if public_locations.len() != representative_locations.len() {
                return Err(RelationPlanError::DefinePreconditionMismatch);
            }
            let mut unmatched = representative_locations.to_vec();
            let mut pairs = Vec::with_capacity(public_locations.len());
            for public_location in public_locations {
                let public_fact = precondition_fact_at(
                    program,
                    public_machine_contracts,
                    public_state_contracts,
                    *public_location,
                )
                .ok_or(RelationPlanError::DefinePreconditionMismatch)?;
                let Some((position, representative_location)) =
                    unmatched.iter().copied().enumerate().find(|(_, location)| {
                        precondition_fact_at(
                            program,
                            representative_machine_contracts,
                            representative_state_contracts,
                            *location,
                        )
                        .is_some_and(|representative_fact| {
                            proof_facts_match(
                                program,
                                public_fact,
                                representative_fact,
                                ProofFactIdentityContext {
                                    values: public_substitutions,
                                    static_bindings: &[],
                                },
                                ProofFactIdentityContext {
                                    values: representative_substitutions,
                                    static_bindings: representative_static,
                                },
                            )
                        })
                    })
                else {
                    return Err(RelationPlanError::DefinePreconditionMismatch);
                };
                unmatched.remove(position);
                pairs.push(DefinePreconditionFactPair {
                    public: *public_location,
                    representative: representative_location,
                });
            }
            Ok(pairs)
        }

        pub(crate) fn precondition_fact_at(
            program: &dyn crate::CheckingInput,
            machine_contracts: HandleSpan<SignatureContract>,
            state_contracts: HandleSpan<SignatureContract>,
            location: ContractFactLocation,
        ) -> Option<&ProofFact> {
            let contracts = match location.owner {
                ContractOwner::Machine => {
                    program.signature_contracts.span_or_empty(machine_contracts)
                }
                ContractOwner::State => program.signature_contracts.span_or_empty(state_contracts),
            };
            let contract = contracts.get(location.contract_position)?;
            if contract.kind != SignatureContractKind::Requires {
                return None;
            }
            program
                .proof_facts
                .span_or_empty(contract.facts)
                .get(location.fact_position)
        }

        fn proof_fact_depends_on_any(
            program: &dyn crate::CheckingInput,
            fact: &ProofFact,
            parameters: &[SymbolHandle],
        ) -> Result<bool, RelationPlanError> {
            match fact {
                ProofFact::Expression(expression) => {
                    expression_depends_on_any(program, *expression, parameters)
                }
                ProofFact::Membership(membership) => {
                    expression_depends_on_any(program, membership.value, parameters)
                }
                ProofFact::Proposition(application) => expression_table(program)
                    .expression_handles(application.arguments)
                    .iter()
                    .try_fold(false, |depends, expression| {
                        let expression_depends =
                            expression_depends_on_any(program, *expression, parameters)?;
                        Ok(depends || expression_depends)
                    }),
            }
        }

        fn expression_depends_on_any(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            parameters: &[SymbolHandle],
        ) -> Result<bool, RelationPlanError> {
            let edges = typed_trees::SubtreeEdges {
                atomic_results: true,
                ..typed_trees::SubtreeEdges::SAVED_SOURCES
            };
            let (mut depends, mut unresolved) = (false, false);
            program.subtree_for_each(expression, edges, |_, node| match node {
                ExpressionNode::SavedSubject(saved) => {
                    unresolved |= program.saved_value_source(saved).is_none();
                }
                ExpressionNode::Name(path)
                    if !path.symbol.is_valid() && !path.head_symbol.is_valid() =>
                {
                    unresolved = true;
                }
                ExpressionNode::Name(path) => {
                    depends |=
                        parameters.contains(&path.symbol) || parameters.contains(&path.head_symbol);
                }
                _ => {}
            });
            if unresolved {
                return Err(RelationPlanError::PreconditionDependencyUnresolved);
            }
            Ok(depends)
        }
    }
    pub(crate) mod proof_fact_identity {
        use super::{RepresentativeStaticApplication, RepresentativeStaticBinding};
        use crate::frozen_program::expression_table;
        use numerics::literals::FloatFormat;
        use numerics::literals::IntegerLanding;
        use source::SourceBytes;
        use std::sync::Arc;
        use symbols::JoinedPath;
        use symbols::SymbolHandle;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::MatchPattern;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
        use typed_trees::proposition::ProofSubstitutions;
        use typed_trees::type_identity::TypeIdentityRequest;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct ProofValueSubstitution {
            pub(crate) symbol: SymbolHandle,
            rendered: String,
            trace: String,
        }

        fn nested_array_rendered<T: std::fmt::Debug>(rows: &[impl AsRef<[T]>]) -> String {
            format!(
                "[{}]",
                rows.iter()
                    .map(|row| format!("{:?}", row.as_ref()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }

        impl ProofValueSubstitution {
            pub(crate) fn canonical_identity(&self) -> &str {
                &self.trace
            }

            pub(crate) fn symbolic(symbol: SymbolHandle, identity: String) -> Self {
                Self {
                    symbol,
                    rendered: identity.clone(),
                    trace: identity,
                }
            }

            /// The public quotient input is the projection of a hypothetical raw
            /// representative. Keep the exact selected quotient type in both keys;
            /// this never identifies the two carriers or changes the raw P view.
            pub(crate) fn quotient_projection(
                &self,
                program: &dyn crate::CheckingInput,
                quotient: typed_trees::types::TypeReferenceHandle,
            ) -> Self {
                Self {
                    symbol: self.symbol,
                    rendered: format!(
                        "({} as {})",
                        self.rendered,
                        program.type_reference_table.display_name(quotient)
                    ),
                    trace: format!(
                        "{}:{:?}:{}",
                        self.trace,
                        SymbolHandle::invalid(),
                        program.normalized_type_identity_text(quotient)
                    ),
                }
            }

            pub(crate) fn boolean(symbol: SymbolHandle, value: bool) -> Self {
                Self {
                    symbol,
                    rendered: value.to_string(),
                    trace: boolean_trace(value),
                }
            }

            pub(crate) fn integer(
                symbol: SymbolHandle,
                spelling: &str,
                landing: IntegerLanding,
            ) -> Self {
                Self {
                    symbol,
                    rendered: spelling.to_owned(),
                    trace: integer_trace(spelling, landing),
                }
            }

            pub(crate) fn float(
                symbol: SymbolHandle,
                spelling: &str,
                landing: FloatFormat,
            ) -> Self {
                Self {
                    symbol,
                    rendered: spelling.to_owned(),
                    trace: float_trace(spelling, Some(landing)),
                }
            }

            pub(crate) fn byte_string(symbol: SymbolHandle, bytes: &[u8]) -> Self {
                Self {
                    symbol,
                    rendered: format!("{bytes:?}"),
                    trace: byte_string_trace(bytes),
                }
            }

            pub(crate) fn fixed_byte_array(symbol: SymbolHandle, bytes: &[u8]) -> Self {
                Self {
                    symbol,
                    rendered: format!("{bytes:?}"),
                    trace: array_trace(bytes.iter().map(|byte| format!("integer:{byte}:unlanded"))),
                }
            }

            pub(crate) fn boolean_array(symbol: SymbolHandle, values: &[bool]) -> Self {
                Self {
                    symbol,
                    rendered: format!("{values:?}"),
                    trace: array_trace(values.iter().map(|value| boolean_trace(*value))),
                }
            }

            pub(crate) fn nested_fixed_byte_array(
                symbol: SymbolHandle,
                rows: &[SourceBytes],
            ) -> Self {
                Self {
                    symbol,
                    rendered: nested_array_rendered(rows),
                    trace: array_trace(rows.iter().map(|row| {
                        array_trace(row.iter().map(|byte| format!("integer:{byte}:unlanded")))
                    })),
                }
            }

            pub(crate) fn nested_boolean_array(symbol: SymbolHandle, rows: &[Arc<[bool]>]) -> Self {
                Self {
                    symbol,
                    rendered: nested_array_rendered(rows),
                    trace: array_trace(
                        rows.iter()
                            .map(|row| array_trace(row.iter().map(|value| boolean_trace(*value)))),
                    ),
                }
            }

            pub(crate) fn boolean_tensor3(
                symbol: SymbolHandle,
                planes: &[Arc<[Arc<[bool]>]>],
            ) -> Self {
                Self {
                    symbol,
                    rendered: format!(
                        "[{}]",
                        planes
                            .iter()
                            .map(|plane| format!(
                                "[{}]",
                                plane
                                    .iter()
                                    .map(|row| format!("{:?}", row.as_ref()))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    trace: array_trace(planes.iter().map(|plane| {
                        array_trace(
                            plane.iter().map(|row| {
                                array_trace(row.iter().map(|value| boolean_trace(*value)))
                            }),
                        )
                    })),
                }
            }

            pub(crate) fn recursive_primitive_array(
                symbol: SymbolHandle,
                elements: &[super::runtime_correspondence::ClosedRecursiveArrayElement],
            ) -> Self {
                Self {
                    symbol,
                    rendered: render_recursive_array(elements),
                    trace: array_trace(elements.iter().map(recursive_array_element_trace)),
                }
            }

            pub(crate) fn integer_array(
                symbol: SymbolHandle,
                elements: impl IntoIterator<Item = (String, IntegerLanding)>,
            ) -> Self {
                Self::flat_array(symbol, elements, integer_trace)
            }

            pub(crate) fn nested_integer_array(
                symbol: SymbolHandle,
                rows: &[Arc<[super::runtime_correspondence::ClosedIntegerArrayElement]>],
            ) -> Self {
                Self::nested_array(symbol, rows, |element| {
                    integer_trace(&element.spelling, element.landing)
                })
            }

            pub(crate) fn float_array(
                symbol: SymbolHandle,
                elements: impl IntoIterator<Item = (String, FloatFormat)>,
            ) -> Self {
                Self::flat_array(symbol, elements, |spelling, landing| {
                    float_trace(spelling, Some(landing))
                })
            }

            pub(crate) fn nested_float_array(
                symbol: SymbolHandle,
                rows: &[Arc<[super::runtime_correspondence::ClosedFloatArrayElement]>],
            ) -> Self {
                Self::nested_array(symbol, rows, |element| {
                    float_trace(&element.spelling, Some(element.landing))
                })
            }

            fn flat_array<L: Copy>(
                symbol: SymbolHandle,
                elements: impl IntoIterator<Item = (String, L)>,
                element_trace: impl Fn(&str, L) -> String,
            ) -> Self {
                let elements = elements.into_iter().collect::<Vec<_>>();
                Self {
                    symbol,
                    rendered: format!(
                        "[{}]",
                        elements
                            .iter()
                            .map(|(spelling, _)| spelling.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    trace: array_trace(
                        elements
                            .iter()
                            .map(|(spelling, landing)| element_trace(spelling, *landing)),
                    ),
                }
            }

            fn nested_array<E: SpelledClosedElement>(
                symbol: SymbolHandle,
                rows: &[Arc<[E]>],
                element_trace: impl Fn(&E) -> String,
            ) -> Self {
                Self {
                    symbol,
                    rendered: format!(
                        "[{}]",
                        rows.iter()
                            .map(|row| {
                                format!(
                                    "[{}]",
                                    row.iter()
                                        .map(|element| element.spelling())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    trace: array_trace(
                        rows.iter()
                            .map(|row| array_trace(row.iter().map(&element_trace))),
                    ),
                }
            }

            pub(crate) fn rebound(&self, symbol: SymbolHandle) -> Self {
                Self {
                    symbol,
                    rendered: self.rendered.clone(),
                    trace: self.trace.clone(),
                }
            }
        }

        pub(crate) trait SpelledClosedElement {
            fn spelling(&self) -> &str;
        }

        impl SpelledClosedElement for super::runtime_correspondence::ClosedIntegerArrayElement {
            fn spelling(&self) -> &str {
                &self.spelling
            }
        }

        impl SpelledClosedElement for super::runtime_correspondence::ClosedFloatArrayElement {
            fn spelling(&self) -> &str {
                &self.spelling
            }
        }

        fn render_recursive_array(
            elements: &[super::runtime_correspondence::ClosedRecursiveArrayElement],
        ) -> String {
            format!(
                "[{}]",
                elements
                    .iter()
                    .map(render_recursive_array_element)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }

        fn render_recursive_array_element(
            element: &super::runtime_correspondence::ClosedRecursiveArrayElement,
        ) -> String {
            use super::runtime_correspondence::ClosedRecursiveArrayElement;
            match element {
                ClosedRecursiveArrayElement::Boolean(value) => value.to_string(),
                ClosedRecursiveArrayElement::Byte(value) => value.to_string(),
                ClosedRecursiveArrayElement::Integer(value) => value.spelling.clone(),
                ClosedRecursiveArrayElement::Float(value) => value.spelling.clone(),
                ClosedRecursiveArrayElement::Array(elements) => render_recursive_array(elements),
            }
        }

        fn recursive_array_element_trace(
            element: &super::runtime_correspondence::ClosedRecursiveArrayElement,
        ) -> String {
            use super::runtime_correspondence::ClosedRecursiveArrayElement;
            match element {
                ClosedRecursiveArrayElement::Boolean(value) => boolean_trace(*value),
                ClosedRecursiveArrayElement::Byte(value) => format!("integer:{value}:unlanded"),
                ClosedRecursiveArrayElement::Integer(value) => {
                    integer_trace(&value.spelling, value.landing)
                }
                ClosedRecursiveArrayElement::Float(value) => {
                    float_trace(&value.spelling, Some(value.landing))
                }
                ClosedRecursiveArrayElement::Array(elements) => {
                    array_trace(elements.iter().map(recursive_array_element_trace))
                }
            }
        }

        pub(crate) struct ProofFactIdentityContext<'a> {
            pub(crate) values: &'a [ProofValueSubstitution],
            pub(crate) static_bindings: &'a [RepresentativeStaticBinding],
        }

        pub(crate) fn proof_facts_match(
            program: &dyn crate::CheckingInput,
            left: &ProofFact,
            right: &ProofFact,
            left_context: ProofFactIdentityContext<'_>,
            right_context: ProofFactIdentityContext<'_>,
        ) -> bool {
            let expression_matches = |left, right| {
                matches!((
            proof_expression_identity(program, left, left_context.values),
            proof_expression_identity(program, right, right_context.values)
        ), (Some(left), Some(right)) if left == right)
            };
            match (left, right) {
                (ProofFact::Expression(left), ProofFact::Expression(right)) => {
                    expression_matches(*left, *right)
                }
                (ProofFact::Membership(left), ProofFact::Membership(right)) => {
                    left.domain_symbol.is_valid()
                        && left.domain_symbol == right.domain_symbol
                        && membership_arguments_match(
                            program,
                            left,
                            right,
                            left_context.static_bindings,
                            right_context.static_bindings,
                        )
                        && expression_matches(left.value, right.value)
                }
                (ProofFact::Proposition(left), ProofFact::Proposition(right)) => {
                    left.proposition.is_valid()
                        && left.proposition == right.proposition
                        && left.binder_arguments.len() == right.binder_arguments.len()
                        && left
                            .binder_arguments
                            .iter()
                            .zip(&right.binder_arguments)
                            .all(|(left, right)| {
                                left.kind == right.kind
                                    && binder_argument_identity(left, left_context.static_bindings)
                                        == binder_argument_identity(
                                            right,
                                            right_context.static_bindings,
                                        )
                            })
                        && {
                            let left_arguments =
                                expression_table(program).expression_handles(left.arguments);
                            let right_arguments =
                                expression_table(program).expression_handles(right.arguments);
                            left_arguments.len() == right_arguments.len()
                                && left_arguments
                                    .iter()
                                    .zip(right_arguments)
                                    .all(|(left, right)| expression_matches(*left, *right))
                        }
                }
                _ => false,
            }
        }

        fn membership_arguments_match(
            program: &dyn crate::CheckingInput,
            left: &typed_trees::domain::ProofMembershipFact,
            right: &typed_trees::domain::ProofMembershipFact,
            left_bindings: &[RepresentativeStaticBinding],
            right_bindings: &[RepresentativeStaticBinding],
        ) -> bool {
            let left_arguments = program
                .type_reference_table
                .type_reference_handles(left.domain_arguments);
            let right_arguments = program
                .type_reference_table
                .type_reference_handles(right.domain_arguments);
            left_arguments.len() == left.domain_arguments.len()
                && right_arguments.len() == right.domain_arguments.len()
                && left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(left, right)| {
                        let independent = |argument, bindings: &[RepresentativeStaticBinding]| {
                            let first = bindings
                                .iter()
                                .map(|binding| {
                                    (binding.parameter, "membership-static-a".to_owned())
                                })
                                .collect::<Vec<_>>();
                            let second = bindings
                                .iter()
                                .map(|binding| {
                                    (binding.parameter, "membership-static-b".to_owned())
                                })
                                .collect::<Vec<_>>();
                            program.type_identity(TypeIdentityRequest {
                                binders: &first,
                                ..TypeIdentityRequest::ordinary(argument)
                            }) == program.type_identity(TypeIdentityRequest {
                                binders: &second,
                                ..TypeIdentityRequest::ordinary(argument)
                            })
                        };
                        program.type_reference_table.contains_type_reference(*left)
                            && program.type_reference_table.contains_type_reference(*right)
                            && independent(*left, left_bindings)
                            && independent(*right, right_bindings)
                            && program.normalized_type_identity(*left)
                                == program.normalized_type_identity(*right)
                    })
        }

        fn proof_expression_identity(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            values: &[ProofValueSubstitution],
        ) -> Option<(String, String)> {
            typed_trees::type_identity::normalized_expression_identity(program, expression, &[])?;
            let mut expressions = Vec::new();
            crate::queries::expression_traversal::collect_expression_tree(
                program,
                expression,
                &mut expressions,
            );
            if expressions.iter().any(|expression| {
                matches!(
                    expression_table(program).expression(*expression),
                    ExpressionNode::SavedSubject(_)
                )
            }) {
                return None;
            }
            let rendered_values = values
                .iter()
                .map(|value| (value.symbol, value.rendered.clone()))
                .collect::<Vec<_>>();
            let trace = expression_symbol_trace(program, expression, values)?;
            Some((
                program.render_proof_expression(
                    expression,
                    ProofSubstitutions::BySymbol(&rendered_values),
                ),
                trace,
            ))
        }

        fn expression_symbol_trace(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            values: &[ProofValueSubstitution],
        ) -> Option<String> {
            let trace = |expression| expression_symbol_trace(program, expression, values);
            Some(match expression_table(program).expression(expression) {
                ExpressionNode::MachineValue(_) => return None,
                ExpressionNode::SavedSubject(saved) => {
                    format!("saved-subject:{}", trace(saved.pattern_head)?)
                }
                ExpressionNode::Match(dispatch) => {
                    let arms = expression_table(program)
                        .match_arms(dispatch.arms)
                        .iter()
                        .map(|arm| {
                            let pattern = match arm.pattern {
                                MatchPattern::Value(pattern) => {
                                    format!("value:{}", trace(pattern)?)
                                }
                                MatchPattern::Wildcard => "wildcard".to_owned(),
                                MatchPattern::Destructure(pattern) => format!(
                                    "destructure:[{}]",
                                    expression_table(program)
                                        .match_pattern_dependencies(MatchPattern::Destructure(
                                            pattern
                                        ))
                                        .map(trace)
                                        .collect::<Option<Vec<_>>>()?
                                        .join(",")
                                ),
                            };
                            Some(format!("({pattern}=>{})", trace(arm.value)?))
                        })
                        .collect::<Option<Vec<_>>>()?
                        .join(",");
                    format!("match:{}:[{arms}]", trace(dispatch.subject)?)
                }
                ExpressionNode::Name(path) => {
                    let members = expression_table(program).name_path_members(path.members);
                    if members.len() == 1
                        && let Some(value) = values.iter().find(|value| {
                            value.symbol == path.symbol || value.symbol == path.head_symbol
                        })
                    {
                        return Some(value.trace.clone());
                    }
                    format!(
                        "name:{:?}:{:?}:[{}]",
                        path.head_symbol,
                        path.symbol,
                        members.joined_path()
                    )
                }
                ExpressionNode::Call(call) => {
                    if call.runtime_call.is_some() {
                        return None;
                    }
                    let mut identity = format!(
                        "call:{:?}:{}:{:?}:{:?}:{:?}:{:?}:[{}]:({})",
                        call.target_symbol,
                        call.machine_arguments
                            .iter()
                            .map(static_argument_identity)
                            .collect::<Vec<_>>()
                            .join(","),
                        call.quotient_operation,
                        call.private_layout_operation,
                        call.evidence_arguments,
                        call.operational_acknowledgement,
                        if call.receiver.is_valid() {
                            trace(call.receiver)?
                        } else {
                            Default::default()
                        },
                        expression_table(program)
                            .expression_handles(call.arguments)
                            .iter()
                            .map(|argument| trace(*argument))
                            .collect::<Option<Vec<_>>>()?
                            .join(","),
                    );
                    if let Some(request) = &call.installed_interrupt_operation {
                        use std::fmt::Write as _;
                        let _ = write!(identity, ":installed-interrupt:{request:?}");
                    }
                    identity
                }
                ExpressionNode::ArrayLiteral(values_span) => array_trace(
                    expression_table(program)
                        .expression_handles(*values_span)
                        .iter()
                        .map(|value| trace(*value))
                        .collect::<Option<Vec<_>>>()?,
                ),
                ExpressionNode::Atomic(value) => trace(value.value)?,
                ExpressionNode::Binary(value) => {
                    format!("{}|{}", trace(value.left)?, trace(value.right)?)
                }
                ExpressionNode::Cast(value) => format!(
                    "{}:{:?}:{}",
                    trace(value.value)?,
                    value.semantic_domain_symbol,
                    program.normalized_type_identity_text(value.target_type)
                ),
                ExpressionNode::Indexed(value) => {
                    format!("{}|{}", trace(value.collection)?, trace(value.index)?)
                }
                ExpressionNode::Member(value) => {
                    format!("{}:{:?}", trace(value.receiver)?, value.member_symbol)
                }
                ExpressionNode::Borrow(value) => trace(value.target)?,
                ExpressionNode::Range(value) => {
                    format!("{}|{}", trace(value.start)?, trace(value.end)?)
                }
                ExpressionNode::StructLiteral(value) => format!(
                    "struct:{:?}:{:?}:[{}]",
                    value.type_symbol,
                    value.case_symbol,
                    expression_table(program)
                        .struct_fields(value.fields)
                        .iter()
                        .map(|field| Some(format!(
                            "{:?}={}",
                            field.field_symbol,
                            trace(field.value)?
                        )))
                        .collect::<Option<Vec<_>>>()?
                        .join(",")
                ),
                ExpressionNode::Unary(value) => trace(value.operand)?,
                ExpressionNode::TypeExpression(value) => {
                    format!("type:{}", program.normalized_type_identity_text(*value))
                }
                ExpressionNode::ZeroValue(value) => {
                    format!("zero:{}", program.normalized_type_identity_text(*value))
                }
                ExpressionNode::Boolean(value) => boolean_trace(*value),
                ExpressionNode::Integer(value) => value.landing().map_or_else(
                    || format!("integer:{value}:unlanded"),
                    |landing| integer_trace(value.canonical_spelling().as_str(), landing),
                ),
                ExpressionNode::Float(value) => value.presented_bits().map_or_else(
                    || float_trace(value.text(), value.landing()),
                    |(format, bits)| format!("float-bits:{}:{bits:016x}", format.name()),
                ),
                ExpressionNode::UndeterminedNaN(format) => {
                    format!("undetermined-nan:{}", format.name())
                }
                ExpressionNode::String(value) => byte_string_trace(value),
            })
        }

        fn array_trace(elements: impl IntoIterator<Item = String>) -> String {
            format!(
                "array:[{}]",
                elements.into_iter().collect::<Vec<_>>().join(",")
            )
        }

        fn boolean_trace(value: bool) -> String {
            format!("boolean:{value}")
        }

        fn integer_trace(spelling: &str, landing: IntegerLanding) -> String {
            format!(
                "integer:{spelling}:{}:{}",
                landing.landed_type.name(),
                landing.domain.name(),
            )
        }

        fn float_trace(spelling: &str, landing: Option<FloatFormat>) -> String {
            landing.map_or_else(
                || format!("float:{spelling}:unlanded"),
                |landing| format!("float:{spelling}:{}", landing.name()),
            )
        }

        fn byte_string_trace(bytes: &[u8]) -> String {
            format!("byte-string:{bytes:?}")
        }

        pub(crate) fn static_type_identities(
            application: &RepresentativeStaticApplication,
        ) -> Vec<(SymbolHandle, String)> {
            application
                .bindings
                .iter()
                .map(|binding| {
                    (
                        binding.parameter,
                        static_argument_identity(&binding.argument),
                    )
                })
                .collect()
        }

        fn binder_argument_identity(
            argument: &typed_trees::proposition::PropositionBinderArgument,
            substitutions: &[RepresentativeStaticBinding],
        ) -> String {
            substitutions
                .iter()
                .find(|binding| binding.parameter == argument.symbol)
                .map(|binding| static_argument_identity(&binding.argument))
                .unwrap_or_else(|| {
                    argument.const_literal.as_ref().map_or_else(
                        || format!("symbol:{:?}", argument.symbol),
                        |value| format!("const:{value}"),
                    )
                })
        }

        pub(crate) fn static_arguments_match(
            left: &StaticMachineArgument,
            right: &StaticMachineArgument,
        ) -> bool {
            static_argument_identity(left) == static_argument_identity(right)
        }

        fn static_argument_identity(argument: &StaticMachineArgument) -> String {
            let head = if argument.symbol.is_valid() {
                format!("symbol:{:?}", argument.symbol)
            } else {
                format!("path:{}", argument.path.joined_path())
            };
            let application =
                argument
                    .application
                    .as_ref()
                    .map_or_else(String::new, |application| {
                        format!(
                            "<{};{}>",
                            application
                                .lifetime_arguments
                                .iter()
                                .map(|lifetime| lifetime.as_str())
                                .collect::<Vec<_>>()
                                .join(","),
                            application
                                .arguments
                                .iter()
                                .map(static_argument_identity)
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    });
            format!(
                "{head}{application}:const={:?}:evidence={:?}",
                argument.const_literal, argument.evidence_projection
            )
        }
    }
    pub(crate) mod representative {
        use super::{RelationPlanError, RepresentativeStaticApplication};
        use arena::HandleSpan;
        use symbols::SymbolHandle;
        use typed_trees::expression::QuotientOperationRequest;
        use typed_trees::machine::Machine;
        use typed_trees::signature::SignatureContract;
        use typed_trees::state::State;
        use typed_trees::types::TypeReferenceHandle;

        use super::static_application::derive_exact_representative_static_application;
        pub(crate) use super::theorem::MachineStateCoordinate;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct RepresentativeRuntimeParameter {
            pub(crate) symbol: SymbolHandle,
            pub(crate) type_reference: TypeReferenceHandle,
            pub(crate) is_mutable: bool,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct RepresentativeTelescope {
            pub(crate) machine_symbol: SymbolHandle,
            pub(crate) state_symbol: SymbolHandle,
            pub(crate) parameters: Vec<RepresentativeRuntimeParameter>,
            pub(crate) return_type: TypeReferenceHandle,
            pub(crate) machine_contracts: HandleSpan<SignatureContract>,
            pub(crate) state_contracts: HandleSpan<SignatureContract>,
            pub(crate) static_application: RepresentativeStaticApplication,
        }

        pub(crate) fn derive_representative_telescope(
            program: &dyn crate::CheckingInput,
            request: &QuotientOperationRequest,
        ) -> Result<RepresentativeTelescope, RelationPlanError> {
            let (machine, state) =
                representative_machine_state(program, request.representative_operation.symbol)?;
            let static_application =
                derive_exact_representative_static_application(program, request)?;
            if !state.return_type.is_valid() {
                return Err(RelationPlanError::RepresentativeResultTypeIsUnresolved);
            }
            let parameters = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| !parameter.is_const)
                .map(|parameter| RepresentativeRuntimeParameter {
                    symbol: parameter.symbol,
                    type_reference: parameter.type_reference,
                    is_mutable: parameter.is_mutable,
                })
                .collect();
            Ok(RepresentativeTelescope {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                parameters,
                return_type: state.return_type,
                machine_contracts: machine.contracts,
                state_contracts: state.contracts,
                static_application,
            })
        }

        pub(crate) fn representative_machine_state(
            program: &dyn crate::CheckingInput,
            state_symbol: SymbolHandle,
        ) -> Result<(&Machine, &State), RelationPlanError> {
            let mut matches = program.machine_states_named(state_symbol);
            let Some((machine, state)) = matches.next() else {
                return Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly);
            };
            if matches.next().is_some() {
                return Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly);
            }
            Ok((machine, state))
        }
    }
    pub(crate) mod result_flow {
        use crate::frozen_program::expression_table;
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::{
            StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
        };

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct ImmutableAliasFallthroughRoot {
            pub(crate) request_expression: ExpressionHandle,
            pub(crate) alias_count: usize,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct CompleteSingleStateResultFlow {
            pub(crate) machine_symbol: SymbolHandle,
            pub(crate) state_symbol: SymbolHandle,
            pub(crate) root: ImmutableAliasFallthroughRoot,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct CompleteStateForwardingResultFlow {
            pub(crate) machine_symbol: SymbolHandle,
            pub(crate) forwarding_edges: Vec<StateForwardingEdge>,
            pub(crate) result_state_symbol: SymbolHandle,
            pub(crate) root: ImmutableAliasFallthroughRoot,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum CompleteResultFlow {
            Single(CompleteSingleStateResultFlow),
            Forwarded(CompleteStateForwardingResultFlow),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct StateForwardingEdge {
            pub(crate) source_state_symbol: SymbolHandle,
            pub(crate) target_state_symbol: SymbolHandle,
        }

        pub(crate) fn fallthrough_result_root(
            program: &dyn crate::CheckingInput,
            state: &State,
        ) -> Option<ImmutableAliasFallthroughRoot> {
            let direct = program
                .statement_table
                .statements(state.statement_nodes)
                .last()
                .and_then(|statement| {
                    let expression = result_expression(program, statement)?;
                    is_quotient_request(program, expression).then_some(
                        ImmutableAliasFallthroughRoot {
                            request_expression: expression,
                            alias_count: 0,
                        },
                    )
                });
            direct.or_else(|| immutable_alias_fallthrough_root(program, state))
        }

        pub(crate) fn immutable_alias_fallthrough_root(
            program: &dyn crate::CheckingInput,
            state: &State,
        ) -> Option<ImmutableAliasFallthroughRoot> {
            if !state.return_type.is_valid() {
                return None;
            }
            let statements = program.statement_table.statements(state.statement_nodes);
            let (result, prefix) = statements.split_last()?;
            let result = result_expression(program, result)?;
            let mut expected_symbol = exact_local_name_symbol(program, result)?;
            let mut seen = Vec::new();
            for (position, statement) in prefix.iter().enumerate().rev() {
                let local = statement.as_local_data()?;
                if local.is_mutable
                    || local.symbol != expected_symbol
                    || seen.contains(&local.symbol)
                    || !local.type_reference.is_valid()
                    || program.normalized_type_identity(local.type_reference)
                        != program.normalized_type_identity(state.return_type)
                {
                    return None;
                }
                seen.push(local.symbol);
                match expression_table(program).expression(local.initial_value) {
                    ExpressionNode::Call(call)
                        if call.quotient_operation.is_some() && position == 0 =>
                    {
                        return Some(ImmutableAliasFallthroughRoot {
                            request_expression: local.initial_value,
                            alias_count: seen.len(),
                        });
                    }
                    ExpressionNode::Name(_) => {
                        expected_symbol = exact_local_name_symbol(program, local.initial_value)?;
                        if seen.contains(&expected_symbol) {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
            None
        }

        pub(crate) fn complete_single_state_result_flow(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            root: ImmutableAliasFallthroughRoot,
        ) -> Option<CompleteSingleStateResultFlow> {
            if !machine.symbol.is_valid() || !state.symbol.is_valid() {
                return None;
            }
            let mut owner_matches = program
                .machines()
                .iter()
                .filter(|candidate| candidate.symbol == machine.symbol);
            let exact_owner = owner_matches.next()?;
            if exact_owner != machine || owner_matches.next().is_some() {
                return None;
            }
            let mut state_matches = program
                .machines()
                .iter()
                .flat_map(|candidate| program.machine_states(candidate))
                .filter(|candidate| candidate.symbol == state.symbol);
            let exact_state = state_matches.next()?;
            if exact_state != state || state_matches.next().is_some() {
                return None;
            }
            let states = program.machine_states(machine);
            if states.len() != 1
                || states[0].symbol != state.symbol
                || fallthrough_result_root(program, state) != Some(root)
                || has_disqualifying_transition(program, state)
            {
                return None;
            }
            Some(CompleteSingleStateResultFlow {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                root,
            })
        }

        pub(crate) fn complete_state_forwarding_result_flow(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            result_state: &State,
            root: ImmutableAliasFallthroughRoot,
        ) -> Option<CompleteStateForwardingResultFlow> {
            if !machine.symbol.is_valid() || !result_state.symbol.is_valid() {
                return None;
            }
            let exact_machine = program
                .machines()
                .iter()
                .filter(|candidate| candidate.symbol == machine.symbol)
                .collect::<Vec<_>>();
            if exact_machine.as_slice() != [machine] {
                return None;
            }
            let states = program.machine_states(machine);
            if states.len() < 2
                || typed_trees::state::state_by_symbol(states, result_state.symbol)
                    != Some(result_state)
                || fallthrough_result_root(program, result_state) != Some(root)
                || has_disqualifying_transition(program, result_state)
            {
                return None;
            }
            for state in states {
                if !state.symbol.is_valid()
                    || program
                        .machines()
                        .iter()
                        .flat_map(|candidate| program.machine_states(candidate))
                        .filter(|candidate| candidate.symbol == state.symbol)
                        .count()
                        != 1
                {
                    return None;
                }
            }

            let mut edges = Vec::with_capacity(states.len() - 1);
            for forwarding_state in states
                .iter()
                .filter(|state| state.symbol != result_state.symbol)
            {
                let [StatementNode::Transition(transition)] = program
                    .statement_table
                    .statements(forwarding_state.statement_nodes)
                else {
                    return None;
                };
                if transition.guard != typed_trees::statement::TransitionGuardNode::Always
                    || transition.exit != typed_trees::statement::TransitionExit::Ordinary
                    || transition.continuation.is_valid()
                {
                    return None;
                }
                let typed_trees::statement::TransitionTargetNode::Named { path, .. } =
                    program.statement_table.transition_target(transition.target)
                else {
                    return None;
                };
                if !states.iter().any(|state| state.symbol == path.symbol) {
                    return None;
                }
                edges.push((forwarding_state.symbol, path.symbol));
            }

            for state in states
                .iter()
                .filter(|state| state.symbol != result_state.symbol)
            {
                let mut current = state.symbol;
                let mut visited = Vec::new();
                while current != result_state.symbol {
                    if visited.contains(&current) {
                        return None;
                    }
                    visited.push(current);
                    current = edges
                        .iter()
                        .find_map(|(source, target)| (*source == current).then_some(*target))?;
                }
            }

            Some(CompleteStateForwardingResultFlow {
                machine_symbol: machine.symbol,
                forwarding_edges: edges
                    .iter()
                    .map(|(source, target)| StateForwardingEdge {
                        source_state_symbol: *source,
                        target_state_symbol: *target,
                    })
                    .collect(),
                result_state_symbol: result_state.symbol,
                root,
            })
        }

        pub(crate) fn complete_result_flow(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            result_state: &State,
            root: ImmutableAliasFallthroughRoot,
        ) -> Option<CompleteResultFlow> {
            complete_single_state_result_flow(program, machine, result_state, root)
                .map(CompleteResultFlow::Single)
                .or_else(|| {
                    complete_state_forwarding_result_flow(program, machine, result_state, root)
                        .map(CompleteResultFlow::Forwarded)
                })
        }

        fn result_expression(
            program: &dyn crate::CheckingInput,
            statement: &StatementNode,
        ) -> Option<ExpressionHandle> {
            if let Some(expression) = statement.as_expression() {
                return Some(*expression);
            }
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            if transition.guard != TransitionGuardNode::Always
                || transition.exit != TransitionExit::Ordinary
                || transition.continuation.is_valid()
                || !transition.patterns.is_empty()
            {
                return None;
            }
            match program.statement_table.transition_target(transition.target) {
                TransitionTargetNode::Value(expression) if expression.is_valid() => {
                    Some(*expression)
                }
                _ => None,
            }
        }

        fn is_quotient_request(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> bool {
            matches!(
                expression_table(program).expression(expression),
                ExpressionNode::Call(call) if call.quotient_operation.is_some()
            )
        }

        fn has_disqualifying_transition(program: &dyn crate::CheckingInput, state: &State) -> bool {
            let statements = program.statement_table.statements(state.statement_nodes);
            let last = statements.len().saturating_sub(1);
            statements.iter().enumerate().any(|(index, statement)| {
                matches!(statement, StatementNode::Transition(_))
                    && (index != last || result_expression(program, statement).is_none())
            })
        }

        fn exact_local_name_symbol(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<SymbolHandle> {
            let path = expression_table(program).name_path_node(expression)?;
            (path.symbol.is_valid()
                && expression_table(program)
                    .name_path_members(path.members)
                    .len()
                    == 1)
                .then_some(path.symbol)
        }
    }
    pub(crate) mod runtime_correspondence {
        use super::{
            ExactQuotientRelation, PositionalRelation, RelationPlanError,
            RepresentativeStaticBinding, RepresentativeTelescope,
        };
        use crate::frozen_program::expression_table;
        use numerics::literals::{FloatFormat, IntegerLanding, IntegerLiteral, LandedIntegerType};
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        fn literal_target_mismatch_or_none<T>(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            position: usize,
        ) -> Result<Option<T>, RelationPlanError> {
            match expression_table(program).expression(expression) {
                ExpressionNode::Boolean(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Float(_) => {
                    Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position))
                }
                _ => Ok(None),
            }
        }

        use super::static_application::substituted_type_matches;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct DefineRuntimePosition {
            pub(crate) public_parameter: SymbolHandle,
            pub(crate) representative_parameter: SymbolHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct DefineRuntimeCorrespondence {
            pub(crate) positions: Vec<DefineRuntimePosition>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct ClosedIntegerArrayElement {
            pub(crate) spelling: String,
            pub(crate) landing: IntegerLanding,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct ClosedFloatArrayElement {
            pub(crate) spelling: String,
            pub(crate) landing: FloatFormat,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum ClosedRecursiveArrayElement {
            Boolean(bool),
            Byte(u8),
            Integer(ClosedIntegerArrayElement),
            Float(ClosedFloatArrayElement),
            Array(std::sync::Arc<[ClosedRecursiveArrayElement]>),
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum ClosedLiftLiteral {
            Boolean(bool),
            Integer {
                spelling: String,
                landing: IntegerLanding,
            },
            Float {
                spelling: String,
                landing: FloatFormat,
            },
            ByteString {
                bytes: source::SourceBytes,
                target_type: typed_trees::type_identity::NormalizedTypeIdentity,
            },
            FixedByteArray {
                bytes: source::SourceBytes,
                target_type: typed_trees::type_identity::NormalizedTypeIdentity,
            },
            BooleanArray {
                values: std::sync::Arc<[bool]>,
                target_type: typed_trees::type_identity::NormalizedTypeIdentity,
            },
            NestedBooleanArray {
                rows: std::sync::Arc<[std::sync::Arc<[bool]>]>,
                target_type: typed_trees::type_identity::NormalizedTypeIdentity,
            },
            NestedFixedByteArray {
                rows: std::sync::Arc<[source::SourceBytes]>,
                target_type: typed_trees::type_identity::NormalizedTypeIdentity,
            },
            NestedIntegerArray {
                rows: std::sync::Arc<[std::sync::Arc<[ClosedIntegerArrayElement]>]>,
                target_type: typed_trees::type_identity::NormalizedTypeIdentity,
            },
            NestedFloatArray {
                rows: std::sync::Arc<[std::sync::Arc<[ClosedFloatArrayElement]>]>,
                target_type: typed_trees::type_identity::NormalizedTypeIdentity,
            },
            BooleanTensor3 {
                planes: std::sync::Arc<[std::sync::Arc<[std::sync::Arc<[bool]>]>]>,
                target_type: typed_trees::type_identity::NormalizedTypeIdentity,
            },
            RecursivePrimitiveArray {
                elements: std::sync::Arc<[ClosedRecursiveArrayElement]>,
                target_type: typed_trees::type_identity::NormalizedTypeIdentity,
            },
            IntegerArray {
                elements: std::sync::Arc<[ClosedIntegerArrayElement]>,
                target_type: typed_trees::type_identity::NormalizedTypeIdentity,
            },
            FloatArray {
                elements: std::sync::Arc<[ClosedFloatArrayElement]>,
                target_type: typed_trees::type_identity::NormalizedTypeIdentity,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum DirectLiftArgumentSource {
            PublicParameter(SymbolHandle),
            Literal(ClosedLiftLiteral),
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct DirectLiftRuntimePosition {
            pub(crate) source: DirectLiftArgumentSource,
            pub(crate) representative_parameter: SymbolHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct DirectLiftRuntimeCorrespondence {
            pub(crate) positions: Vec<DirectLiftRuntimePosition>,
        }

        pub(crate) fn derive_define_runtime_correspondence(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            call: &TableCallExpression,
            input_relations: &[PositionalRelation],
            result_relation: PositionalRelation,
            representative: &RepresentativeTelescope,
        ) -> Result<DefineRuntimeCorrespondence, RelationPlanError> {
            derive_define_position_runtime_correspondence(
                program,
                machine,
                state,
                call,
                input_relations,
                result_relation,
                representative,
            )
            .map(|positions| DefineRuntimeCorrespondence { positions })
        }

        pub(crate) fn derive_direct_lift_runtime_correspondence(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            call: &TableCallExpression,
            input_relations: &[PositionalRelation],
            result_relation: PositionalRelation,
            representative: &RepresentativeTelescope,
        ) -> Result<DirectLiftRuntimeCorrespondence, RelationPlanError> {
            if !program.machine_type_parameters(machine).is_empty() {
                return Err(RelationPlanError::DirectLiftOwnerRequiresSubstitution);
            }
            let public_parameters = program.public_state_parameters(state).collect::<Vec<_>>();
            let arguments = expression_table(program).expression_handles(call.arguments);
            if arguments.len() != representative.parameters.len()
                || input_relations.len() != arguments.len()
            {
                return Err(RelationPlanError::DirectLiftRuntimeArityMismatch);
            }
            if has_duplicate_parameter_symbols(
                public_parameters.iter().map(|parameter| parameter.symbol),
            ) || has_duplicate_parameter_symbols(
                representative
                    .parameters
                    .iter()
                    .map(|parameter| parameter.symbol),
            ) {
                return Err(RelationPlanError::DirectLiftParameterIdentityNotUnique);
            }

            let mut positions = Vec::with_capacity(arguments.len());
            for (position, ((argument, relation), representative_parameter)) in arguments
                .iter()
                .zip(input_relations)
                .zip(&representative.parameters)
                .enumerate()
            {
                let source = if let Some(argument_symbol) =
                    direct_public_parameter_symbol(program, *argument)
                {
                    let public = public_parameters
                        .iter()
                        .copied()
                        .find(|parameter| parameter.symbol == argument_symbol)
                        .ok_or(RelationPlanError::DirectLiftArgumentIsNotPublicParameter(
                            position,
                        ))?;
                    if public.is_mutable != representative_parameter.is_mutable {
                        return Err(RelationPlanError::DirectLiftParameterModeMismatch(position));
                    }
                    if !input_relation_matches_public_type(
                        program,
                        *relation,
                        public.type_reference,
                    ) || !input_relation_matches_representative_type(
                        program,
                        *relation,
                        representative_parameter.type_reference,
                        &representative.static_application.bindings,
                    ) {
                        return Err(RelationPlanError::DirectLiftParameterTypeMismatch(position));
                    }
                    DirectLiftArgumentSource::PublicParameter(public.symbol)
                } else {
                    if representative_parameter.is_mutable {
                        return Err(RelationPlanError::DirectLiftParameterModeMismatch(position));
                    }
                    let literal = closed_lift_literal_for_representative(
                        program,
                        *argument,
                        representative_parameter.type_reference,
                        position,
                    )?
                    .ok_or(
                        RelationPlanError::DirectLiftArgumentIsNotPublicParameter(position),
                    )?;
                    if *relation
                        != PositionalRelation::ExactEquality(
                            representative_parameter.type_reference,
                        )
                    {
                        return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
                    }
                    DirectLiftArgumentSource::Literal(literal)
                };
                positions.push(DirectLiftRuntimePosition {
                    source,
                    representative_parameter: representative_parameter.symbol,
                });
            }
            if !input_relation_matches_representative_type(
                program,
                result_relation,
                representative.return_type,
                &representative.static_application.bindings,
            ) {
                return Err(RelationPlanError::DirectLiftResultTypeMismatch);
            }
            Ok(DirectLiftRuntimeCorrespondence { positions })
        }

        #[allow(clippy::too_many_arguments)]
        fn closed_array_elements(
            program: &dyn crate::CheckingInput,
            elements: &[ExpressionHandle],
            element_type: TypeReferenceHandle,
            leaf_type: TypeReferenceHandle,
            leaf_primitive: PrimitiveType,
            position: usize,
        ) -> Result<Vec<ClosedRecursiveArrayElement>, RelationPlanError> {
            elements
                .iter()
                .map(|element| {
                    closed_recursive_array_element(
                        program,
                        *element,
                        element_type,
                        leaf_type,
                        leaf_primitive,
                        position,
                    )
                })
                .collect()
        }

        fn derive_define_position_runtime_correspondence(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            call: &TableCallExpression,
            input_relations: &[PositionalRelation],
            result_relation: PositionalRelation,
            representative: &RepresentativeTelescope,
        ) -> Result<Vec<DefineRuntimePosition>, RelationPlanError> {
            if !program.machine_type_parameters(machine).is_empty() {
                return Err(RelationPlanError::DefineOwnerRequiresSubstitution);
            }
            let public_parameters = program.public_state_parameters(state).collect::<Vec<_>>();
            let arguments = expression_table(program).expression_handles(call.arguments);
            let arity_matches = arguments.len() == representative.parameters.len()
                && input_relations.len() == arguments.len()
                && public_parameters.len() == arguments.len();
            if !arity_matches {
                return Err(RelationPlanError::DefineRuntimeArityMismatch);
            }
            if has_duplicate_parameter_symbols(
                public_parameters.iter().map(|parameter| parameter.symbol),
            ) || has_duplicate_parameter_symbols(
                representative
                    .parameters
                    .iter()
                    .map(|parameter| parameter.symbol),
            ) {
                return Err(RelationPlanError::DefineParameterIdentityNotUnique);
            }

            let mut positions = Vec::with_capacity(arguments.len());
            for (position, ((argument, relation), representative_parameter)) in arguments
                .iter()
                .zip(input_relations)
                .zip(&representative.parameters)
                .enumerate()
            {
                let argument_symbol = direct_public_parameter_symbol(program, *argument).ok_or(
                    RelationPlanError::DefineArgumentIsNotPublicParameter(position),
                )?;
                let public = public_parameters[position];
                if argument_symbol != public.symbol {
                    return Err(RelationPlanError::DefineArgumentOrderMismatch(position));
                }
                if public.is_mutable != representative_parameter.is_mutable {
                    return Err(RelationPlanError::DefineParameterModeMismatch(position));
                }
                if !input_relation_matches_public_type(program, *relation, public.type_reference)
                    || !input_relation_matches_representative_type(
                        program,
                        *relation,
                        representative_parameter.type_reference,
                        &representative.static_application.bindings,
                    )
                {
                    return Err(RelationPlanError::DefineParameterTypeMismatch(position));
                }
                positions.push(DefineRuntimePosition {
                    public_parameter: public.symbol,
                    representative_parameter: representative_parameter.symbol,
                });
            }
            if !input_relation_matches_representative_type(
                program,
                result_relation,
                representative.return_type,
                &representative.static_application.bindings,
            ) {
                return Err(RelationPlanError::DefineResultTypeMismatch);
            }
            Ok(positions)
        }

        pub(crate) fn closed_lift_literal_for_representative(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            representative_type: TypeReferenceHandle,
            position: usize,
        ) -> Result<Option<ClosedLiftLiteral>, RelationPlanError> {
            if let Some(elements) = expression_table(program).array_literal_node(expression) {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(width),
                } = program
                    .type_reference_table
                    .type_reference(representative_type)
                else {
                    return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
                };
                let elements = expression_table(program).expression_handles(*elements);
                if elements.len() != *width {
                    return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
                }
                if let TypeReferenceNode::FixedArray {
                    element_type: row_type,
                    length: typed_trees::types::FixedArrayLength::Literal(plane_height),
                } = program.type_reference_table.type_reference(*element_type)
                    && let TypeReferenceNode::FixedArray {
                        element_type: leaf_type,
                        length: typed_trees::types::FixedArrayLength::Literal(row_width),
                    } = program.type_reference_table.type_reference(*row_type)
                    && exact_primitive_type(program, *leaf_type) == Some(PrimitiveType::Bool)
                {
                    let planes = elements
                        .iter()
                        .map(|plane| {
                            let rows = expression_table(program).array_literal_node(*plane)?;
                            let rows = expression_table(program).expression_handles(*rows);
                            if rows.len() != *plane_height {
                                return None;
                            }
                            rows.iter()
                                .map(|row| {
                                    let leaves =
                                        expression_table(program).array_literal_node(*row)?;
                                    let leaves =
                                        expression_table(program).expression_handles(*leaves);
                                    if leaves.len() != *row_width {
                                        return None;
                                    }
                                    leaves
                                        .iter()
                                        .map(|leaf| {
                                            let value =
                                                expression_table(program).boolean_node(*leaf)?;
                                            Some(*value)
                                        })
                                        .collect::<Option<Vec<_>>>()
                                        .map(std::sync::Arc::from)
                                })
                                .collect::<Option<Vec<_>>>()
                                .map(std::sync::Arc::from)
                        })
                        .collect::<Option<Vec<_>>>()
                        .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
                    return Ok(Some(ClosedLiftLiteral::BooleanTensor3 {
                        planes: planes.into(),
                        target_type: program.normalized_type_identity(representative_type),
                    }));
                }
                if let Some((depth, leaf_type, leaf_primitive)) =
                    literal_fixed_array_leaf(program, representative_type)
                    && depth >= 3
                {
                    let values = closed_array_elements(
                        program,
                        elements,
                        *element_type,
                        leaf_type,
                        leaf_primitive,
                        position,
                    )?;
                    return Ok(Some(ClosedLiftLiteral::RecursivePrimitiveArray {
                        elements: values.into(),
                        target_type: program.normalized_type_identity(representative_type),
                    }));
                }
                if let TypeReferenceNode::FixedArray {
                    element_type: row_element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(row_width),
                } = program.type_reference_table.type_reference(*element_type)
                {
                    let row_primitive = exact_primitive_type(program, *row_element_type)
                        .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
                    if row_primitive == PrimitiveType::U8 {
                        let rows = nested_literal_rows(program, elements, *row_width, position)?
                            .into_iter()
                            .map(|row_elements| {
                                canonical_fixed_bytes(program, row_elements)
                                    .map(source::SourceBytes::from)
                            })
                            .collect::<Option<Vec<_>>>()
                            .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
                        return Ok(Some(ClosedLiftLiteral::NestedFixedByteArray {
                            rows: rows.into(),
                            target_type: program.normalized_type_identity(representative_type),
                        }));
                    }
                    if crate::value_custody::literals::primitive_landing_type(row_primitive)
                        .is_some()
                    {
                        let rows = nested_literal_rows(program, elements, *row_width, position)?
                            .into_iter()
                            .map(|row_elements| {
                                row_elements
                                    .iter()
                                    .map(|element| {
                                        closed_integer_array_element(
                                            program,
                                            *element,
                                            *row_element_type,
                                            row_primitive,
                                            position,
                                        )
                                    })
                                    .collect::<Result<Vec<_>, _>>()
                                    .map(std::sync::Arc::from)
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        return Ok(Some(ClosedLiftLiteral::NestedIntegerArray {
                            rows: rows.into(),
                            target_type: program.normalized_type_identity(representative_type),
                        }));
                    }
                    if matches!(row_primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                        let rows = nested_literal_rows(program, elements, *row_width, position)?
                            .into_iter()
                            .map(|row_elements| {
                                row_elements
                                    .iter()
                                    .map(|element| {
                                        closed_float_array_element(
                                            program,
                                            *element,
                                            row_primitive,
                                            position,
                                        )
                                    })
                                    .collect::<Result<Vec<_>, _>>()
                                    .map(std::sync::Arc::from)
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        return Ok(Some(ClosedLiftLiteral::NestedFloatArray {
                            rows: rows.into(),
                            target_type: program.normalized_type_identity(representative_type),
                        }));
                    }
                    if row_primitive != PrimitiveType::Bool {
                        return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
                    }
                    let rows = nested_literal_rows(program, elements, *row_width, position)?
                        .into_iter()
                        .map(|row_elements| {
                            closed_boolean_array_elements(program, row_elements)
                                .map(std::sync::Arc::from)
                        })
                        .collect::<Option<Vec<_>>>()
                        .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
                    return Ok(Some(ClosedLiftLiteral::NestedBooleanArray {
                        rows: rows.into(),
                        target_type: program.normalized_type_identity(representative_type),
                    }));
                }
                let element_primitive = exact_primitive_type(program, *element_type)
                    .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
                if element_primitive == PrimitiveType::Bool {
                    let values = closed_boolean_array_elements(program, elements)
                        .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
                    return Ok(Some(ClosedLiftLiteral::BooleanArray {
                        values: values.into(),
                        target_type: program.normalized_type_identity(representative_type),
                    }));
                }
                if element_primitive == PrimitiveType::U8
                    && let Some(bytes) = canonical_fixed_bytes(program, elements)
                {
                    return Ok(Some(ClosedLiftLiteral::FixedByteArray {
                        bytes: bytes.into(),
                        target_type: program.normalized_type_identity(representative_type),
                    }));
                }
                if crate::value_custody::literals::primitive_landing_type(element_primitive)
                    .is_some()
                {
                    let elements = elements
                        .iter()
                        .map(|element| {
                            closed_integer_array_element(
                                program,
                                *element,
                                *element_type,
                                element_primitive,
                                position,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(Some(ClosedLiftLiteral::IntegerArray {
                        elements: elements.into(),
                        target_type: program.normalized_type_identity(representative_type),
                    }));
                }
                if matches!(element_primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                    let elements = elements
                        .iter()
                        .map(|element| {
                            closed_float_array_element(
                                program,
                                *element,
                                element_primitive,
                                position,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(Some(ClosedLiftLiteral::FloatArray {
                        elements: elements.into(),
                        target_type: program.normalized_type_identity(representative_type),
                    }));
                }
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            }
            if let Some(bytes) = expression_table(program).string_node(expression) {
                let exact_target = exact_shared_byte_slice(program, representative_type)
                    || crate::value_custody::expression_types::bounded_byte_buffer_capacity(
                        program,
                        representative_type,
                    )
                    .is_some_and(|capacity| bytes.len() <= capacity);
                return exact_target
                    .then(|| ClosedLiftLiteral::ByteString {
                        bytes: bytes.clone(),
                        target_type: program.normalized_type_identity(representative_type),
                    })
                    .map(Some)
                    .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            }
            let TypeReferenceNode::Named { name, .. } = program
                .type_reference_table
                .type_reference(representative_type)
            else {
                return literal_target_mismatch_or_none(program, expression, position);
            };
            let Some(primitive) = PrimitiveType::from_name(name.as_str()) else {
                return literal_target_mismatch_or_none(program, expression, position);
            };
            if name.as_str() != primitive.name() {
                return literal_target_mismatch_or_none(program, expression, position);
            }
            match expression_table(program).expression(expression) {
                ExpressionNode::Boolean(value) if primitive == PrimitiveType::Bool => {
                    Ok(Some(ClosedLiftLiteral::Boolean(*value)))
                }
                ExpressionNode::Boolean(_) => {
                    Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position))
                }
                ExpressionNode::Integer(literal) => {
                    let landing = exact_integer_landing(
                        program,
                        representative_type,
                        primitive,
                        literal,
                        position,
                    )?;
                    Ok(Some(ClosedLiftLiteral::Integer {
                        spelling: literal.to_string(),
                        landing,
                    }))
                }
                ExpressionNode::Float(literal) => {
                    let landing = exact_float_landing(primitive, literal, position)?;
                    Ok(Some(ClosedLiftLiteral::Float {
                        spelling: literal.text().to_owned(),
                        landing,
                    }))
                }
                _ => Ok(None),
            }
        }

        fn exact_shared_byte_slice(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> bool {
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = program.type_reference_table.type_reference(type_reference)
            else {
                return false;
            };
            if *access != language_core::ReferenceAccess::Shared {
                return false;
            }
            let TypeReferenceNode::Slice { element_type } =
                program.type_reference_table.type_reference(*referee)
            else {
                return false;
            };
            matches!(
                program
                    .type_reference_table
                    .type_reference(*element_type),
                TypeReferenceNode::Named { name, .. } if name.as_str() == PrimitiveType::U8.name()
            )
        }

        fn exact_primitive_type(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<PrimitiveType> {
            let TypeReferenceNode::Named { name, .. } =
                program.type_reference_table.type_reference(type_reference)
            else {
                return None;
            };
            PrimitiveType::from_name(name.as_str())
                .filter(|primitive| name.as_str() == primitive.name())
        }

        fn literal_fixed_array_leaf(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<(usize, TypeReferenceHandle, PrimitiveType)> {
            let mut depth = 0;
            let mut current = type_reference;
            loop {
                match program.type_reference_table.type_reference(current) {
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: typed_trees::types::FixedArrayLength::Literal(_),
                    } => {
                        depth += 1;
                        current = *element_type;
                    }
                    TypeReferenceNode::FixedArray { .. } => return None,
                    _ => {
                        return exact_primitive_type(program, current)
                            .map(|leaf| (depth, current, leaf));
                    }
                }
            }
        }

        fn closed_recursive_array_element(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            target_type: TypeReferenceHandle,
            leaf_type: TypeReferenceHandle,
            leaf_primitive: PrimitiveType,
            position: usize,
        ) -> Result<ClosedRecursiveArrayElement, RelationPlanError> {
            match program.type_reference_table.type_reference(target_type) {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(width),
                } => {
                    let Some(elements) = expression_table(program).array_literal_node(expression)
                    else {
                        return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
                    };
                    let elements = expression_table(program).expression_handles(*elements);
                    if elements.len() != *width {
                        return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
                    }
                    closed_array_elements(
                        program,
                        elements,
                        *element_type,
                        leaf_type,
                        leaf_primitive,
                        position,
                    )
                    .map(std::sync::Arc::from)
                    .map(ClosedRecursiveArrayElement::Array)
                }
                _ if target_type == leaf_type
                    && exact_primitive_type(program, target_type) == Some(leaf_primitive) =>
                {
                    match (
                        leaf_primitive,
                        expression_table(program).expression(expression),
                    ) {
                        (PrimitiveType::Bool, ExpressionNode::Boolean(value)) => {
                            Ok(ClosedRecursiveArrayElement::Boolean(*value))
                        }
                        (PrimitiveType::U8, ExpressionNode::Integer(_)) => {
                            let byte =
                                canonical_fixed_bytes(program, std::slice::from_ref(&expression))
                                    .and_then(|bytes| bytes.into_iter().next())
                                    .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(
                                        position,
                                    ))?;
                            Ok(ClosedRecursiveArrayElement::Byte(byte))
                        }
                        (primitive, ExpressionNode::Integer(literal))
                            if crate::value_custody::literals::primitive_landing_type(
                                primitive,
                            )
                            .is_some() =>
                        {
                            let landing = exact_integer_landing(
                                program,
                                target_type,
                                primitive,
                                literal,
                                position,
                            )?;
                            Ok(ClosedRecursiveArrayElement::Integer(
                                ClosedIntegerArrayElement {
                                    spelling: literal.to_string(),
                                    landing,
                                },
                            ))
                        }
                        (
                            primitive @ (PrimitiveType::F32 | PrimitiveType::F64),
                            ExpressionNode::Float(literal),
                        ) => {
                            let landing = exact_float_landing(primitive, literal, position)?;
                            Ok(ClosedRecursiveArrayElement::Float(
                                ClosedFloatArrayElement {
                                    spelling: literal.text().to_owned(),
                                    landing,
                                },
                            ))
                        }
                        _ => Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
                    }
                }
                _ => Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
            }
        }

        fn nested_literal_rows<'program>(
            program: &'program dyn crate::CheckingInput,
            elements: &[ExpressionHandle],
            row_width: usize,
            position: usize,
        ) -> Result<Vec<&'program [ExpressionHandle]>, RelationPlanError> {
            elements
                .iter()
                .map(|row| {
                    let Some(row_elements) = expression_table(program).array_literal_node(*row)
                    else {
                        return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
                    };
                    let row_elements = expression_table(program).expression_handles(*row_elements);
                    if row_elements.len() != row_width {
                        return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
                    }
                    Ok(row_elements)
                })
                .collect()
        }

        fn closed_integer_array_element(
            program: &dyn crate::CheckingInput,
            element: ExpressionHandle,
            element_type: TypeReferenceHandle,
            primitive: PrimitiveType,
            position: usize,
        ) -> Result<ClosedIntegerArrayElement, RelationPlanError> {
            let Some(literal) = expression_table(program).integer_node(element) else {
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            };
            let landing =
                exact_integer_landing(program, element_type, primitive, literal, position)?;
            Ok(ClosedIntegerArrayElement {
                spelling: literal.to_string(),
                landing,
            })
        }

        fn closed_float_array_element(
            program: &dyn crate::CheckingInput,
            element: ExpressionHandle,
            primitive: PrimitiveType,
            position: usize,
        ) -> Result<ClosedFloatArrayElement, RelationPlanError> {
            let ExpressionNode::Float(literal) = expression_table(program).expression(element)
            else {
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            };
            let landing = exact_float_landing(primitive, literal, position)?;
            Ok(ClosedFloatArrayElement {
                spelling: literal.text().to_owned(),
                landing,
            })
        }

        fn closed_boolean_array_elements(
            program: &dyn crate::CheckingInput,
            elements: &[ExpressionHandle],
        ) -> Option<Vec<bool>> {
            elements
                .iter()
                .map(|element| {
                    let value = expression_table(program).boolean_node(*element)?;
                    Some(*value)
                })
                .collect()
        }

        fn canonical_fixed_bytes(
            program: &dyn crate::CheckingInput,
            elements: &[ExpressionHandle],
        ) -> Option<Vec<u8>> {
            elements
                .iter()
                .map(|element| {
                    let literal = expression_table(program).integer_node(*element)?;
                    let value = literal
                        .value_u64()
                        .and_then(|value| u8::try_from(value).ok())?;
                    (literal.landing().is_none()
                        && literal
                            == &numerics::literals::IntegerLiteral::from_value(i64::from(value)))
                        .then_some(value)
                })
                .collect()
        }

        fn exact_integer_landing(
            program: &dyn crate::CheckingInput,
            target_type: TypeReferenceHandle,
            primitive: PrimitiveType,
            literal: &IntegerLiteral,
            position: usize,
        ) -> Result<IntegerLanding, RelationPlanError> {
            let target_domain = program.arithmetic_domain_for_type_reference(target_type);
            let landing = match literal.landing() {
                Some(landing) => landing,
                None => IntegerLanding {
                    landed_type: crate::value_custody::literals::primitive_landing_type(primitive)
                        .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?,
                    domain: target_domain,
                },
            };
            if landed_primitive(landing.landed_type) != primitive
                || landing.domain != target_domain
                || !integer_literal_fits(literal, landing.landed_type)
            {
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            }
            Ok(landing)
        }

        fn exact_float_landing(
            primitive: PrimitiveType,
            literal: &numerics::literals::FloatLiteral,
            position: usize,
        ) -> Result<FloatFormat, RelationPlanError> {
            let expected = match primitive {
                PrimitiveType::F32 => FloatFormat::F32,
                PrimitiveType::F64 => FloatFormat::F64,
                _ => return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
            };
            let landing = literal.landing().unwrap_or(expected);
            if landing != expected {
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            }
            Ok(landing)
        }

        pub(crate) fn landed_primitive(landed: LandedIntegerType) -> PrimitiveType {
            PrimitiveType::from_landed_integer(landed)
        }

        fn integer_literal_fits(literal: &IntegerLiteral, landed: LandedIntegerType) -> bool {
            let width = landed.bit_width();
            if landed.is_signed() {
                literal.value_i64().is_some_and(|value| {
                    width == 64 || {
                        let minimum = -(1i64 << (width - 1));
                        let maximum = (1i64 << (width - 1)) - 1;
                        (minimum..=maximum).contains(&value)
                    }
                })
            } else {
                !literal.is_negative()
                    && literal
                        .value_u64()
                        .is_some_and(|value| width == 64 || value < (1u64 << width))
            }
        }

        fn has_duplicate_parameter_symbols(
            symbols: impl IntoIterator<Item = SymbolHandle>,
        ) -> bool {
            let mut seen = Vec::new();
            for symbol in symbols {
                if seen.contains(&symbol) {
                    return true;
                }
                seen.push(symbol);
            }
            false
        }

        pub(crate) fn direct_public_parameter_symbol(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<SymbolHandle> {
            let expression = match expression_table(program).expression(expression) {
                ExpressionNode::Borrow(inner) => inner.target,
                _ => expression,
            };
            let path = expression_table(program).name_path_node(expression)?;
            (expression_table(program)
                .name_path_members(path.members)
                .len()
                == 1
                && path.symbol.is_valid())
            .then_some(path.symbol)
        }

        fn input_relation_matches_representative_type(
            program: &dyn crate::CheckingInput,
            relation: PositionalRelation,
            representative_type: TypeReferenceHandle,
            substitutions: &[RepresentativeStaticBinding],
        ) -> bool {
            match relation {
                PositionalRelation::ExactEquality(public_type) => substituted_type_matches(
                    program,
                    representative_type,
                    public_type,
                    substitutions,
                ),
                PositionalRelation::Quotient(relation) => quotient_carrier_matches_type(
                    program,
                    relation,
                    representative_type,
                    substitutions,
                ),
            }
        }

        fn input_relation_matches_public_type(
            program: &dyn crate::CheckingInput,
            relation: PositionalRelation,
            public_type: TypeReferenceHandle,
        ) -> bool {
            let relation_type = match relation {
                PositionalRelation::Quotient(relation) => relation.quotient_type,
                PositionalRelation::ExactEquality(type_reference) => type_reference,
            };
            program.normalized_type_identity(relation_type)
                == program.normalized_type_identity(public_type)
        }

        fn quotient_carrier_matches_type(
            program: &dyn crate::CheckingInput,
            relation: ExactQuotientRelation,
            representative_type: TypeReferenceHandle,
            substitutions: &[RepresentativeStaticBinding],
        ) -> bool {
            if !matches!(
                program
                    .type_reference_table
                    .type_reference(relation.quotient_type),
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
            ) {
                return false;
            }
            let Some(quotient) = crate::declarations::data_queries::data_definition_by_symbol(
                program,
                relation.quotient_symbol,
            ) else {
                return false;
            };
            let Some(metadata) = quotient.quotient.as_ref() else {
                return false;
            };
            let Some(carrier_symbol) =
                crate::proof_contracts::quotients::equivalence_selection::base_data_symbol(
                    program,
                    metadata.carrier,
                )
            else {
                return false;
            };
            let Some(carrier) = crate::declarations::data_queries::data_definition_by_symbol(
                program,
                carrier_symbol,
            ) else {
                return false;
            };
            quotient.properties.multiplicity == carrier.properties.multiplicity
                && substituted_type_matches(
                    program,
                    representative_type,
                    metadata.carrier,
                    substitutions,
                )
        }
    }
    pub(crate) mod static_application {
        use typed_trees::data::{TypeParameter, TypeParameterKind};
        use typed_trees::expression::{QuotientOperationRequest, StaticMachineArgument};
        use typed_trees::name::Identifier;
        use typed_trees::types::{
            FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
        };

        use super::{
            RelationPlanError, RepresentativeStaticApplication, RepresentativeStaticBinding,
            RepresentativeStaticBindingKind, representative_machine_state,
        };

        pub(crate) fn substituted_type_matches(
            program: &dyn crate::CheckingInput,
            template: TypeReferenceHandle,
            concrete: TypeReferenceHandle,
            substitutions: &[RepresentativeStaticBinding],
        ) -> bool {
            let template_node = program.type_reference_table.type_reference(template);
            let concrete_node = program.type_reference_table.type_reference(concrete);
            match (template_node, concrete_node) {
                (
                    TypeReferenceNode::Named { symbol, .. },
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. },
                ) => substitutions
                    .iter()
                    .find(|binding| binding.parameter == *symbol)
                    .map_or_else(
                        || {
                            program.normalized_type_identity(template)
                                == program.normalized_type_identity(concrete)
                        },
                        |binding| {
                            binding.kind != RepresentativeStaticBindingKind::Const
                                && static_argument_matches_type(
                                    program,
                                    &binding.argument,
                                    concrete,
                                    substitutions,
                                )
                        },
                    ),
                (
                    TypeReferenceNode::Reference {
                        referee: template_referee,
                        access: template_access,
                        ..
                    },
                    TypeReferenceNode::Reference {
                        referee: concrete_referee,
                        access: concrete_access,
                        ..
                    },
                ) => {
                    template_access == concrete_access
                        && substituted_type_matches(
                            program,
                            *template_referee,
                            *concrete_referee,
                            substitutions,
                        )
                }
                (
                    TypeReferenceNode::FixedArray {
                        element_type: template_element,
                        length: template_length,
                    },
                    TypeReferenceNode::FixedArray {
                        element_type: concrete_element,
                        length: concrete_length,
                    },
                ) => {
                    substituted_type_matches(
                        program,
                        *template_element,
                        *concrete_element,
                        substitutions,
                    ) && substituted_array_length_matches(
                        template_length,
                        concrete_length,
                        substitutions,
                    )
                }
                (
                    TypeReferenceNode::Slice {
                        element_type: template_element,
                    },
                    TypeReferenceNode::Slice {
                        element_type: concrete_element,
                    },
                ) => substituted_type_matches(
                    program,
                    *template_element,
                    *concrete_element,
                    substitutions,
                ),
                (
                    TypeReferenceNode::Generic {
                        base_symbol: template_base,
                        lifetime_arguments: template_lifetimes,
                        arguments: template_arguments,
                        ..
                    },
                    TypeReferenceNode::Generic {
                        base_symbol: concrete_base,
                        lifetime_arguments: concrete_lifetimes,
                        arguments: concrete_arguments,
                        ..
                    },
                ) => {
                    if let Some(binding) = substitutions
                        .iter()
                        .find(|binding| binding.parameter == *template_base)
                    {
                        return binding.kind != RepresentativeStaticBindingKind::Const
                            && static_argument_matches_type(
                                program,
                                &binding.argument,
                                concrete,
                                substitutions,
                            );
                    }
                    let template_arguments = program
                        .type_reference_table
                        .type_reference_handles(*template_arguments);
                    let concrete_arguments = program
                        .type_reference_table
                        .type_reference_handles(*concrete_arguments);
                    template_base == concrete_base
                        && template_lifetimes == concrete_lifetimes
                        && template_arguments.len() == concrete_arguments.len()
                        && template_arguments.iter().zip(concrete_arguments).all(
                            |(template, concrete)| {
                                substituted_type_matches(
                                    program,
                                    *template,
                                    *concrete,
                                    substitutions,
                                )
                            },
                        )
                }
                (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
                _ => {
                    program.normalized_type_identity(template)
                        == program.normalized_type_identity(concrete)
                }
            }
        }

        fn substituted_array_length_matches(
            template: &FixedArrayLength,
            concrete: &FixedArrayLength,
            substitutions: &[RepresentativeStaticBinding],
        ) -> bool {
            match (template, concrete) {
                (FixedArrayLength::Literal(template), FixedArrayLength::Literal(concrete)) => {
                    template == concrete
                }
                (
                    FixedArrayLength::ConstParameter { symbol, .. },
                    FixedArrayLength::Literal(concrete),
                ) => {
                    substitutions
                        .iter()
                        .find(|binding| {
                            binding.parameter == *symbol
                                && binding.kind == RepresentativeStaticBindingKind::Const
                        })
                        .and_then(|binding| binding.argument.const_literal.as_ref())
                        .and_then(|literal| literal.value_u64())
                        .and_then(|literal| usize::try_from(literal).ok())
                        == Some(*concrete)
                }
                (FixedArrayLength::ConstParameter { symbol, .. }, _) => {
                    !substitutions
                        .iter()
                        .any(|binding| binding.parameter == *symbol)
                        && template == concrete
                }
                _ => template == concrete,
            }
        }

        fn static_argument_matches_type(
            program: &dyn crate::CheckingInput,
            argument: &StaticMachineArgument,
            concrete: TypeReferenceHandle,
            _substitutions: &[RepresentativeStaticBinding],
        ) -> bool {
            if argument.const_literal.is_some() || argument.evidence_projection.is_some() {
                return false;
            }
            let concrete_node = program.type_reference_table.type_reference(concrete);
            let Some(application) = argument.application.as_ref() else {
                let TypeReferenceNode::Named { symbol, name } = concrete_node else {
                    return false;
                };
                return if argument.symbol.is_valid() {
                    argument.symbol == *symbol
                } else {
                    !symbol.is_valid()
                        && argument.path.len() == 1
                        && argument.path[0].as_str() == name.as_str()
                };
            };
            let TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } = concrete_node
            else {
                return false;
            };
            if argument.symbol != *base_symbol
                || application.lifetime_arguments.as_ref() != lifetime_arguments.as_slice()
            {
                return false;
            }
            let concrete_arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            application.arguments.len() == concrete_arguments.len()
                && application.arguments.iter().zip(concrete_arguments).all(
                    |(argument, concrete)| {
                        static_argument_matches_type(program, argument, *concrete, _substitutions)
                    },
                )
        }

        pub(crate) fn derive_exact_representative_static_application(
            program: &dyn crate::CheckingInput,
            request: &QuotientOperationRequest,
        ) -> Result<RepresentativeStaticApplication, RelationPlanError> {
            let (machine, _) =
                representative_machine_state(program, request.representative_operation.symbol)?;
            validate_static_application(
                program,
                &machine.lifetime_parameters,
                program.machine_type_parameters(machine),
                &request.representative_operation,
            )
        }

        pub(crate) fn validate_static_application(
            program: &dyn crate::CheckingInput,
            lifetime_parameters: &[Identifier],
            parameters: &[TypeParameter],
            selected: &StaticMachineArgument,
        ) -> Result<RepresentativeStaticApplication, RelationPlanError> {
            if !lifetime_parameters.is_empty() {
                return Err(RelationPlanError::RepresentativeLifetimeApplicationRequiresElision);
            }
            let empty_lifetimes: &[Identifier] = &[];
            let empty_arguments: &[StaticMachineArgument] = &[];
            let (lifetime_arguments, arguments) = selected
                .application
                .as_ref()
                .map(|application| {
                    (
                        application.lifetime_arguments.as_ref(),
                        application.arguments.as_ref(),
                    )
                })
                .unwrap_or((empty_lifetimes, empty_arguments));
            if !lifetime_arguments.is_empty() || arguments.len() != parameters.len() {
                return Err(RelationPlanError::RepresentativeStaticArityMismatch);
            }

            let mut bindings = Vec::with_capacity(arguments.len());
            for (position, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
                let kind = match &parameter.kind {
                    TypeParameterKind::Type => {
                        validate_closed_type_argument(program, argument, position)?;
                        RepresentativeStaticBindingKind::Type
                    }
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                        if argument.const_literal.is_none()
                            || argument.symbol.is_valid()
                            || argument.application.is_some()
                            || argument.evidence_projection.is_some()
                        {
                            return Err(
                                RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(
                                    position,
                                ),
                            );
                        }
                        RepresentativeStaticBindingKind::Const
                    }
                    TypeParameterKind::Machine { .. } => {
                        if argument.const_literal.is_some()
                            || argument.evidence_projection.is_some()
                        {
                            return Err(
                                RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(
                                    position,
                                ),
                            );
                        }
                        let (machine, _) = representative_machine_state(program, argument.symbol)
                            .map_err(|_| {
                            RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(
                                position,
                            )
                        })?;
                        validate_static_application(
                            program,
                            &machine.lifetime_parameters,
                            program.machine_type_parameters(machine),
                            argument,
                        )?;
                        RepresentativeStaticBindingKind::Machine
                    }
                    TypeParameterKind::Proposition { .. } => {
                        return Err(
                            RelationPlanError::RepresentativePropositionApplicationUnsupported(
                                position,
                            ),
                        );
                    }
                    TypeParameterKind::Implementation => {
                        return Err(
                            RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(
                                position,
                            ),
                        );
                    }
                };
                bindings.push(RepresentativeStaticBinding {
                    parameter: parameter.symbol,
                    kind,
                    argument: argument.clone(),
                });
            }
            Ok(RepresentativeStaticApplication {
                lifetime_arguments: lifetime_arguments.to_vec(),
                bindings,
            })
        }

        fn validate_closed_type_argument(
            program: &dyn crate::CheckingInput,
            argument: &StaticMachineArgument,
            position: usize,
        ) -> Result<(), RelationPlanError> {
            if argument.const_literal.is_some() || argument.evidence_projection.is_some() {
                return Err(
                    RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position),
                );
            }
            let Some(data) = crate::declarations::data_queries::data_definition_by_symbol(
                program,
                argument.symbol,
            ) else {
                let primitive = argument.application.is_none()
                    && argument.path.len() == 1
                    && PrimitiveType::from_name(argument.path[0].as_str()).is_some();
                if primitive {
                    return Ok(());
                }
                return Err(
                    RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position),
                );
            };
            let nested = validate_static_application(
                program,
                &data.lifetime_parameters,
                program.data_type_parameters(data),
                argument,
            );
            match nested {
                Ok(_) => Ok(()),
                Err(RelationPlanError::RepresentativeStaticArityMismatch) => Err(
                    RelationPlanError::RepresentativeStaticArgumentIsOpen(position),
                ),
                Err(error) => Err(error),
            }
        }
    }
    pub(crate) use normalized_judgment::relation_identity;
    pub(crate) mod theorem {
        use super::representative::RepresentativeRuntimeParameter;
        use super::static_application::validate_static_application;
        use super::{RelationPlanError, RepresentativeStaticApplication};
        use arena::HandleSpan;
        use language_semantics::MachineSupplyMode;
        use symbols::SymbolHandle;
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::machine::Machine;
        use typed_trees::signature::SignatureContract;
        use typed_trees::state::State;
        use typed_trees::types::TypeReferenceNode;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct MachineStateCoordinate {
            pub(crate) machine_symbol: SymbolHandle,
            pub(crate) state_symbol: SymbolHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct SelectedTheoremTelescope {
            pub(crate) machine_symbol: SymbolHandle,
            pub(crate) state_symbol: SymbolHandle,
            pub(crate) parameters: Vec<RepresentativeRuntimeParameter>,
            pub(crate) machine_contracts: HandleSpan<SignatureContract>,
            pub(crate) state_contracts: HandleSpan<SignatureContract>,
            pub(crate) static_application: RepresentativeStaticApplication,
        }

        pub(crate) fn derive_selected_theorem_telescope(
            program: &dyn crate::CheckingInput,
            selected: &StaticMachineArgument,
        ) -> Result<SelectedTheoremTelescope, RelationPlanError> {
            let (machine, state) = selected_theorem_machine_state(program, selected.symbol)?;
            if machine.supply_mode != MachineSupplyMode::CheckedBody || !machine.body_is_present {
                return Err(RelationPlanError::TheoremMustBeCheckedBody);
            }
            if !matches!(
                program
                    .type_reference_table
                    .type_reference(state.return_type),
                TypeReferenceNode::Unit
            ) {
                return Err(RelationPlanError::TheoremMustBeResultless);
            }
            let static_application = validate_static_application(
                program,
                &machine.lifetime_parameters,
                program.machine_type_parameters(machine),
                selected,
            )
            .map_err(|_| RelationPlanError::TheoremStaticApplicationInvalid)?;
            let parameters = program
                .public_state_parameters(state)
                .map(|parameter| RepresentativeRuntimeParameter {
                    symbol: parameter.symbol,
                    type_reference: parameter.type_reference,
                    is_mutable: parameter.is_mutable,
                })
                .collect();
            Ok(SelectedTheoremTelescope {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                parameters,
                machine_contracts: machine.contracts,
                state_contracts: state.contracts,
                static_application,
            })
        }

        fn selected_theorem_machine_state(
            program: &dyn crate::CheckingInput,
            state_symbol: SymbolHandle,
        ) -> Result<(&Machine, &State), RelationPlanError> {
            let mut matches = program.machine_states_named(state_symbol);
            let Some(selected) = matches.next() else {
                return Err(RelationPlanError::TheoremEntryDoesNotResolveExactly);
            };
            if matches.next().is_some() {
                return Err(RelationPlanError::TheoremEntryDoesNotResolveExactly);
            }
            Ok(selected)
        }
    }
    pub(crate) mod theorem_schema {
        use super::theorem::SelectedTheoremTelescope;
        use super::{
            ExactQuotientRelation, PositionalRelation, RelationPlanError, RepresentativeTelescope,
        };
        use symbols::SymbolHandle;
        use typed_trees::domain::ProofFact;
        use typed_trees::signature::SignatureContractKind;
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) use super::{ContractFactLocation, ContractOwner};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum TheoremParameterRole {
            QuotientLeft,
            QuotientRight,
            Shared,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct TheoremSchemaParameter {
            pub(crate) representative_position: usize,
            pub(crate) role: TheoremParameterRole,
            pub(crate) type_reference: TypeReferenceHandle,
            pub(crate) is_mutable: bool,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct TheoremRelationPremise {
            pub(crate) representative_position: usize,
            pub(crate) relation: ExactQuotientRelation,
            pub(crate) left_parameter: usize,
            pub(crate) right_parameter: usize,
        }

        #[derive(Clone, Copy)]
        pub(crate) struct LocatedFact<'a> {
            pub(crate) location: ContractFactLocation,
            pub(crate) fact: &'a ProofFact,
        }

        pub(crate) fn collect_contract_facts<'a>(
            program: &'a dyn crate::CheckingInput,
            theorem: &SelectedTheoremTelescope,
        ) -> Result<(Vec<LocatedFact<'a>>, Vec<LocatedFact<'a>>), RelationPlanError> {
            let mut requires = Vec::new();
            let mut ensures = Vec::new();
            for (owner, span) in [
                (ContractOwner::Machine, theorem.machine_contracts),
                (ContractOwner::State, theorem.state_contracts),
            ] {
                for (contract_position, contract) in program
                    .signature_contracts
                    .span_or_empty(span)
                    .iter()
                    .enumerate()
                {
                    if contract.binding.is_some() {
                        return Err(RelationPlanError::TheoremSchemaNamedEvidenceLane);
                    }
                    let destination = match contract.kind {
                        SignatureContractKind::Requires => &mut requires,
                        SignatureContractKind::Ensures => &mut ensures,
                        SignatureContractKind::EnsuresForResultCase { .. }
                        | SignatureContractKind::Crashes { .. } => {
                            return Err(RelationPlanError::TheoremSchemaUnexpectedContractKind);
                        }
                    };
                    for (fact_position, fact) in program
                        .proof_facts
                        .span_or_empty(contract.facts)
                        .iter()
                        .enumerate()
                    {
                        destination.push(LocatedFact {
                            location: ContractFactLocation {
                                owner,
                                contract_position,
                                fact_position,
                            },
                            fact,
                        });
                    }
                }
            }
            Ok((requires, ensures))
        }

        pub(crate) type TheoremApplicationSide =
            language_semantics::quotient_correspondence::QuotientTheoremApplicationSide;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct TheoremLegalityPremise {
            pub(crate) application: TheoremApplicationSide,
            pub(crate) fact: ContractFactLocation,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct TheoremRepresentativeApplication {
            pub(crate) machine_symbol: SymbolHandle,
            pub(crate) state_symbol: SymbolHandle,
            pub(crate) arguments: Vec<usize>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct ExpectedTheoremSchema {
            pub(crate) parameters: Vec<TheoremSchemaParameter>,
            pub(crate) relation_premises: Vec<TheoremRelationPremise>,
            pub(crate) legality_premises: Vec<TheoremLegalityPremise>,
            pub(crate) left_application: TheoremRepresentativeApplication,
            pub(crate) right_application: TheoremRepresentativeApplication,
            pub(crate) result_relation: PositionalRelation,
        }

        pub(crate) fn derive_expected_theorem_schema(
            program: &dyn crate::CheckingInput,
            input_relations: &[PositionalRelation],
            result_relation: PositionalRelation,
            representative: &RepresentativeTelescope,
        ) -> Result<ExpectedTheoremSchema, RelationPlanError> {
            if input_relations.len() != representative.parameters.len() {
                return Err(RelationPlanError::TheoremSchemaRuntimeArityMismatch);
            }

            let mut parameters = Vec::with_capacity(
                input_relations
                    .iter()
                    .map(|relation| {
                        usize::from(matches!(relation, PositionalRelation::Quotient(_))) + 1
                    })
                    .sum(),
            );
            let mut relation_premises = Vec::new();
            let mut left_arguments = Vec::with_capacity(input_relations.len());
            let mut right_arguments = Vec::with_capacity(input_relations.len());
            for (representative_position, (relation, parameter)) in input_relations
                .iter()
                .zip(&representative.parameters)
                .enumerate()
            {
                match relation {
                    PositionalRelation::Quotient(relation) => {
                        let left_parameter = parameters.len();
                        parameters.push(TheoremSchemaParameter {
                            representative_position,
                            role: TheoremParameterRole::QuotientLeft,
                            type_reference: parameter.type_reference,
                            is_mutable: parameter.is_mutable,
                        });
                        let right_parameter = parameters.len();
                        parameters.push(TheoremSchemaParameter {
                            representative_position,
                            role: TheoremParameterRole::QuotientRight,
                            type_reference: parameter.type_reference,
                            is_mutable: parameter.is_mutable,
                        });
                        left_arguments.push(left_parameter);
                        right_arguments.push(right_parameter);
                        relation_premises.push(TheoremRelationPremise {
                            representative_position,
                            relation: *relation,
                            left_parameter,
                            right_parameter,
                        });
                    }
                    PositionalRelation::ExactEquality(_) => {
                        let shared = parameters.len();
                        parameters.push(TheoremSchemaParameter {
                            representative_position,
                            role: TheoremParameterRole::Shared,
                            type_reference: parameter.type_reference,
                            is_mutable: parameter.is_mutable,
                        });
                        left_arguments.push(shared);
                        right_arguments.push(shared);
                    }
                }
            }

            let left_application = TheoremRepresentativeApplication {
                machine_symbol: representative.machine_symbol,
                state_symbol: representative.state_symbol,
                arguments: left_arguments,
            };
            let right_application = TheoremRepresentativeApplication {
                machine_symbol: representative.machine_symbol,
                state_symbol: representative.state_symbol,
                arguments: right_arguments,
            };
            let mut legality_premises = Vec::new();
            for (owner, contracts) in [
                (
                    ContractOwner::Machine,
                    program
                        .signature_contracts
                        .span_or_empty(representative.machine_contracts),
                ),
                (
                    ContractOwner::State,
                    program
                        .signature_contracts
                        .span_or_empty(representative.state_contracts),
                ),
            ] {
                for (contract_position, contract) in contracts.iter().enumerate() {
                    if contract.kind != SignatureContractKind::Requires {
                        continue;
                    }
                    for fact_position in 0..contract.facts.len() {
                        let fact = ContractFactLocation {
                            owner,
                            contract_position,
                            fact_position,
                        };
                        legality_premises.push(TheoremLegalityPremise {
                            application: TheoremApplicationSide::Left,
                            fact,
                        });
                        legality_premises.push(TheoremLegalityPremise {
                            application: TheoremApplicationSide::Right,
                            fact,
                        });
                    }
                }
            }

            Ok(ExpectedTheoremSchema {
                parameters,
                relation_premises,
                legality_premises,
                left_application,
                right_application,
                result_relation,
            })
        }
    }
    pub(crate) mod theorem_schema_verification {
        use super::proof_fact_identity::{
            ProofFactIdentityContext, ProofValueSubstitution, proof_facts_match,
            static_arguments_match, static_type_identities,
        };
        use super::theorem::SelectedTheoremTelescope;
        use super::theorem_schema::{
            ContractFactLocation, ContractOwner, ExpectedTheoremSchema, TheoremApplicationSide,
            TheoremLegalityPremise, TheoremRepresentativeApplication, collect_contract_facts,
        };
        use super::{PositionalRelation, RelationPlanError, RepresentativeTelescope};
        use crate::frozen_program::expression_table;
        use symbols::SymbolHandle;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::signature::{SignatureContractKind, StateParameter};
        use typed_trees::type_identity::TypeIdentityRequest;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct VerifiedTheoremParameter {
            pub(crate) expected_position: usize,
            pub(crate) theorem_symbol: SymbolHandle,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct VerifiedTheoremFact {
            pub(crate) expected_position: usize,
            pub(crate) actual: ContractFactLocation,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct VerifiedTheoremSchema {
            pub(crate) theorem_machine_symbol: SymbolHandle,
            pub(crate) theorem_state_symbol: SymbolHandle,
            pub(crate) parameters: Vec<VerifiedTheoremParameter>,
            pub(crate) relation_premises: Vec<VerifiedTheoremFact>,
            pub(crate) legality_premises: Vec<VerifiedTheoremFact>,
            pub(crate) conclusion: ContractFactLocation,
        }

        pub(crate) fn verify_selected_theorem_schema(
            program: &dyn crate::CheckingInput,
            representative: &RepresentativeTelescope,
            theorem: &SelectedTheoremTelescope,
            expected: &ExpectedTheoremSchema,
        ) -> Result<VerifiedTheoremSchema, RelationPlanError> {
            let theorem_state =
                typed_trees::machine::machine_by_symbol(program.machines(), theorem.machine_symbol)
                    .and_then(|machine| {
                        typed_trees::state::state_by_symbol(
                            program.machine_states(machine),
                            theorem.state_symbol,
                        )
                    })
                    .ok_or(RelationPlanError::TheoremEntryDoesNotResolveExactly)?;
            let theorem_parameters = program.state_parameters(theorem_state);
            let verified_parameters = verify_parameters(
                program,
                representative,
                theorem,
                theorem_parameters,
                expected,
            )?;

            let (mut requires, ensures) = collect_contract_facts(program, theorem)?;
            if requires.len() != expected.relation_premises.len() + expected.legality_premises.len()
            {
                return Err(RelationPlanError::TheoremSchemaPremiseCountMismatch);
            }
            if ensures.len() != 1 {
                return Err(RelationPlanError::TheoremSchemaConclusionCountMismatch);
            }

            let mut relation_premises = Vec::with_capacity(expected.relation_premises.len());
            for (position, premise) in expected.relation_premises.iter().enumerate() {
                let Some(relation) =
                    program.proposition_definition_by_symbol(premise.relation.relation_symbol)
                else {
                    return Err(RelationPlanError::TheoremSchemaRelationPremiseMismatch(
                        position,
                    ));
                };
                let left = &theorem_parameters[premise.left_parameter];
                let right = &theorem_parameters[premise.right_parameter];
                let Some(found) = requires.iter().position(|actual| {
            crate::proof_contracts::quotients::equivalence_selection::fact_is_exact_relation_pair(
                program,
                actual.fact,
                relation,
                left,
                right,
                theorem_parameters,
            )
        }) else {
            return Err(RelationPlanError::TheoremSchemaRelationPremiseMismatch(
                position,
            ));
        };
                let actual = requires.remove(found).location;
                relation_premises.push(VerifiedTheoremFact {
                    expected_position: position,
                    actual,
                });
            }

            let mut legality_premises = Vec::with_capacity(expected.legality_premises.len());
            for (position, premise) in expected.legality_premises.iter().enumerate() {
                let representative_fact = representative_fact(program, representative, premise)
                    .ok_or(RelationPlanError::TheoremSchemaLegalityPremiseMismatch(
                        position,
                    ))?;
                let representative_parameters = &representative.parameters;
                let application = match premise.application {
                    TheoremApplicationSide::Left => &expected.left_application,
                    TheoremApplicationSide::Right => &expected.right_application,
                };
                let representative_values = representative_parameters
                    .iter()
                    .zip(&application.arguments)
                    .map(|(parameter, theorem_position)| {
                        ProofValueSubstitution::symbolic(
                            parameter.symbol,
                            format!("$theorem_parameter_{theorem_position}"),
                        )
                    })
                    .collect::<Vec<_>>();
                let theorem_values = theorem_parameters
                    .iter()
                    .enumerate()
                    .map(|(position, parameter)| {
                        ProofValueSubstitution::symbolic(
                            parameter.symbol,
                            format!("$theorem_parameter_{position}"),
                        )
                    })
                    .collect::<Vec<_>>();
                let Some(found) = requires.iter().position(|actual| {
                    proof_facts_match(
                        program,
                        representative_fact,
                        actual.fact,
                        ProofFactIdentityContext {
                            values: &representative_values,
                            static_bindings: &representative.static_application.bindings,
                        },
                        ProofFactIdentityContext {
                            values: &theorem_values,
                            static_bindings: &theorem.static_application.bindings,
                        },
                    )
                }) else {
                    return Err(RelationPlanError::TheoremSchemaLegalityPremiseMismatch(
                        position,
                    ));
                };
                let actual = requires.remove(found).location;
                legality_premises.push(VerifiedTheoremFact {
                    expected_position: position,
                    actual,
                });
            }
            if !requires.is_empty() {
                return Err(RelationPlanError::TheoremSchemaPremiseCountMismatch);
            }

            let conclusion = ensures[0];
            if !conclusion_matches(
                program,
                conclusion.fact,
                representative,
                theorem_parameters,
                expected,
            ) {
                return Err(RelationPlanError::TheoremSchemaConclusionMismatch);
            }

            Ok(VerifiedTheoremSchema {
                theorem_machine_symbol: theorem.machine_symbol,
                theorem_state_symbol: theorem.state_symbol,
                parameters: verified_parameters,
                relation_premises,
                legality_premises,
                conclusion: conclusion.location,
            })
        }
        fn representative_fact<'a>(
            program: &'a dyn crate::CheckingInput,
            representative: &RepresentativeTelescope,
            premise: &TheoremLegalityPremise,
        ) -> Option<&'a ProofFact> {
            let span = match premise.fact.owner {
                ContractOwner::Machine => representative.machine_contracts,
                ContractOwner::State => representative.state_contracts,
            };
            let contract = program
                .signature_contracts
                .span_or_empty(span)
                .get(premise.fact.contract_position)?;
            (contract.kind == SignatureContractKind::Requires)
                .then(|| {
                    program
                        .proof_facts
                        .span_or_empty(contract.facts)
                        .get(premise.fact.fact_position)
                })
                .flatten()
        }

        fn conclusion_matches(
            program: &dyn crate::CheckingInput,
            actual: &ProofFact,
            representative: &RepresentativeTelescope,
            theorem_parameters: &[StateParameter],
            expected: &ExpectedTheoremSchema,
        ) -> bool {
            let arguments = match expected.result_relation {
                PositionalRelation::Quotient(relation) => {
                    let Some(application) = actual.as_proposition() else {
                        return false;
                    };
                    if application.proposition != relation.relation_symbol
                        || !application.binder_arguments.is_empty()
                    {
                        return false;
                    }
                    let [left, right] =
                        expression_table(program).expression_handles(application.arguments)
                    else {
                        return false;
                    };
                    [*left, *right]
                }
                PositionalRelation::ExactEquality(_) => {
                    let Some(expression) = actual.as_expression() else {
                        return false;
                    };
                    let Some(binary) = expression_table(program).binary_node(*expression) else {
                        return false;
                    };
                    if binary.operator != typed_trees::expression::BinaryOperator::Equal {
                        return false;
                    }
                    [binary.left, binary.right]
                }
            };
            matches!(&arguments, [left, right]
            if representative_application_matches(
                program,
                *left,
                representative,
                theorem_parameters,
                &expected.left_application,
            ) && representative_application_matches(
                program,
                *right,
                representative,
                theorem_parameters,
                &expected.right_application,
            ))
        }

        fn representative_application_matches(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            representative: &RepresentativeTelescope,
            theorem_parameters: &[StateParameter],
            expected: &TheoremRepresentativeApplication,
        ) -> bool {
            let Some(call) = expression_table(program).call_node(expression) else {
                return false;
            };
            if expected.machine_symbol != representative.machine_symbol
                || expected.state_symbol != representative.state_symbol
                || call.target_symbol != expected.state_symbol
                || !call.selects_only_nominal_route()
                || !call.evidence_arguments.is_empty()
                || call.machine_arguments.len() != representative.static_application.bindings.len()
                || !call
                    .machine_arguments
                    .iter()
                    .zip(&representative.static_application.bindings)
                    .all(|(actual, expected)| static_arguments_match(actual, &expected.argument))
            {
                return false;
            }
            let expected_arguments = expected.arguments.iter().copied();
            if call.receiver.is_valid() {
                return false;
            }
            let arguments = expression_table(program).expression_handles(call.arguments);
            arguments.len() == expected_arguments.len()
                && arguments
                    .iter()
                    .zip(expected_arguments)
                    .all(|(actual, position)| {
                        expression_is_parameter(program, *actual, theorem_parameters, position)
                    })
        }

        fn expression_is_parameter(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            parameters: &[StateParameter],
            position: usize,
        ) -> bool {
            let Some(parameter) = parameters.get(position) else {
                return false;
            };
            matches!(expression_table(program).expression(expression),
        ExpressionNode::Name(path) if parameter.symbol.is_valid() && path.symbol == parameter.symbol)
        }

        pub(crate) fn verify_parameters(
            program: &dyn crate::CheckingInput,
            representative: &RepresentativeTelescope,
            theorem: &SelectedTheoremTelescope,
            theorem_parameters: &[StateParameter],
            expected: &ExpectedTheoremSchema,
        ) -> Result<Vec<VerifiedTheoremParameter>, RelationPlanError> {
            if theorem_parameters.len() != expected.parameters.len() {
                return Err(RelationPlanError::TheoremSchemaParameterArityMismatch);
            }
            let representative_type_bindings =
                static_type_identities(&representative.static_application);
            let theorem_type_bindings = static_type_identities(&theorem.static_application);
            theorem_parameters
                .iter()
                .zip(&expected.parameters)
                .enumerate()
                .map(|(position, (actual, expected))| {
                    if actual.is_const {
                        return Err(RelationPlanError::TheoremSchemaConstParameter(position));
                    }
                    if actual.is_mutable != expected.is_mutable {
                        return Err(RelationPlanError::TheoremSchemaParameterModeMismatch(
                            position,
                        ));
                    }
                    let expected_identity = program.type_identity(TypeIdentityRequest {
                        binders: &representative_type_bindings,
                        ..TypeIdentityRequest::ordinary(expected.type_reference)
                    });
                    let actual_identity = program.type_identity(TypeIdentityRequest {
                        binders: &theorem_type_bindings,
                        ..TypeIdentityRequest::ordinary(actual.type_reference)
                    });
                    if expected_identity != actual_identity {
                        return Err(RelationPlanError::TheoremSchemaParameterTypeMismatch(
                            position,
                        ));
                    }
                    Ok(VerifiedTheoremParameter {
                        expected_position: position,
                        theorem_symbol: actual.symbol,
                    })
                })
                .collect()
        }
    }
    pub(crate) mod transport_schema {
        use super::correspondence_certificate::proof_value_substitutions;
        use super::precondition::{
            ContractFactLocation, ContractOwner, RepresentativePreconditionPartition,
            precondition_fact_at,
        };
        use super::proof_fact_identity::{
            ProofFactIdentityContext, ProofValueSubstitution, proof_facts_match,
        };
        use super::theorem::SelectedTheoremTelescope;
        use super::theorem_schema::{
            ExpectedTheoremSchema, TheoremApplicationSide, collect_contract_facts,
        };
        use super::theorem_schema_verification::{VerifiedTheoremParameter, verify_parameters};
        use super::{DirectLiftRuntimeCorrespondence, RelationPlanError, RepresentativeTelescope};
        use symbols::SymbolHandle;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct VerifiedTransportFact {
            pub(crate) application: TheoremApplicationSide,
            pub(crate) source: ContractFactLocation,
            pub(crate) actual: ContractFactLocation,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct VerifiedForwardPreconditionTransportSchema {
            pub(crate) role: typed_trees::expression::QuotientTheoremRole,
            pub(crate) selected_application: SelectedTheoremTelescope,
            pub(crate) theorem_machine_symbol: SymbolHandle,
            pub(crate) theorem_state_symbol: SymbolHandle,
            pub(crate) parameters: Vec<VerifiedTheoremParameter>,
            pub(crate) public_premises: Vec<VerifiedTransportFact>,
            pub(crate) representative_conclusions: Vec<VerifiedTransportFact>,
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn verify_forward_precondition_transport_schema(
            program: &dyn crate::CheckingInput,
            public_machine: &Machine,
            public_state: &State,
            representative: &RepresentativeTelescope,
            theorem: &SelectedTheoremTelescope,
            runtime: &DirectLiftRuntimeCorrespondence,
            expected_congruence: &ExpectedTheoremSchema,
            public: &RepresentativePreconditionPartition,
            representative_partition: &RepresentativePreconditionPartition,
        ) -> Result<VerifiedForwardPreconditionTransportSchema, RelationPlanError> {
            let theorem_state = exact_theorem_state(program, theorem)?;
            let theorem_parameters = program.state_parameters(theorem_state);
            let parameters = verify_parameters(
                program,
                representative,
                theorem,
                theorem_parameters,
                expected_congruence,
            )?;
            let (requires, ensures) = collect_contract_facts(program, theorem)?;
            let public_locations = ordered_locations(public);
            let representative_locations = ordered_locations(representative_partition);
            let expected_public_count = public_locations
                .len()
                .checked_mul(2)
                .ok_or(RelationPlanError::TransportSchemaPremiseCountMismatch)?;
            let expected_representative_count = representative_locations
                .len()
                .checked_mul(2)
                .ok_or(RelationPlanError::TransportSchemaConclusionCountMismatch)?;
            if requires.len() != expected_public_count {
                return Err(RelationPlanError::TransportSchemaPremiseCountMismatch);
            }
            if ensures.len() != expected_representative_count {
                return Err(RelationPlanError::TransportSchemaConclusionCountMismatch);
            }

            let theorem_values = theorem_parameters
                .iter()
                .enumerate()
                .map(|(position, parameter)| {
                    ProofValueSubstitution::symbolic(
                        parameter.symbol,
                        format!("$transport_parameter_{position}"),
                    )
                })
                .collect::<Vec<_>>();
            // Q is stated over public quotient values, whereas theorem parameters
            // are raw representatives. Its substitution therefore uses each exact
            // selected projection; P keeps its independent representative namespace.
            let mut public_projections: Vec<
                Option<(SymbolHandle, typed_trees::types::TypeReferenceHandle)>,
            > = Vec::new();
            for premise in &expected_congruence.relation_premises {
                let position = runtime
                    .positions
                    .get(premise.representative_position)
                    .ok_or(RelationPlanError::DirectLiftRuntimeArityMismatch)?;
                let super::runtime_correspondence::DirectLiftArgumentSource::PublicParameter(
                    symbol,
                ) = position.source
                else {
                    return Err(RelationPlanError::TransportSchemaPublicPremiseMismatch(0));
                };
                let quotient = premise.relation.quotient_type;
                let index = symbol.arena_index() as usize;
                if public_projections.len() <= index {
                    public_projections.resize(index + 1, None);
                }
                if let Some((retained, previous)) = public_projections[index] {
                    if retained != symbol
                        || program.normalized_type_identity(previous)
                            != program.normalized_type_identity(quotient)
                    {
                        return Err(RelationPlanError::TransportSchemaPublicPremiseMismatch(0));
                    }
                } else {
                    public_projections[index] = Some((symbol, quotient));
                }
            }
            let mut public_premises = Vec::with_capacity(requires.len());
            let mut representative_conclusions = Vec::with_capacity(ensures.len());
            let side_values = [TheoremApplicationSide::Left, TheoremApplicationSide::Right]
                .into_iter()
                .map(|application| {
                    let application_schema = match application {
                        TheoremApplicationSide::Left => &expected_congruence.left_application,
                        TheoremApplicationSide::Right => &expected_congruence.right_application,
                    };
                    if runtime.positions.len() != application_schema.arguments.len() {
                        return Err(RelationPlanError::DirectLiftRuntimeArityMismatch);
                    }
                    let (mut public_values, representative_values) =
                        proof_value_substitutions(runtime, |position| {
                            format!(
                                "$transport_parameter_{}",
                                application_schema.arguments[position]
                            )
                        });
                    for value in &mut public_values {
                        if let Some(Some((symbol, quotient))) =
                            public_projections.get(value.symbol.arena_index() as usize)
                        {
                            if *symbol != value.symbol {
                                return Err(
                                    RelationPlanError::TransportSchemaPublicPremiseMismatch(0),
                                );
                            }
                            *value = value.quotient_projection(program, *quotient);
                        }
                    }
                    Ok((application, (public_values, representative_values)))
                })
                .collect::<Result<Vec<_>, RelationPlanError>>()?;

            let mut premise_position = 0;
            for source in &public_locations {
                for (application, (public_values, _)) in &side_values {
                    let expected = precondition_fact_at(
                        program,
                        public_machine.contracts,
                        public_state.contracts,
                        *source,
                    )
                    .ok_or(
                        RelationPlanError::TransportSchemaPublicPremiseMismatch(premise_position),
                    )?;
                    let actual = requires[premise_position];
                    if !proof_facts_match(
                        program,
                        expected,
                        actual.fact,
                        ProofFactIdentityContext {
                            values: public_values,
                            static_bindings: &[],
                        },
                        ProofFactIdentityContext {
                            values: &theorem_values,
                            static_bindings: &theorem.static_application.bindings,
                        },
                    ) {
                        return Err(RelationPlanError::TransportSchemaPublicPremiseMismatch(
                            premise_position,
                        ));
                    }
                    public_premises.push(VerifiedTransportFact {
                        application: *application,
                        source: *source,
                        actual: actual.location,
                    });
                    premise_position += 1;
                }
            }
            let mut conclusion_position = 0;
            for source in &representative_locations {
                for (application, (_, representative_values)) in &side_values {
                    let expected = precondition_fact_at(
                        program,
                        representative.machine_contracts,
                        representative.state_contracts,
                        *source,
                    )
                    .ok_or(
                        RelationPlanError::TransportSchemaRepresentativeConclusionMismatch(
                            conclusion_position,
                        ),
                    )?;
                    let actual = ensures[conclusion_position];
                    if !proof_facts_match(
                        program,
                        expected,
                        actual.fact,
                        ProofFactIdentityContext {
                            values: representative_values,
                            static_bindings: &representative.static_application.bindings,
                        },
                        ProofFactIdentityContext {
                            values: &theorem_values,
                            static_bindings: &theorem.static_application.bindings,
                        },
                    ) {
                        return Err(
                            RelationPlanError::TransportSchemaRepresentativeConclusionMismatch(
                                conclusion_position,
                            ),
                        );
                    }
                    representative_conclusions.push(VerifiedTransportFact {
                        application: *application,
                        source: *source,
                        actual: actual.location,
                    });
                    conclusion_position += 1;
                }
            }

            Ok(VerifiedForwardPreconditionTransportSchema {
                role: typed_trees::expression::QuotientTheoremRole::ForwardPreconditionTransport,
                selected_application: theorem.clone(),
                theorem_machine_symbol: theorem.machine_symbol,
                theorem_state_symbol: theorem.state_symbol,
                parameters,
                public_premises,
                representative_conclusions,
            })
        }

        fn exact_theorem_state<'a>(
            program: &'a dyn crate::CheckingInput,
            theorem: &SelectedTheoremTelescope,
        ) -> Result<&'a State, RelationPlanError> {
            let mut matches = program
                .machine_states_named(theorem.state_symbol)
                .filter(|(machine, _)| machine.symbol == theorem.machine_symbol)
                .map(|(_, state)| state);
            let Some(state) = matches.next() else {
                return Err(RelationPlanError::TheoremEntryDoesNotResolveExactly);
            };
            if matches.next().is_some() {
                return Err(RelationPlanError::TheoremEntryDoesNotResolveExactly);
            }
            Ok(state)
        }

        fn ordered_locations(
            partition: &RepresentativePreconditionPartition,
        ) -> Vec<ContractFactLocation> {
            let mut locations =
                Vec::with_capacity(partition.dependent.len() + partition.fixed.len());
            locations.extend_from_slice(&partition.dependent);
            locations.extend_from_slice(&partition.fixed);
            locations.sort_by_key(|location| {
                (
                    match location.owner {
                        ContractOwner::Machine => 0,
                        ContractOwner::State => 1,
                    },
                    location.contract_position,
                    location.fact_position,
                )
            });
            locations
        }
    }

    use correspondence_certificate::{
        DirectLiftPreconditionImplication, FixedRepresentativeCallPreconditions,
        QuotientCorrespondenceCertificate, compose_define_correspondence_certificate,
        compose_lift_correspondence_certificate, compose_lift_transport_correspondence_certificate,
        derive_direct_lift_precondition_implication,
        derive_fixed_representative_call_preconditions,
    };
    use precondition::{
        DefinePreconditionCorrespondence, RepresentativePreconditionPartition,
        derive_define_precondition_correspondence,
        derive_direct_lift_public_precondition_partition, derive_public_precondition_partition,
        derive_representative_precondition_partition,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ContractOwner {
        Machine,
        State,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ContractFactLocation {
        pub(crate) owner: ContractOwner,
        pub(crate) contract_position: usize,
        pub(crate) fact_position: usize,
    }

    use representative::{
        RepresentativeTelescope, derive_representative_telescope, representative_machine_state,
    };
    use runtime_correspondence::DefineRuntimePosition;
    use runtime_correspondence::{
        DefineRuntimeCorrespondence, DirectLiftRuntimeCorrespondence,
        closed_lift_literal_for_representative, derive_define_runtime_correspondence,
        derive_direct_lift_runtime_correspondence,
    };
    use theorem::{MachineStateCoordinate, SelectedTheoremTelescope};
    use theorem_schema::{ExpectedTheoremSchema, derive_expected_theorem_schema};
    use theorem_schema_verification::{VerifiedTheoremSchema, verify_selected_theorem_schema};
    use transport_schema::{
        VerifiedForwardPreconditionTransportSchema, verify_forward_precondition_transport_schema,
    };

    pub(crate) use normalized_judgment::normalized_direct_correspondence;
    pub(crate) use result_flow::{
        CompleteResultFlow, complete_result_flow, fallthrough_result_root,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ExactQuotientRelation {
        pub(crate) quotient_type: TypeReferenceHandle,
        pub(crate) quotient_symbol: SymbolHandle,
        pub(crate) relation_symbol: SymbolHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum PositionalRelation {
        Quotient(ExactQuotientRelation),
        ExactEquality(TypeReferenceHandle),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct DirectTerminalRelationPlan {
        pub(crate) input_relations: Vec<PositionalRelation>,
        pub(crate) result_relation: PositionalRelation,
        pub(crate) representative: RepresentativeTelescope,
        pub(crate) representative_termination: Option<MachineStateCoordinate>,
        pub(crate) representative_purity: Option<MachineStateCoordinate>,
        pub(crate) theorem_evidence: Vec<PlannedQuotientTheoremEvidence>,
        expected_theorem_schema: ExpectedTheoremSchema,
        pub(crate) theorem_schema_verification: Result<VerifiedTheoremSchema, RelationPlanError>,
        pub(crate) transport_schema_verification:
            Option<Result<VerifiedForwardPreconditionTransportSchema, RelationPlanError>>,
        pub(crate) direct_lift_correspondence: Option<DirectLiftRuntimeCorrespondence>,
        pub(crate) define_correspondence: Option<DefineRuntimeCorrespondence>,
        pub(crate) public_precondition: Option<RepresentativePreconditionPartition>,
        pub(crate) representative_precondition: Option<RepresentativePreconditionPartition>,
        pub(crate) direct_lift_precondition_implication: Option<DirectLiftPreconditionImplication>,
        pub(crate) fixed_representative_call_preconditions:
            Option<FixedRepresentativeCallPreconditions>,
        pub(crate) define_precondition_correspondence: Option<DefinePreconditionCorrespondence>,
        pub(crate) correspondence_certificate: Option<QuotientCorrespondenceCertificate>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PlannedQuotientTheoremEvidence {
        pub(crate) role: QuotientTheoremRole,
        pub(crate) selected_application: SelectedTheoremTelescope,
        pub(crate) termination: Option<MachineStateCoordinate>,
        pub(crate) purity: Option<MachineStateCoordinate>,
        pub(crate) crash_free: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum RepresentativeStaticBindingKind {
        Type,
        Const,
        Machine,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct RepresentativeStaticBinding {
        pub(crate) parameter: SymbolHandle,
        pub(crate) kind: RepresentativeStaticBindingKind,
        pub(crate) argument: StaticMachineArgument,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct RepresentativeStaticApplication {
        pub(crate) lifetime_arguments: Vec<Identifier>,
        pub(crate) bindings: Vec<RepresentativeStaticBinding>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum RelationPlanError {
        UnresolvedArgumentType(usize),
        UnresolvedPositionalRelationApplication(usize),
        UnresolvedResultRelationApplication,
        RepresentativeEntryDoesNotResolveExactly,
        RepresentativeResultTypeIsUnresolved,
        RepresentativeStaticArityMismatch,
        RepresentativeStaticArgumentCategoryMismatch(usize),
        RepresentativeStaticArgumentIsOpen(usize),
        RepresentativeLifetimeApplicationRequiresElision,
        RepresentativePropositionApplicationUnsupported(usize),
        RepresentativeClosureIsNotChecked,
        TheoremEntryDoesNotResolveExactly,
        SelectedTheoremClosureIsNotChecked,
        NonCanonicalTheoremRoleCollection,
        TheoremMustBeCheckedBody,
        TheoremMustBeResultless,
        TheoremStaticApplicationInvalid,
        TheoremSchemaRuntimeArityMismatch,
        TheoremSchemaParameterArityMismatch,
        TheoremSchemaConstParameter(usize),
        TheoremSchemaParameterModeMismatch(usize),
        TheoremSchemaParameterTypeMismatch(usize),
        TheoremSchemaNamedEvidenceLane,
        TheoremSchemaUnexpectedContractKind,
        TheoremSchemaPremiseCountMismatch,
        TheoremSchemaRelationPremiseMismatch(usize),
        TheoremSchemaLegalityPremiseMismatch(usize),
        TheoremSchemaConclusionCountMismatch,
        TheoremSchemaConclusionMismatch,
        TransportSchemaPremiseCountMismatch,
        TransportSchemaPublicPremiseMismatch(usize),
        TransportSchemaConclusionCountMismatch,
        TransportSchemaRepresentativeConclusionMismatch(usize),
        DirectLiftOwnerRequiresSubstitution,
        DirectLiftRuntimeArityMismatch,
        DirectLiftParameterIdentityNotUnique,
        DirectLiftArgumentIsNotPublicParameter(usize),
        DirectLiftParameterModeMismatch(usize),
        DirectLiftParameterTypeMismatch(usize),
        DirectLiftLiteralTargetMismatch(usize),
        DirectLiftResultTypeMismatch,
        DirectLiftLeftPreconditionNotImplied(usize),
        DirectLiftRightPreconditionNotImplied(usize),
        DirectLiftTheoremLegalityMismatch,
        DirectLiftFixedPreconditionNotImplied(usize),
        DirectLiftFixedTheoremLegalityMismatch,
        DefineOwnerRequiresSubstitution,
        DefineRuntimeArityMismatch,
        DefineParameterIdentityNotUnique,
        DefineArgumentIsNotPublicParameter(usize),
        DefineArgumentOrderMismatch(usize),
        DefineParameterModeMismatch(usize),
        DefineParameterTypeMismatch(usize),
        DefineResultTypeMismatch,
        PreconditionDependencyUnresolved,
        DefinePreconditionMismatch,
    }

    impl fmt::Display for RelationPlanError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
            Self::UnresolvedArgumentType(position) => write!(
                formatter,
                "argument {position} has no exact declared type; adapted lift arguments require later expression typing"
            ),
            Self::UnresolvedPositionalRelationApplication(position) => write!(
                formatter,
                "argument {position}'s quotient relation has an open binder application that requires the representative-operation telescope"
            ),
            Self::UnresolvedResultRelationApplication => formatter.write_str(
                "the result quotient relation has an open binder application that requires the representative-operation result telescope",
            ),
            Self::RepresentativeEntryDoesNotResolveExactly => formatter.write_str(
                "the retained representative entry symbol does not resolve to exactly one machine state",
            ),
            Self::RepresentativeResultTypeIsUnresolved => formatter.write_str(
                "the representative operation has no exact result type",
            ),
            Self::RepresentativeStaticArityMismatch => formatter.write_str(
                "the representative static application does not exactly match its declaration parameter arity",
            ),
            Self::RepresentativeStaticArgumentCategoryMismatch(position) => write!(
                formatter,
                "representative static argument {position} has the wrong declaration category"
            ),
            Self::RepresentativeStaticArgumentIsOpen(position) => write!(
                formatter,
                "representative static argument {position} is not one closed application"
            ),
            Self::RepresentativeLifetimeApplicationRequiresElision => formatter.write_str(
                "representative lifetime arguments require the ordinary call-site elision judgment",
            ),
            Self::RepresentativePropositionApplicationUnsupported(position) => write!(
                formatter,
                "representative proposition argument {position} has no closed application boundary yet"
            ),
            Self::RepresentativeClosureIsNotChecked => formatter.write_str(
                "the selected representative operation's transitive call closure reaches an admitted or boundary machine; a representative-level result whose closure needs a quotient assumption cannot be accepted under a policy refusing them",
            ),
            Self::SelectedTheoremClosureIsNotChecked => formatter.write_str(
                "a selected theorem's transitive call closure reaches an admitted or boundary proof machine; admitted evidence cannot license quotient substitution",
            ),
            Self::TheoremEntryDoesNotResolveExactly => formatter.write_str(
                "the selected theorem does not resolve to one exact machine entry",
            ),
            Self::NonCanonicalTheoremRoleCollection => formatter.write_str(
                "the quotient theorem evidence is missing, duplicated, surplus, or not in canonical Congruence/ForwardPreconditionTransport order",
            ),
            Self::TheoremMustBeCheckedBody => formatter.write_str(
                "the selected theorem must be one bodyful checked machine; boundary, accepted, and external proof sources cannot license quotient substitution",
            ),
            Self::TheoremMustBeResultless => formatter.write_str(
                "the selected theorem must return Unit; a result-bearing machine is not proof-static authority",
            ),
            Self::TheoremStaticApplicationInvalid => formatter.write_str(
                "the selected theorem's complete static application is open, mismatched, or otherwise unresolved",
            ),
            Self::TheoremSchemaRuntimeArityMismatch => formatter.write_str(
                "the representative runtime telescope does not match the quotient operation argument telescope",
            ),
            Self::TheoremSchemaParameterArityMismatch => formatter.write_str(
                "the selected theorem's ordinary parameter arity does not exactly match the derived theorem schema",
            ),
            Self::TheoremSchemaConstParameter(position) => write!(
                formatter,
                "selected theorem parameter {position} is const; theorem-schema parameters must all be ordinary proof-static values"
            ),
            Self::TheoremSchemaParameterModeMismatch(position) => write!(
                formatter,
                "selected theorem parameter {position} changes the derived representative access mode"
            ),
            Self::TheoremSchemaParameterTypeMismatch(position) => write!(
                formatter,
                "selected theorem parameter {position} does not have the exact derived type after both static applications are substituted"
            ),
            Self::TheoremSchemaNamedEvidenceLane => formatter.write_str(
                "the selected theorem schema contains a named contract binding and would expose a runtime evidence lane",
            ),
            Self::TheoremSchemaUnexpectedContractKind => formatter.write_str(
                "the selected theorem schema contains a result-case or crash contract outside the exact requires/ensures theorem shape",
            ),
            Self::TheoremSchemaPremiseCountMismatch => formatter.write_str(
                "the selected theorem's requires fact count does not exactly match all relation and representative-legality premises",
            ),
            Self::TheoremSchemaRelationPremiseMismatch(position) => write!(
                formatter,
                "selected theorem relation premise {position} is missing, duplicated, finer, or names a different relation"
            ),
            Self::TheoremSchemaLegalityPremiseMismatch(position) => write!(
                formatter,
                "selected theorem representative-legality premise {position} does not exactly match its substituted representative requires fact"
            ),
            Self::TheoremSchemaConclusionCountMismatch => formatter.write_str(
                "the selected theorem must have exactly one ordinary ensures fact and no other conclusion lane",
            ),
            Self::TheoremSchemaConclusionMismatch => formatter.write_str(
                "the selected theorem conclusion is not the exact result relation over the two exact representative applications",
            ),
            Self::TransportSchemaPremiseCountMismatch => formatter.write_str(
                "the selected transport theorem's requires fact count does not exactly match the complete ordered public-Q roster for both representative sides",
            ),
            Self::TransportSchemaPublicPremiseMismatch(position) => write!(
                formatter,
                "selected transport public-Q premise {position} is missing, reordered, or does not exactly match its side-specific substitution"
            ),
            Self::TransportSchemaConclusionCountMismatch => formatter.write_str(
                "the selected transport theorem's ensures fact count does not exactly match the complete ordered representative-P roster for both representative sides",
            ),
            Self::TransportSchemaRepresentativeConclusionMismatch(position) => write!(
                formatter,
                "selected transport representative-P conclusion {position} is missing, reordered, or does not exactly match its side-specific substitution"
            ),
            Self::DirectLiftOwnerRequiresSubstitution => formatter.write_str(
                "the direct-lift precondition rung does not yet substitute a generic quotient owner",
            ),
            Self::DirectLiftRuntimeArityMismatch => formatter.write_str(
                "the bounded direct-lift rung requires equal authored-call, representative, and relation arity; the authored call may omit or repeat public parameters",
            ),
            Self::DirectLiftParameterIdentityNotUnique => formatter.write_str(
                "the bounded direct-lift rung requires unique public and representative parameter identities",
            ),
            Self::DirectLiftArgumentIsNotPublicParameter(position) => write!(
                formatter,
                "direct-lift argument {position} is neither a direct public parameter nor an admitted closed literal"
            ),
            Self::DirectLiftParameterModeMismatch(position) => write!(
                formatter,
                "direct-lift parameter {position} changes mutable/borrow mode"
            ),
            Self::DirectLiftParameterTypeMismatch(position) => write!(
                formatter,
                "direct-lift parameter {position} does not map its exact quotient carrier or ordinary type to the representative parameter"
            ),
            Self::DirectLiftLiteralTargetMismatch(position) => write!(
                formatter,
                "direct-lift literal {position} does not have an admitted exact representative target and value identity"
            ),
            Self::DirectLiftResultTypeMismatch => formatter.write_str(
                "the direct-lift result quotient carrier does not match the representative result",
            ),
            Self::DirectLiftLeftPreconditionNotImplied(position) => write!(
                formatter,
                "public Q does not contain dependent representative P fact {position} after exact left-application substitution"
            ),
            Self::DirectLiftRightPreconditionNotImplied(position) => write!(
                formatter,
                "public Q does not contain dependent representative P fact {position} after exact right-application substitution"
            ),
            Self::DirectLiftTheoremLegalityMismatch => formatter.write_str(
                "the direct-lift implication row does not join to one exact verified theorem-legality coordinate",
            ),
            Self::DirectLiftFixedPreconditionNotImplied(position) => write!(
                formatter,
                "public fixed Q does not discharge representative fixed call precondition {position} after exact runtime substitution"
            ),
            Self::DirectLiftFixedTheoremLegalityMismatch => formatter.write_str(
                "the fixed representative call-precondition row does not join to both exact verified theorem-legality coordinates",
            ),
            Self::DefineOwnerRequiresSubstitution => formatter.write_str(
                "the quotient-facing definition is generic and requires exact owner-telescope substitution",
            ),
            Self::DefineRuntimeArityMismatch => formatter.write_str(
                "the public, authored-call, and representative runtime telescopes have different arity",
            ),
            Self::DefineParameterIdentityNotUnique => formatter.write_str(
                "the public or representative runtime telescope repeats one parameter identity",
            ),
            Self::DefineArgumentIsNotPublicParameter(position) => write!(
                formatter,
                "define argument {position} is not one exact direct public parameter"
            ),
            Self::DefineArgumentOrderMismatch(position) => write!(
                formatter,
                "define argument {position} does not name the public parameter at the same position"
            ),
            Self::DefineParameterModeMismatch(position) => write!(
                formatter,
                "define parameter {position} changes mutable/borrow mode"
            ),
            Self::DefineParameterTypeMismatch(position) => write!(
                formatter,
                "define parameter {position} does not map its exact quotient carrier or ordinary type to the representative parameter"
            ),
            Self::DefineResultTypeMismatch => formatter.write_str(
                "the exact quotient result carrier does not match the representative result",
            ),
            Self::PreconditionDependencyUnresolved => formatter.write_str(
                "a quotient-facing or representative precondition contains an unresolved value identity and cannot be partitioned by quotient-bearing position",
            ),
            Self::DefinePreconditionMismatch => formatter.write_str(
                "the quotient-facing and representative preconditions are not one exact position-substituted bijection",
            ),
        }
        }
    }

    pub(crate) fn derive_direct_terminal_plan(
        program: &dyn crate::CheckingInput,
        termination: &dyn super::CheckedTerminationOracle,
        machine: &Machine,
        state: &State,
        call: &TableCallExpression,
        request: &QuotientOperationRequest,
    ) -> Result<DirectTerminalRelationPlan, RelationPlanError> {
        let (input_relations, result_relation, representative) =
            derive_relation_and_representative(program, machine, state, call, request)?;
        let representative_termination = unconditional_termination_fence(
            termination,
            representative.machine_symbol,
            representative.state_symbol,
        );
        validate_theorem_role_collection(request)?;
        let operational = crate::infer_operational_may(program);
        let service_reaches = crate::infer_service_reaches(program, &operational);
        let representative_purity = pure_effect_fence(
            program,
            representative.machine_symbol,
            representative.state_symbol,
            representative
                .parameters
                .iter()
                .any(|parameter| parameter.is_mutable),
            &operational,
            &service_reaches,
        );
        let theorem_evidence = request
            .theorem_evidence
            .iter()
            .map(|evidence| {
                let selected_application =
                    theorem::derive_selected_theorem_telescope(program, &evidence.application)?;
                let termination = unconditional_termination_fence(
                    termination,
                    selected_application.machine_symbol,
                    selected_application.state_symbol,
                );
                let purity = pure_effect_fence(
                    program,
                    selected_application.machine_symbol,
                    selected_application.state_symbol,
                    selected_application
                        .parameters
                        .iter()
                        .any(|parameter| parameter.is_mutable),
                    &operational,
                    &service_reaches,
                );
                let crash_free = crate::machine_calls::denotational_calls::has_no_crash_routes(
                    program,
                    selected_application.machine_symbol,
                    &operational,
                );
                Ok(PlannedQuotientTheoremEvidence {
                    role: evidence.role,
                    selected_application,
                    termination,
                    purity,
                    crash_free,
                })
            })
            .collect::<Result<Vec<_>, RelationPlanError>>()?;
        require_checked_proof_closure(
            program,
            representative.machine_symbol,
            RelationPlanError::RepresentativeClosureIsNotChecked,
        )?;
        for evidence in &theorem_evidence {
            require_checked_proof_closure(
                program,
                evidence.selected_application.machine_symbol,
                RelationPlanError::SelectedTheoremClosureIsNotChecked,
            )?;
        }
        let has_explicit_transport = theorem_evidence.len() == 2;
        let selected_theorem = &theorem_evidence[0].selected_application;
        let expected_theorem_schema = derive_expected_theorem_schema(
            program,
            &input_relations,
            result_relation,
            &representative,
        )?;
        let theorem_schema_verification = verify_selected_theorem_schema(
            program,
            &representative,
            selected_theorem,
            &expected_theorem_schema,
        );
        let direct_lift_correspondence = (request.kind == QuotientOperationKind::Lift)
            .then(|| {
                derive_direct_lift_runtime_correspondence(
                    program,
                    machine,
                    state,
                    call,
                    &input_relations,
                    result_relation,
                    &representative,
                )
            })
            .transpose()?;
        let define_correspondence = (request.kind == QuotientOperationKind::Define)
            .then(|| {
                derive_define_runtime_correspondence(
                    program,
                    machine,
                    state,
                    call,
                    &input_relations,
                    result_relation,
                    &representative,
                )
            })
            .transpose()?;
        #[allow(clippy::type_complexity)]
        fn transport_gated_derivation<T>(
            has_explicit_transport: bool,
            runtime: Option<&DirectLiftRuntimeCorrespondence>,
            public: Option<&RepresentativePreconditionPartition>,
            representative_partition: Option<&RepresentativePreconditionPartition>,
            verified_theorem: Option<&VerifiedTheoremSchema>,
            derive: impl FnOnce(
                &DirectLiftRuntimeCorrespondence,
                &RepresentativePreconditionPartition,
                &RepresentativePreconditionPartition,
                &VerifiedTheoremSchema,
            ) -> Result<T, RelationPlanError>,
        ) -> Result<Option<T>, RelationPlanError> {
            Ok(
                match (
                    has_explicit_transport,
                    runtime,
                    public,
                    representative_partition,
                    verified_theorem,
                ) {
                    (
                        false,
                        Some(runtime),
                        Some(public),
                        Some(representative_partition),
                        Some(verified_theorem),
                    ) => Some(derive(
                        runtime,
                        public,
                        representative_partition,
                        verified_theorem,
                    )?),
                    _ => None,
                },
            )
        }

        let has_runtime_correspondence =
            direct_lift_correspondence.is_some() || define_correspondence.is_some();
        let representative_precondition = has_runtime_correspondence
            .then(|| {
                derive_representative_precondition_partition(
                    program,
                    &input_relations,
                    &representative,
                )
            })
            .transpose()?;
        let public_precondition = match (
            direct_lift_correspondence.as_ref(),
            define_correspondence.as_ref(),
        ) {
            (Some(runtime), None) => Some(derive_direct_lift_public_precondition_partition(
                program,
                machine,
                state,
                &input_relations,
                runtime,
            )?),
            (None, Some(runtime)) => Some(derive_public_precondition_partition(
                program,
                machine,
                state,
                &input_relations,
                &runtime.positions,
            )?),
            _ => None,
        };
        let define_precondition_correspondence = match (
            define_correspondence.as_ref(),
            public_precondition.as_ref(),
            representative_precondition.as_ref(),
        ) {
            (Some(runtime), Some(public), Some(representative_partition)) => {
                Some(derive_define_precondition_correspondence(
                    program,
                    machine,
                    state,
                    &representative,
                    public,
                    representative_partition,
                    runtime,
                )?)
            }
            _ => None,
        };
        let direct_lift_precondition_implication = transport_gated_derivation(
            has_explicit_transport,
            direct_lift_correspondence.as_ref(),
            public_precondition.as_ref(),
            representative_precondition.as_ref(),
            theorem_schema_verification.as_ref().ok(),
            |runtime, public, representative_partition, verified_theorem| {
                derive_direct_lift_precondition_implication(
                    program,
                    machine,
                    state,
                    &representative,
                    public,
                    representative_partition,
                    runtime,
                    &expected_theorem_schema,
                    verified_theorem,
                )
            },
        )?;
        let fixed_representative_call_preconditions = transport_gated_derivation(
            has_explicit_transport,
            direct_lift_correspondence.as_ref(),
            public_precondition.as_ref(),
            representative_precondition.as_ref(),
            theorem_schema_verification.as_ref().ok(),
            |runtime, public, representative_partition, verified_theorem| {
                derive_fixed_representative_call_preconditions(
                    program,
                    machine,
                    state,
                    &representative,
                    public,
                    representative_partition,
                    runtime,
                    &expected_theorem_schema,
                    verified_theorem,
                )
            },
        )?;
        let transport_schema_verification = match (
            has_explicit_transport,
            direct_lift_correspondence.as_ref(),
            public_precondition.as_ref(),
            representative_precondition.as_ref(),
        ) {
            (true, Some(runtime), Some(public), Some(representative_partition)) => {
                let transport = &theorem_evidence[1].selected_application;
                Some(verify_forward_precondition_transport_schema(
                    program,
                    machine,
                    state,
                    &representative,
                    transport,
                    runtime,
                    &expected_theorem_schema,
                    public,
                    representative_partition,
                ))
            }
            _ => None,
        };
        let correspondence_certificate = match request.kind {
            QuotientOperationKind::Lift if has_explicit_transport => direct_lift_correspondence
                .as_ref()
                .zip(transport_schema_verification.as_ref())
                .and_then(|(runtime, transport)| {
                    let selected_transport = &theorem_evidence[1];
                    compose_lift_transport_correspondence_certificate(
                        &theorem_schema_verification,
                        &theorem_evidence[0],
                        transport,
                        selected_transport,
                        runtime,
                    )
                }),
            QuotientOperationKind::Lift => direct_lift_correspondence
                .as_ref()
                .zip(direct_lift_precondition_implication.as_ref())
                .zip(fixed_representative_call_preconditions.as_ref())
                .zip(representative_precondition.as_ref())
                .and_then(
                    |(((runtime, precondition), fixed), representative_partition)| {
                        compose_lift_correspondence_certificate(
                            &theorem_schema_verification,
                            runtime,
                            precondition,
                            fixed,
                            representative_partition,
                        )
                    },
                ),
            QuotientOperationKind::Define => define_correspondence
                .as_ref()
                .zip(define_precondition_correspondence.as_ref())
                .zip(representative_precondition.as_ref())
                .and_then(|((runtime, precondition), representative_partition)| {
                    compose_define_correspondence_certificate(
                        &theorem_schema_verification,
                        runtime,
                        precondition,
                        representative_partition,
                    )
                }),
        };
        Ok(DirectTerminalRelationPlan {
            input_relations,
            result_relation,
            representative,
            representative_termination,
            representative_purity,
            theorem_evidence,
            expected_theorem_schema,
            theorem_schema_verification,
            transport_schema_verification,
            direct_lift_correspondence,
            define_correspondence,
            public_precondition,
            representative_precondition,
            direct_lift_precondition_implication,
            fixed_representative_call_preconditions,
            define_precondition_correspondence,
            correspondence_certificate,
        })
    }

    fn unconditional_termination_fence(
        termination: &dyn super::CheckedTerminationOracle,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<MachineStateCoordinate> {
        termination
            .unconditionally_terminates(machine_symbol)
            .then_some(MachineStateCoordinate {
                machine_symbol,
                state_symbol,
            })
    }

    fn pure_effect_fence(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        has_mutable_parameter: bool,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
    ) -> Option<MachineStateCoordinate> {
        crate::machine_calls::denotational_calls::has_pure_effect_closure(
            program,
            machine_symbol,
            state_symbol,
            has_mutable_parameter,
            operational,
            service_reaches,
        )
        .then_some(MachineStateCoordinate {
            machine_symbol,
            state_symbol,
        })
    }

    fn require_checked_proof_closure(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
        error: RelationPlanError,
    ) -> Result<(), RelationPlanError> {
        let Some(machine) = program.machine(machine_symbol) else {
            return Err(error);
        };
        super::equivalence_selection::checked_proof_dependency(
            program,
            machine,
            &mut std::collections::HashSet::new(),
        )
        .map_err(|_| error)
    }

    fn derive_relation_and_representative(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        call: &TableCallExpression,
        request: &QuotientOperationRequest,
    ) -> Result<
        (
            Vec<PositionalRelation>,
            PositionalRelation,
            RepresentativeTelescope,
        ),
        RelationPlanError,
    > {
        let mut representative = None;
        let mut input_relations = Vec::new();
        for (position, argument) in expression_table(program)
            .expression_handles(call.arguments)
            .iter()
            .enumerate()
        {
            let argument_type = crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                *argument,
            );
            let Some(argument_type) = argument_type else {
                let is_closed_literal_candidate = matches!(
                    expression_table(program).expression(*argument),
                    typed_trees::expression::ExpressionNode::Boolean(_)
                        | typed_trees::expression::ExpressionNode::Integer(_)
                        | typed_trees::expression::ExpressionNode::Float(_)
                        | typed_trees::expression::ExpressionNode::String(_)
                        | typed_trees::expression::ExpressionNode::ArrayLiteral(_)
                );
                if request.kind == QuotientOperationKind::Lift && is_closed_literal_candidate {
                    if representative.is_none() {
                        representative = Some(derive_representative_telescope(program, request)?);
                    }
                    if let Some(parameter) = representative
                        .as_ref()
                        .and_then(|representative| representative.parameters.get(position))
                        && closed_lift_literal_for_representative(
                            program,
                            *argument,
                            parameter.type_reference,
                            position,
                        )?
                        .is_some()
                    {
                        input_relations
                            .push(PositionalRelation::ExactEquality(parameter.type_reference));
                        continue;
                    }
                }
                return Err(RelationPlanError::UnresolvedArgumentType(position));
            };
            input_relations.push(match exact_quotient_relation(program, argument_type) {
                ExactRelationLookup::NotQuotient => {
                    PositionalRelation::ExactEquality(argument_type)
                }
                ExactRelationLookup::Exact(relation) => PositionalRelation::Quotient(relation),
                ExactRelationLookup::OpenApplication => {
                    return Err(RelationPlanError::UnresolvedPositionalRelationApplication(
                        position,
                    ));
                }
            });
        }
        let result_relation = match exact_quotient_relation(program, state.return_type) {
            ExactRelationLookup::NotQuotient => {
                PositionalRelation::ExactEquality(state.return_type)
            }
            ExactRelationLookup::Exact(relation) => PositionalRelation::Quotient(relation),
            ExactRelationLookup::OpenApplication => {
                return Err(RelationPlanError::UnresolvedResultRelationApplication);
            }
        };
        let representative = match representative {
            Some(representative) => representative,
            None => derive_representative_telescope(program, request)?,
        };
        Ok((input_relations, result_relation, representative))
    }

    pub(crate) fn render_failed_builtin_implication(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        call: &TableCallExpression,
        request: &QuotientOperationRequest,
        error: RelationPlanError,
    ) -> Option<String> {
        enum FailureKind {
            Dependent {
                application: theorem_schema::TheoremApplicationSide,
                position: usize,
            },
            Fixed {
                position: usize,
            },
        }

        let failure = match error {
            RelationPlanError::DirectLiftLeftPreconditionNotImplied(position) => {
                FailureKind::Dependent {
                    application: theorem_schema::TheoremApplicationSide::Left,
                    position,
                }
            }
            RelationPlanError::DirectLiftRightPreconditionNotImplied(position) => {
                FailureKind::Dependent {
                    application: theorem_schema::TheoremApplicationSide::Right,
                    position,
                }
            }
            RelationPlanError::DirectLiftFixedPreconditionNotImplied(position) => {
                FailureKind::Fixed { position }
            }
            _ => return None,
        };
        if request.kind != QuotientOperationKind::Lift || request.theorem_evidence.len() != 1 {
            return None;
        }

        let (input_relations, result_relation, representative) =
            derive_relation_and_representative(program, machine, state, call, request).ok()?;
        let runtime = derive_direct_lift_runtime_correspondence(
            program,
            machine,
            state,
            call,
            &input_relations,
            result_relation,
            &representative,
        )
        .ok()?;
        let public = derive_direct_lift_public_precondition_partition(
            program,
            machine,
            state,
            &input_relations,
            &runtime,
        )
        .ok()?;
        let representative_partition = derive_representative_precondition_partition(
            program,
            &input_relations,
            &representative,
        )
        .ok()?;

        let (application, public_coordinates, representative_coordinate) = match failure {
            FailureKind::Dependent {
                application,
                position,
            } => (
                match application {
                    theorem_schema::TheoremApplicationSide::Left => {
                        "left representative application"
                    }
                    theorem_schema::TheoremApplicationSide::Right => {
                        "right representative application"
                    }
                },
                public.dependent.as_slice(),
                *representative_partition.dependent.get(position)?,
            ),
            FailureKind::Fixed { position } => (
                "runtime representative application",
                public.fixed.as_slice(),
                *representative_partition.fixed.get(position)?,
            ),
        };

        Some(render_implication_coordinate_diagnostic(
            application,
            public_coordinates,
            representative_coordinate,
        ))
    }

    fn render_implication_coordinate_diagnostic(
        application: &str,
        public_coordinates: &[ContractFactLocation],
        representative_coordinate: ContractFactLocation,
    ) -> String {
        let public_coordinates = public_coordinates
            .iter()
            .map(|coordinate| render_contract_coordinate("public-Q", *coordinate))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "built-in public Q => representative P implication failed for the {application}: expected public fact coordinates [{public_coordinates}] to imply representative fact coordinate {}; use `Quotient::lift<F, Congruence, Transport>(...)` and select one exact checked forward-precondition transport theorem",
            render_contract_coordinate("representative-P", representative_coordinate),
        )
    }

    fn render_contract_coordinate(scope: &str, coordinate: ContractFactLocation) -> String {
        let owner = match coordinate.owner {
            ContractOwner::Machine => "machine-contract",
            ContractOwner::State => "state-contract",
        };
        format!(
            "{scope}.{owner}[{}].fact[{}]",
            coordinate.contract_position, coordinate.fact_position,
        )
    }

    fn validate_theorem_role_collection(
        request: &QuotientOperationRequest,
    ) -> Result<(), RelationPlanError> {
        let expected_roles: &[QuotientTheoremRole] = match request.kind {
            QuotientOperationKind::Define => &[QuotientTheoremRole::Congruence],
            QuotientOperationKind::Lift if request.theorem_evidence.len() == 1 => {
                &[QuotientTheoremRole::Congruence]
            }
            QuotientOperationKind::Lift if request.theorem_evidence.len() == 2 => &[
                QuotientTheoremRole::Congruence,
                QuotientTheoremRole::ForwardPreconditionTransport,
            ],
            QuotientOperationKind::Lift => {
                return Err(RelationPlanError::NonCanonicalTheoremRoleCollection);
            }
        };
        if request
            .theorem_evidence
            .iter()
            .map(|evidence| evidence.role)
            .ne(expected_roles.iter().copied())
        {
            return Err(RelationPlanError::NonCanonicalTheoremRoleCollection);
        }
        Ok(())
    }

    pub(crate) enum ExactRelationLookup {
        NotQuotient,
        Exact(ExactQuotientRelation),
        OpenApplication,
    }

    fn exact_quotient_relation(
        program: &dyn crate::CheckingInput,
        quotient_type: TypeReferenceHandle,
    ) -> ExactRelationLookup {
        let Some(quotient) =
            crate::proof_contracts::quotients::equivalence_selection::quotient_for_type(
                program,
                quotient_type,
            )
        else {
            return ExactRelationLookup::NotQuotient;
        };
        let Some(metadata) = quotient.quotient.as_ref() else {
            return ExactRelationLookup::NotQuotient;
        };
        let Some(relation) = program.proposition_definition_by_symbol(metadata.relation_symbol)
        else {
            return ExactRelationLookup::OpenApplication;
        };
        if !program.proposition_binders(relation).is_empty() {
            return ExactRelationLookup::OpenApplication;
        }
        ExactRelationLookup::Exact(ExactQuotientRelation {
            quotient_type,
            quotient_symbol: quotient.symbol,
            relation_symbol: metadata.relation_symbol,
        })
    }

    impl DirectTerminalRelationPlan {
        pub(crate) fn render_ra(&self, program: &dyn crate::CheckingInput) -> String {
            let positions = self
                .input_relations
                .iter()
                .enumerate()
                .map(|(position, relation)| {
                    let relation = match relation {
                        PositionalRelation::Quotient(relation) => {
                            relation_name(program, relation.relation_symbol)
                        }
                        PositionalRelation::ExactEquality(type_reference) => format!(
                            "==<{}>",
                            program.display_type_reference_with_constraints(*type_reference)
                        ),
                    };
                    format!("{position}:{relation}")
                })
                .collect::<Vec<_>>();
            format!("RA=[{}]", positions.join(", "))
        }

        pub(crate) fn render_rr(&self, program: &dyn crate::CheckingInput) -> String {
            format!(
                "RR={}",
                match self.result_relation {
                    PositionalRelation::Quotient(relation) =>
                        relation_name(program, relation.relation_symbol),
                    PositionalRelation::ExactEquality(reference) => format!(
                        "==<{}>",
                        program.display_type_reference_with_constraints(reference)
                    ),
                }
            )
        }

        pub(crate) fn render_representative_telescope(
            &self,
            program: &dyn crate::CheckingInput,
        ) -> String {
            let parameters = self
                .representative
                .parameters
                .iter()
                .map(|parameter| {
                    program.display_type_reference_with_constraints(parameter.type_reference)
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{}({parameters})->{}",
                machine_state_label(
                    program,
                    self.representative.machine_symbol,
                    self.representative.state_symbol,
                ),
                program.display_type_reference_with_constraints(self.representative.return_type),
            )
        }

        pub(crate) fn render_representative_termination(
            &self,
            program: &dyn crate::CheckingInput,
        ) -> Option<String> {
            self.representative_termination.map(|termination| {
                format!(
                    "unconditional termination of {}",
                    machine_state_label(
                        program,
                        termination.machine_symbol,
                        termination.state_symbol
                    ),
                )
            })
        }

        pub(crate) fn render_selected_theorem(&self, program: &dyn crate::CheckingInput) -> String {
            let selected_theorem = &self.theorem_evidence[0].selected_application;
            let parameters = self.theorem_evidence[0]
                .selected_application
                .parameters
                .iter()
                .map(|parameter| {
                    program.display_type_reference_with_constraints(parameter.type_reference)
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "theorem {}({parameters})[static-bindings:{}]",
                machine_state_label(
                    program,
                    selected_theorem.machine_symbol,
                    selected_theorem.state_symbol,
                ),
                selected_theorem.static_application.bindings.len(),
            )
        }

        pub(crate) fn render_selected_transport(
            &self,
            program: &dyn crate::CheckingInput,
        ) -> Option<String> {
            let selected = self.theorem_evidence.get(1)?;
            let parameters = selected
                .selected_application
                .parameters
                .iter()
                .map(|parameter| {
                    program.display_type_reference_with_constraints(parameter.type_reference)
                })
                .collect::<Vec<_>>()
                .join(", ");
            Some(format!(
                "forward transport {}({parameters})[static-bindings:{}]",
                machine_state_label(
                    program,
                    selected.selected_application.machine_symbol,
                    selected.selected_application.state_symbol,
                ),
                selected
                    .selected_application
                    .static_application
                    .bindings
                    .len(),
            ))
        }

        pub(crate) fn render_transport_schema_verification(&self) -> Option<String> {
            match self.transport_schema_verification.as_ref()? {
                Ok(verified) => Some(format!(
                    "transport-schema=[parameters:{}, public-Q:{}, representative-P:{}]",
                    verified.parameters.len(),
                    verified.public_premises.len(),
                    verified.representative_conclusions.len(),
                )),
                Err(reason) => Some(format!("transport-schema verification failed: {reason}")),
            }
        }

        pub(crate) fn render_expected_theorem_schema(&self) -> String {
            format!(
                "theorem-schema=[parameters:{}, relations:{}, legality:{}, applications:2, conclusion:1]",
                self.expected_theorem_schema.parameters.len(),
                self.expected_theorem_schema.relation_premises.len(),
                self.expected_theorem_schema.legality_premises.len(),
            )
        }

        fn render_runtime_correspondence<T>(kind: &str, positions: &[T]) -> String {
            format!(
                "{kind}-runtime=[{}]",
                positions
                    .iter()
                    .enumerate()
                    .map(|(position, _)| position.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }

        pub(crate) fn render_define_correspondence(&self) -> Option<String> {
            self.define_correspondence.as_ref().map(|correspondence| {
                Self::render_runtime_correspondence("define", &correspondence.positions)
            })
        }

        pub(crate) fn render_direct_lift_correspondence(&self) -> Option<String> {
            self.direct_lift_correspondence
                .as_ref()
                .map(|correspondence| {
                    Self::render_runtime_correspondence("direct-lift", &correspondence.positions)
                })
        }

        pub(crate) fn render_representative_precondition(&self) -> Option<String> {
            self.representative_precondition.as_ref().map(|partition| {
                format!(
                    "P=[dependent:{}, fixed:{}]",
                    partition.dependent.len(),
                    partition.fixed.len()
                )
            })
        }

        pub(crate) fn has_undischarged_fixed_representative_preconditions(&self) -> bool {
            let Some(partition) = self.representative_precondition.as_ref() else {
                return false;
            };
            if self.direct_lift_correspondence.is_some() {
                if matches!(
                self.correspondence_certificate
                    .as_ref()
                    .map(|certificate| &certificate.evidence),
                Some(
                    correspondence_certificate::QuotientCorrespondenceEvidence::DirectLiftWithTransport {
                        ..
                    }
                )
            ) {
                return false;
            }
                return self
                    .fixed_representative_call_preconditions
                    .as_ref()
                    .is_none_or(|proof| proof.rows.len() != partition.fixed.len());
            }
            if self.define_correspondence.is_some() {
                return self
                    .define_precondition_correspondence
                    .as_ref()
                    .is_none_or(|proof| proof.fixed.len() != partition.fixed.len());
            }
            !partition.fixed.is_empty()
        }

        pub(crate) fn render_public_precondition(&self) -> Option<String> {
            self.public_precondition.as_ref().map(|partition| {
                format!(
                    "Q=[dependent:{}, fixed:{}]",
                    partition.dependent.len(),
                    partition.fixed.len()
                )
            })
        }

        pub(crate) fn render_define_precondition_correspondence(&self) -> Option<String> {
            self.define_precondition_correspondence
                .as_ref()
                .map(|correspondence| {
                    format!(
                        "Q<->P=[dependent:{}, fixed:{}]",
                        correspondence.dependent.len(),
                        correspondence.fixed.len()
                    )
                })
        }

        pub(crate) fn render_direct_lift_precondition_implication(&self) -> Option<String> {
            self.direct_lift_precondition_implication
            .as_ref()
            .map(|implication| {
                let left = implication
                    .rows
                    .iter()
                    .filter(|row| row.application == theorem_schema::TheoremApplicationSide::Left)
                    .count();
                let right = implication.rows.len() - left;
                let arithmetic = implication
                    .rows
                    .iter()
                    .filter(|row| {
                        matches!(
                            row.proof,
                            correspondence_certificate::DirectLiftPreconditionProof::ArithmeticEntailment { .. }
                        )
                    })
                    .count();
                format!("Q=>P=[left:{left}, right:{right}, arithmetic:{arithmetic}]")
            })
        }

        pub(crate) fn render_fixed_representative_call_preconditions(&self) -> Option<String> {
            self.fixed_representative_call_preconditions
            .as_ref()
            .map(|preconditions| {
                let arithmetic = preconditions
                    .rows
                    .iter()
                    .filter(|row| {
                        matches!(
                            row.proof,
                            correspondence_certificate::FixedRepresentativeCallProof::ArithmeticEntailment { .. }
                        )
                    })
                    .count();
                format!(
                    "fixed-call-P=[rows:{}, arithmetic:{arithmetic}]",
                    preconditions.rows.len()
                )
            })
        }

        pub(crate) fn render_correspondence_certificate(
            &self,
            program: &dyn crate::CheckingInput,
        ) -> Option<String> {
            self.correspondence_certificate.as_ref().map(|certificate| {
            let kind = match &certificate.evidence {
                correspondence_certificate::QuotientCorrespondenceEvidence::DirectLift {
                    ..
                } => "direct-lift",
                correspondence_certificate::QuotientCorrespondenceEvidence::DirectLiftWithTransport {
                    ..
                } => "direct-lift-with-transport",
                correspondence_certificate::QuotientCorrespondenceEvidence::Define { .. } => {
                    "define"
                }
            };
            format!(
                "{kind}-certificate=[theorem {}]",
                machine_state_label(
                    program,
                    certificate.theorem.theorem_machine_symbol,
                    certificate.theorem.theorem_state_symbol,
                ),
            )
        })
        }
    }

    fn machine_state_label(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> String {
        let Some(machine) = program.machine(machine_symbol) else {
            return "an unresolved machine".to_owned();
        };
        let states = program.machine_states(machine);
        match program.state_position_by_symbol(machine_symbol, state_symbol) {
            Some(position) if position > 0 => {
                format!(
                    "`{}` state `{}`",
                    machine.name,
                    states[position].name.as_str()
                )
            }
            _ => format!("`{}`", machine.name),
        }
    }

    fn relation_name(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> String {
        program
            .proposition_definition_by_symbol(symbol)
            .map(|proposition| proposition.name.as_str().to_owned())
            .unwrap_or_else(|| format!("relation#{symbol:?}"))
    }
}
pub(crate) mod request_admission {
    use crate::frozen_program::expression_table;
    use language_semantics::quotient_correspondence::QuotientCorrespondenceJudgment;
    use language_semantics::quotient_correspondence::QuotientResultFlow;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    pub(crate) fn extract(
        program: &dyn crate::CheckingInput,
        termination: &dyn super::CheckedTerminationOracle,
    ) -> Result<Vec<super::ValidatedQuotientOperation>, Vec<String>> {
        let requests = expression_table(program)
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.quotient_operation.is_some())
                .then_some(handle)
        })
        .collect::<Vec<_>>();
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let mut rows = Vec::with_capacity(requests.len());
        let mut errors = Vec::new();
        for request_expression in requests {
            match extract_one(program, termination, request_expression) {
                Ok(row) => rows.push(row),
                Err(error) => errors.push(error),
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let statement_position = |row: &QuotientCorrespondenceJudgment| match &row.result_flow {
            QuotientResultFlow::Direct {
                statement_position, ..
            }
            | QuotientResultFlow::Forwarded {
                statement_position, ..
            } => *statement_position,
        };
        rows.sort_by(|left, right| {
            (
                &left.correspondence.public_operation.declaration,
                &left.correspondence.public_operation.overload,
                statement_position(&left.correspondence),
            )
                .cmp(&(
                    &right.correspondence.public_operation.declaration,
                    &right.correspondence.public_operation.overload,
                    statement_position(&right.correspondence),
                ))
        });
        if rows.windows(2).any(|rows| rows[0] == rows[1]) {
            return Err(vec![
                "duplicate canonical quotient correspondence identity".to_owned(),
            ]);
        }
        Ok(rows)
    }

    fn extract_one(
        program: &dyn crate::CheckingInput,
        termination: &dyn super::CheckedTerminationOracle,
        request_expression: ExpressionHandle,
    ) -> Result<super::ValidatedQuotientOperation, String> {
        let owners = program
            .machines()
            .iter()
            .flat_map(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .filter_map(move |state| {
                        let root = super::relation_plan::fallthrough_result_root(program, state)?;
                        (root.request_expression == request_expression)
                            .then_some((machine, state, root))
                    })
            })
            .collect::<Vec<_>>();
        let [(machine, state, root)] = owners.as_slice() else {
            return Err(
                "a request must be the unique exact terminal result of one owner state".to_owned(),
            );
        };
        let Some(call) = expression_table(program).call_node(request_expression) else {
            return Err("the retained request is not a call".to_owned());
        };
        let request = call
            .quotient_operation
            .as_ref()
            .ok_or_else(|| "the retained call lost its quotient request".to_owned())?;
        let plan = super::relation_plan::derive_direct_terminal_plan(
            program,
            termination,
            machine,
            state,
            call,
            request,
        )
        .map_err(|error| format!("direct faithful plan is unresolved: {error}"))?;
        let representative_purity = plan
            .representative_purity
            .ok_or_else(|| "representative closure is not exactly pure".to_owned())?;
        let result_flow = super::relation_plan::complete_result_flow(
            program, machine, state, *root,
        )
        .ok_or_else(|| {
            "result flow is not complete direct single-state or state-forwarded flow".to_owned()
        })?;

        let correspondence = super::relation_plan::normalized_direct_correspondence(
            program,
            machine,
            state,
            request_expression,
            &plan,
            representative_purity,
            result_flow,
        )?;
        Ok(super::ValidatedQuotientOperation {
            request: request_expression,
            public_machine: machine.symbol,
            public_state: state.symbol,
            representative_state: request.representative_operation.symbol,
            correspondence,
        })
    }
}

pub(crate) use equivalence_selection::exact_relation_application_matches;

use crate::frozen_program::expression_table;
use diagnostics::Diagnostic;
use language_semantics::TerminationGuarantee;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::proof_only::ProofOnlyClassification;
use typed_trees::types::TypeReferenceHandle;

use crate::proof_contracts::quotients::formation_collection::{
    collect_validated_quotient_formations, reject_quotient_operation_requests,
};

const CORE_EQUIVALENCE_SOURCE: &str = "relation.omg";

pub trait CheckedTerminationOracle {
    fn checked_termination(&self, machine: SymbolHandle) -> Option<TerminationGuarantee>;

    fn unconditionally_terminates(&self, machine: SymbolHandle) -> bool {
        matches!(
            self.checked_termination(machine),
            Some(TerminationGuarantee::Terminates { premises })
                if premises.is_empty()
        )
    }
}

impl CheckedTerminationOracle for TypedTrees {
    fn checked_termination(&self, machine: SymbolHandle) -> Option<TerminationGuarantee> {
        self.machine(machine)
            .map(|candidate| candidate.termination_plan.checked_summary.clone())
    }

    fn unconditionally_terminates(&self, machine: SymbolHandle) -> bool {
        crate::machine_calls::denotational_calls::unconditionally_terminates(self, machine)
    }
}

impl<Lookup> CheckedTerminationOracle for Lookup
where
    Lookup: Fn(SymbolHandle) -> Option<TerminationGuarantee>,
{
    fn checked_termination(&self, machine: SymbolHandle) -> Option<TerminationGuarantee> {
        self(machine)
    }
}

pub type ValidatedQuotientFormation = checked_trees::CheckedQuotientFormation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedQuotientOperation {
    pub request: typed_trees::expression::ExpressionHandle,
    pub public_machine: SymbolHandle,
    pub public_state: SymbolHandle,
    pub representative_state: SymbolHandle,
    pub correspondence: language_semantics::quotient_correspondence::QuotientCorrespondenceJudgment,
}

pub fn validate_quotient_formations(
    program: &dyn crate::CheckingInput,
) -> Result<Vec<ValidatedQuotientFormation>, Vec<Diagnostic>> {
    let proof_only = crate::proof_only_classification(program);
    let mut diagnostics = Vec::new();
    let formations = collect_validated_quotient_formations(program, &proof_only, &mut diagnostics);
    if diagnostics.is_empty() {
        Ok(formations)
    } else {
        Err(diagnostics)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotientRequestAdmission {
    AtValidation,
    AfterCheckedFacts,
}

pub(crate) fn validate_quotients(
    program: &dyn crate::CheckingInput,
    proof_only: &ProofOnlyClassification,
    admission: QuotientRequestAdmission,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedQuotientFormation> {
    match admission {
        QuotientRequestAdmission::AtValidation => {
            reject_quotient_operation_requests(program, program.typed_program(), diagnostics);
        }
        QuotientRequestAdmission::AfterCheckedFacts => {}
    }

    collect_validated_quotient_formations(program, proof_only, diagnostics)
}

pub fn admit_checked_quotient_requests(
    program: &dyn crate::CheckingInput,
    termination: &dyn CheckedTerminationOracle,
) -> Result<Vec<ValidatedQuotientOperation>, Vec<Diagnostic>> {
    if !expression_table(program)
        .iter_expressions()
        .any(|(_, expression)| {
            matches!(
                expression,
                typed_trees::expression::ExpressionNode::Call(call)
                    if call.quotient_operation.is_some()
            )
        })
    {
        return Ok(Vec::new());
    }
    if let Ok(rows) = request_admission::extract(program, termination)
        && !rows.is_empty()
    {
        return Ok(rows);
    }
    let mut diagnostics = Vec::new();
    reject_quotient_operation_requests(program, termination, &mut diagnostics);
    if diagnostics.is_empty() {
        diagnostics.push(Diagnostic::error(
            "quotient operation requests were not admitted by the proof-only bridge".to_owned(),
        ));
    }
    Err(diagnostics)
}

pub fn checked_quotient_relation_identity(
    program: &dyn crate::CheckingInput,
    quotient_type: TypeReferenceHandle,
    formations: &[ValidatedQuotientFormation],
) -> Result<language_semantics::quotient_correspondence::QuotientRelationIdentity, String> {
    let symbol = equivalence_selection::base_data_symbol(program, quotient_type)
        .ok_or_else(|| "quotient application has no nominal declaration".to_owned())?;
    let formation = formations
        .iter()
        .find(|row| row.data_symbol == symbol)
        .ok_or_else(|| "quotient application has no checked formation".to_owned())?;
    relation_plan::relation_identity(
        program,
        relation_plan::ExactQuotientRelation {
            quotient_symbol: symbol,
            quotient_type,
            relation_symbol: formation.relation_symbol,
        },
    )
}
