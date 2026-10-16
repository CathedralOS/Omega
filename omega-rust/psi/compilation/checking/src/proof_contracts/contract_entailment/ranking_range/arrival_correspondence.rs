//! Completed arrivals retain coordinates, not a preferred carrier of a type.
//!
//! One constructor can put an entry subject beside an unrelated value of the
//! same type. Preserve the selected declaration path and the actual occurrence
//! that supplied it. These rows locate observations only: arithmetic formation,
//! range preservation, writes and descent remain the edge owner's obligations.
//! Finite alternatives are retained independently; no decreasing copy wins an
//! identity contest. This is invocation-owned query storage, not a certificate.
//! The complete edge also checks the chosen role against its local operand
//! transports. Independent receiving geometry cannot replace a transported
//! subject, even when a different sibling supplies a complete decreasing rank.

use super::{ExpressionHandle, ExpressionNode, Machine, State, saved_arguments};
use facts::PlaceSegment;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

/// Check the chosen receiving role against this edge's completed operands.
/// Discovery retains fresh locations without entry ancestry, but formation of
/// a fresh sibling cannot replace a subject which this edge transports into
/// another location. Keep every exact transport alternative; selecting among
/// them remains the complete witness owner's decision.
///
/// When there is no transported subject, qualification-preserving reception
/// may replace its value along the same full declared selector route, under
/// an additional containing owner if necessary. It may not discard or replace
/// a selector merely because the final leaf has the same carrier. This check
/// grants no equality, range or descent; the edge owner still proves those
/// obligations under the chosen bindings.
pub fn ranking_range_arrival_preserves_roles(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    source: super::RankingRangeState<'_>,
    destination: super::RankingRangeState<'_>,
    statement_index: usize,
    arguments: &[ExpressionHandle],
) -> bool {
    let parameters = program.state_parameters(destination.state);
    if parameters.len() != arguments.len()
        || source.entry_parameters.state != source.state.symbol
        || destination.entry_parameters.state != destination.state.symbol
    {
        return false;
    }
    let completed = arguments
        .iter()
        .map(|argument| {
            completed_operand_inventory(program, machine, source.state, statement_index, *argument)
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    // Known routes remain usable beside an unsupported auxiliary. Unknown
    // completion cannot, however, prove that no operand transports the subject
    // and thereby authorize a fresh receiving role. Constructor children retain
    // that distinction rather than silently erasing an opaque child's lineage.
    for binding in &source.entry_parameters.coordinates {
        let Some(arrived) = destination.entry_parameters.destination(&binding.entry) else {
            return false;
        };
        let mut transported = false;
        let mut chosen_transport = false;
        for (parameter, completed) in parameters.iter().zip(&completed) {
            for value in &completed.coordinates {
                if value.entry.parameter != binding.destination.parameter {
                    continue;
                }
                let Some(suffix) = binding
                    .destination
                    .path
                    .strip_prefix(value.entry.path.as_slice())
                else {
                    continue;
                };
                let mut path = value.relative_path.clone();
                path.extend_from_slice(suffix);
                transported = true;
                chosen_transport |= arrived.parameter == parameter.symbol && arrived.path == path;
            }
        }
        if transported {
            if !chosen_transport {
                return false;
            }
        } else if completed.iter().any(|completion| !completion.complete)
            || !arrived.path.ends_with(&binding.destination.path)
        {
            return false;
        }
    }
    true
}

/// An exact coordinate in a state or entry telescope. Paths use the same
/// declared field/case/index identities as ordinary receiving operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankSubjectCoordinate {
    pub parameter: SymbolHandle,
    pub path: Vec<PlaceSegment>,
}

/// A direct observation of a coordinate differs from an operation depending
/// on that coordinate. Neither implies equality with the entry value: the
/// source formal already denotes its own arrival value.
pub use checked_trees::CheckedRankValueDerivation as RankValueDerivation;

/// A stable local saves its initializer at this prefix. Keep the declaration
/// and use occurrences so consumers cannot replay it at the later edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankLocalCapture {
    pub symbol: SymbolHandle,
    pub statement_index: usize,
    pub initializer: ExpressionHandle,
    pub occurrence: ExpressionHandle,
}

