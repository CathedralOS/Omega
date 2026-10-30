//! Outcome-specific arm facts and the exact outcome case tests.

use crate::proof::proposition_vocabulary::{
    lower_checked_proposition_application, proposition_application_label, proposition_label_parts,
};
use checked_trees::{CheckedEvidenceTerm, ContractProofFactOwner, ProofFacts};
use symbols::SymbolHandle;
use typed_trees::proposition::{ProofSubstitutions, PropositionLabels};

/// Bind outcome-specific producer guarantees to the one transition arm that
/// tests the saved result of a direct call, including immutable binding copies.
/// Mutable bindings and non-binding value transformations remain fail-closed.
pub(crate) fn bind_outcome_specific_arm_facts(
    program: &dyn ::validation::CheckingInput,
    proof: &mut ProofFacts,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    use typed_trees::statement::TransitionGuardNode;

    let mut diagnostics = Vec::new();
    let mut arms = arena::Arena::default();

    for caller_machine in program.machines() {
        for caller_state in program.machine_states(caller_machine) {
            let statements = program
                .statement_table
                .statements(caller_state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                let selectors = program
                    .statement_table
                    .outcome_proof_selectors(transition.proof_selectors);
                let TransitionGuardNode::When(guard) = transition.guard else {
                    if !selectors.is_empty() {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "outcome proof selectors require an exact guarded result-case arm",
                        ));
                    }
                    continue;
                };
                let Some((result_expression, result_case)) =
                    exact_outcome_case_test(program, guard)
                else {
                    if !selectors.is_empty() {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "outcome proof selectors require one exact nominal result-case test",
                        ));
                    }
                    continue;
                };
                let Some((result_call_statement_index, result_call_expression, result_call)) =
                    saved_result_call(program, caller_state, statement_index, result_expression)
                else {
                    if !selectors.is_empty() {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "outcome proof selectors require one immutable saved direct-call result",
                        ));
                    }
                    continue;
                };
                let Some((target_machine, target_state)) =
                    validation::find_state(program, result_call.target_symbol)
                else {
                    if !selectors.is_empty() {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "outcome proof selector source call `{}` must target a concrete machine state",
                            result_call.target
                        )));
                    }
                    continue;
                };
                let Some(result_data) = program
                    .variant_owner(result_case)
                    .map(|(definition, _)| definition.symbol)
                else {
                    if !selectors.is_empty() {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "outcome proof selector case does not resolve to a declared nominal sum case",
                        ));
                    }
                    continue;
                };

                let matching_rows = proof
                    .outcome_specific_guarantees
                    .iter()
                    .filter_map(|(handle, row)| {
                        (row.machine_symbol == target_machine.symbol
                            && row.result_data == result_data
                            && row.result_case == result_case)
                            .then_some((handle, row.clone()))
                    })
                    .collect::<Vec<_>>();
                if matching_rows.is_empty() && selectors.is_empty() {
                    continue;
                }
                let mut selected_names = std::collections::BTreeSet::new();
                let mut invalid = false;
                for selector in selectors {
                    if !selected_names.insert(selector.binding.as_str()) {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "outcome evidence term `{}` is bound more than once in this arm",
                            selector.binding
                        )));
                        invalid = true;
                    }
                    if !matching_rows.iter().any(|(_, row)| {
                        row.public_selector.as_deref() == Some(selector.output_field.as_str())
                    }) {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "outcome proof selector `{}` is not a named guarantee of the matching result case",
                            selector.output_field
                        )));
                        invalid = true;
                    }
                }
                if invalid {
                    continue;
                }

                let substitutions = outcome_call_substitutions(
                    program,
                    caller_state.symbol,
                    result_expression,
                    result_call,
                    target_state,
                );
                let mut arm_rows = Vec::with_capacity(matching_rows.len());
                for (guarantee, row) in matching_rows {
                    let (
                        instantiated_proposition,
                        instantiated_label,
                        captured_argument_identities,
                    ) = instantiate_outcome_arm_fact(
                        program,
                        row.fact,
                        &substitutions,
                        caller_state.symbol,
                        result_call_statement_index,
                        validation::CallSite::Expression {
                            expression: result_call_expression,
                            call: result_call,
                        },
                        target_state,
                    );
                    let referenced_occurrences =
                        crate::facts::contract_occurrences::fact_referenced_occurrences(
                            program, row.fact,
                        )
                        .into_iter()
                        .map(|occurrence| checked_trees::OutcomeReferencedOccurrence {
                            occurrence,
                            root: occurrence_root(program, target_state, occurrence),
                        })
                        .collect::<Vec<_>>();
                    let argument_roots = row.argument_roots.clone();
                    let validity = checked_trees::OutcomeSpecificValidityFact {
                        result_occurrence: result_expression,
                        evidence_interface_scope: instantiated_proposition.as_ref().and_then(
                            |proposition| {
                                outcome_evidence_interface_scope(
                                    program,
                                    proposition,
                                    &referenced_occurrences,
                                )
                            },
                        ),
                        referenced_occurrences,
                    };
                    let selected = row.public_selector.as_deref().and_then(|public| {
                        selectors
                            .iter()
                            .find(|selector| selector.output_field.as_str() == public)
                    });
                    let selected_term = selected.and_then(|selector| {
                        let producer = row.evidence_term?;
                        let mut term = proof.evidence_terms.get(producer).clone();
                        let proposition = instantiated_proposition.clone()?;
                        term.name = selector.binding.as_str().to_owned();
                        term.owner = ContractProofFactOwner::MachineState {
                            machine_symbol: caller_machine.symbol,
                            state_symbol: caller_state.symbol,
                        };
                        term.lane_position = arm_rows.len();
                        term.proposition = proposition;
                        Some(proof.evidence_terms.append(term))
                    });
                    arm_rows.push(checked_trees::OutcomeSpecificArmRowFact {
                        guarantee,
                        instantiated_proposition,
                        instantiated_label,
                        captured_argument_identities,
                        argument_roots,
                        validity,
                        selected_term,
                    });
                }
                arms.append(checked_trees::OutcomeSpecificArmFact {
                    caller_machine_symbol: caller_machine.symbol,
                    caller_state_symbol: caller_state.symbol,
                    statement_index,
                    result_call_statement_index,
                    result_call_expression,
                    result_call_target: target_state.symbol,
                    result_data,
                    result_case,
                    result_expression,
                    rows: arm_rows,
                });
            }
        }
    }

    if diagnostics.is_empty() {
        proof.outcome_specific_arms = arms;
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Follow exact immutable binding copies back to the completed call. Dispatch
/// subject capture introduces the same kind of copy as an authored local; it
/// does not change the event that established the outcome. Each step must be
/// earlier in admitted execution order, so malformed cycles and later bindings
/// fail without replaying an initializer or admitting a mutable snapshot.
fn saved_result_call<'program>(
    program: &'program dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    mut expression: typed_trees::expression::ExpressionHandle,
) -> Option<(
    usize,
    typed_trees::expression::ExpressionHandle,
    &'program typed_trees::expression::TableCallExpression,
)> {
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let mut before = order.position(statement_index)?;
    loop {
        let path = program.expression_table.name_path_node(expression)?;
        if !path.symbol.is_valid() || path.head_symbol != path.symbol {
            return None;
        }
        let (ordinal, local) = program.state_local_declaration(state, path.symbol)?;
        let position = order.position(ordinal)?;
        if local.is_mutable || !local.initial_value.is_valid() || position >= before {
            return None;
        }
        expression = local.initial_value;
        if let Some(call) = program.expression_table.call_node(expression) {
            return Some((ordinal, expression, call));
        }
        before = position;
    }
}

