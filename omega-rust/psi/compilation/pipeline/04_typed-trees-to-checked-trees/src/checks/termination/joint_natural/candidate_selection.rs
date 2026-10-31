//! Select independently observed candidate coordinates for every participant.

use super::{
    coordinates::{Bound, Rank, coordinates},
    ranges,
    views::Order,
};
use crate::checks::current_scalar_values::CurrentScalarValues;
use typed_trees::expression::BinaryOperator;
use validation::ScopedArithmeticValue;

pub(super) struct Candidates {
    pub(super) ranks: Vec<Vec<Rank>>,
    pub(super) order: Order,
    pub(super) field_paths: Vec<Vec<facts::PlaceSegment>>,
}

pub(super) fn prepare(
    program: &dyn validation::CheckingInput,
    machines: &[&typed_trees::machine::Machine],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Result<Candidates, &'static str>> {
    let mut authored = Vec::with_capacity(machines.len());
    for machine in machines {
        let [entry] = program.machine_states(machine) else {
            return None;
        };
        authored.push(
            if machine.termination_plan.implementation_witness.is_some() {
                Some(super::views::select(program, machine, entry)?)
            } else {
                None
            },
        );
    }
    let mut field_paths = Vec::new();
    for machine in machines {
        for state in program.machine_states(machine) {
            for path in crate::checks::current_scalar_values::parameter_projection_paths(
                program, machine, state,
            ) {
                if !field_paths.contains(&path) {
                    field_paths.push(path);
                }
            }
        }
    }
    // Projection requirements are produced by the selected views, not
    // rediscovered only when a body happens to spell the same member access.
    for view in authored.iter().flatten() {
        if !view.coordinate.path.is_empty() && !field_paths.contains(&view.coordinate.path) {
            field_paths.push(view.coordinate.path.clone());
        }
    }
    let mut candidates = Vec::new();
    let mut selected_order = None;
    let mut available = Vec::new();
    let mut entry_values = Vec::new();
    for (machine, view) in machines.iter().zip(&authored) {
        // Source declarations have one entry. Legacy multi-state arrival
        // telescopes remain with the ordinary full-coordinate owner.
        let [entry] = program.machine_states(machine) else {
            return None;
        };
        let execution = program
            .statement_table
            .admitted_execution_order(entry.statement_nodes);
        let first = *execution.indices().first()?;
        let values = CurrentScalarValues::at_observing_projections(
            program,
            machine,
            entry,
            first,
            Default::default(),
            frames,
            &field_paths,
        )?;
        available.push(coordinates(program, entry, &values));
        entry_values.push(values);
        let Some(view) = view else {
            candidates.push(Vec::new());
            continue;
        };
        let order = view.order;
        let values = entry_values.last()?;
        let selected = available
            .last()?
            .iter()
            .find(|coordinate| {
                coordinate.parameter == view.coordinate.parameter
                    && values.same_scalar_projection(&coordinate.path, &view.coordinate.path)
                    && coordinate.observation == view.observation
            })?
            .clone();
        let carrier = selected.primitive;
        if selected_order.is_some_and(|previous| previous != (order, carrier, view.declaration)) {
            return None;
        }
        if order != Order::Descending && order.function().complement_ceiling(carrier).is_none() {
            return None;
        }
        selected_order = Some((order, carrier, view.declaration));
        if let Err(reason) = ranges::entry(program, machine, view.measure) {
            return Some(Err(reason));
        }
        let bound = if let Some(expression) = view.upper {
            let value = values.current_value(expression)?;
            if order == Order::BoundedDistance
                && !values.proves_values_relation(
                    program,
                    machine,
                    BinaryOperator::LessOrEqual,
                    &selected.entry_value,
                    &value,
                )
            {
                return Some(Err(
                    "a bounded cursor rank lacks independent entry formation",
                ));
            }
            Some(Bound::Authored {
                expression,
                entry_value: value,
            })
        } else {
            None
        };
        candidates.push(vec![Rank {
            coordinate: selected,
            bound,
        }]);
    }
    let (order, primitive, _) = selected_order?;
    let closed_bounds = candidates
        .iter()
        .flatten()
        .filter_map(|rank| rank.bound.as_ref())
        .filter_map(|bound| match bound.entry() {
            value @ ScopedArithmeticValue::Integer(_) => Some(value.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for (position, (machine, candidates)) in machines.iter().zip(&mut candidates).enumerate() {
        if !candidates.is_empty() {
            continue;
        }
        // Private correspondence changes neither public schemas nor source
        // signatures. Only exact runtime leaves in this invocation participate.
        if machine.is_public {
            return None;
        }
        for coordinate in available[position]
            .iter()
            .filter(|coordinate| coordinate.primitive == primitive)
        {
            if order == Order::Descending {
                candidates.push(Rank {
                    coordinate: coordinate.clone(),
                    bound: None,
                });
                continue;
            }
            let bounds = available[position]
                .iter()
                .cloned()
                .map(Bound::Coordinate)
                .chain(closed_bounds.iter().cloned().map(Bound::Closed));
            for bound in bounds {
                if order == Order::BoundedDistance
                    && !entry_values[position].proves_values_relation(
                        program,
                        machine,
                        BinaryOperator::LessOrEqual,
                        &coordinate.entry_value,
                        bound.entry(),
                    )
                {
                    continue;
                }
                candidates.push(Rank {
                    coordinate: coordinate.clone(),
                    bound: Some(bound),
                });
            }
        }
        if candidates.is_empty() {
            return Some(Err(
                "a private member has no compatible natural rank coordinate",
            ));
        }
    }
    Some(Ok(Candidates {
        ranks: candidates,
        order,
        field_paths,
    }))
}