/// The exact completion relating one source coordinate to an arrival slot.
/// `actual` is the whole actual for the destination formal; `expression` is
/// the leaf completion within it. These are provenance, not proof premises.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankCoordinateTransport {
    pub source_state: SymbolHandle,
    pub source: RankSubjectCoordinate,
    pub statement_index: usize,
    pub target: typed_trees::statement::TransitionTargetHandle,
    pub actual: ExpressionHandle,
    pub expression: ExpressionHandle,
    pub derivation: RankValueDerivation,
    pub captures: Vec<RankLocalCapture>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankArrivalCorrespondence {
    pub entry: RankSubjectCoordinate,
    pub destination: RankSubjectCoordinate,
    pub transports: Vec<RankCoordinateTransport>,
}

/// One independently completed receiving location. There is no entry
/// ancestry here: a fresh value can fill this exact declared slot. Only a
/// complete arrival judgment may select it as an observation of the rank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankReceivingCoordinate {
    pub destination: RankSubjectCoordinate,
    pub type_reference: TypeReferenceHandle,
    pub source_state: SymbolHandle,
    pub statement_index: usize,
    pub target: typed_trees::statement::TransitionTargetHandle,
    pub actual: ExpressionHandle,
    pub expression: ExpressionHandle,
}

/// Every retained coordinate alternative at one state. A coordinate cannot
/// gain an entry fact merely because another field shares its primitive type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateEntryCorrespondence {
    pub state: SymbolHandle,
    pub parameters: Vec<SymbolHandle>,
    pub coordinates: Vec<RankArrivalCorrespondence>,
    pub receiving: Vec<RankReceivingCoordinate>,
}

impl StateEntryCorrespondence {
    pub(super) fn initial(program: &dyn crate::CheckingInput, state: &State) -> Self {
        let parameters = program
            .state_parameters(state)
            .iter()
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let coordinates = parameters
            .iter()
            .map(|parameter| {
                let coordinate = RankSubjectCoordinate {
                    parameter: *parameter,
                    path: Vec::new(),
                };
                RankArrivalCorrespondence {
                    entry: coordinate.clone(),
                    destination: coordinate,
                    transports: Vec::new(),
                }
            })
            .collect();
        Self {
            state: state.symbol,
            parameters,
            coordinates,
            receiving: Vec::new(),
        }
    }

    /// Declared receiving locations are a separate candidate domain from
    /// value lineage. Exact selectors retain the selected declaration route;
    /// carrier compatibility supplies no equality or qualification premise.
    /// Every alternative still passes the same complete-edge callback.
    pub(super) fn receiving_destinations(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        entry: &RankSubjectCoordinate,
    ) -> Option<Vec<RankSubjectCoordinate>> {
        // Receiving geometry can preserve an authored declared selector even
        // when a fresh value has no operand lineage. An unprojected subject
        // has no such selector: matching its carrier to another formal or a
        // newly introduced field would choose a different ranking subject.
        // Its ordinary computed/constructed transports must instead come from
        // candidate_destinations, which retains the exact value dependency.
        if entry.path.is_empty() {
            return Some(Vec::new());
        }
        let root = program.machine_states(machine).first()?;
        let parameter = program.state_parameter_by_symbol(root, entry.parameter)?;
        let reference = crate::value_custody::places::project_type_reference_from_segments(
            program,
            parameter.type_reference,
            &entry.path,
        )?;
        let mut destinations = Vec::new();
        for receiving in &self.receiving {
            // Retain the receiving root once; this exact authored selector is
            // the demand. Declared collection extent never expands the query.
            for (path, actual_type) in
                receiving_projection_paths(program, receiving.type_reference, &entry.path)
            {
                if !crate::value_custody::places::declared_carriers_match(
                    program,
                    reference,
                    actual_type,
                ) || program.arithmetic_domain_for_type_reference(reference)
                    != program.arithmetic_domain_for_type_reference(actual_type)
                {
                    continue;
                }
                let destination = RankSubjectCoordinate {
                    parameter: receiving.destination.parameter,
                    path,
                };
                if !destinations.contains(&destination) {
                    destinations.push(destination);
                }
            }
        }
        Some(destinations)
    }