fn outcome_evidence_interface_scope(
    program: &dyn ::validation::CheckingInput,
    proposition: &checked_trees::CheckedPropositionApplication,
    retained_occurrences: &[checked_trees::OutcomeReferencedOccurrence],
) -> Option<checked_trees::OutcomeSpecificEvidenceInterfaceScopeFact> {
    let interface = proposition.evidence_interface.clone()?;
    let definition = program.proposition_definition_by_symbol(proposition.declaration)?;
    let typed_trees::proposition::PropositionBody::Witness { evidence } = definition.body else {
        return None;
    };
    let mut reference_regions = Vec::new();
    append_evidence_interface_reference_regions(program, evidence, &mut reference_regions);
    reference_regions.sort_by_key(|reference| reference.arena_index());
    reference_regions.dedup();
    Some(checked_trees::OutcomeSpecificEvidenceInterfaceScopeFact {
        interface,
        evidence_type: evidence,
        reference_regions,
        retained_occurrences: retained_occurrences.to_vec(),
    })
}

fn append_evidence_interface_reference_regions(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    regions: &mut Vec<typed_trees::types::TypeReferenceHandle>,
) {
    if !type_reference.is_valid() {
        return;
    }
    use typed_trees::types::TypeReferenceNode;
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            regions.push(type_reference);
            append_evidence_interface_reference_regions(program, *referee, regions);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            append_evidence_interface_reference_regions(program, *base_type, regions);
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            append_evidence_interface_reference_regions(program, *element_type, regions);
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                append_evidence_interface_reference_regions(program, *argument, regions);
            }
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => {}
    }
}

