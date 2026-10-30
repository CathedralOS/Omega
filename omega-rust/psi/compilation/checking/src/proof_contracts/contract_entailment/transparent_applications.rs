//! Entailment of transparent proposition applications.
//!
//! A transparent proposition is its body: unfolding an application binds the
//! declaration's parameters to the actual argument terms and yields one
//! identity equation per conjunct, following `= other(..)` definitions to
//! their own bodies. The same unfolding serves both directions -- a `requires`
//! application installs its equations as hypotheses, an `ensures` application
//! must prove every one -- so a defined premise means exactly what the
//! defined goal means.

use crate::proof_contracts::contract_entailment::structural_judgment::{
    StructuralJudge, StructuralJudgment, StructuralTerm,
};
use crate::proof_contracts::contract_entailment::structural_terms::{
    structural_call_machine_name, structural_term,
};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;

/// The identity equations one transparent application denotes, one per
/// conjunct of its unfolded body. `None` when the application is not
/// transparent. A `None` conjunct lies outside the structural language: a
/// hypothesis drops it (only weakening), a goal cannot be proven. A literal
/// `true` conjunct contributes no equation.
pub(super) fn transparent_application_conjuncts(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    application: &typed_trees::proposition::PropositionApplication,
) -> Option<Vec<Option<(StructuralTerm, StructuralTerm)>>> {
    let declaration = program.proposition_definition_by_symbol(application.proposition)?;
    if !matches!(
        declaration.body,
        typed_trees::proposition::PropositionBody::Transparent { .. }
    ) {
        return None;
    }
    let parameters = program.proposition_parameters(declaration);
    let arguments = program
        .expression_table
        .expression_handles(application.arguments);
    if parameters.len() != arguments.len() {
        return None;
    }
    let environment = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            Some((
                parameter.name.as_str().to_owned(),
                argument_term(program, machine, state, *argument)?,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let mut conjuncts = Vec::new();
    unfold_definition(
        program,
        declaration.symbol,
        &environment,
        &mut Vec::new(),
        &mut conjuncts,
    );
    Some(conjuncts)
}

/// Append the conjuncts of `proposition`'s body with its parameters bound to
/// `environment`. A body that applies another proposition unfolds that one
/// under the substituted arguments; a cycle or an opaque body contributes one
/// unreadable conjunct.
fn unfold_definition(
    program: &dyn crate::CheckingInput,
    proposition: symbols::SymbolHandle,
    environment: &[(String, StructuralTerm)],
    visiting: &mut Vec<symbols::SymbolHandle>,
    conjuncts: &mut Vec<Option<(StructuralTerm, StructuralTerm)>>,
) {
    let Some(declaration) = program.proposition_definition_by_symbol(proposition) else {
        conjuncts.push(None);
        return;
    };
    let typed_trees::proposition::PropositionBody::Transparent {
        proposition: formula,
    } = &declaration.body
    else {
        conjuncts.push(None);
        return;
    };
    if visiting.contains(&proposition) {
        conjuncts.push(None);
        return;
    }
    let parameters = program.proposition_parameters(declaration);
    visiting.push(proposition);
    match formula {
        typed_trees::proposition::PropositionFormula::BooleanExpression(expression) => {
            unfold_boolean(
                program,
                declaration.symbol,
                parameters,
                *expression,
                environment,
                conjuncts,
            );
        }
        typed_trees::proposition::PropositionFormula::Application(expansion) => {
            let inner = program
                .proposition_definition_by_symbol(expansion.proposition)
                .map(|inner| program.proposition_parameters(inner));
            let arguments = program
                .expression_table
                .expression_handles(expansion.arguments);
            let inner_environment = inner
                .filter(|inner| inner.len() == arguments.len())
                .and_then(|inner| {
                    inner
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| {
                            let term = structural_term(
                                program,
                                None,
                                Some((declaration.symbol, parameters)),
                                *argument,
                                None,
                            )?;
                            Some((
                                parameter.name.as_str().to_owned(),
                                StructuralJudge::substitute_term(&term, environment),
                            ))
                        })
                        .collect::<Option<Vec<_>>>()
                });
            match inner_environment {
                Some(inner_environment) => unfold_definition(
                    program,
                    expansion.proposition,
                    &inner_environment,
                    visiting,
                    conjuncts,
                ),
                None => conjuncts.push(None),
            }
        }
    }
    visiting.pop();
}

fn unfold_boolean(
    program: &dyn crate::CheckingInput,
    scope: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    expression: ExpressionHandle,
    environment: &[(String, StructuralTerm)],
    conjuncts: &mut Vec<Option<(StructuralTerm, StructuralTerm)>>,
) {
    if let Some(binary) = program.expression_table.binary_node(expression)
        && binary.operator == BinaryOperator::And
    {
        unfold_boolean(
            program,
            scope,
            parameters,
            binary.left,
            environment,
            conjuncts,
        );
        unfold_boolean(
            program,
            scope,
            parameters,
            binary.right,
            environment,
            conjuncts,
        );
        return;
    }
    // `true` is the unit of conjunction: it owes a goal nothing and grants a
    // premise nothing. A total relation (`= true`) therefore unfolds to no
    // equations rather than one unreadable conjunct.
    if program.expression_table.boolean_node(expression) == Some(&true) {
        return;
    }
    conjuncts.push(
        super::structural_terms::identity_equality_terms(program, scope, parameters, expression)
            .map(|(left, right)| {
                (
                    StructuralJudge::substitute_term(&left, environment),
                    StructuralJudge::substitute_term(&right, environment),
                )
            }),
    );
}

/// Install the hypotheses `requires` contracts grant `machine` in `state`:
/// every unfolded conjunct of each transparent proposition application.
pub(super) fn intake_transparent_requires(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    contracts: &[typed_trees::signature::SignatureContract],
    judge: &mut StructuralJudge<'_>,
) {
    for application in contracts
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(ProofFact::as_proposition)
    {
        for (left, right) in transparent_application_conjuncts(program, machine, state, application)
            .into_iter()
            .flatten()
            .flatten()
        {
            judge.intake_equation(left, right, 0);
        }
    }
}

/// The conformance edge a requirement-authored contract fact migrated through
/// onto its satisfying state.
pub struct InheritedRequirementEdge<'edge> {
    pub trait_definition: &'edge typed_trees::trait_definition::TraitDefinition,
    pub requirement: &'edge typed_trees::signature::StateSignature,
    pub trait_arguments: &'edge [typed_trees::types::TypeReferenceHandle],
}

