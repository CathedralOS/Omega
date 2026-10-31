//! One declared integer application, shared by source rank admission and
//! captured-value queries. Every child and qualification forms before the
//! complete produced value and its proven carrier facts may escape.

use super::{Engine, Polynomial};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;

impl Engine<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn form_declared_integer_application(
        &mut self,
        measure: symbols::SymbolHandle,
        input: &Polynomial,
    ) -> Option<Polynomial> {
        let program = self.program;
        let declaration = program
            .measures()
            .iter()
            .find(|definition| definition.symbol == measure)?;
        let formal = declaration.parameter.as_ref()?;
        if !formal.symbol.is_valid() || program.symbols.get(formal.symbol).parent != measure {
            return None;
        }
        let [body] = program
            .expression_table
            .expression_handles(declaration.body)
        else {
            return None;
        };
        let primitive = program.primitive_type_reference(declaration.return_type)?;
        let (minimum, maximum) = primitive.integer_bounds()?;
        let mut admitted = self.clone();
        let value = admitted.with_formal_arguments(
            measure,
            &[(formal.symbol, input.clone())],
            |engine| {
                if !formed_type(engine, program, formal.type_reference, input) {
                    return None;
                }
                let value = formed_value(engine, program, *body, 0)?;
                formed_type(engine, program, declaration.return_type, &value).then_some(value)
            },
        )?;
        // These are consequences of the completed application, never premises
        // for a child operation or the application's own result formation.
        for bound in [
            (value.clone(), super::super::BigInt::from_i128(minimum)),
            (
                value.neg(),
                super::super::BigInt::from_i128(maximum).negate(),
            ),
        ] {
            if !admitted.bounds.contains(&bound) {
                admitted.bounds.push(bound);
            }
        }
        admitted.refresh_difference_bounds();
        *self = admitted;
        Some(value)
    }
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
        ExpressionNode::Cast(cast)
            if !cast.form.is_recast()
                && cast.domain == ArithmeticDomain::Exact
                && cast.semantic_domain.is_empty() =>
        {
            let value = formed_value(engine, program, cast.value, depth + 1)?;
            formed_type(engine, program, cast.target_type, &value).then_some(value)
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