    /// Retain a selected receiving observation from its completed root. The
    /// complete edge has already selected this coordinate; declaration lookup
    /// supplies its qualified type and exact arrival site for publication,
    /// without expanding other children or repeating the edge proof.
    pub fn receiving_at_coordinate(
        &self,
        program: &dyn crate::CheckingInput,
        destination: &RankSubjectCoordinate,
    ) -> Vec<RankReceivingCoordinate> {
        self.receiving
            .iter()
            .filter_map(|receiving| {
                if receiving.destination.parameter != destination.parameter {
                    return None;
                }
                let suffix = destination
                    .path
                    .strip_prefix(receiving.destination.path.as_slice())?;
                let type_reference =
                    declared_receiving_projection_type(program, receiving.type_reference, suffix)?;
                let mut projected = receiving.clone();
                projected.destination = destination.clone();
                projected.type_reference = type_reference;
                Some(projected)
            })
            .collect()
    }

    /// Candidate readings of an entry coordinate at this state. Prefix transport
    /// composes an ordinary projected read; it does not search the carrier's
    /// type for a similarly shaped field. Repeated derivations deduplicate the
    /// same reading, while distinct destinations remain alternatives. This
    /// union does not certify that every arrival supplies the coordinate. A
    /// chosen witness must independently prove every arrival, including those
    /// whose literal or unsupported actual contributed no lineage row.
    pub fn candidate_destinations(
        &self,
        entry: &RankSubjectCoordinate,
    ) -> Vec<RankSubjectCoordinate> {
        let mut destinations = Vec::new();
        for correspondence in &self.coordinates {
            if correspondence.entry.parameter != entry.parameter {
                continue;
            }
            let Some(suffix) = entry
                .path
                .strip_prefix(correspondence.entry.path.as_slice())
            else {
                continue;
            };
            let mut destination = correspondence.destination.clone();
            destination.path.extend_from_slice(suffix);
            if !destinations.contains(&destination) {
                destinations.push(destination);
            }
        }
        destinations
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompletedCoordinate {
    pub relative_path: Vec<PlaceSegment>,
    entry: RankSubjectCoordinate,
    pub source: RankSubjectCoordinate,
    producer: ExpressionHandle,
    pub derivation: RankValueDerivation,
    captures: Vec<RankLocalCapture>,
}

/// Discovery may retain known alternatives from a partial completion. Only a
/// complete inventory can additionally establish absence of operand lineage;
/// an opaque child and a literal with no dependencies are different results.
#[derive(Default)]
struct CompletedCoordinateInventory {
    coordinates: Vec<CompletedCoordinate>,
    complete: bool,
}

impl CompletedCoordinateInventory {
    fn complete(coordinates: Vec<CompletedCoordinate>) -> Self {
        Self {
            coordinates,
            complete: true,
        }
    }
}

/// The current declaration's completed operand dependencies, independent of
/// any chosen rank assignment. These rows constrain coordinate discovery only;
/// consumers must prove the value relation and complete incoming coverage.
pub(super) fn completed_operand_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<Vec<CompletedCoordinate>> {
    completed_operand_inventory(program, machine, state, statement_index, expression)
        .map(|completed| completed.coordinates)
}

fn completed_operand_inventory(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<CompletedCoordinateInventory> {
    completed_coordinates(
        program,
        machine,
        state,
        statement_index,
        expression,
        &StateEntryCorrespondence::initial(program, state),
        0,
    )
}

/// One ordinary completion's coordinate roster. Constructor fields use the
/// shared selected-construction binding query; snapshots use the same stable
/// binding owner as saved arithmetic observations, at their binding prefix.
fn completed_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    source: &StateEntryCorrespondence,
    depth: usize,
) -> Option<CompletedCoordinateInventory> {
    let node = program
        .expression_table
        .expression_at_depth(expression, depth)?;
    match node {
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            if let Some(receiver) = crate::value_custody::places::collection_length_receiver(
                program,
                machine,
                Some(state),
                expression,
            ) {
                let mut completed = completed_coordinates(
                    program,
                    machine,
                    state,
                    statement_index,
                    receiver,
                    source,
                    depth + 1,
                )?;
                // An extent is a computed observation of this descriptor,
                // never an unresolved or synthetic declared field.
                for value in &mut completed.coordinates {
                    if !value.relative_path.is_empty() {
                        return None;
                    }
                    value.producer = expression;
                    value.derivation = RankValueDerivation::ComputedDependency;
                }
                return Some(completed);
            }
            let Some((place, symbol)) = crate::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                expression,
            )
            .and_then(|place| place.root.as_symbol().map(|symbol| (place, symbol))) else {
                // A projected completed value need not denote symbol-rooted
                // caller storage. Expression-rooted places use the same
                // normal-result owner as every other producer.
                return completed_projected_coordinates(
                    program,
                    machine,
                    state,
                    statement_index,
                    expression,
                    source,
                    depth,
                );
            };
            // A selector expression denotes its value at this completion,
            // not at every later observation of the same path. Until the
            // ordinary capture owner supplies that selector value, no reusable
            // coordinate is available. Fixed indices remain exact coordinates.
            if !place.segments.iter().all(|segment| match segment {
                PlaceSegment::Index { .. } | PlaceSegment::OpaqueRange => false,
                PlaceSegment::Field { symbol } => {
                    symbol.is_valid() && program.any_field_owner(*symbol).is_some()
                }
                PlaceSegment::Case { variant } => {
                    variant.is_valid() && program.variant_owner(*variant).is_some()
                }
                PlaceSegment::FixedIndex { .. } | PlaceSegment::FixedRange { .. } => true,
            }) {
                return None;
            }
            if source.parameters.contains(&symbol) {
                return Some(completed_parameter_coordinates(
                    source,
                    symbol,
                    &place.segments,
                    expression,
                ));
            }
            let (binding_index, _) = program.state_local_declaration(state, symbol)?;
            if binding_index >= statement_index {
                return None;
            }
            let binding = saved_arguments::saved(program, state, symbol)?;
            let completed = completed_coordinates(
                program,
                machine,
                state,
                binding_index,
                binding.initial_value,
                source,
                depth + 1,
            )?;
            let mut selected = select_completed(completed, &place.segments, expression);
            for value in &mut selected.coordinates {
                value.captures.push(RankLocalCapture {
                    symbol,
                    statement_index: binding_index,
                    initializer: binding.initial_value,
                    occurrence: expression,
                });
            }
            Some(selected)
        }
        ExpressionNode::StructLiteral(literal) => {
            let mut completed = CompletedCoordinateInventory::complete(Vec::new());
            let mut supplied = Vec::new();
            for field in program.expression_table.struct_fields(literal.fields) {
                if supplied.contains(&field.field_symbol) {
                    return None;
                }
                crate::value_custody::struct_literals::selected_construction_field_type(
                    program,
                    literal,
                    field.field_symbol,
                )?;
                supplied.push(field.field_symbol);
                let mut prefix = Vec::new();
                crate::value_custody::places::push_field_place_segments(
                    program,
                    &mut prefix,
                    field.field_symbol,
                );
                let child = completed_coordinates(
                    program,
                    machine,
                    state,
                    statement_index,
                    field.value,
                    source,
                    depth + 1,
                )
                .unwrap_or_default();
                completed.complete &= child.complete;
                for mut value in child.coordinates {
                    let mut path = prefix.clone();
                    path.extend(value.relative_path);
                    value.relative_path = path;
                    completed.coordinates.push(value);
                }
            }
            Some(completed)
        }
        ExpressionNode::ArrayLiteral(values) => {
            let mut completed = CompletedCoordinateInventory::complete(Vec::new());
            for (index, value) in program
                .expression_table
                .expression_handles(*values)
                .iter()
                .enumerate()
            {
                let child = completed_coordinates(
                    program,
                    machine,
                    state,
                    statement_index,
                    *value,
                    source,
                    depth + 1,
                )
                .unwrap_or_default();
                completed.complete &= child.complete;
                for mut value in child.coordinates {
                    value
                        .relative_path
                        .insert(0, PlaceSegment::FixedIndex { index });
                    completed.coordinates.push(value);
                }
            }
            Some(completed)
        }
        ExpressionNode::Borrow(borrow)
            if matches!(
                borrow.access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            ) =>
        {
            // A readable borrow completes the same selected contents through
            // the destination's declared reference boundary. These coordinates
            // locate that observation; the complete edge owner still proves
            // receiving formation, live qualification and prefix preservation.
            completed_coordinates(
                program,
                machine,
                state,
                statement_index,
                borrow.target,
                source,
                depth + 1,
            )
        }
        ExpressionNode::Cast(cast) => completed_coordinates(
            program,
            machine,
            state,
            statement_index,
            cast.value,
            source,
            depth + 1,
        )
        .map(|mut completed| {
            for value in &mut completed.coordinates {
                value.producer = expression;
                value.derivation = RankValueDerivation::ComputedDependency;
            }
            completed
        }),
        ExpressionNode::Binary(binary) => {
            // Only the selected builtin operation has this operand equation.
            // This admission supplies no totality or arithmetic safety proof.
            super::meanings::admitted_carrier(program, machine, state, expression, depth)?;
            let mut completed = CompletedCoordinateInventory::complete(Vec::new());
            for operand in [binary.left, binary.right] {
                let child = completed_coordinates(
                    program,
                    machine,
                    state,
                    statement_index,
                    operand,
                    source,
                    depth + 1,
                )?;
                completed.complete &= child.complete;
                for mut value in child.coordinates {
                    if !value.relative_path.is_empty() {
                        return None;
                    }
                    value.producer = expression;
                    value.derivation = RankValueDerivation::ComputedDependency;
                    if !completed.coordinates.contains(&value) {
                        completed.coordinates.push(value);
                    }
                }
            }
            Some(completed)
        }
        ExpressionNode::Indexed(indexed)
            if crate::value_custody::places::has_builtin_subslice_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) =>
        {
            completed_coordinates(
                program,
                machine,
                state,
                statement_index,
                indexed.collection,
                source,
                depth + 1,
            )
            .map(|mut completed| {
                for value in &mut completed.coordinates {
                    value.producer = expression;
                    value.derivation = RankValueDerivation::ComputedDependency;
                }
                completed
            })
        }
        ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_) => Some(CompletedCoordinateInventory::complete(Vec::new())),
        _ => completed_projected_coordinates(
            program,
            machine,
            state,
            statement_index,
            expression,
            source,
            depth,
        ),
    }
}

