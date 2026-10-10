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
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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
            if !receiving.destination.path.ends_with(&entry.path)
                || !crate::value_custody::places::declared_carriers_match(
                    program,
                    reference,
                    receiving.type_reference,
                )
                || program.arithmetic_domain_for_type_reference(reference)
                    != program.arithmetic_domain_for_type_reference(receiving.type_reference)
            {
                continue;
            }
            if !destinations.contains(&receiving.destination) {
                destinations.push(receiving.destination.clone());
            }
        }
        Some(destinations)
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
            let place = crate::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                expression,
            )?;
            let symbol = place.root.as_symbol()?;
            // A selector expression denotes its value at this completion,
            // not at every later observation of the same path. Until the
            // ordinary capture owner supplies that selector value, no reusable
            // coordinate is available. Fixed indices remain exact coordinates.
            if !place.segments.iter().all(|segment| match segment {
                PlaceSegment::Index { .. } => false,
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
                let mut completed = Vec::new();
                for correspondence in &source.coordinates {
                    if correspondence.destination.parameter != symbol {
                        continue;
                    }
                    if let Some(suffix) = place
                        .segments
                        .strip_prefix(correspondence.destination.path.as_slice())
                    {
                        let mut entry = correspondence.entry.clone();
                        entry.path.extend_from_slice(suffix);
                        let value = CompletedCoordinate {
                            relative_path: Vec::new(),
                            entry,
                            producer: expression,
                            source: RankSubjectCoordinate {
                                parameter: symbol,
                                path: place.segments.clone(),
                            },
                            derivation: RankValueDerivation::CapturedCoordinate,
                            captures: Vec::new(),
                        };
                        if !completed.contains(&value) {
                            completed.push(value);
                        }
                    } else if let Some(suffix) = correspondence
                        .destination
                        .path
                        .strip_prefix(place.segments.as_slice())
                    {
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
                return Some(CompletedCoordinateInventory::complete(completed));
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
            super::meanings::builtin(program, machine, state, expression, depth)?;
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
        _ => None,
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

/// Retain receiving geometry independently of operand dependency lineage.
/// Constructor children are selected through the ordinary construction owner;
/// array children project the destination's declared extent. This traversal
/// records occurrences only and never evaluates a leaf or proves formation.
fn completed_receiving_coordinates(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    path: &mut Vec<PlaceSegment>,
    receiving: &mut Vec<(Vec<PlaceSegment>, TypeReferenceHandle, ExpressionHandle)>,
    depth: usize,
) -> Option<()> {
    let node = program
        .expression_table
        .expression_at_depth(expression, depth)?;
    receiving.push((path.clone(), reference, expression));
    match node {
        ExpressionNode::StructLiteral(literal) => {
            let (owner, _) = super::fields::record_referent(program, reference)?;
            if owner != literal.type_symbol {
                return None;
            }
            let mut supplied = Vec::new();
            for field in program.expression_table.struct_fields(literal.fields) {
                if supplied.contains(&field.field_symbol) {
                    return None;
                }
                let field_type =
                    crate::value_custody::struct_literals::selected_construction_field_type(
                        program,
                        literal,
                        field.field_symbol,
                    )?;
                supplied.push(field.field_symbol);
                let previous = path.len();
                crate::value_custody::places::push_field_place_segments(
                    program,
                    path,
                    field.field_symbol,
                );
                completed_receiving_coordinates(
                    program,
                    field.value,
                    field_type,
                    path,
                    receiving,
                    depth + 1,
                )?;
                path.truncate(previous);
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for (index, value) in program
                .expression_table
                .expression_handles(*values)
                .iter()
                .enumerate()
            {
                let segment = PlaceSegment::FixedIndex { index };
                let element_type =
                    crate::value_custody::places::project_type_reference_from_segments(
                        program,
                        reference,
                        std::slice::from_ref(&segment),
                    )?;
                path.push(segment);
                completed_receiving_coordinates(
                    program,
                    *value,
                    element_type,
                    path,
                    receiving,
                    depth + 1,
                )?;
                path.pop();
            }
        }
        ExpressionNode::Borrow(borrow)
            if matches!(
                borrow.access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            ) =>
        {
            let reference =
                crate::value_custody::places::peel_declared_constraints(program, reference);
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = program.type_reference_table.type_reference(reference)
            else {
                return None;
            };
            if *access == language_core::ReferenceAccess::WriteOnly {
                return None;
            }
            completed_receiving_coordinates(
                program,
                borrow.target,
                *referee,
                path,
                receiving,
                depth + 1,
            )?;
        }
        _ => {}
    }
    Some(())
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
                let mut receiving = Vec::new();
                if completed_receiving_coordinates(
                    program,
                    *actual,
                    formal.type_reference,
                    &mut Vec::new(),
                    &mut receiving,
                    0,
                )
                .is_none()
                {
                    // Unsupported geometry contributes no receiving candidate,
                    // without suppressing independently retained lineage.
                    receiving.clear();
                }
                for (path, reference, expression) in receiving {
                    let coordinate = RankReceivingCoordinate {
                        destination: RankSubjectCoordinate {
                            parameter: formal.symbol,
                            path,
                        },
                        type_reference: reference,
                        source_state: source.symbol,
                        statement_index: arrival.statement_index,
                        target: arrival.target,
                        actual: *actual,
                        expression,
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
