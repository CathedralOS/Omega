//! Integer contradictions over the checker's current value observations.
//!
//! This adapter preserves the ordered-value owner's resolved identities and
//! mutation lifetime. It supplies mathematical comparisons to the shared
//! integer judgment; it neither replays source guards nor establishes the
//! formation of operand arithmetic. Callers must check those operands first.

use super::ordered_values::{Operand, RelationConstraint};
use super::{ArithmeticDomain, BinaryOperator, ExpressionHandle, Machine, State, ValueEnvironment};
use crate::proof_contracts::contract_entailment::{
    ScopedArithmeticComparison, ScopedArithmeticValue, scoped_arithmetic_comparisons_unsatisfiable,
};
use numerics::bignum::BigInt;

/// Substitute the exact evaluated actual operands into a callee condition.
/// Keep the caller's complete live relation graph: rebinding only delivered
/// roots would discard intermediate values needed for captured-view geometry.
/// The source condition still owns selected builtin meaning and integer types.
pub(super) fn proves_condition(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
    bindings: &[(Operand, Operand)],
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return program.expression_table.boolean_node(expression) == Some(&true);
    };
    if !crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning(
        program,
        machine,
        Some(state),
        expression,
    ) {
        return false;
    }
    match binary.operator {
        BinaryOperator::And => {
            return proves_condition(program, machine, state, environment, binary.left, bindings)
                && proves_condition(program, machine, state, environment, binary.right, bindings);
        }
        BinaryOperator::Or => {
            return proves_condition(program, machine, state, environment, binary.left, bindings)
                || proves_condition(program, machine, state, environment, binary.right, bindings);
        }
        _ => {}
    }
    let Some(operator) = binary.operator.complement() else {
        return false;
    };
    // Names and fields are storage observations for more than integers. An
    // IEEE comparison cannot inherit integer complement/order laws.
    let integer = |value| {
        program.expression_table.integer_node(value).is_some()
            // Builtin descriptor counts are u64 observations, not nominal
            // fields with declaration type handles.
            || crate::collection_length_receiver(program, machine, Some(state), value).is_some()
            || super::declared_place_type_raw(program, machine, Some(state), value)
                .and_then(|reference| program.primitive_type_reference(reference))
                .is_some_and(|primitive| {
                    matches!(
                        primitive,
                        typed_trees::types::PrimitiveType::I8
                            | typed_trees::types::PrimitiveType::I16
                            | typed_trees::types::PrimitiveType::I32
                            | typed_trees::types::PrimitiveType::I64
                            | typed_trees::types::PrimitiveType::U8
                            | typed_trees::types::PrimitiveType::U16
                            | typed_trees::types::PrimitiveType::U32
                            | typed_trees::types::PrimitiveType::U64
                    )
                })
    };
    if !integer(binary.left) || !integer(binary.right) {
        return false;
    }
    let (Some(left), Some(right)) = (
        super::ordered_values::operand(program, machine, state, binary.left, None),
        super::ordered_values::operand(program, machine, state, binary.right, None),
    ) else {
        return false;
    };
    let (left, right) = if bindings.is_empty() {
        (left, right)
    } else {
        let rebound_left = left.rebound(bindings);
        let rebound_right = right.rebound(bindings);
        let ([left], [right]) = (rebound_left.as_slice(), rebound_right.as_slice()) else {
            return false;
        };
        (left.clone(), right.clone())
    };
    // The ordered operand owner admits fixed-integer observations only.
    let mut observations = Observations::default();
    let (Some(left), Some(right)) = (observations.value(&left), observations.value(&right)) else {
        return false;
    };
    contradicts(
        program,
        machine,
        environment,
        observations,
        vec![ScopedArithmeticComparison {
            operator,
            left,
            right,
        }],
    )
}

/// Whether the current facts exclude this simultaneous assignment of values.
/// Each expression denotes its current observation, not an earlier source
/// occurrence which merely has the same spelling.
pub(super) fn excludes_values(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    assignments: &[(ExpressionHandle, i64)],
) -> bool {
    let Some(state) = state else {
        return false;
    };
    let mut observations = Observations::default();
    let mut comparisons = Vec::new();
    for &(expression, value) in assignments {
        let Some(operand) =
            super::ordered_values::operand(program, machine, state, expression, None)
        else {
            return false;
        };
        let Some(left) = observations.value(&operand) else {
            return false;
        };
        comparisons.push(ScopedArithmeticComparison {
            operator: BinaryOperator::Equal,
            left,
            right: integer(value),
        });
    }
    contradicts(program, machine, environment, observations, comparisons)
}