fn completed_parameter_coordinates(
    source: &StateEntryCorrespondence,
    symbol: SymbolHandle,
    path: &[PlaceSegment],
    expression: ExpressionHandle,
) -> CompletedCoordinateInventory {
    let mut completed = Vec::new();
    for correspondence in &source.coordinates {
        if correspondence.destination.parameter != symbol {
            continue;
        }
        if let Some(suffix) = path.strip_prefix(correspondence.destination.path.as_slice()) {
            let mut entry = correspondence.entry.clone();
            entry.path.extend_from_slice(suffix);
            let value = CompletedCoordinate {
                relative_path: Vec::new(),
                entry,
                producer: expression,
                source: RankSubjectCoordinate {
                    parameter: symbol,
                    path: path.to_vec(),
                },
                derivation: RankValueDerivation::CapturedCoordinate,
                captures: Vec::new(),
            };
            if !completed.contains(&value) {
                completed.push(value);
            }
        } else if let Some(suffix) = correspondence.destination.path.strip_prefix(path) {
            let value = CompletedCoordinate {
                relative_path: suffix.to_vec(),
                entry: correspondence.entry.clone(),
                producer: expression,
                source: correspondence.destination.clone(),
                derivation: RankValueDerivation::CapturedCoordinate,
                captures: Vec::new(),
            };
            if !completed.contains(&value) {
                completed.push(value);
            }
        }
    }
    CompletedCoordinateInventory::complete(completed)
}

