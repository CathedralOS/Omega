//! Proposition law conformance and indexed law binder labels.

use crate::frozen_program::expression_table;
use crate::proof_contracts::contract_entailment::{
    BinaryOperator, Diagnostic, ExpressionHandle, Machine, StateSignature, StructuralTerm,
};
use typed_trees::proposition::{ProofSubstitutions, PropositionLabels};

#[allow(clippy::too_many_arguments)]
pub(crate) fn check_proposition_law_conformance(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    application: &typed_trees::trait_definition::AppliedTraitScope<'_>,
    requirement: &StateSignature,
    proposition_laws: &[&typed_trees::proposition::PropositionApplication],
    proven_propositions: &[&typed_trees::proposition::PropositionApplication],
    proven_expressions: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let trait_definition = application.declaration;
    if proposition_laws.is_empty() {
        return;
    }
    let Some(entry_state) = program.machine_states(machine).first() else {
        return;
    };
    let substitutions = crate::declarations::state_signatures::signature_entry_substitutions(
        program,
        requirement,
        entry_state,
    );

    for law in proposition_laws {
        let mut instantiated = (*law).clone();
        if program.symbols.get(law.proposition).kind == symbols::SymbolKind::PropositionParameter {
            let argument = application.parameter_argument(program.typed_program(), law.proposition);
            if !argument.is_valid() {
                diagnostics.push(Diagnostic::error(format!("machine `{}` cannot instantiate proposition law `{}` through its complete trait application", machine.name, law.name)));
                continue;
            }
            let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(argument)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` proposition law `{}` does not select a named proposition family",
                    machine.name, law.name
                )));
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
        let argument_labels = expression_table(program)
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
    program: &dyn crate::CheckingInput,
    authored_law: &typed_trees::proposition::PropositionApplication,
    instantiated_law: &typed_trees::proposition::PropositionApplication,
    requirement: &StateSignature,
    entry_state: &typed_trees::state::State,
) -> Option<Vec<String>> {
    let pack = indexed_law_pack(
        program,
        authored_law,
        instantiated_law.proposition,
        program.state_signature_parameters(requirement),
        program.state_parameters(entry_state),
    )?;
    Some(
        pack.iter()
            .map(|argument| program.display_type_reference(*argument))
            .collect(),
    )
}

/// The index pack a family law omits. A requirement over a carrier family
/// `C` states `R(left, right)` without packs; the laws quantify them
/// independently, and the realizer instantiates them through its own
/// parameter types: the subject at requirement position `k` has the
/// realizer's type `C<I>` at `k`, whose arguments are that subject's pack.
/// `None` when a subject is not a requirement parameter, a realizer subject
/// type has no arguments, or the recovered pack does not fill the family's
/// binder telescope.
fn indexed_law_pack(
    program: &dyn crate::CheckingInput,
    law: &typed_trees::proposition::PropositionApplication,
    family: symbols::SymbolHandle,
    required_parameters: &[typed_trees::signature::StateParameter],
    actual_parameters: &[typed_trees::signature::StateParameter],
) -> Option<Vec<typed_trees::types::TypeReferenceHandle>> {
    let declaration = program.proposition_definition_by_symbol(family)?;
    let binder_count = program.proposition_binders(declaration).len();
    if binder_count == 0 {
        return Some(Vec::new());
    }
    let mut pack = Vec::with_capacity(binder_count);
    for argument in expression_table(program).expression_handles(law.arguments) {
        let required_index =
            proposition_law_parameter_index(program, *argument, required_parameters)?;
        let actual = actual_parameters.get(required_index)?;
        pack.extend_from_slice(generic_type_argument_handles(
            program,
            actual.type_reference,
        )?);
    }
    (pack.len() == binder_count).then_some(pack)
}

/// The binder arguments of a family law whose pack the requirement omits,
/// recovered by [`indexed_law_pack`] from the realizer's parameter types.
/// Each pack member must name one static binder of the family's kind.
pub(crate) fn indexed_law_binder_arguments(
    program: &dyn crate::CheckingInput,
    law: &typed_trees::proposition::PropositionApplication,
    required_parameters: &[typed_trees::signature::StateParameter],
    actual_parameters: &[typed_trees::signature::StateParameter],
) -> Option<Vec<typed_trees::proposition::PropositionBinderArgument>> {
    use typed_trees::proposition::{
        PropositionBinderArgument, PropositionBinderArgumentKind, PropositionBinderKind,
    };
    let declaration = program.proposition_definition_by_symbol(law.proposition)?;
    let pack = indexed_law_pack(
        program,
        law,
        law.proposition,
        required_parameters,
        actual_parameters,
    )?;
    program
        .proposition_binders(declaration)
        .iter()
        .zip(pack)
        .map(|(binder, argument)| {
            let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(argument)
            else {
                return None;
            };
            Some(PropositionBinderArgument {
                kind: match binder.kind {
                    PropositionBinderKind::Type => PropositionBinderArgumentKind::Type,
                    PropositionBinderKind::Const { .. } => PropositionBinderArgumentKind::Const,
                    PropositionBinderKind::Machine => PropositionBinderArgumentKind::Machine,
                },
                path: Box::new([name.clone()]),
                const_literal: None,
                evidence_projection: None,
                symbol: *symbol,
            })
        })
        .collect()
}

fn proposition_law_parameter_index(
    program: &dyn crate::CheckingInput,
    argument: ExpressionHandle,
    parameters: &[typed_trees::signature::StateParameter],
) -> Option<usize> {
    let path = expression_table(program).name_path_node(argument)?;
    let name = expression_table(program)
        .name_path_members(path.members)
        .last()?;
    parameters.iter().position(|parameter| {
        (path.symbol.is_valid() && parameter.symbol == path.symbol)
            || parameter.name.as_str() == name.as_str()
    })
}

fn generic_type_argument_handles(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    out: &mut Vec<ExpressionHandle>,
) {
    if super::super::structural_terms::is_case_observation(program, expression) {
        return;
    }
    let Some(binary) = expression_table(program).binary_node(expression) else {
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
