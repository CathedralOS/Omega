//! One declared integer application, shared by source rank admission and
//! captured-value queries. Every child and qualification forms before the
//! complete produced value and its proven carrier facts may escape.

use super::{Engine, Polynomial};
use crate::frozen_program::expression_table;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::PrimitiveType;

/// Child decisions from one completed declared application. This is not an
/// unconditional theorem about the declaration: its consumer must retain the
/// exact state and supplying input bindings alongside this result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerApplicationFormation {
    measure: symbols::SymbolHandle,
    body: ExpressionHandle,
    casts: Vec<crate::ExactIntegerCastFact>,
}

impl IntegerApplicationFormation {
    pub fn measure(&self) -> symbols::SymbolHandle {
        self.measure
    }

    pub fn body(&self) -> ExpressionHandle {
        self.body
    }

    pub fn casts(&self) -> &[crate::ExactIntegerCastFact] {
        &self.casts
    }

    /// Rehydrate decisions already retained by checking, not a new proof.
    /// The enclosing runtime row and executable application authenticate the
    /// state, declaration and complete coordinate bindings separately.
    pub(crate) fn from_retained(
        measure: symbols::SymbolHandle,
        body: ExpressionHandle,
        casts: Vec<crate::ExactIntegerCastFact>,
    ) -> Option<Self> {
        if !measure.is_valid() || !body.is_valid() {
            return None;
        }
        for cast in &casts {
            let (source_min, source_max) = cast.source_type.integer_bounds()?;
            let (target_min, target_max) = cast.target_type.integer_bounds()?;
            if !cast.expression.is_valid()
                || cast.minimum > cast.maximum
                || cast.minimum < super::super::BigInt::from_i128(source_min.max(target_min))
                || cast.maximum > super::super::BigInt::from_i128(source_max.min(target_max))
            {
                return None;
            }
        }
        Some(Self {
            measure,
            body,
            casts,
        })
    }
}

pub(in crate::proof_contracts::contract_entailment) struct FormedIntegerApplication {
    pub value: Polynomial,
    pub formation: Option<IntegerApplicationFormation>,
}

impl Engine<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn form_declared_integer_application(
        &mut self,
        measure: symbols::SymbolHandle,
        input: &Polynomial,
    ) -> Option<FormedIntegerApplication> {
        let program = self.program;
        let declaration = program
            .measures()
            .iter()
            .find(|definition| definition.symbol == measure)?;
        let formal = declaration.parameter.as_ref()?;
        if !formal.symbol.is_valid() || program.symbols.get(formal.symbol).parent != measure {
            return None;
        }
        let [body] = expression_table(program).expression_handles(declaration.body) else {
            return None;
        };
        let primitive = program.primitive_type_reference(declaration.return_type)?;
        let (minimum, maximum) = primitive.integer_bounds()?;
        let mut admitted = self.clone();
        let mut casts = Vec::new();
        let value = admitted.with_formal_arguments(
            measure,
            &[(formal.symbol, input.clone())],
            |engine| {
                if !formed_type(engine, program, formal.type_reference, input, &mut casts) {
                    return None;
                }
                let value = formed_value(engine, program, *body, 0, &mut casts)?;
                formed_type(engine, program, declaration.return_type, &value, &mut casts)
                    .then_some(value)
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
        // An inconsistent path can establish a vacuous proposition, but it
        // supplies no concrete application decision for executable lowering.
        let formation = (!admitted.requires_unsatisfiable).then_some(IntegerApplicationFormation {
            measure,
            body: *body,
            casts,
        });
        *self = admitted;
        Some(FormedIntegerApplication { value, formation })
    }
}

fn formed_value(
    engine: &mut Engine<'_>,
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    depth: usize,
    casts: &mut Vec<crate::ExactIntegerCastFact>,
) -> Option<Polynomial> {
    if depth >= 128 {
        return None;
    }
    // Wholly anonymous integers have unbounded intermediates until landing.
    if super::super::proof_integer::anonymous_integer_value(program, expression).is_some() {
        return engine.normalize(expression);
    }
    match expression_table(program).expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            formed_value(engine, program, atomic.value, depth + 1, casts)
        }
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::BitwiseNot => {
            let operand = formed_value(engine, program, unary.operand, depth + 1, casts)?;
            let reference = engine.source_result_type(expression)?;
            let primitive = program.primitive_type_reference(reference)?;
            engine.admit_integer_complement(primitive, &operand)
        }
        ExpressionNode::Cast(cast)
            if !cast.form.is_recast()
                && cast.domain == ArithmeticDomain::Exact
                && cast.semantic_domain.is_empty() =>
        {
            let value = formed_value(engine, program, cast.value, depth + 1, casts)?;
            if !formed_type(engine, program, cast.target_type, &value, casts) {
                return None;
            }
            let target = program.primitive_type_reference(cast.target_type)?;
            let source =
                if super::super::proof_integer::anonymous_integer_value(program, cast.value)
                    .is_some()
                {
                    Some(target)
                } else {
                    engine
                        .source_result_type(cast.value)
                        .and_then(|reference| program.primitive_type_reference(reference))
                };
            if let Some(source) = source
                && let (Some((source_min, source_max)), Some((target_min, target_max))) =
                    (source.integer_bounds(), target.integer_bounds())
            {
                // Both the child's actual carrier and target landing were
                // proved above. This conservative intersection cannot assert
                // stronger bounds than those independent decisions supply.
                let fact = crate::ExactIntegerCastFact {
                    expression,
                    source_type: source,
                    target_type: target,
                    minimum: super::super::BigInt::from_i128(source_min.max(target_min)),
                    maximum: super::super::BigInt::from_i128(source_max.min(target_max)),
                };
                if !casts.contains(&fact) {
                    casts.push(fact);
                }
            }
            Some(value)
        }
        ExpressionNode::Binary(binary) => {
            // Selection is authenticated in the expression's supplying
            // declaration, not the runtime frame supplying its actual values.
            if !engine.selected_binary_meaning(expression, binary) {
                return None;
            }
            let left = formed_value(engine, program, binary.left, depth + 1, casts)?;
            let right = formed_value(engine, program, binary.right, depth + 1, casts)?;
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
                expression_table(program).expression(expression),
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
    casts: &mut Vec<crate::ExactIntegerCastFact>,
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
            let Some(minimum) = formed_value(engine, program, *minimum, 0, casts) else {
                return false;
            };
            let Some(maximum) = formed_value(engine, program, *maximum, 0, casts) else {
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
