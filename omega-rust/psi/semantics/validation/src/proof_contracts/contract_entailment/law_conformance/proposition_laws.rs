//! Proposition law conformance and indexed law binder labels.

use crate::proof_contracts::contract_entailment::{
    BinaryOperator, Diagnostic, ExpressionHandle, ExpressionNode, Machine, StateSignature,
    StructuralTerm, TraitDefinition, TypedTrees,
};
use typed_trees::proposition::{ProofSubstitutions, PropositionLabels};

#[allow(clippy::too_many_arguments)]
pub(crate) fn check_proposition_law_conformance(
    program: &TypedTrees,
    machine: &Machine,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    explicit_trait_arguments: &[typed_trees::types::TypeReferenceHandle],
    proposition_laws: &[&typed_trees::proposition::PropositionApplication],
    proven_propositions: &[&typed_trees::proposition::PropositionApplication],
    proven_expressions: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if proposition_laws.is_empty() {
        return;
    }
    let Some(entry_state) = program.machine_states(machine).first() else {
        return;
    };
    let substitutions = program
        .state_signature_parameters(requirement)
        .iter()
        .zip(program.state_parameters(entry_state))
        .map(|(required, actual)| {
            (
                required.symbol,
                required.name.as_str().to_owned(),
                actual.name.as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let trait_parameters = program.trait_type_parameters(trait_definition);

    for law in proposition_laws {
        let mut instantiated = (*law).clone();
        if program.symbols.get(law.proposition).kind == symbols::SymbolKind::PropositionParameter {
            let Some(parameter_index) = trait_parameters
                .iter()
                .position(|parameter| parameter.symbol == law.proposition)
            else {
                continue;
            };
            let Some(argument) = explicit_trait_arguments.get(parameter_index) else {
                continue;
            };
            let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(*argument)
            else {
                continue;
            };
            instantiated.proposition = *symbol;
            instantiated.name = name.clone();
        }
        let binder_labels = if instantiated.binder_arguments.is_empty() {
            match synthesize_indexed_law_binder_labels(
                program,
                law,
                &instantiated,
                requirement,
                entry_state,
            ) {
                Some(labels) => labels,
                None => {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` satisfies `{}::{}` but the selected proposition family `{}` requires an indexed binder telescope that cannot be synthesized from the law's representative parameters",
                        machine.name,
                        trait_definition.name,
                        requirement.name,
                        instantiated.name,
                    )));
                    continue;
                }
            }
        } else {
            instantiated
                .binder_arguments
                .iter()
                .map(|argument| argument.display_name())
                .collect::<Vec<_>>()
        };
        let argument_labels = program
            .expression_table
            .expression_handles(instantiated.arguments)
            .iter()
            .map(|argument| {
                program.render_proof_expression(
                    *argument,
                    ProofSubstitutions::ByParameter(&substitutions),
                )
            })
            .collect::<Vec<_>>();
        let Some(expected_formula) = program.normalize_proposition_application(
            &instantiated,
            Some(PropositionLabels {
                binder_labels: &binder_labels,
                argument_labels: &argument_labels,
            }),
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but its proposition law `{}` does not normalize after trait-family and indexed-binder substitution",
                machine.name,
                trait_definition.name,
                requirement.name,
                instantiated.name,
            )));
            continue;
        };
        let expected = expected_formula.identity_label();
        let expected_nominal = program.normalize_nominal_proposition_application(
            &instantiated,
            Some(PropositionLabels {
                binder_labels: &binder_labels,
                argument_labels: &argument_labels,
            }),
        );
        let matched = if let Some(expected_nominal) = expected_nominal {
            proven_propositions.iter().any(|proven| {
                program
                    .normalize_nominal_proposition_application(proven, None)
                    .is_some_and(|actual| actual == expected_nominal)
            })
        } else if let typed_trees::proposition::NormalizedPropositionFormula::Boolean {
            label: expected_boolean,
        } = &expected_formula
        {
            proven_propositions.iter().any(|proven| {
                matches!(
                    program.normalize_proposition_application(proven, None),
                    Some(typed_trees::proposition::NormalizedPropositionFormula::Boolean {
                        label,
                    }) if label == *expected_boolean
                )
            }) || proven_expressions.iter().any(|proven| {
                program.render_proof_expression(*proven, ProofSubstitutions::None)
                    == *expected_boolean
            })
        } else {
            false
        };
        if !matched {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but proves no ensures matching proposition law `{expected}` after trait-family substitution",
                machine.name, trait_definition.name, requirement.name,
            )));
        }
    }
}

pub(crate) fn synthesize_indexed_law_binder_labels(
    program: &TypedTrees,
    authored_law: &typed_trees::proposition::PropositionApplication,
    instantiated_law: &typed_trees::proposition::PropositionApplication,
    requirement: &StateSignature,
    entry_state: &typed_trees::state::State,
) -> Option<Vec<String>> {
    let declaration = program
        .propositions()
        .iter()
        .find(|declaration| declaration.symbol == instantiated_law.proposition)?;
    let binder_count = program.proposition_binders(declaration).len();
    if binder_count == 0 {
        return Some(Vec::new());
    }
    let required_parameters = program.state_signature_parameters(requirement);
    let actual_parameters = program.state_parameters(entry_state);
    let mut labels = Vec::with_capacity(binder_count);
    for argument in program
        .expression_table
        .expression_handles(authored_law.arguments)
    {
        let required_index =
            proposition_law_parameter_index(program, *argument, required_parameters)?;
        let actual = actual_parameters.get(required_index)?;
        let generic_arguments = generic_type_argument_handles(program, actual.type_reference)?;
        labels.extend(
            generic_arguments
                .iter()
                .map(|argument| program.display_type_reference(*argument)),
        );
    }
    (labels.len() == binder_count).then_some(labels)
}

fn proposition_law_parameter_index(
    program: &TypedTrees,
    argument: ExpressionHandle,
    parameters: &[typed_trees::signature::StateParameter],
) -> Option<usize> {
    let ExpressionNode::Name(path) = program.expression_table.expression(argument) else {
        return None;
    };
    let name = program
        .expression_table
        .name_path_members(path.members)
        .last()?;
    parameters.iter().position(|parameter| {
        (path.symbol.is_valid() && parameter.symbol == path.symbol)
            || parameter.name.as_str() == name.as_str()
    })
}

fn generic_type_argument_handles(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<&[typed_trees::types::TypeReferenceHandle]> {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            generic_type_argument_handles(program, *referee)
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            generic_type_argument_handles(program, *base_type)
        }
        typed_trees::types::TypeReferenceNode::Generic { arguments, .. } => Some(
            program
                .type_reference_table
                .type_reference_handles(*arguments),
        ),
        _ => None,
    }
}

/// Forall-to-forall sharpening: every law parameter must bind to a DISTINCT
/// plain parameter VARIABLE of the satisfier -- binding two law parameters to
/// one satisfier parameter (or to a compound term) proves only a weaker
/// instance of the law.
pub(crate) fn bindings_are_forall_general(
    bindings: &[(String, StructuralTerm)],
    satisfier_parameters: &[String],
) -> bool {
    let mut seen: Vec<&String> = Vec::new();
    for (_, bound) in bindings {
        let StructuralTerm::Variable(name) = bound else {
            return false;
        };
        if !satisfier_parameters
            .iter()
            .any(|parameter| parameter == name)
        {
            return false;
        }
        if seen.contains(&name) {
            return false;
        }
        seen.push(name);
    }
    true
}

/// Split an ensures fact into its `==` conjuncts (And-chains recursively).
pub(crate) fn collect_equality_conjuncts(
    program: &TypedTrees,
    expression: ExpressionHandle,
    out: &mut Vec<ExpressionHandle>,
) {
    if super::super::structural_terms::is_case_observation(program, expression) {
        return;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return;
    };
    match binary.operator {
        BinaryOperator::And => {
            collect_equality_conjuncts(program, binary.left, out);
            collect_equality_conjuncts(program, binary.right, out);
        }
        BinaryOperator::Equal => out.push(expression),
        _ => {}
    }
}
