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
            let (input_type, bindings) = subject.bindings(
                program,
                application,
                || {
                    validation::captured_entry_owned_snapshot(
                        program,
                        machine,
                        values.state(),
                        &subject.root(),
                    )
                },
                |coordinate| Some(coordinate.entry_value.clone()),
            )?;
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
                let expression = values.integer_expression(upper.expression)?;
                let Some((formation, entry_value)) =
                    values.integer_expression_formation(machine, &expression)
                else {
                    return Some(Err("a rank upper expression lacks independent formation"));
                };
                // These are the upper expression's caller inputs. Its retained
                // call formations own any callee contexts separately.
                let bindings = inputs
                    .iter()
                    .filter(|input| input.observation == validation::RankPremiseObservation::Value)
                    .map(|input| {
                        let coordinate = validation::RankSubjectCoordinate {
                            parameter: input.parameter,
                            path: input.path.clone(),
                        };
                        validation::RuntimeRankApplicationBinding {
                            calls: Vec::new(),
                            formal: coordinate.clone(),
                            actual: coordinate,
                            entry_indexes: input.entry_indexes.clone(),
                        }
                    })
                    .collect();
                Bound::Authored {
                    expression: upper.expression,
                    entry_value,
                    formation,
                    bindings,
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
    let authored_endpoints = candidates
        .iter()
        .enumerate()
        .flat_map(|(peer, ranks)| ranks.iter().map(move |rank| (peer, rank)))
        .filter_map(|(peer, rank)| match rank.bound.as_ref()? {
            Bound::Authored { expression, .. } => Some((peer, *expression)),
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
        let transported = authored_endpoints
            .iter()
            .flat_map(|(peer, expression)| {
                transported_bounds(program, machine, machines[*peer], *expression, values)
            })
            .collect::<Vec<_>>();
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
                .chain(closed_bounds.iter().cloned().map(Bound::Closed))
                .chain(transported.iter().cloned());
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

/// A helper with no ranking of its own may measure toward the endpoint a peer
/// authored, read through the helper's own call to that peer. Each formal the
/// endpoint reads must arrive as storage the helper already holds at entry:
/// one of its unreassigned parameters, or a field projection of one. A
/// computed argument is not entry storage. The pairing only proposes a bound;
/// every actual edge and complete cycle is still proved by the component search.
fn transported_bounds(
    program: &dyn validation::CheckingInput,
    helper: &Machine,
    owner: &Machine,
    expression: typed_trees::expression::ExpressionHandle,
    values: &CurrentScalarValues<'_>,
) -> Vec<Bound> {
    let entry = &program.machine_states(helper)[0];
    let owner_state = &program.machine_states(owner)[0];
    let formals = program.state_parameters(owner_state);
    let actuals = program.state_parameters(entry);
    let read = read_formals(program, formals, expression);
    let mut bounds = Vec::<Bound>::new();
    for (statement, _) in crate::flow::indexed_statements(program, entry.statement_nodes) {
        for site in validation::collect_call_sites_in_statement(program, helper, entry, statement) {
            let target = site.target_symbol(program);
            if target != owner.symbol && target != owner_state.symbol {
                continue;
            }
            let arguments = validation::call_site_argument_expressions(program, &site);
            let pairs = read
                .iter()
                .map(|formal| {
                    let place = validation::canonical_place_from_expression(
                        program,
                        *arguments.get(*formal)?,
                    )?;
                    if place.segments.iter().any(|segment| {
                        !matches!(
                            segment,
                            facts::PlaceSegment::Field { symbol } if symbol.is_valid()
                        )
                    }) {
                        return None;
                    }
                    let actual = actuals.iter().position(|parameter| {
                        place.root == facts::PlaceRoot::Symbol(parameter.symbol)
                            && !parameter.is_mutable
                    })?;
                    Some(super::coordinates::TransportedFormal {
                        formal: *formal,
                        actual,
                        path: place.segments.clone(),
                    })
                })
                .collect::<Option<Vec<_>>>();
            let Some(pairs) = pairs else {
                continue;
            };
            let roster = pairs
                .iter()
                .map(|supply| formals[supply.formal].clone())
                .collect::<Vec<_>>();
            let supplied = pairs
                .iter()
                .map(|supply| arguments[supply.formal])
                .collect::<Vec<_>>();
            let Some((formation, entry_value)) = values
                .bind_integer_expression_formals(owner, owner_state, expression, &roster, &supplied)
                .and_then(|bound| values.integer_expression_formation(helper, &bound))
            else {
                continue;
            };
            if bounds.iter().any(
                |known| matches!(known, Bound::Transported { formals, .. } if *formals == pairs),
            ) {
                continue;
            }
            bounds.push(Bound::Transported {
                owner: owner_state.symbol,
                expression,
                formals: pairs,
                entry_value,
                formation,
            });
        }
    }
    bounds
}

/// Positions of the formals an authored expression reads.
fn read_formals(
    program: &dyn validation::CheckingInput,
    formals: &[typed_trees::signature::StateParameter],
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<usize> {
    struct Reads {
        symbols: Vec<symbols::SymbolHandle>,
        visited: Vec<typed_trees::expression::ExpressionHandle>,
    }
    impl Extend<symbols::SymbolHandle> for Reads {
        fn extend<T: IntoIterator<Item = symbols::SymbolHandle>>(&mut self, symbols: T) {
            self.symbols.extend(symbols);
        }
    }
    impl typed_trees::references::TypedReferences for Reads {
        fn expression(&mut self, expression: typed_trees::expression::ExpressionHandle) -> bool {
            if self.visited.contains(&expression) {
                return false;
            }
            self.visited.push(expression);
            true
        }
        fn type_reference(&mut self, _: typed_trees::types::TypeReferenceHandle) -> bool {
            false
        }
    }
    let mut reads = Reads {
        symbols: Vec::new(),
        visited: Vec::new(),
    };
    typed_trees::references::visit_expression(program, &mut reads, expression);
    formals
        .iter()
        .enumerate()
        .filter(|(_, formal)| reads.symbols.contains(&formal.symbol))
        .map(|(position, _)| position)
        .collect()
}
