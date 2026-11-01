//! Select independently observed inputs for every participant. Declared views
//! select a whole typed root before capturing its complete scalar-read roster.

use super::{
    applications::Application,
    coordinates::{Bound, Rank},
    ranges,
    subjects::Subject,
    views::Order,
};
use crate::checks::current_scalar_values::{CurrentScalarValues, parameter_projection_paths};
use typed_trees::{expression::BinaryOperator, machine::Machine, state::State};
use validation::{RankSubjectCoordinate, ScopedArithmeticValue};

pub(super) struct Candidates {
    pub(super) ranks: Vec<Vec<Rank>>,
    pub(super) order: Order,
    pub(super) field_paths: Vec<Vec<facts::PlaceSegment>>,
}

/// Candidate roots preserve nominal/generic identity. Paths are prefixes of
/// actual projections already demanded by this component, never independently
/// selected leaves from unrelated parameters.
fn roots(
    program: &dyn validation::CheckingInput,
    state: &State,
    application: Application,
    paths: &[Vec<facts::PlaceSegment>],
) -> Vec<RankSubjectCoordinate> {
    let mut prefixes = vec![Vec::new()];
    for path in paths {
        for length in 1..=path.len() {
            let prefix = path[..length].to_vec();
            if !prefixes.contains(&prefix) {
                prefixes.push(prefix);
            }
        }
    }
    let mut roots = Vec::new();
    for parameter in program.state_parameters(state) {
        if parameter.is_const || parameter.relevance.is_erased() {
            continue;
        }
        for prefix in &prefixes {
            if validation::project_type_reference_from_segments(
                program,
                parameter.type_reference,
                prefix,
            )
            .is_some_and(|actual| {
                validation::declared_input_type_matches(program, actual, application.input_type)
            }) {
                roots.push(RankSubjectCoordinate {
                    parameter: parameter.symbol,
                    path: prefix.clone(),
                });
            }
        }
    }
    roots
}

fn observe(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    values: &CurrentScalarValues<'_>,
    subject: Subject,
    application: Option<Application>,
    bound: Option<Bound>,
    minimum: i128,
) -> Option<Rank> {
    let (entry_value, formation) = match application {
        Some(application) => {
            let (input_type, bindings) = subject.bindings(program, application, |coordinate| {
                Some(coordinate.entry_value.clone())
            })?;
            let formed = application.observe(values, machine, input_type, bindings)?;
            (formed.value, Some(formed.formation))
        }
        None => {
            let coordinate = subject.scalar()?;
            (
                coordinate.bounded_value(coordinate.entry_value.clone())?,
                None,
            )
        }
    };
    Some(Rank {
        subject,
        bound,
        application,
        entry_value,
        formation,
        minimum,
    })
}