/// Prove one transparent proposition application through the same
/// structural entailment engine used for ordinary Boolean contracts. Named
/// propositions remain the public fact identity; transparency is only the
/// proof-side expansion. Every conjunct of the unfolded body must be an
/// identity equation the hypotheses prove; a primitive or witness-bearing
/// proposition is never transparent. A requirement-authored `goal` first
/// moves through its `inherited` edge: the selected family replaces the
/// proposition parameter and the requirement's parameters rename
/// positionally onto the satisfier's.
pub fn transparent_proposition_application_entailed(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    goal: &typed_trees::proposition::PropositionApplication,
    inherited: Option<InheritedRequirementEdge<'_>>,
) -> bool {
    let (conjuncts, renames) = match inherited {
        None => (
            transparent_application_conjuncts(program, machine, state, goal),
            Vec::new(),
        ),
        Some(edge) => {
            let Some(endpoint) = crate::inherited_requirement_proposition_application(
                program,
                edge.trait_definition,
                edge.requirement,
                state,
                edge.trait_arguments,
                goal,
            ) else {
                return false;
            };
            let Some(satisfier_parameters) = crate::inherited_satisfier_parameters(
                program,
                edge.trait_definition,
                edge.requirement,
                state,
            ) else {
                return false;
            };
            let renames = program
                .state_signature_parameters(edge.requirement)
                .iter()
                .zip(satisfier_parameters)
                .map(|(required, actual)| {
                    (
                        required.name.as_str().to_owned(),
                        StructuralTerm::Variable(actual.name.as_str().to_owned()),
                    )
                })
                .collect::<Vec<_>>();
            (
                transparent_application_conjuncts(program, machine, state, &endpoint.application),
                renames,
            )
        }
    };
    premises_prove(program, machine, state, conjuncts, &renames)
}