/// Prove a requested closed range from the same live observation relations.
/// The two excluded complements are independent questions to the existing
/// integer judgment; neither the requested bound nor a cast's target range is
/// installed as a premise.
pub(super) fn proves_range(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
    minimum: Option<&BigInt>,
    maximum: Option<&BigInt>,
) -> bool {
    // No relational premise can improve the caller's ordinary interval
    // result. This also keeps operand meaning's empty-environment carrier
    // query independent of this live-fact judgment.
    if environment.ordered_values.is_empty() {
        return false;
    }
    let Some(operand) = super::ordered_values::operand(program, machine, state, expression, None)
    else {
        return false;
    };
    [
        (BinaryOperator::Less, minimum),
        (BinaryOperator::Greater, maximum),
    ]
    .into_iter()
    .filter_map(|(operator, bound)| bound.map(|bound| (operator, bound)))
    .all(|(operator, bound)| {
        let mut observations = Observations::default();
        let Some(left) = observations.value(&operand) else {
            return false;
        };
        contradicts(
            program,
            machine,
            environment,
            observations,
            vec![ScopedArithmeticComparison {
                operator,
                left,
                right: ScopedArithmeticValue::Integer(bound.clone()),
            }],
        )
    })
}

/// A fixed-width observation consumes collection elements, not a guessed
/// descriptor capacity. Form the endpoint in mathematical integers so this
/// proof does not introduce a runtime addition or assume its lack of overflow.
pub(super) fn proves_collection_footprint(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    environment: &ValueEnvironment,
    collection: ExpressionHandle,
    start: ExpressionHandle,
    width: i64,
) -> bool {
    if width < 0 {
        return false;
    }
    let Some(start) = super::ordered_values::operand(program, machine, state, start, None) else {
        return false;
    };
    let mut observations = Observations::default();
    let (Some(length), Some(start)) = (
        collection_length(program, machine, state, collection, &mut observations),
        observations.value(&start),
    ) else {
        return false;
    };
    let end = ScopedArithmeticValue::IntegerOperation {
        operator: BinaryOperator::Add,
        left: Box::new(start.clone()),
        right: Box::new(integer(width)),
    };
    [
        (BinaryOperator::Less, start, integer(0)),
        (BinaryOperator::Greater, end, length),
    ]
    .into_iter()
    .all(|(operator, left, right)| {
        contradicts(
            program,
            machine,
            environment,
            observations.clone(),
            vec![ScopedArithmeticComparison {
                operator,
                left,
                right,
            }],
        )
    })
}

/// Use the ordered observation owner's length denotation for both the goal
/// and its supporting guard relations; neither may fold geometry separately.
fn collection_length(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    collection: ExpressionHandle,
    observations: &mut Observations,
) -> Option<ScopedArithmeticValue> {
    let observed = super::ordered_values::operand(program, machine, state, collection, None)?;
    let length = super::ordered_values::collection_length_operand(
        program, machine, state, collection, observed,
    )?;
    observations.value(&length)
}

