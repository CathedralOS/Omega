//! Form a declaration expression under independently captured actual values.
//!
//! Denotation normalization may cancel an unsafe intermediate. Formation visits
//! children first and asks the ordinary selected-operation admission owner at
//! every finite arithmetic node, before publishing its value to its parent.

use super::{ScopedArithmeticExpression, ScopedArithmeticHypothesis, calls, install_hypotheses};
use crate::proof_contracts::contract_entailment::arithmetic_judgment::{Engine, Polynomial};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::PrimitiveType;

/// The caller supplies exact captured values and previously established facts.
/// Neither the proposed result nor its desired interval is an input premise.
pub fn scoped_declared_integer_application_is_formed(
    program: &dyn crate::CheckingInput,
    context_machine: &Machine,
    hypotheses: &[ScopedArithmeticHypothesis],
    expression: &ScopedArithmeticExpression,
    result: PrimitiveType,
) -> bool {
    let Some(declaration) = program
        .measures()
        .iter()
        .find(|definition| definition.symbol == expression.owner)
    else {
        return false;
    };
    let Some(formal) = declaration.parameter.as_ref() else {
        return false;
    };
    if program
        .expression_table
        .expression_handles(declaration.body)
        != [expression.expression]
        || program.primitive_type_reference(declaration.return_type) != Some(result)
    {
        return false;
    }
    let mut calls = calls::ScopedCalls::new(program, context_machine);
    let Some(expression) = calls.expression(expression) else {
        return false;
    };
    let Some(hypotheses) = calls.hypotheses(hypotheses) else {
        return false;
    };
    let mut engine = Engine::strict(program, context_machine, &[]);
    if !install_hypotheses(program, &mut engine, &hypotheses)
        || engine.requires_unsatisfiable
        || !engine.install_scoped_expression(&expression)
    {
        return false;
    }
    let Some(input) = engine.strict_symbol_value(formal.symbol) else {
        return false;
    };
    if !formed_type(&mut engine, program, formal.type_reference, &input) {
        return false;
    }
    formed_value(&mut engine, program, expression.expression, 0)
        .is_some_and(|value| formed_type(&mut engine, program, declaration.return_type, &value))
}

fn formed_value(
    engine: &mut Engine<'_>,
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<Polynomial> {
    if depth >= 128 {
        return None;
    }
    // Wholly anonymous integers have unbounded intermediates until landing.
    if super::super::proof_integer::anonymous_integer_value(program, expression).is_some() {
        return engine.normalize(expression);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => formed_value(engine, program, atomic.value, depth + 1),
        ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
            let value = formed_value(engine, program, cast.value, depth + 1)?;
            let target = program.primitive_type_reference(cast.target_type)?;
            engine.integer_value_lands(target, &value).then_some(value)
        }
        ExpressionNode::Binary(binary) => {
            // Selection is authenticated in the expression's supplying
            // declaration, not the runtime frame supplying its actual values.
            if !engine.selected_binary_meaning(expression, binary) {
                return None;
            }
            let left = formed_value(engine, program, binary.left, depth + 1)?;
            let right = formed_value(engine, program, binary.right, depth + 1)?;
            let references =
                [binary.left, binary.right].map(|operand| engine.source_result_type(operand));
            let primitives = references
                .map(|reference| reference.and_then(|r| program.primitive_type_reference(r)));
            let left_primitive = primitives[0].or(primitives[1])?;
            let right_primitive = primitives[1].unwrap_or(left_primitive);
            let policies = references.map(|reference| {
                reference.map(|r| program.arithmetic_domain_for_type_reference(r))
            });
            let left_policy = policies[0]
                .or(policies[1])
                .unwrap_or(ArithmeticDomain::Exact);
            let right_policy = policies[1].unwrap_or(left_policy);
            engine.admit_selected_integer_operation(
                binary.operator,
                [
                    (left_primitive, left_policy),
                    (right_primitive, right_policy),
                ],
                &left,
                &right,
                None,
            )
        }
        _ => {
            // Captured leaves are read under the installed roster. Unknown
            // executable parents do not acquire formation by normalization.
            let mut has_children = false;
            crate::expression_traversal::executable_children(
                program,
                program.expression_table.expression(expression),
                |_| has_children = true,
            );
            if has_children {
                return None;
            }
            engine.normalize(expression)
        }
    }
}

// A declaration's input and output bounds constrain different values. Prove
// both under the actual roster; neither constraint is installed as a premise.
fn formed_type(
    engine: &mut Engine<'_>,
    program: &dyn crate::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
    value: &Polynomial,
) -> bool {
    use numerics::bignum::BigInt;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            let TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
            else {
                return false;
            };
            let Some(minimum) = formed_value(engine, program, *minimum, 0) else {
                return false;
            };
            let Some(maximum) = formed_value(engine, program, *maximum, 0) else {
                return false;
            };
            if !engine.prove_at_least(&value.sub(&minimum), &BigInt::zero())
                || !engine.prove_at_least(
                    &maximum.sub(value),
                    &BigInt::from_i64(if *end_inclusive { 0 } else { 1 }),
                )
            {
                return false;
            }
        }
        reference = *base_type;
    }
    // Qualification kinds outside this selected unsigned scalar-view owner
    // require their own application judgment; erasing them is not formation.
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    program
        .symbols
        .builtin_type_atom(*symbol)
        .and_then(PrimitiveType::from_builtin_atom)
        .is_some_and(|primitive| engine.integer_value_lands(primitive, value))
}
