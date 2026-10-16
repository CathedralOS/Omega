//! Finite arrival alternatives are constraints on one coherent observation.
//!
//! Discovery owns provenance, not coverage. Selection keeps one scratch path
//! through the alternatives and asks the complete-edge proof owner to reject
//! incompatible prefixes as soon as its inputs are available. It never builds
//! a Cartesian roster, ranks candidates by arithmetic shape, or turns lineage
//! into an equality premise. Only a complete accepted path may be published.

use super::{
    ExpressionHandle, ExpressionNode, Machine, RankSubjectCoordinate, RankingRangeMeasure,
    RankingRangePremises, State, StateEntryCorrespondence,
};
use symbols::SymbolHandle;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankCoordinateBinding {
    pub entry: RankSubjectCoordinate,
    pub destination: RankSubjectCoordinate,
}

/// A single state's selected readings. Unrelated formals retain their own
/// values; only an exact entry prefix can read through a selected binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankStateBindings {
    pub state: SymbolHandle,
    pub coordinates: Vec<RankCoordinateBinding>,
}

impl RankStateBindings {
    /// Read the accepted arena row without discovery or a new proof attempt.
    /// The caller selects the exact authored body before locating its state.
    pub fn from_checked(
        plan: &checked_trees::CheckedRankArrivalPlan,
        state: &checked_trees::CheckedStateRankCorrespondence,
    ) -> Option<Self> {
        let coordinates = plan
            .bindings
            .span(state.bindings)?
            .iter()
            .map(|binding| {
                let coordinate = |handle| {
                    let coordinate = plan.coordinates.get(handle);
                    Some(RankSubjectCoordinate {
                        parameter: coordinate.parameter,
                        path: plan.path_segments.span(coordinate.path)?.to_vec(),
                    })
                };
                Some(RankCoordinateBinding {
                    entry: coordinate(binding.entry)?,
                    destination: coordinate(binding.destination)?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            state: state.state,
            coordinates,
        })
    }

    /// Resolve an exact authored coordinate through the selected prefix. If
    /// several retained prefixes cover it, they must give the same reading.
    pub fn destination(&self, entry: &RankSubjectCoordinate) -> Option<RankSubjectCoordinate> {
        rebase(&self.coordinates, entry, false)
    }

    /// Inverse lookup is for substitution geometry only. It establishes no
    /// equality between other coordinates sharing the same entry ancestry.
    pub fn entry(&self, destination: &RankSubjectCoordinate) -> Option<RankSubjectCoordinate> {
        rebase(&self.coordinates, destination, true)
    }

    /// Bind a declaration formal to the accepted readings below its authored
    /// subject. A copied leaf does not establish identity for the whole record:
    /// retain each relative leaf prefix instead of manufacturing that identity.
    pub fn declaration_bindings(
        &self,
        formal: SymbolHandle,
        subject: &RankSubjectCoordinate,
    ) -> Vec<RankCoordinateBinding> {
        let mut bindings = Vec::new();
        if let Some(destination) = self.destination(subject) {
            bindings.push(RankCoordinateBinding {
                entry: RankSubjectCoordinate {
                    parameter: formal,
                    path: Vec::new(),
                },
                destination,
            });
        }
        for binding in &self.coordinates {
            if binding.entry.parameter != subject.parameter {
                continue;
            }
            let Some(suffix) = binding.entry.path.strip_prefix(subject.path.as_slice()) else {
                continue;
            };
            let projected = RankCoordinateBinding {
                entry: RankSubjectCoordinate {
                    parameter: formal,
                    path: suffix.to_vec(),
                },
                destination: binding.destination.clone(),
            };
            if !bindings.contains(&projected) {
                bindings.push(projected);
            }
        }
        bindings
    }
}

fn rebase(
    bindings: &[RankCoordinateBinding],
    coordinate: &RankSubjectCoordinate,
    inverse: bool,
) -> Option<RankSubjectCoordinate> {
    let mut selected = None;
    for binding in bindings {
        let (source, target) = if inverse {
            (&binding.destination, &binding.entry)
        } else {
            (&binding.entry, &binding.destination)
        };
        if source.parameter != coordinate.parameter {
            continue;
        }
        let Some(suffix) = coordinate.path.strip_prefix(source.path.as_slice()) else {
            continue;
        };
        let mut result = target.clone();
        result.path.extend_from_slice(suffix);
        if selected
            .as_ref()
            .is_some_and(|selected| *selected != result)
        {
            return None;
        }
        selected = Some(result);
    }
    selected
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankArrivalDemand {
    pub state: SymbolHandle,
    pub entry: RankSubjectCoordinate,
}

/// A prefix can leave some edges unevaluable. Complete means the source owner
/// has independently proved every applicable edge, not merely that every
/// requested coordinate has a candidate. Refuted prefixes are never extended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RankArrivalCompatibility {
    Pending,
    Complete,
    Refuted,
}

struct DemandAlternatives {
    state_position: usize,
    entry: RankSubjectCoordinate,
    destinations: Vec<RankSubjectCoordinate>,
}

/// Search a finite authentic candidate domain without materializing products.
/// The callback owns typed coordinate compatibility and complete-edge proof;
/// it must return Complete only for a full source judgment under one regime.
/// A complete prefix still receives all demanded bindings before publication.
pub fn select_rank_arrival_correspondence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    discovered: &[StateEntryCorrespondence],
    demands: &[RankArrivalDemand],
    mut compatibility: impl FnMut(&[RankStateBindings]) -> RankArrivalCompatibility,
) -> Option<Vec<RankStateBindings>> {
    let mut selected = discovered
        .iter()
        .map(|state| RankStateBindings {
            state: state.state,
            coordinates: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut alternatives: Vec<DemandAlternatives> = Vec::new();
    for demand in demands {
        let mut matching = discovered
            .iter()
            .enumerate()
            .filter(|(_, state)| state.state == demand.state);
        let (state_position, state) = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        if alternatives
            .iter()
            .any(|known| known.state_position == state_position && known.entry == demand.entry)
        {
            continue;
        }
        let mut destinations = state.candidate_destinations(&demand.entry);
        for receiving in state
            .receiving_destinations(program, machine, &demand.entry)
            .unwrap_or_default()
        {
            if !destinations.contains(&receiving) {
                destinations.push(receiving);
            }
        }
        if destinations.is_empty() {
            return None;
        }
        alternatives.push(DemandAlternatives {
            state_position,
            entry: demand.entry.clone(),
            destinations,
        });
    }
    if alternatives.is_empty() {
        return (compatibility(&selected) == RankArrivalCompatibility::Complete)
            .then_some(selected);
    }
    // The next candidate ordinal belongs to the current prefix. Backtracking
    // removes precisely that demand's appended binding, preserving earlier
    // assignments and their constraints. All vectors here are query scratch.
    let mut next_candidates = vec![0; alternatives.len()];
    let mut depth = 0;
    loop {
        let demand = &alternatives[depth];
        let next = next_candidates[depth];
        let Some(destination) = demand.destinations.get(next) else {
            next_candidates[depth] = 0;
            if depth == 0 {
                return None;
            }
            depth -= 1;
            selected[alternatives[depth].state_position]
                .coordinates
                .pop()?;
            continue;
        };
        next_candidates[depth] += 1;
        selected[demand.state_position]
            .coordinates
            .push(RankCoordinateBinding {
                entry: demand.entry.clone(),
                destination: destination.clone(),
            });
        let judgment = compatibility(&selected);
        if judgment == RankArrivalCompatibility::Refuted {
            selected[demand.state_position].coordinates.pop()?;
            continue;
        }
        if depth + 1 == alternatives.len() {
            if judgment == RankArrivalCompatibility::Complete {
                return Some(selected);
            }
            selected[demand.state_position].coordinates.pop()?;
            continue;
        }
        depth += 1;
    }
}

/// Exact entry coordinates read by one premise regime. Member projections
/// retain their declaration paths; collection metadata reads the authentic
/// descriptor coordinate instead of synthesizing a field named `len`.
pub fn ranking_range_required_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
) -> Option<Vec<RankSubjectCoordinate>> {
    let root = program.machine_states(machine).first()?;
    let mut expressions = if range.is_valid() {
        let range = program.expression_table.range_node(range)?;
        vec![range.start, range.end]
    } else {
        Vec::new()
    };
    super::push_measure_subjects(measure, &mut expressions);
    let mut coordinates = Vec::new();
    if matches!(premises, RankingRangePremises::EntryInvariant) {
        expressions.extend(super::required_contract_expressions(program, machine));
        let mut constrained = Vec::new();
        super::constrained_parameter_bound_expressions(
            program,
            root,
            &mut expressions,
            Some(&mut constrained),
        );
        coordinates.extend(
            constrained
                .into_iter()
                .map(|parameter| RankSubjectCoordinate {
                    parameter,
                    path: Vec::new(),
                }),
        );
    }
    if let RankingRangeMeasure::Field { subject, measure } = measure {
        let field = super::fields::FieldCoordinate::resolve_field(program, root, subject, measure)?;
        coordinates.push(field.location(program));
        expressions.retain(|expression| *expression != subject);
    }
    for expression in expressions {
        collect_read_coordinates(program, machine, root, expression, &mut coordinates, 0)?;
    }
    Some(coordinates)
}

fn collect_read_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    root: &State,
    expression: ExpressionHandle,
    coordinates: &mut Vec<RankSubjectCoordinate>,
    depth: usize,
) -> Option<()> {
    let node = program
        .expression_table
        .expression_at_depth(expression, depth)?;
    match node {
        ExpressionNode::Integer(_) | ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => {}
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            let source = crate::value_custody::places::collection_length_receiver(
                program,
                machine,
                Some(root),
                expression,
            )
            .unwrap_or(expression);
            let place =
                crate::canonical_place_from_expression_in_state(program, root.symbol, 0, source)?;
            let parameter = place.root.as_symbol()?;
            program.state_parameter_by_symbol(root, parameter)?;
            if !place.segments.iter().all(|segment| match segment {
                facts::PlaceSegment::Field { symbol } => {
                    symbol.is_valid() && program.any_field_owner(*symbol).is_some()
                }
                facts::PlaceSegment::Case { variant } => {
                    variant.is_valid() && program.variant_owner(*variant).is_some()
                }
                facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::FixedRange { .. } => {
                    true
                }
                facts::PlaceSegment::Index { .. } | facts::PlaceSegment::OpaqueRange => false,
            }) {
                return None;
            }
            let coordinate = RankSubjectCoordinate {
                parameter,
                path: place.segments,
            };
            if !coordinates.contains(&coordinate) {
                coordinates.push(coordinate);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_read_coordinates(program, machine, root, atomic.value, coordinates, depth + 1)?
        }
        ExpressionNode::Unary(unary) => collect_read_coordinates(
            program,
            machine,
            root,
            unary.operand,
            coordinates,
            depth + 1,
        )?,
        ExpressionNode::Binary(binary) => {
            collect_read_coordinates(program, machine, root, binary.left, coordinates, depth + 1)?;
            collect_read_coordinates(program, machine, root, binary.right, coordinates, depth + 1)?;
        }
        _ => return None,
    }
    Some(())
}
