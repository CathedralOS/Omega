//! Equivalence selection and exact relation application matching.

use crate::proof_contracts::quotients::CORE_EQUIVALENCE_SOURCE;
use diagnostics::Diagnostic;
use std::collections::HashSet;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;
use typed_trees::proposition::ProofSubstitutions;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_equivalence_selection(
    program: &TypedTrees,
    definition: &typed_trees::data::DataDefinition,
    quotient: &typed_trees::data::QuotientDefinition,
    relation: &typed_trees::proposition::PropositionDefinition,
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
        typed_trees::trait_definition::ConformanceSubject::Subjectless
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
        let Some(declaring_arguments) = crate::declarations::traits::arguments_for_declaring_trait(
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
        crate::proof_contracts::contract_entailment::check_law_conformance(
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
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
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

pub(crate) fn fact_is_exact_relation_pair(
    program: &TypedTrees,
    fact: &typed_trees::domain::ProofFact,
    relation: &typed_trees::proposition::PropositionDefinition,
    left: &typed_trees::signature::StateParameter,
    right: &typed_trees::signature::StateParameter,
    parameters: &[typed_trees::signature::StateParameter],
) -> bool {
    match fact {
        typed_trees::domain::ProofFact::Proposition(application) => {
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
        typed_trees::domain::ProofFact::Expression(actual) => {
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
        typed_trees::domain::ProofFact::Membership(_) => false,
    }
}

pub(crate) fn exact_relation_application_matches(
    program: &TypedTrees,
    application: &typed_trees::proposition::PropositionApplication,
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
    expression: typed_trees::expression::ExpressionHandle,
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
    name: &typed_trees::name::Identifier,
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
    let source_matches = source.origin == source::SourceOrigin::Toolchain
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
            if matches!(carrier.kind, typed_trees::data::TypeParameterKind::Type)
                && matches!(relation.kind, typed_trees::data::TypeParameterKind::Proposition { .. })
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

/// Walk a proof machine's transitive call closure and report the first entry
/// that is not an ordinary checked body.
///
/// wiki/spec/proofs/quotients.md requires acceptance under a policy refusing
/// quotient assumptions, and that control is "conversion-independent transitive
/// assumption closure" rather than an inspection of the directly named entry.
/// Helper types and statements remain outside this machine-call closure.
pub(super) fn checked_proof_dependency<'program>(
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
    for dependency in
        crate::machine_calls::call_cycles::machine_call_dependency_symbols(program, machine)
    {
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

pub(crate) fn base_data_symbol(
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