/// Whether `caller`'s premises in `caller_state` establish the transparent
/// `requirement` its call to `callee` owes, with the callee's parameters
/// bound to the call's argument terms.
pub fn transparent_call_requirement_entailed(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    caller_state: &typed_trees::state::State,
    callee: &Machine,
    callee_state: &typed_trees::state::State,
    arguments: &[ExpressionHandle],
    requirement: &typed_trees::proposition::PropositionApplication,
) -> bool {
    let parameters = program.state_parameters(callee_state);
    if parameters.len() != arguments.len() {
        return false;
    }
    let Some(bindings) = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            Some((
                parameter.name.as_str().to_owned(),
                argument_term(program, caller, caller_state, *argument)?,
            ))
        })
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    premises_prove(
        program,
        caller,
        caller_state,
        transparent_application_conjuncts(program, callee, callee_state, requirement),
        &bindings,
    )
}

/// Every conjunct, renamed by `renames`, holds under the machine and state
/// `requires` hypotheses, transparent applications unfolded among them.
fn premises_prove(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    conjuncts: Option<Vec<Option<(StructuralTerm, StructuralTerm)>>>,
    renames: &[(String, StructuralTerm)],
) -> bool {
    let Some(equations) =
        conjuncts.and_then(|conjuncts| conjuncts.into_iter().collect::<Option<Vec<_>>>())
    else {
        return false;
    };
    let expression_requires = program
        .contract_expressions(
            program
                .machine_contracts(machine)
                .iter()
                .chain(program.state_contracts(state)),
            SignatureContractKind::Requires,
        )
        .collect::<Vec<_>>();
    let mut judge = StructuralJudge::from_requires(program, machine, &expression_requires, true);
    for contracts in [
        program.machine_contracts(machine),
        program.state_contracts(state),
    ] {
        intake_transparent_requires(program, machine, state, contracts, &mut judge);
    }
    judge.hypotheses_contradictory
        || equations.into_iter().all(|(left, right)| {
            let left = StructuralJudge::substitute_term(&left, renames);
            let right = StructuralJudge::substitute_term(&right, renames);
            matches!(
                judge.judge_equation(judge.resolve(left), judge.resolve(right), 0),
                StructuralJudgment::Proven
            )
        })
}

fn argument_term(
    program: &dyn crate::CheckingInput,
    caller_machine: &Machine,
    caller_state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<StructuralTerm> {
    let Some(call) = program.expression_table.call_node(expression) else {
        return structural_term(
            program,
            Some((caller_machine, caller_state, Default::default())),
            None,
            expression,
            None,
        );
    };
    if call.receiver.is_valid()
        || !call.evidence_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
    {
        return structural_term(
            program,
            Some((caller_machine, caller_state, Default::default())),
            None,
            expression,
            None,
        );
    }
    let state = crate::machine_calls::calls::signature_queries::unique_state_by_symbol(
        program,
        call.target_symbol,
    )?;
    let Some(data_symbol) =
        crate::value_custody::type_shapes::head_data_symbol(program, state.return_type)
    else {
        return structural_term(
            program,
            Some((caller_machine, caller_state, Default::default())),
            None,
            expression,
            None,
        );
    };
    let data = crate::declarations::data_queries::data_definition_by_symbol(program, data_symbol)?;
    // Product eta reconstructs plain records only. A sum's common
    // fields do not identify its case or active payload.
    if program
        .data_members(data)
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
    {
        return structural_term(
            program,
            Some((caller_machine, caller_state, Default::default())),
            None,
            expression,
            None,
        );
    }
    let arguments = program
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .map(|argument| {
            structural_term(
                program,
                Some((caller_machine, caller_state, Default::default())),
                None,
                *argument,
                None,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let machine = structural_call_machine_name(call.target.as_str(), &call.machine_arguments, &[]);
    let mut fields = program
        .data_members(data)
        .iter()
        .filter_map(|member| {
            let field = member.as_field()?;
            Some((
                field.name.as_str().to_owned(),
                StructuralTerm::CallProjection {
                    target: call.target_symbol,
                    selections: call.machine_arguments.to_vec(),
                    machine: machine.clone(),
                    result_type: state.return_type,
                    field: field.symbol,
                    field_name: field.name.as_str().to_owned(),
                    arguments: arguments.clone(),
                },
            ))
        })
        .collect::<Vec<_>>();
    fields.sort_by(|left, right| left.0.cmp(&right.0));
    Some(StructuralTerm::Constructor {
        data: data.name.as_str().to_owned(),
        case: String::new(),
        fields,
    })
}