/// Produced values use the same demanded projection query as edge arithmetic.
/// A known equation locates exact observations; a missing equation remains an
/// incomplete inventory and cannot authorize a fresh receiving role.
fn completed_projected_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    source: &StateEntryCorrespondence,
    depth: usize,
) -> Option<CompletedCoordinateInventory> {
    use super::super::completed_values::completed_value_observations;
    let reference = crate::expression_result_type_reference(program, machine, state, expression)?;
    let projections = completed_value_observations(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        &[],
    )?;
    let mut completed = CompletedCoordinateInventory::complete(Vec::new());
    for projection in projections {
        let mut complete_projection = false;
        for observation in projection.sources {
            let observed = completed_source_coordinates(
                program,
                machine,
                state,
                &observation,
                expression,
                source,
                depth + 1,
            );
            let Some(observed) = observed else {
                continue;
            };
            complete_projection |= observed.complete;
            for mut value in observed.coordinates {
                let mut path = projection.path.clone();
                path.extend(value.relative_path);
                value.relative_path = path;
                value.producer = expression;
                if !completed.coordinates.contains(&value) {
                    completed.coordinates.push(value);
                }
            }
        }
        if projection.terminal {
            completed.complete &= complete_projection;
        }
    }
    Some(completed)
}

/// Public result equations describe dependencies of a produced scalar, not
/// identity with any operand. Traverse their captured leaves through the same
/// source/capture owner, retaining every dependency and incomplete child.
fn completed_source_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    observation: &super::super::completed_values::CompletedValueSource,
    expression: ExpressionHandle,
    source: &StateEntryCorrespondence,
    depth: usize,
) -> Option<CompletedCoordinateInventory> {
    use super::super::completed_values::{CompletedValueSource, expression_statement_index};
    if depth >= 64 {
        return None;
    }
    match observation {
        CompletedValueSource::Parameter { symbol, path } => Some(completed_parameter_coordinates(
            source, *symbol, path, expression,
        )),
        CompletedValueSource::Expression(operand) => completed_coordinates(
            program,
            machine,
            state,
            expression_statement_index(program, state, *operand)?,
            *operand,
            source,
            depth + 1,
        ),
        CompletedValueSource::Integer(_) => {
            Some(CompletedCoordinateInventory::complete(Vec::new()))
        }
        CompletedValueSource::ScalarBinary { operands, .. } => {
            let mut completed = CompletedCoordinateInventory::complete(Vec::new());
            for operand in operands.iter() {
                let child = completed_source_coordinates(
                    program,
                    machine,
                    state,
                    operand,
                    expression,
                    source,
                    depth + 1,
                )
                .unwrap_or_default();
                completed.complete &= child.complete;
                for mut value in child.coordinates {
                    if !value.relative_path.is_empty() {
                        return None;
                    }
                    value.producer = expression;
                    value.derivation = RankValueDerivation::ComputedDependency;
                    if !completed.coordinates.contains(&value) {
                        completed.coordinates.push(value);
                    }
                }
            }
            Some(completed)
        }
    }
}