pub(crate) fn exact_outcome_case_test(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(typed_trees::expression::ExpressionHandle, SymbolHandle)> {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    if binary.operator == BinaryOperator::CaseMembership {
        // Validation has checked the exact nominal subject and classifier.
        // Only the left operand is observed; the right operand is not a value
        // to copy, move, or reverse with the subject as ordinary equality can.
        let case = program.expression_table.name_path_node(binary.right)?;
        return Some((binary.left, case.symbol));
    }
    if binary.operator == BinaryOperator::Equal {
        let is_true = |candidate| {
            matches!(
                program.expression_table.expression(candidate),
                ExpressionNode::Boolean(true)
            )
        };
        if is_true(binary.left) {
            return exact_outcome_case_test(program, binary.right);
        }
        if is_true(binary.right) {
            return exact_outcome_case_test(program, binary.left);
        }
        let case = |candidate| {
            match program.expression_table.expression(candidate) {
            ExpressionNode::Name(path) if program.data_definitions().iter().any(|definition| {
                program.data_members(definition).iter().any(|member| {
                    matches!(member, typed_trees::data::DataMember::Variant(variant) if variant.symbol == path.symbol)
                })
            }) => Some(path.symbol),
            // A payload-free case spelled as a value (`subject == T::Case`) is
            // the same exact-outcome test: the construction resolves its case
            // symbol and owns no fields to compare.
            ExpressionNode::StructLiteral(literal)
                if literal.case_symbol.is_some()
                    && program
                        .expression_table
                        .struct_fields(literal.fields)
                        .is_empty() =>
            {
                literal.case_symbol
            }
            _ => None,
        }
        };
        if let Some(case) = case(binary.right) {
            return Some((binary.left, case));
        }
        if let Some(case) = case(binary.left) {
            return Some((binary.right, case));
        }
    }
    None
}

pub(super) use validation::contract_argument_root as occurrence_root;

fn outcome_call_substitutions(
    program: &dyn ::validation::CheckingInput,
    _caller_state_symbol: SymbolHandle,
    result_expression: typed_trees::expression::ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    target_state: &typed_trees::state::State,
) -> Vec<(SymbolHandle, String, String)> {
    let mut substitutions = Vec::new();
    let arguments = program.expression_table.expression_handles(call.arguments);
    let mut argument_index = 0usize;
    for parameter in program.state_parameters(target_state) {
        let value = {
            let value = arguments.get(argument_index).copied();
            argument_index += 1;
            value
        };
        if let Some(value) = value {
            substitutions.push((
                parameter.symbol,
                parameter.name.as_str().to_owned(),
                program.render_proof_expression(value, ProofSubstitutions::None),
            ));
        }
    }
    substitutions.push((
        SymbolHandle::invalid(),
        "result".to_owned(),
        program.render_proof_expression(result_expression, ProofSubstitutions::None),
    ));
    substitutions
}

fn instantiate_outcome_arm_fact(
    program: &dyn ::validation::CheckingInput,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
    substitutions: &[(SymbolHandle, String, String)],
    caller_state: SymbolHandle,
    statement_index: usize,
    call: validation::CallSite<'_>,
    target_state: &typed_trees::state::State,
) -> (
    Option<checked_trees::CheckedPropositionApplication>,
    Option<String>,
    Option<Vec<Option<String>>>,
) {
    let Some(application) = program.proof_facts.get(fact).as_proposition() else {
        let identity = match program.proof_facts.get(fact) {
            typed_trees::domain::ProofFact::Expression(expression) => {
                Some(program.render_proof_expression(
                    *expression,
                    ProofSubstitutions::ByParameter(substitutions),
                ))
            }
            typed_trees::domain::ProofFact::Membership(_) => None,
            typed_trees::domain::ProofFact::Proposition(_) => unreachable!(),
        };
        return (None, identity, Some(Vec::new()));
    };
    let parameters = program.state_parameters(target_state);
    let (binder_labels, argument_labels) =
        proposition_label_parts(program, application, substitutions);
    // Internal capture keys compare checked values. The public proposition
    // retains its canonical source vocabulary; Terminal joins typed formal
    // subjects with evaluated call operands, never source arena handles.
    let captured_argument_identities = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| {
            if validation::owned_contract_subject(program, parameters, *argument).is_some() {
                validation::captured_call_subject_identity(
                    program,
                    caller_state,
                    statement_index,
                    call,
                    parameters,
                    *argument,
                )
                .map(Some)
            } else if matches!(
                occurrence_root(program, target_state, *argument),
                checked_trees::OutcomeOccurrenceRoot::CalleeResult
                    | checked_trees::OutcomeOccurrenceRoot::ClosedValue
            ) {
                Some(None)
            } else {
                None
            }
        })
        .collect::<Option<Vec<_>>>();
    let proposition = program
        .normalize_nominal_proposition_application(
            application,
            Some(PropositionLabels {
                binder_labels: &binder_labels,
                argument_labels: &argument_labels,
            }),
        )
        .map(lower_checked_proposition_application);
    let identity = captured_argument_identities.as_ref().and_then(|captures| {
        let labels = argument_labels
            .iter()
            .zip(captures)
            .map(|(label, capture)| capture.clone().unwrap_or_else(|| label.clone()))
            .collect::<Vec<_>>();
        program.normalized_identity_label(application, &binder_labels, &labels)
    });
    (proposition, identity, captured_argument_identities)
}