pub(super) fn prepare(
    program: &dyn validation::CheckingInput,
    machines: &[&Machine],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Result<Candidates, &'static str>> {
    let mut authored = Vec::with_capacity(machines.len());
    let mut field_paths = Vec::new();
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
        for path in parameter_projection_paths(program, machine, entry) {
            if !field_paths.contains(&path) {
                field_paths.push(path);
            }
        }
    }
    for view in authored.iter().flatten() {
        for coordinate in std::iter::once(&view.coordinate).chain(
            view.upper
                .as_ref()
                .and_then(|upper| upper.coordinate.as_ref()),
        ) {
            if !coordinate.path.is_empty() && !field_paths.contains(&coordinate.path) {
                field_paths.push(coordinate.path.clone());
            }
        }
    }
    let application = authored.iter().flatten().find_map(|view| view.application);
    if let Some(application) = application {
        let shape = application.shape(program)?;
        let existing_paths = field_paths.clone();
        for (machine, view) in machines.iter().zip(&authored) {
            let entry = &program.machine_states(machine)[0];
            let roots = view
                .as_ref()
                .map(|view| vec![view.coordinate.clone()])
                .unwrap_or_else(|| roots(program, entry, application, &existing_paths));
            for root in roots {
                for read in &shape.reads {
                    let path = root
                        .path
                        .iter()
                        .chain(&read.coordinate.path)
                        .cloned()
                        .collect::<Vec<_>>();
                    if !path.is_empty() && !field_paths.contains(&path) {
                        field_paths.push(path);
                    }
                }
            }
        }
    }
    let mut candidates = Vec::new();
    let mut available = Vec::new();
    let mut entry_values = Vec::new();
    let mut selected_order = None;
    for (machine, view) in machines.iter().zip(&authored) {
        let entry = &program.machine_states(machine)[0];
        let prepared =
            super::super::rank_inputs::at_entry(program, machine, entry, frames, &field_paths)?;
        available.push(prepared.coordinates);
        entry_values.push(prepared.values);
        let Some(view) = view else {
            candidates.push(Vec::new());
            continue;
        };
        let values = entry_values.last()?;
        let inputs = available.last()?;
        let subject = match view.application {
            Some(application) => Subject::declared(
                program,
                entry,
                &view.coordinate,
                application,
                inputs,
                values,
            )?,
            None => Subject::Scalar(
                inputs
                    .iter()
                    .find(|coordinate| {
                        coordinate.parameter == view.coordinate.parameter
                            && values
                                .same_scalar_projection(&coordinate.path, &view.coordinate.path)
                            && coordinate.observation == view.observation
                    })?
                    .clone(),
            ),
        };
        let carrier = match view.application {
            Some(application) => application.result_primitive,
            None => subject.scalar()?.primitive,
        };
        if selected_order
            .is_some_and(|previous| previous != (view.order, carrier, view.declaration))
        {
            return None;
        }
        if view.order != Order::Descending
            && !matches!(carrier, typed_trees::unsigned_integer_primitive_pattern!())
        {
            return None;
        }
        selected_order = Some((view.order, carrier, view.declaration));
        if let Err(reason) = ranges::entry(program, machine, view.measure, frames) {
            return Some(Err(reason));
        }
        let bound = if let Some(upper) = &view.upper {
            let bound = if let Some(input) = &upper.coordinate {
                let Some(coordinate) = inputs.iter().find(|coordinate| {
                    coordinate.parameter == input.parameter
                        && values.same_scalar_projection(&coordinate.path, &input.path)
                        && coordinate.observation == validation::RankPremiseObservation::Value
                }) else {
                    return Some(Err(
                        "a natural rank upper coordinate lacks independent entry receipts",
                    ));
                };
                Bound::Coordinate(coordinate.clone())
            } else {
                Bound::Authored {
                    expression: upper.expression,
                    entry_value: values.current_value(upper.expression)?,
                }
            };
            if view.order == Order::BoundedDistance
                && !values.proves_values_relation(
                    program,
                    machine,
                    BinaryOperator::LessOrEqual,
                    &subject.scalar()?.entry_value,
                    bound.entry(),
                )
            {
                return Some(Err(
                    "a bounded cursor rank lacks independent entry formation",
                ));
            }
            Some(bound)
        } else {
            None
        };
        let minimum = match view.application {
            Some(application) => application.result_primitive.integer_bounds()?.0,
            None => 0,
        };
        let Some(rank) = observe(
            program,
            machine,
            values,
            subject,
            view.application,
            bound,
            minimum,
        ) else {
            return Some(Err(
                "a computed natural rank application has no independent entry formation",
            ));
        };
        candidates.push(vec![rank]);
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
        if machine.is_public {
            return None;
        }
        let entry = &program.machine_states(machine)[0];
        let values = &entry_values[position];
        let mut subjects = Vec::new();
        if let Some(application) = application {
            subjects.extend(
                roots(program, entry, application, &field_paths)
                    .iter()
                    .filter_map(|root| {
                        Subject::declared(
                            program,
                            entry,
                            root,
                            application,
                            &available[position],
                            values,
                        )
                    })
                    .map(|subject| (subject, Some(application))),
            );
        }
        // A helper may carry the already-produced rank rather than the input
        // record. This is ordinary result transport, not a fabricated record
        // or a body-shape optimization. Every actual edge must still compare
        // that scalar with the independently formed application on its peer.
        subjects.extend(
            available[position]
                .iter()
                .filter(|coordinate| coordinate.primitive == primitive)
                .cloned()
                .map(|coordinate| (Subject::Scalar(coordinate), None)),
        );
        let minimum = if application.is_some() {
            primitive.integer_bounds()?.0
        } else {
            0
        };
        for (subject, selected_application) in subjects {
            if order == Order::Descending {
                if let Some(rank) = observe(
                    program,
                    machine,
                    values,
                    subject,
                    selected_application,
                    None,
                    minimum,
                ) {
                    candidates.push(rank);
                }
                continue;
            }
            let bounds = available[position]
                .iter()
                .cloned()
                .map(Bound::Coordinate)
                .chain(closed_bounds.iter().cloned().map(Bound::Closed));
            for bound in bounds {
                if order == Order::BoundedDistance
                    && !values.proves_values_relation(
                        program,
                        machine,
                        BinaryOperator::LessOrEqual,
                        &subject.scalar()?.entry_value,
                        bound.entry(),
                    )
                {
                    continue;
                }
                if let Some(rank) = observe(
                    program,
                    machine,
                    values,
                    subject.clone(),
                    None,
                    Some(bound),
                    minimum,
                ) {
                    candidates.push(rank);
                }
            }
        }
        if candidates.is_empty() {
            return Some(Err("a private member has no compatible natural rank input"));
        }
    }
    Some(Ok(Candidates {
        ranks: candidates,
        order,
        field_paths,
    }))
}