fn select_completed(
    completed: CompletedCoordinateInventory,
    path: &[PlaceSegment],
    expression: ExpressionHandle,
) -> CompletedCoordinateInventory {
    let mut selected = CompletedCoordinateInventory {
        coordinates: Vec::new(),
        complete: completed.complete,
    };
    for mut value in completed.coordinates {
        if let Some(suffix) = path.strip_prefix(value.relative_path.as_slice()) {
            value.entry.path.extend_from_slice(suffix);
            value.source.path.extend_from_slice(suffix);
            value.relative_path.clear();
        } else if let Some(suffix) = value.relative_path.strip_prefix(path) {
            value.relative_path = suffix.to_vec();
        } else {
            continue;
        }
        value.producer = expression;
        if !selected.coordinates.contains(&value) {
            selected.coordinates.push(value);
        }
    }
    selected
}

/// Receiving candidates require declaration geometry, independently of the
/// later expression formation proof. The ordinary type projection query does
/// not check index bounds or case ownership, so retain those exact obligations
/// which the previous declaration enumeration established by construction.
fn declared_receiving_projection_type(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    demand: &[PlaceSegment],
) -> Option<TypeReferenceHandle> {
    for segment in demand {
        let carrier = crate::value_custody::places::unwrapped_type_reference(program, reference)?;
        match segment {
            PlaceSegment::Case { variant } => {
                let (owner, _) = program.variant_owner(*variant)?;
                if crate::value_custody::type_shapes::head_data_symbol(program, carrier)
                    != Some(owner.symbol)
                {
                    return None;
                }
            }
            PlaceSegment::Field { symbol }
                if symbol.is_valid() && program.any_field_owner(*symbol).is_some() => {}
            PlaceSegment::FixedIndex { index } => {
                let capacity =
                    crate::value_custody::field_domains::type_reference_fixed_array_capacity(
                        program, carrier,
                    )?;
                if *index >= capacity {
                    return None;
                }
            }
            PlaceSegment::Field { .. }
            | PlaceSegment::Index { .. }
            | PlaceSegment::OpaqueRange
            | PlaceSegment::FixedRange { .. } => return None,
        }
        reference = crate::value_custody::places::project_type_reference_from_segments(
            program,
            reference,
            std::slice::from_ref(segment),
        )?;
    }
    Some(reference)
}

