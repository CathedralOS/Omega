//! Complete actual argument observations and exact edge comparisons.

use super::{coordinates::Rank, views::Order};
use crate::checks::current_scalar_values::CurrentScalarValues;
use crate::flow::indexed_statements;
use numerics::bignum::BigInt;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use validation::ScopedArithmeticValue;

pub(super) struct Edge<'program> {
    pub(super) source: usize,
    pub(super) destination: usize,
    pub(super) statement: usize,
    pub(super) arguments: &'program [ExpressionHandle],
    pub(super) site: validation::CallSite<'program>,
    pub(super) values: CurrentScalarValues<'program>,
}

pub(super) fn collect<'program>(
    program: &'program dyn validation::CheckingInput,
    machines: &[&'program typed_trees::machine::Machine],
    frames: Option<&validation::CallFrameResolver<'program>>,
    field_paths: &[Vec<facts::PlaceSegment>],
) -> Result<Vec<Edge<'program>>, &'static str> {
    let mut edges = Vec::new();
    for (source, machine) in machines.iter().enumerate() {
        let entry = &program.machine_states(machine)[0];
        for (statement, _) in indexed_statements(program, entry.statement_nodes) {
            for (ordinal, call) in
                validation::collect_call_sites_in_statement(program, machine, entry, statement)
                    .into_iter()
                    .enumerate()
            {
                let target = call.target_symbol(program);
                let Some(destination) = machines.iter().position(|machine| {
                    target == machine.symbol || target == program.machine_states(machine)[0].symbol
                }) else {
                    continue;
                };
                let Some(arrival) =
                    validation::transition_call_target(program, machine, entry, statement, ordinal)
                else {
                    return Err("a recurrent natural call has no exact terminal arrival");
                };
                if !call.completes_transition(program, arrival) {
                    return Err("a recurrent natural call is not the whole terminal result");
                }
                let arguments = validation::call_site_argument_expressions(program, &call);
                if arguments.iter().any(|argument| {
                    !frames.is_some_and(|frames| {
                        frames
                            .expression_write_frame(machine, *argument)
                            .into_complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                    })
                }) {
                    return Err("natural rank operands have an unpreserved evaluation prefix");
                }
                let Some(values) = CurrentScalarValues::at_observing_projections(
                    program,
                    machine,
                    entry,
                    statement,
                    arrival,
                    frames,
                    field_paths,
                ) else {
                    return Err("a natural call has no complete current-value observation");
                };
                edges.push(Edge {
                    source,
                    destination,
                    statement,
                    arguments,
                    site: call,
                    values,
                });
            }
        }
    }
    Ok(edges)
}

pub(super) fn compare(
    program: &dyn validation::CheckingInput,
    machines: &[&typed_trees::machine::Machine],
    edge: &Edge<'_>,
    source: &Rank,
    destination: &Rank,
    order: Order,
) -> Option<bool> {
    let actual = *edge.arguments.get(destination.coordinate.position)?;
    let initial = &source.entry_value;
    let next = destination.coordinate.current(
        &edge.values,
        actual,
        machines[edge.destination],
        edge.arguments,
    )?;
    let next = if let Some(application) = destination.application {
        application
            .observe(&edge.values, machines[edge.source], next)?
            .value
    } else {
        next
    };
    let proves = |operator, left: &ScopedArithmeticValue, right: &ScopedArithmeticValue| {
        edge.values
            .proves_values_relation(program, machines[edge.source], operator, left, right)
    };
    let (initial, next, strict_allowed) = match order {
        Order::Descending => (initial.clone(), next, true),
        Order::BoundedDistance => {
            let upper = source.bound.as_ref()?.entry();
            let next_upper =
                destination
                    .bound
                    .as_ref()?
                    .arriving(program, machines[edge.destination], edge)?;
            // Each exact distance forms in its own invocation. Neither a
            // pending comparison nor the other endpoint supplies this order.
            if !proves(BinaryOperator::LessOrEqual, initial, upper)
                || !proves(BinaryOperator::LessOrEqual, &next, &next_upper)
            {
                return None;
            }
            let difference = |upper: &ScopedArithmeticValue, lower: &ScopedArithmeticValue| {
                ScopedArithmeticValue::IntegerOperation {
                    operator: BinaryOperator::Subtract,
                    left: Box::new(upper.clone()),
                    right: Box::new(lower.clone()),
                }
            };
            (
                difference(upper, initial),
                difference(&next_upper, &next),
                true,
            )
        }
        Order::IncreasingTo => {
            let bound = source.bound.as_ref()?.entry();
            let arriving =
                destination
                    .bound
                    .as_ref()?
                    .arriving(program, machines[edge.destination], edge)?;
            // The named limit belongs to this view. Its existing total
            // complement conservatively refines clamped edges; only an entry
            // below that conserved limit can contribute strict descent.
            if !proves(BinaryOperator::Equal, bound, &arriving) {
                return None;
            }
            let normalize = |value: &ScopedArithmeticValue| {
                let (minimum, maximum) = source.coordinate.primitive.integer_bounds()?;
                let value = ScopedArithmeticValue::BoundedInteger {
                    value: Box::new(value.clone()),
                    minimum: Some(BigInt::from_i128(minimum)),
                    maximum: Some(BigInt::from_i128(maximum)),
                };
                Some(ScopedArithmeticValue::IntegerOperation {
                    operator: BinaryOperator::Subtract,
                    left: Box::new(ScopedArithmeticValue::Integer(BigInt::from_u64(
                        order
                            .function()
                            .complement_ceiling(source.coordinate.primitive)?,
                    ))),
                    right: Box::new(value),
                })
            };
            (
                normalize(initial)?,
                normalize(&next)?,
                proves(BinaryOperator::Less, initial, bound),
            )
        }
    };
    // An explicit declaration's result is a finite integer, including its
    // negative half. Builtin distance normalizations keep their zero floor.
    let minimum = |rank: &Rank| -> Option<ScopedArithmeticValue> {
        let minimum = if order == Order::Descending {
            match rank.application {
                Some(application) => application.result_primitive.integer_bounds()?.0,
                None => 0,
            }
        } else {
            0
        };
        Some(ScopedArithmeticValue::Integer(BigInt::from_i128(minimum)))
    };
    let source_minimum = minimum(source)?;
    let destination_minimum = minimum(destination)?;
    (proves(BinaryOperator::GreaterOrEqual, &initial, &source_minimum)
        && proves(BinaryOperator::GreaterOrEqual, &next, &destination_minimum)
        && proves(BinaryOperator::LessOrEqual, &next, &initial))
    .then(|| strict_allowed && proves(BinaryOperator::Less, &next, &initial))
}
