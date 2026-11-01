//! Resolve scalar demands in the declaration that owns each expression.
//! Calls contribute their selected public contract, never a private body.
//! Formal projections are substituted to the outer input coordinate before
//! publication; foreign expression handles never become caller bindings.

use super::{DeclaredInputRead, DeclaredInputShape};
use crate::proof_contracts::contract_entailment::specification_calls::{
    call_parameter_scope, owner_contracts, owner_symbol, selected_call_contract_for_parameters,
};
use crate::{CheckingInput, RankSubjectCoordinate};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::measure::MeasureDefinition;
use typed_trees::signature::{SignatureContractKind, StateParameter};

pub(super) fn collect(
    program: &dyn CheckingInput,
    measure: &MeasureDefinition,
    shape: &mut DeclaredInputShape,
) -> Option<()> {
    let parameter = measure.parameter.as_ref()?;
    scope(
        program,
        measure.symbol,
        std::slice::from_ref(parameter),
        &[(
            parameter.symbol,
            RankSubjectCoordinate {
                parameter: parameter.symbol,
                path: Vec::new(),
            },
        )],
        program.expression_table.expression_handles(measure.body),
        shape,
        &mut vec![measure.symbol],
    )
}

fn scope(
    program: &dyn CheckingInput,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    inputs: &[(SymbolHandle, RankSubjectCoordinate)],
    expressions: &[ExpressionHandle],
    shape: &mut DeclaredInputShape,
    active: &mut Vec<SymbolHandle>,
) -> Option<()> {
    let mut pending = expressions.to_vec();
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Indexed(indexed) = node {
            let operand_types = [indexed.collection, indexed.index].map(|operand| {
                crate::parameter_expression_result_type_reference(
                    program, owner, parameters, operand,
                )
            });
            typed_trees::operator::builtin_index_read_is_spelled(
                program,
                owner,
                expression,
                &operand_types,
            )
            .then_some(())?;
        }
        if let Some(coordinate) = coordinate(program, expression, inputs)
            && let Some(geometry) = crate::structural_scalar_coordinate_in_type(
                program,
                facts::PlaceRoot::Symbol(shape.parameter),
                shape.type_reference,
                &coordinate.path,
            )
        {
            geometry.primitive_type.is_fixed_integer().then_some(())?;
            if !shape.reads.iter().any(|read| read.coordinate == coordinate) {
                shape.reads.push(DeclaredInputRead {
                    coordinate,
                    type_reference: geometry.result_type_reference,
                    primitive_type: geometry.primitive_type,
                });
            }
        }
        if let ExpressionNode::Call(call) = node {
            let selected = selected_call_contract_for_parameters(program, call, owner, parameters)?;
            let selected_owner = owner_symbol(&selected);
            if active.contains(&selected_owner) {
                return None;
            }
            let site = crate::CallSite::Expression { expression, call };
            let selected_parameters = call_parameter_scope(program, owner, &selected, site)?;
            let arguments = program.expression_table.expression_handles(call.arguments);
            if arguments.len() != selected_parameters.len() {
                return None;
            }
            let selected_inputs = selected_parameters
                .iter()
                .zip(arguments)
                .filter_map(|(formal, actual)| {
                    coordinate(program, *actual, inputs).map(|actual| (formal.symbol, actual))
                })
                .collect::<Vec<_>>();
            let public = owner_contracts(program, &selected)
                .filter(|contract| {
                    matches!(
                        contract.kind,
                        SignatureContractKind::Requires
                            | SignatureContractKind::Ensures
                            | SignatureContractKind::EnsuresForResultCase { .. }
                    )
                })
                .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                .filter_map(|fact| match fact {
                    ProofFact::Expression(expression) => Some(*expression),
                    _ => None,
                })
                .collect::<Vec<_>>();
            active.push(selected_owner);
            let result = scope(
                program,
                selected_owner,
                &selected_parameters,
                &selected_inputs,
                &public,
                shape,
                active,
            );
            active.pop();
            result?;
        }
        crate::expression_traversal::executable_children(program, node, |child| {
            pending.push(child)
        });
    }
    Some(())
}

fn coordinate(
    program: &dyn CheckingInput,
    expression: ExpressionHandle,
    inputs: &[(SymbolHandle, RankSubjectCoordinate)],
) -> Option<RankSubjectCoordinate> {
    let place = crate::canonical_place_from_expression(program, expression)?;
    let root = place.root.as_symbol()?;
    let (_, input) = inputs.iter().find(|(formal, _)| *formal == root)?;
    let mut coordinate = input.clone();
    coordinate.path.extend(place.segments);
    Some(coordinate)
}