/// Apply one authored selector through finite declared containing owners.
/// Record fields supply owner prefixes; collections contribute only the exact
/// indices present in the demand. No carrier match chooses an array element.
fn receiving_projection_paths(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    demand: &[PlaceSegment],
) -> Vec<(Vec<PlaceSegment>, TypeReferenceHandle)> {
    fn append(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        demand: &[PlaceSegment],
        prefix: &mut Vec<PlaceSegment>,
        owners: &mut Vec<SymbolHandle>,
        projections: &mut Vec<(Vec<PlaceSegment>, TypeReferenceHandle)>,
    ) {
        if let Some(actual) = declared_receiving_projection_type(program, reference, demand) {
            let mut path = prefix.clone();
            path.extend_from_slice(demand);
            if !projections.iter().any(|(previous, _)| previous == &path) {
                projections.push((path, actual));
            }
        }
        let peeled = program.type_reference_table.unconstrained_type_reference(
            program
                .type_reference_table
                .dereferenced_type_reference(reference),
        );
        let Some(owner) = crate::value_custody::type_shapes::head_data_symbol(program, peeled)
        else {
            return;
        };
        if owners.contains(&owner) {
            return;
        }
        let Some(data) = program.data_definition_by_symbol(owner) else {
            return;
        };
        owners.push(owner);
        for field in program
            .data_members(data)
            .iter()
            .flat_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
                typed_trees::data::DataMember::Variant(variant) => {
                    program.data_payload_fields(variant)
                }
                _ => &[],
            })
        {
            let previous = prefix.len();
            crate::value_custody::places::push_field_place_segments(program, prefix, field.symbol);
            if let Some(actual) = crate::value_custody::places::project_type_reference_from_segments(
                program,
                reference,
                &prefix[previous..],
            ) {
                append(program, actual, demand, prefix, owners, projections);
            }
            prefix.truncate(previous);
        }
        owners.pop();
    }
    let mut projections = Vec::new();
    append(
        program,
        reference,
        demand,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut projections,
    );
    projections
}

