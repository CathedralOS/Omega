//! Select independently observed candidate coordinates for every participant.

use super::{
    coordinates::{Bound, Rank},
    ranges,
    views::Order,
};
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
        let prepared =
            super::super::rank_inputs::at_entry(program, machine, entry, frames, &field_paths)?;
        available.push(prepared.coordinates);
        entry_values.push(prepared.values);
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
        if let Err(reason) = ranges::entry(program, machine, view.measure, frames) {
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
        let (entry_value, formation) = if let Some(application) = view.application {
            if application.primitive != carrier {
                return Some(Err(
                    "a computed natural rank has incompatible input and result carriers",
                ));
            }
            let Some(value) = application.observe(values, machine, selected.entry_value.clone())
            else {
                return Some(Err(
                    "a computed natural rank application has no independent entry formation",
                ));
            };
            (value.value, Some(value.formation))
        } else {
            (selected.entry_value.clone(), None)
        };
        candidates.push(vec![Rank {
            coordinate: selected,
            bound,
            application: view.application,
            entry_value,
            formation,
        }]);
    }
    let (order, primitive, _) = selected_order?;
    let application = authored.iter().flatten().find_map(|view| view.application);
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
                let (entry_value, formation) = if let Some(application) = application {
                    let Some(value) = application.observe(
                        &entry_values[position],
                        machine,
                        coordinate.entry_value.clone(),
                    ) else {
                        continue;
                    };
                    (value.value, Some(value.formation))
                } else {
                    (coordinate.entry_value.clone(), None)
                };
                candidates.push(Rank {
                    coordinate: coordinate.clone(),
                    bound: None,
                    application,
                    entry_value,
                    formation,
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
                    application: None,
                    entry_value: coordinate.entry_value.clone(),
                    formation: None,
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
