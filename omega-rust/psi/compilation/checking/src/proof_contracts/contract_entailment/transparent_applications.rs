//! Entailment of transparent proposition applications.

use crate::proof_contracts::contract_entailment::structural_judgment::{
    StructuralJudge, StructuralJudgment, StructuralTerm,
};
use crate::proof_contracts::contract_entailment::structural_terms::{
    structural_call_machine_name, structural_term_in_scope,
};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;

/// Prove one transparent Boolean proposition application through the same
/// structural entailment engine used for ordinary Boolean contracts. Named
/// propositions remain the public fact identity; transparency is only the
/// proof-side expansion. The first rung accepts equality formulas directly
/// and never treats a primitive or witness-bearing proposition as transparent.
pub fn transparent_proposition_application_entailed(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    goal: &typed_trees::proposition::PropositionApplication,
) -> bool {
    fn equation(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &typed_trees::state::State,
        application: &typed_trees::proposition::PropositionApplication,
    ) -> Option<(StructuralTerm, StructuralTerm)> {
        fn argument_term(
            program: &dyn crate::CheckingInput,
            caller_machine: &Machine,
            caller_state: &typed_trees::state::State,
            expression: ExpressionHandle,
        ) -> Option<StructuralTerm> {
            let Some(call) = program.expression_table.call_node(expression) else {
                return structural_term_in_scope(
                    program,
                    Some((caller_machine, caller_state, Default::default())),
                    expression,
                );
            };
            if call.receiver.is_valid()
                || !call.evidence_arguments.is_empty()
                || call.static_requirement_dispatch.is_some()
            {
                return structural_term_in_scope(
                    program,
                    Some((caller_machine, caller_state, Default::default())),
                    expression,
                );
            }
            let Some(state) =
                crate::machine_calls::calls::signature_queries::unique_state_by_symbol(
                    program,
                    call.target_symbol,
                )
            else {
                return None;
            };
            let Some(data_symbol) =
                crate::value_custody::type_shapes::head_data_symbol(program, state.return_type)
            else {
                return structural_term_in_scope(
                    program,
                    Some((caller_machine, caller_state, Default::default())),
                    expression,
                );
            };
            let data =
                crate::declarations::data_queries::data_definition_by_symbol(program, data_symbol)?;
            // Product eta reconstructs plain records only. A sum's common
            // fields do not identify its case or active payload.
            if program
                .data_members(data)
                .iter()
                .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
            {
                return structural_term_in_scope(
                    program,
                    Some((caller_machine, caller_state, Default::default())),
                    expression,
                );
            }
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| {
                    structural_term_in_scope(
                        program,
                        Some((caller_machine, caller_state, Default::default())),
                        *argument,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            let machine =
                structural_call_machine_name(call.target.as_str(), &call.machine_arguments, &[]);
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

        let declaration = program.proposition_definition_by_symbol(application.proposition)?;
        let typed_trees::proposition::PropositionBody::Transparent {
            proposition: typed_trees::proposition::PropositionFormula::BooleanExpression(formula),
        } = declaration.body
        else {
            return None;
        };
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
        let Some(binary) = program.expression_table.binary_node(formula) else {
            return None;
        };
        if binary.operator != BinaryOperator::Equal
            || super::structural_terms::is_case_observation(program, formula)
            || !super::structural_terms::has_identity_equality_meaning(
                program,
                declaration.symbol,
                parameters,
                formula,
            )
        {
            return None;
        }
        let (left, right) = super::structural_terms::structural_term_pair_for_parameters(
            program,
            declaration.symbol,
            parameters,
            binary,
        )?;
        Some((
            StructuralJudge::substitute_term(&left, &environment),
            StructuralJudge::substitute_term(&right, &environment),
        ))
    }

    let expression_requires = program
        .contract_expressions(
            program
                .machine_contracts(machine)
                .iter()
                .chain(program.state_contracts(state)),
            SignatureContractKind::Requires,
        )
        .collect::<Vec<_>>();
    let mut judge = StructuralJudge::from_requires(program, machine, &expression_requires);
    for application in program
        .contract_facts(
            program
                .machine_contracts(machine)
                .iter()
                .chain(program.state_contracts(state)),
            SignatureContractKind::Requires,
        )
        .filter_map(|fact| match fact {
            ProofFact::Proposition(application) => Some(application),
            _ => None,
        })
    {
        if let Some((left, right)) = equation(program, machine, state, application) {
            judge.intake_equation(left, right, 0);
        }
    }
    let Some((left, right)) = equation(program, machine, state, goal) else {
        return false;
    };
    matches!(
        judge.judge_equation(judge.resolve(left), judge.resolve(right), 0),
        StructuralJudgment::Proven
    )
}