/// Build the finite coordinate closure over the exact executable arrival
/// inventory. Properly growing entry paths cannot be a finite correspondence;
/// decline rather than generalize them or truncate the path.
pub(super) fn discover(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> Option<Vec<StateEntryCorrespondence>> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    let mut mappings = states
        .iter()
        .map(|state| StateEntryCorrespondence {
            state: state.symbol,
            parameters: program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.symbol)
                .collect(),
            coordinates: Vec::new(),
            receiving: Vec::new(),
        })
        .collect::<Vec<_>>();
    mappings[0] = StateEntryCorrespondence::initial(program, root);
    let arrivals = crate::machine_calls::call_cycles::runtime_ranking::internal_arrival_sites(
        program, machine,
    );
    loop {
        let previous = mappings.clone();
        for arrival in &arrivals {
            if arrival.destination == 0 {
                continue;
            }
            let source = states.get(arrival.source)?;
            let target = states.get(arrival.destination)?;
            let formals = program.state_parameters(target);
            if formals.len() != arrival.arguments.len() {
                return None;
            }
            for (formal, actual) in formals.iter().zip(arrival.arguments) {
                if program.expression_table.expression_is_valid(*actual) {
                    let coordinate = RankReceivingCoordinate {
                        destination: RankSubjectCoordinate {
                            parameter: formal.symbol,
                            path: Vec::new(),
                        },
                        type_reference: formal.type_reference,
                        source_state: source.symbol,
                        statement_index: arrival.statement_index,
                        target: arrival.target,
                        actual: *actual,
                        expression: *actual,
                    };
                    let rows = &mut mappings[arrival.destination].receiving;
                    if !rows.contains(&coordinate) {
                        rows.push(coordinate);
                    }
                }
                let Some(completed) = completed_coordinates(
                    program,
                    machine,
                    source,
                    arrival.statement_index,
                    *actual,
                    &previous[arrival.source],
                    0,
                ) else {
                    continue;
                };
                for value in completed.coordinates {
                    let destination = RankSubjectCoordinate {
                        parameter: formal.symbol,
                        path: value.relative_path,
                    };
                    let transport = RankCoordinateTransport {
                        source_state: source.symbol,
                        target: arrival.target,
                        source: value.source,
                        statement_index: arrival.statement_index,
                        actual: *actual,
                        expression: value.producer,
                        derivation: value.derivation,
                        captures: value.captures,
                    };
                    let rows = &mut mappings[arrival.destination].coordinates;
                    if let Some(existing) = rows
                        .iter_mut()
                        .find(|row| row.entry == value.entry && row.destination == destination)
                    {
                        if !existing.transports.contains(&transport) {
                            existing.transports.push(transport);
                        }
                    } else {
                        if arrival.recurrent
                            && rows.iter().any(|row| {
                                (row.destination == destination
                                    && row.entry.parameter == value.entry.parameter
                                    && row.entry.path.len() < value.entry.path.len()
                                    && value.entry.path.starts_with(&row.entry.path))
                                    || (row.entry == value.entry
                                        && row.destination.parameter == destination.parameter
                                        && row.destination.path.len() < destination.path.len()
                                        && destination.path.starts_with(&row.destination.path))
                            })
                        {
                            return None;
                        }
                        rows.push(RankArrivalCorrespondence {
                            entry: value.entry,
                            destination,
                            transports: vec![transport],
                        });
                    }
                }
            }
        }
        if previous == mappings {
            return Some(mappings);
        }
    }
}
