//! Form a declaration expression under independently captured actual values.
//!
//! Denotation normalization may cancel an unsafe intermediate. Formation visits
//! children first and asks the ordinary selected-operation admission owner at
//! every finite arithmetic node, before publishing its value to its parent.

use super::{ScopedArithmeticExpression, ScopedArithmeticHypothesis, calls, install_hypotheses};
use crate::frozen_program::expression_table;
use crate::proof_contracts::contract_entailment::arithmetic_judgment::Engine;
use typed_trees::machine::Machine;
use typed_trees::types::PrimitiveType;

/// The caller supplies exact captured values and previously established facts.
/// Neither the proposed result nor its desired interval is an input premise.
pub fn scoped_declared_integer_application_formation(
    program: &dyn crate::CheckingInput,
    context_machine: &Machine,
    hypotheses: &[ScopedArithmeticHypothesis],
    expression: &ScopedArithmeticExpression,
    result: PrimitiveType,
) -> Option<super::super::IntegerApplicationFormation> {
    let Some(declaration) = program
        .measures()
        .iter()
        .find(|definition| definition.symbol == expression.owner)
    else {
        return None;
    };
    let Some(formal) = declaration.parameter.as_ref() else {
        return None;
    };
    if expression_table(program).expression_handles(declaration.body) != [expression.expression]
        || program.primitive_type_reference(declaration.return_type) != Some(result)
    {
        return None;
    }
    let mut calls = calls::ScopedCalls::new(program, context_machine);
    let Some(expression) = calls.expression(expression) else {
        return None;
    };
    let Some(hypotheses) = calls.hypotheses(hypotheses) else {
        return None;
    };
    let mut engine = Engine::strict(program, context_machine, &[]);
    if !install_hypotheses(program, &mut engine, &hypotheses)
        || engine.requires_unsatisfiable
        || !engine.install_scoped_expression(&expression)
    {
        return None;
    }
    let Some(input) = engine.strict_symbol_value(formal.symbol) else {
        return None;
    };
    engine
        .form_declared_integer_application(declaration.symbol, &input)
        .and_then(|application| application.formation)
}
