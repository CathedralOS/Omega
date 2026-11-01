//! Form ordinary integer calls before consuming their public result facts.
//! The outer declaration application owns the transaction and exact inputs;
//! operational purity and totality are independent specification-call debts.

use super::{
    ApplicationDecisions, Engine, IntegerApplicationInput, Polynomial, dispatch, formed_type,
    formed_value,
};
use crate::frozen_program::expression_table;
use crate::proof_contracts::contract_entailment::specification_calls::{
    SelectedCallContract, owner_contracts, owner_parameters, owner_return_type, owner_symbol,
    requirements_are_acyclic, selected_call_contract,
};
use numerics::bignum::BigInt;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::SignatureContractKind;

pub(super) fn form(
    engine: &mut Engine<'_>,
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    depth: usize,
    decisions: &mut ApplicationDecisions,
) -> Option<Polynomial> {
    let call = expression_table(program).call_node(expression)?;
    if super::super::super::struct_trace() {
        eprintln!(
            "STRUCT declaration-call begin expression={expression:?} owner={:?} owned={} reads={}",
            engine.source_owner,
            engine.owned_arguments.len(),
            engine.formal_read_arguments.len()
        );
    }
    let owner = selected_call_contract(program, engine.source_owner, call)?;
    let parameters = owner_parameters(program, &owner);
    let arguments = expression_table(program).expression_handles(call.arguments);
    if parameters.len() != arguments.len()
        || !requirements_are_acyclic(program, &owner, &mut Vec::new(), &mut Vec::new())
    {
        return None;
    }
    let actuals = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            let input = if program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
            {
                IntegerApplicationInput::Scalar(formed_value(
                    engine,
                    program,
                    *argument,
                    depth + 1,
                    decisions,
                )?)
            } else {
                engine.captured_owned_call_input(*argument)?
            };
            if !crate::has_anonymous_numeric_results(program, *argument) {
                let actual_type = engine.source_result_type(*argument)?;
                if !crate::declared_input_type_matches(
                    program,
                    actual_type,
                    parameter.type_reference,
                ) {
                    return None;
                }
            }
            Some((*argument, input))
        })
        .collect::<Option<Vec<_>>>()?;
    if super::super::super::struct_trace() {
        eprintln!("STRUCT declaration-call inputs expression={expression:?} actuals={actuals:?}");
    }
    with_arguments(
        engine,
        owner_symbol(&owner),
        parameters,
        &actuals,
        |engine| {
            for (parameter, (_, input)) in parameters.iter().zip(&actuals) {
                match input {
                    IntegerApplicationInput::Scalar(value) => {
                        formed_type(engine, program, parameter.type_reference, value)
                            .then_some(())?;
                    }
                    IntegerApplicationInput::Record {
                        type_reference,
                        snapshot,
                        ..
                    } => {
                        let snapshot = snapshot.as_ref()?;
                        if !crate::declared_input_type_matches(
                            program,
                            snapshot.actual_type(),
                            *type_reference,
                        ) || !crate::declared_input_type_matches(
                            program,
                            *type_reference,
                            parameter.type_reference,
                        ) {
                            return None;
                        }
                    }
                }
            }
            for contract in owner_contracts(program, &owner)
                .filter(|contract| contract.kind == SignatureContractKind::Requires)
            {
                if contract.binding.is_some() {
                    return None;
                }
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Expression(requirement) = fact else {
                        return None;
                    };
                    dispatch::require_condition(engine, *requirement, depth + 1)?;
                }
            }
            Some(())
        },
    )?;
    if super::super::super::struct_trace() {
        eprintln!("STRUCT declaration-call requires expression={expression:?}");
    }
    let value = engine.selected_call_value(
        engine.source_owner,
        expression,
        owner_symbol(&owner),
        actuals.iter().map(|(_, value)| value.clone()).collect(),
    )?;
    // The result's inherent carrier follows from the checked callee only
    // after its invocation forms. It cannot help prove that invocation.
    let result_type = owner_return_type(&owner);
    let primitive = program.primitive_type_reference(result_type)?;
    let (minimum, maximum) = primitive.integer_bounds()?;
    engine
        .bounds
        .push((value.clone(), BigInt::from_i128(minimum)));
    engine
        .bounds
        .push((value.neg(), BigInt::from_i128(maximum).negate()));
    engine.refresh_difference_bounds();
    if !engine.complete_formed_call(expression, &actuals, &value) {
        return None;
    }
    if super::super::super::struct_trace() {
        eprintln!("STRUCT declaration-call guarantees expression={expression:?}");
    }
    with_arguments(
        engine,
        owner_symbol(&owner),
        parameters,
        &actuals,
        |engine| formed_type(engine, program, result_type, &value).then_some(()),
    )?;
    let SelectedCallContract::Concrete(machine, state) = owner else {
        return None;
    };
    let decision = checked_trees::CheckedDeclarationCall {
        expression,
        authored_target: call.target_symbol,
        arguments: call.arguments,
        checked_arguments: arena::HandleSpan::empty(),
        target_machine: machine.symbol,
        target_state: state.symbol,
        target_contract: None,
    };
    if !decisions.calls.contains(&decision) {
        decisions.calls.push(decision);
    }
    Some(value)
}

/// Every actual has formed in its caller before the selected public telescope
/// becomes visible. Scalar reads and whole-value identity share this cut.
fn with_arguments<Output>(
    engine: &mut Engine<'_>,
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    actuals: &[(ExpressionHandle, IntegerApplicationInput)],
    query: impl FnOnce(&mut Engine<'_>) -> Option<Output>,
) -> Option<Output> {
    let scalar = parameters
        .iter()
        .zip(actuals)
        .filter_map(|(formal, (_, input))| match input {
            IntegerApplicationInput::Scalar(value) => Some((formal.symbol, value.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    engine.with_formal_arguments(owner, &scalar, |engine| {
        for (formal, (_, input)) in parameters.iter().zip(actuals) {
            if let IntegerApplicationInput::Record {
                snapshot, reads, ..
            } = input
            {
                engine.owned_arguments.push((
                    crate::ScopedArithmeticBinder::Symbol(formal.symbol),
                    snapshot.clone()?,
                ));
                engine
                    .formal_read_arguments
                    .extend(reads.iter().map(|(coordinate, value)| {
                        (
                            crate::RankSubjectCoordinate {
                                parameter: formal.symbol,
                                path: coordinate.path.clone(),
                            },
                            value.clone(),
                        )
                    }));
            }
        }
        query(engine)
    })
}