fn contradicts(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    environment: &ValueEnvironment,
    mut observations: Observations,
    mut comparisons: Vec<ScopedArithmeticComparison>,
) -> bool {
    for (position, relation) in environment.ordered_values.iter().enumerate() {
        // The live owner stores equality as two opposite non-strict orders
        // so its edge projection and invalidation have one relation form.
        // Present that equality once to the polynomial substitution owner;
        // independent inequalities would hide a captured composite value.
        let equality = (relation.constraint == RelationConstraint::MinimumDifference(0))
            .then(|| {
                environment.ordered_values.iter().position(|opposite| {
                    opposite.constraint == RelationConstraint::MinimumDifference(0)
                        && opposite.left == relation.right
                        && opposite.right == relation.left
                })
            })
            .flatten();
        if equality.is_some_and(|opposite| opposite < position) {
            continue;
        }
        let (Some(left), Some(right)) = (
            observations.value(&relation.left),
            observations.value(&relation.right),
        ) else {
            continue;
        };
        let (operator, right) = match relation.constraint {
            RelationConstraint::Unequal => (BinaryOperator::NotEqual, right),
            RelationConstraint::MinimumDifference(0) if equality.is_some() => {
                (BinaryOperator::Equal, right)
            }
            RelationConstraint::MinimumDifference(floor) => (
                BinaryOperator::GreaterOrEqual,
                ScopedArithmeticValue::IntegerOperation {
                    operator: BinaryOperator::Add,
                    left: Box::new(right),
                    right: Box::new(integer(floor)),
                },
            ),
        };
        comparisons.push(ScopedArithmeticComparison {
            operator,
            left,
            right,
        });
    }
    // Retain every opaque observation before relating congruent constructors.
    // A copied result may be hoisted: hoist == f(copy), copy == original.
    // Deduplicating f(copy) into hoist while building the atoms would erase
    // the bridge to f(original). Publish proved equalities after all atoms
    // exist instead. Carrier/policy and child identity remain the shared
    // congruence owner's judgment; Wrapping is never expanded as Exact math.
    let mut comparison = super::ordered_values::ValueComparison::new(&environment.ordered_values);
    for (position, operand) in observations.atoms.iter().enumerate() {
        for (other_position, other) in observations.atoms[..position].iter().enumerate() {
            if comparison.same_value(operand, other) {
                comparisons.push(ScopedArithmeticComparison {
                    operator: BinaryOperator::Equal,
                    left: atom(position),
                    right: atom(other_position),
                });
            }
        }
    }
    // Only paths belonging to resolved current observations can contribute
    // interval facts. The environment already owns their invalidation and
    // all-predecessor joins; unrelated same-spelled source expressions are not
    // invented to turn an interval into a symbol binding.
    for (position, operand) in observations.atoms.iter().enumerate() {
        // An observed integer always fits its own carrier, including u64's
        // unsigned half outside the interval adapter's i64 window. Resolve
        // storage through declaration handles, not its diagnostic path. Exact
        // arithmetic was expanded above and cannot assume its result fits.
        let primitive = match operand {
            Operand::Place { root, .. } => {
                crate::value_custody::places::declaration_type_reference(program, *root)
                    .and_then(|reference| program.primitive_type_reference(reference))
            }
            Operand::CollectionLength(_) => Some(typed_trees::types::PrimitiveType::U64),
            Operand::Indexed { primitive, .. } | Operand::Field { primitive, .. } => *primitive,
            Operand::Binary { primitive, .. } => *primitive,
            _ => None,
        };
        if let Some((minimum, maximum)) = primitive
            .filter(|primitive| primitive.is_fixed_integer())
            .and_then(|primitive| primitive.integer_bounds())
        {
            for (operator, bound) in [
                (BinaryOperator::GreaterOrEqual, minimum),
                (BinaryOperator::LessOrEqual, maximum),
            ] {
                comparisons.push(ScopedArithmeticComparison {
                    operator,
                    left: atom(position),
                    right: ScopedArithmeticValue::Integer(BigInt::from_i128(bound)),
                });
            }
        }
        let Some(path) = operand.static_path() else {
            continue;
        };
        let Some(interval) = environment.intervals.get(&path) else {
            continue;
        };
        for (operator, bound) in [
            (BinaryOperator::GreaterOrEqual, interval.low()),
            (BinaryOperator::LessOrEqual, interval.high()),
        ] {
            if let Some(bound) = bound {
                comparisons.push(ScopedArithmeticComparison {
                    operator,
                    left: atom(position),
                    right: integer(bound),
                });
            }
        }
    }
    scoped_arithmetic_comparisons_unsatisfiable(program, machine, &comparisons)
}

#[derive(Clone, Default)]
struct Observations {
    atoms: Vec<Operand>,
}

impl Observations {
    fn value(&mut self, operand: &Operand) -> Option<ScopedArithmeticValue> {
        match operand {
            Operand::Integer(literal, _) => {
                literal.value_bignum().map(ScopedArithmeticValue::Integer)
            }
            Operand::Binary {
                operator,
                domain: ArithmeticDomain::Exact,
                operands,
                ..
            } if matches!(
                operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
            ) =>
            {
                let [left, right] = operands.as_slice() else {
                    return None;
                };
                Some(ScopedArithmeticValue::IntegerOperation {
                    operator: *operator,
                    left: Box::new(self.value(left)?),
                    right: Box::new(self.value(right)?),
                })
            }
            // An opaque admitted integer observation can share identity with
            // another read, but its implementation is not mathematical Exact
            // arithmetic (notably Wrapping and Saturating operations).
            _ => {
                let position = self
                    .atoms
                    .iter()
                    .position(|candidate| candidate == operand)
                    .unwrap_or_else(|| {
                        self.atoms.push(operand.clone());
                        self.atoms.len() - 1
                    });
                Some(atom(position))
            }
        }
    }
}

fn atom(position: usize) -> ScopedArithmeticValue {
    ScopedArithmeticValue::Atom {
        identity: format!("observation:{position}"),
        unsigned: false,
    }
}

fn integer(value: i64) -> ScopedArithmeticValue {
    ScopedArithmeticValue::Integer(BigInt::from_i64(value))
}