pub(crate) fn exact_result_case(
    program: &dyn ::validation::CheckingInput,
    result: typed_trees::expression::ExpressionHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    use typed_trees::expression::ExpressionNode;
    let (data_symbol, case_symbol) = match program.expression_table.expression(result) {
        ExpressionNode::Name(path) if path.head_symbol.is_valid() && path.symbol.is_valid() => {
            (path.head_symbol, path.symbol)
        }
        ExpressionNode::StructLiteral(literal) => (literal.type_symbol, literal.case_symbol?),
        _ => return None,
    };
    let data = program.data_definition_by_symbol(data_symbol)?;
    program
        .data_members(data)
        .iter()
        .any(|member| {
            matches!(
                member,
                typed_trees::data::DataMember::Variant(variant)
                    if variant.symbol == case_symbol
            )
        })
        .then_some((data_symbol, case_symbol))
}

pub(crate) fn outcome_specific_fact_is_proved(
    program: &dyn ::validation::CheckingInput,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
    result: Option<typed_trees::expression::ExpressionHandle>,
    known: &std::collections::BTreeSet<String>,
) -> bool {
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::ExpressionNode;

    let Some(result) = result else {
        return false;
    };
    let result_label = program.render_proof_expression(result, ProofSubstitutions::None);
    let substitutions = [(SymbolHandle::invalid(), "result".to_owned(), result_label)];
    match program.proof_facts.get(fact) {
        ProofFact::Expression(expression) => {
            matches!(
                program.expression_table.expression(*expression),
                ExpressionNode::Boolean(true)
            ) || known.contains(&format!(
                "boolean:{}",
                program.render_proof_expression(
                    *expression,
                    ProofSubstitutions::ByParameter(&substitutions)
                )
            ))
        }
        ProofFact::Proposition(application) => {
            proposition_application_label(program, application, &substitutions)
                .is_some_and(|goal| known.contains(&goal))
        }
        ProofFact::Membership(_) => false,
    }
}

pub(crate) fn outcome_specific_assignment_matches_result(
    program: &dyn ::validation::CheckingInput,
    proof: &ProofFacts,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
    result: Option<typed_trees::expression::ExpressionHandle>,
    source: arena::Handle<CheckedEvidenceTerm>,
) -> bool {
    let Some(result) = result else {
        return false;
    };
    let Some(application) = program.proof_facts.get(fact).as_proposition() else {
        return false;
    };
    let result_label = program.render_proof_expression(result, ProofSubstitutions::None);
    let substitutions = [(SymbolHandle::invalid(), "result".to_owned(), result_label)];
    let (binder_labels, argument_labels) =
        proposition_label_parts(program, application, &substitutions);
    program
        .normalize_nominal_proposition_application(
            application,
            Some(PropositionLabels {
                binder_labels: &binder_labels,
                argument_labels: &argument_labels,
            }),
        )
        .map(lower_checked_proposition_application)
        .is_some_and(|expected| proof.evidence_terms.get(source).proposition == expected)
}
