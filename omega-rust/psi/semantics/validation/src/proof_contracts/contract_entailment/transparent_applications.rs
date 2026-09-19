//! Entailment of transparent proposition applications.

use crate::proof_contracts::contract_entailment::structural_judgment::{
    StructuralJudge, StructuralJudgment, StructuralTerm,
};
use crate::proof_contracts::contract_entailment::structural_terms::{
    structural_call_machine_name, structural_term,
};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;

/// Prove one transparent Boolean proposition application through the same
/// structural entailment engine used for ordinary Boolean contracts. Named
/// propositions remain the public fact identity; transparency is only the
/// proof-side expansion. The first rung accepts equality formulas directly
/// and never treats a primitive or witness-bearing proposition as transparent.
pub fn transparent_proposition_application_entailed(
    program: &TypedTrees,
    machine: &Machine,
    state: &typed_trees::state::State,
    goal: &typed_trees::proposition::PropositionApplication,
) -> bool {
    fn equation(
        program: &TypedTrees,
        judge: &StructuralJudge<'_>,
        application: &typed_trees::proposition::PropositionApplication,
    ) -> Option<(StructuralTerm, StructuralTerm)> {
        fn argument_term(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Option<StructuralTerm> {
            let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
                return structural_term(program, expression);
            };
            if call.receiver.is_valid() {
                return structural_term(program, expression);
            }
            let matches = program
                .machines()
                .iter()
                .flat_map(|machine| program.machine_states(machine))
                .filter(|state| state.symbol == call.target_symbol)
                .collect::<Vec<_>>();
            let [state] = matches.as_slice() else {
                return None;
            };
            let data_symbol = match program
                .type_reference_table
                .type_reference(state.return_type)
            {
                typed_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
                typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                _ => return structural_term(program, expression),
            };
            let data = program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == data_symbol)?;
            // Product eta reconstructs plain records only. A sum's common
            // fields do not identify its case or active payload.
            if program
                .data_members(data)
                .iter()
                .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
            {
                return structural_term(program, expression);
            }
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| structural_term(program, *argument))
                .collect::<Option<Vec<_>>>()?;
            let machine =
                structural_call_machine_name(call.target.as_str(), &call.machine_arguments, &[]);
            let mut fields = program
                .data_members(data)
                .iter()
                .filter_map(|member| {
                    let typed_trees::data::DataMember::Field(field) = member else {
                        return None;
                    };
                    Some((
                        field.name.as_str().to_owned(),
                        StructuralTerm::CallProjection {
                            target: call.target_symbol,
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

        let declaration = program
            .propositions()
            .iter()
            .find(|candidate| candidate.symbol == application.proposition)?;
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
                    argument_term(program, *argument)?,
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        let ExpressionNode::Binary(binary) = program.expression_table.expression(formula) else {
            return None;
        };
        if binary.operator != BinaryOperator::Equal
            || super::structural_terms::is_case_observation(program, formula)
        {
            return None;
        }
        Some((
            judge.callee_term(binary.left, &environment, 0)?,
            judge.callee_term(binary.right, &environment, 0)?,
        ))
    }

    let expression_requires = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut judge = StructuralJudge::from_requires(program, machine, &expression_requires);
    for application in program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| match fact {
            ProofFact::Proposition(application) => Some(application),
            _ => None,
        })
    {
        if let Some((left, right)) = equation(program, &judge, application) {
            judge.intake_equation(left, right, 0);
        }
    }
    let Some((left, right)) = equation(program, &judge, goal) else {
        return false;
    };
    matches!(
        judge.judge_equation(judge.resolve(left), judge.resolve(right), 0),
        StructuralJudgment::Proven
    )
}
