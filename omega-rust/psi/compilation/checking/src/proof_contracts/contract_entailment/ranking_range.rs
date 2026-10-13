//! Integer coordinates use the ordinary arithmetic engine. Ranking owners
//! select one actual edge; operand owners supply one captured evidence epoch.
//! Numeric meaning and descriptor geometry grant no bounds or borrow authority.
use super::{
    BTreeMap, BigInt, BinaryOperator, Engine, ExpressionHandle, ExpressionNode, Machine,
    Polynomial, ProofFact, SignatureContractKind, StrictArithmeticBindingValue,
    StrictArithmeticSymbolBinding, inductive_judgment,
};
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeConstraintNode, TypeReferenceNode};

mod arrival_correspondence;
mod arrival_selection;
pub use arrival_selection::{
    RankArrivalCompatibility, RankArrivalDemand, RankCoordinateBinding, RankStateBindings,
    ranking_range_required_coordinates, select_rank_arrival_correspondence,
};
mod calls;
pub use arrival_correspondence::{
    RankArrivalCorrespondence, RankCoordinateTransport, RankLocalCapture, RankReceivingCoordinate,
    RankSubjectCoordinate, RankValueDerivation, StateEntryCorrespondence,
    ranking_range_arrival_preserves_roles,
};

/// Enumerate exact completed-arrival lineage and receiving locations. This is not a rank judgment;
/// only the complete-edge owner may select and retain an accepted witness.
pub fn discover_rank_arrival_correspondence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> Option<Vec<StateEntryCorrespondence>> {
    arrival_correspondence::discover(program, machine)
}
mod coordinate;
mod field_coordinates;
mod fields;
pub use fields::unique_literal_field;
mod lengths;

mod meanings;
mod projections;
mod requirements;
mod saved_arguments;
mod state_aliases;

pub use requirements::{
    ArithmeticCallObservation, arithmetic_entry_requirement_is_covered,
    prove_arithmetic_call_requirement, prove_captured_arithmetic_call_requirement,
};
mod identity_views;
pub use identity_views::{
    ComputationBodyShape, DeclaredIdentityView, DeclaredScalarView, MeasureBodyShape,
    ProjectionStep, ScalarViewComputation, computation_body_shape, declared_scalar_view,
    find_declared_measure, identity_subject_matches, measure_body_shape,
    measure_constraints_cover_subject,
};

pub(crate) use calls::{
    RankingRangeCallEdge, RankingRangeCallMember, RankingRangeCallProgress, RankingRangeCallSite,
    call_member_premise_symbols, mixed_call_endpoints_are_pinned, prove_ranking_range_call,
};

/// A collection descriptor's closed extent, using the same checked geometry
/// as ranking and saved-argument proofs. Immutable bindings retain their
/// initializer only while writes and exclusive exposure cannot replace it.
pub fn closed_collection_length(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    collection: ExpressionHandle,
) -> Option<u64> {
    let mut engine = Engine::strict(program, machine, &[]);
    saved_arguments::install(program, machine, state, &[], &mut engine);
    lengths::actual(program, machine, state, collection, &[], &mut engine)?
        .constant_value()?
        .to_u64()
}

/// Close a completed collection's extent with observations from that operand's
/// own evidence epoch. The existing length evaluator owns descriptor geometry
/// and bounds; this query installs only exact integer place values. It grants
/// neither selector bounds nor loans, and imports no live caller hypotheses.
/// Saved initializers receive no captured symbol bindings, so a later source
/// value cannot be replayed as the initializer's earlier operand.
pub fn captured_collection_length(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    collection: ExpressionHandle,
    mut observe: impl FnMut(&crate::CanonicalPlace) -> Option<BigInt>,
) -> Option<u64> {
    let mut engine = captured_integer_engine(program, machine, state, collection, &mut observe)?;
    lengths::actual(program, machine, state, collection, &[], &mut engine)?
        .constant_value()?
        .to_u64()
}

/// Evaluate an integer coordinate from its exact captured facts and builtin
/// meaning. No saved initializer, caller hypothesis or declared operator body
/// is replayed. Absence leaves the coordinate symbolic.
pub fn captured_integer_value(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    mut observe: impl FnMut(&crate::CanonicalPlace) -> Option<BigInt>,
) -> Option<BigInt> {
    meanings::admitted_carrier(program, machine, state, expression, 0)?;
    let mut engine = captured_integer_engine(program, machine, state, expression, &mut observe)?;
    engine.normalize(expression)?.constant_value()
}

fn captured_integer_engine<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: &State,
    root: ExpressionHandle,
    observe: &mut impl FnMut(&crate::CanonicalPlace) -> Option<BigInt>,
) -> Option<Engine<'program>> {
    let mut engine = Engine::strict(program, machine, &[]);
    let mut expressions = Vec::new();
    crate::expression_traversal::collect_expression_tree(program, root, &mut expressions);
    for &expression in &expressions {
        let Some(place) = crate::canonical_place_from_expression(program, expression) else {
            continue;
        };
        let Some(value) = observe(&place) else {
            continue;
        };
        if !engine.bind_captured_integer_place(machine, state, expression, value) {
            return None;
        }
    }
    // Metadata is not an ordinary stored integer field. Bind only the exact
    // collection measure, using its independently declared fixed extent;
    // a record field with the same spelling receives no such substitution.
    for expression in expressions.into_iter().rev() {
        if let Some(receiver) = crate::value_custody::places::collection_length_receiver(
            program,
            machine,
            Some(state),
            expression,
        ) && let Some(length) = crate::value_custody::places::declared_collection_extent(
            program,
            machine,
            Some(state),
            receiver,
        ) && !engine.bind_strict_projection(
            expression,
            Polynomial::constant(BigInt::from_u64(u64::try_from(length).ok()?)),
        ) {
            return None;
        }
    }
    Some(engine)
}

/// The same declared slice coordinate used by range entry and arrival
/// checking. A bare slice retains the ordinary whole-formal length binding;
/// a projected or packed arrival retains the exact field chain selected by
/// the source coordinate and its independently established entry telescope.
fn slice_rank_coordinate<'program>(
    program: &'program dyn crate::CheckingInput,
    root: &State,
    state: &'program State,
    entry_parameters: Option<&RankStateBindings>,
    subject: ExpressionHandle,
) -> Option<Option<lengths::SliceCoordinate<'program>>> {
    match lengths::SliceCoordinate::resolve_length(program, root, subject) {
        Some(coordinate) => Some(Some(coordinate.at_entry_arrival(
            program,
            state,
            entry_parameters,
        )?)),
        None => Some(
            match (entry_parameters, lengths::parameter(program, root, subject)) {
                (Some(entries), Some(formal)) => entries
                    .destination(&RankSubjectCoordinate {
                        parameter: formal.symbol,
                        path: Vec::new(),
                    })
                    .filter(|destination| !destination.path.is_empty())
                    .and_then(|destination| {
                        lengths::SliceCoordinate::at_coordinate(
                            program,
                            state,
                            &destination,
                            coordinate::CoordinateKind::Length,
                        )
                    }),
                _ => None,
            },
        ),
    }
}

/// Compile the source range owner's selected sequence coordinate into the
/// existing checked scalar observation vocabulary. The supplied telescope is
/// an established arrival correspondence, not a type-based guess. This query
/// grants no range, descent, access or write-preservation authority; those
/// remain independently proved by the caller's ranking judgment.
pub fn ranking_slice_length_observation(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    entry_parameters: &RankStateBindings,
    subject: ExpressionHandle,
) -> Option<(
    symbols::SymbolHandle,
    u32,
    checked_trees::CheckedScalarExpression,
)> {
    let root = program.machine_states(machine).first()?;
    let coordinate = slice_rank_coordinate(program, root, state, Some(entry_parameters), subject)?;
    let (parameter, path) = if let Some(coordinate) = coordinate {
        let path = coordinate
            .steps
            .iter()
            .chain([&coordinate.field])
            .map(|field| {
                checked_trees::CheckedStructuralPredicatePathSegment::Field(
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ),
                )
            })
            .collect();
        (coordinate.parameter, path)
    } else {
        let formal = lengths::parameter(program, root, subject)?;
        let destination = entry_parameters.destination(&RankSubjectCoordinate {
            parameter: formal.symbol,
            path: Vec::new(),
        })?;
        if !destination.path.is_empty() {
            return None;
        }
        let carrier = program.state_parameter_by_symbol(state, destination.parameter)?;
        lengths::is_slice(program, carrier.type_reference).then_some(())?;
        (carrier, Vec::new())
    };
    let position = program
        .state_parameters(state)
        .iter()
        .position(|candidate| candidate.symbol == parameter.symbol)?;
    let position = u32::try_from(position).ok()?;
    Some((
        parameter.symbol,
        position,
        checked_trees::CheckedScalarExpression::StructuralSequenceLength {
            root: checked_trees::CheckedStorageRoot::Parameter { index: position },
            path: path
                .into_iter()
                .map(checked_trees::CheckedScalarReadProjection::Static)
                .collect(),
        },
    ))
}

/// Exact collection geometry at a canonical storage place, independent of
/// which element membership facts happen to exist. Fixed arrays retain their
/// declared extent under replacement. A slice descriptor uses the ordinary
/// length evaluator and its stable-binding gate; unknown geometry stays unknown.
pub fn closed_collection_length_at_place(
    program: &dyn crate::CheckingInput,
    state_symbol: symbols::SymbolHandle,
    collection: &crate::CanonicalPlace,
) -> Option<u64> {
    let (machine, state) = crate::find_state(program, state_symbol)?;
    if let Some(reference) =
        crate::canonical_place_type_reference(program, state_symbol, usize::MAX, collection)
        && let Some(length) = crate::fixed_array_type_length(program, reference)
    {
        return u64::try_from(length).ok();
    }
    if !collection.segments.is_empty() {
        return None;
    }
    if let Some(expression) = collection.root.as_expression() {
        return closed_collection_length(program, machine, state, expression);
    }
    let symbol = collection.root.as_symbol()?;
    let engine = Engine::strict(program, machine, &[]);
    lengths::saved_length(program, machine, state, symbol, &engine, 0)?
        .constant_value()?
        .to_u64()
}

/// The scalar rank produced by an independently selected ranking view.
#[derive(Clone, Copy)]
pub enum RankingRangeMeasure {
    Single(ExpressionHandle),
    /// A declared scalar view's computed rank: `body` with the measure's
    /// `parameter` bound to `subject` (validation's `declared_scalar_view`
    /// admits the body). The rank must also form inside `carrier`; the
    /// judgment proves that upper bound from the same hypotheses that prove
    /// membership, so an unbounded subject cannot claim `{ value * 2 }`.
    Computed {
        subject: ExpressionHandle,
        parameter: symbols::SymbolHandle,
        body: ExpressionHandle,
        carrier: symbols::BuiltinTypeAtom,
    },
    SliceLength(ExpressionHandle),
    /// A declared field view's rank: the `u64` field the measure `measure`
    /// projects from `subject`'s record, through the measure body's exact
    /// nested path and, for a borrowed subject, through its reference. The
    /// judgment re-resolves that chain against the subject's declaration; the
    /// checked owner's selection is not trusted as the coordinate.
    Field {
        subject: ExpressionHandle,
        measure: symbols::SymbolHandle,
    },
    /// Natural distance to a fixed upper coordinate. Without an authored
    /// range, entry formation also permits lower > upper; the raw difference
    /// therefore is not an arrival invariant. Recurrent edges establish its
    /// nonnegativity and preserve the upper coordinate independently.
    Distance {
        lower: ExpressionHandle,
        upper: ExpressionHandle,
    },
    IncreasingTo {
        subject: ExpressionHandle,
        limit: ExpressionHandle,
    },
}

/// Edge premises for a complete checked state graph. EntryInvariant must be
/// checked on every edge; InitialEntry is the non-reentered-root exception to
/// RankInvariant. Entry facts are never automatically renewed at reentry.
#[derive(Clone, Copy)]
pub enum RankingRangePremises {
    RankInvariant,
    EntryInvariant,
    /// Entry facts can support the first transfer without surviving it, but
    /// only when no local transition can return to this root state.
    InitialEntry,
}

/// Prove current and next rank membership, with invocation-fixed endpoints.
/// This query neither mutates source/evidence nor admits an unknown judgment.
/// The caller must provide an exact root self-edge and its live guard facts;
/// arbitrary state-to-root substitutions are deliberately not inferred here.
/// `evaluated_prefix` must come from a premise-preserving prefix: live
/// write-frame evidence that no earlier statement writes the path of any
/// premise carrier (`ranking_range_premise_symbols`), so each such mutable
/// parameter still denotes its arrival value at the transition.
pub fn prove_ranking_range_edge(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
    guards: &[(ExpressionHandle, bool)],
    evaluated_prefix: &[ExpressionHandle],
    arguments: &[ExpressionHandle],
) -> Option<RankingRangeEdgeProof> {
    prove_edge(
        program,
        machine,
        state,
        range,
        measure,
        premises,
        guards,
        evaluated_prefix,
        Some(arguments),
        EdgeContext::Root,
    )
}

/// An exact state telescope over the entry witness. The parameter list follows
/// non-self formal order and names the entry symbol represented by each slot.
/// Entry parameters may be absent or repeated. Required scalar copies carry an
/// equality invariant checked at every arrival; ancestry alone is not equality.
/// An invalid handle marks a slot with no entry role: a payload computed from
/// several auxiliary inputs. It binds nothing, so no template premise can reach
/// it, and a required symbol folded into it is rejected as a missing premise.
#[derive(Clone, Copy)]
pub struct RankingRangeState<'program> {
    pub state: &'program State,
    pub entry_parameters: &'program RankStateBindings,
}

/// Every length binding is non-negative: each bound symbol satisfies
/// `symbol >= 0`, the premise every ranking-range site extends into its
/// comparison set.
pub(super) fn length_bound_comparisons<'a>(
    length_bindings: &'a [(symbols::SymbolHandle, String)],
) -> impl Iterator<Item = (BinaryOperator, Polynomial, Polynomial)> + 'a {
    length_bindings.iter().map(|(_, identity)| {
        (
            BinaryOperator::GreaterOrEqual,
            Polynomial::atom(identity.clone()),
            Polynomial::default(),
        )
    })
}

/// Push a measure's subject expressions into `expressions` — every measure
/// shape's operand roster in one match.
pub(super) fn push_measure_subjects(
    measure: RankingRangeMeasure,
    expressions: &mut Vec<ExpressionHandle>,
) {
    match measure {
        RankingRangeMeasure::Single(subject)
        | RankingRangeMeasure::Computed { subject, .. }
        | RankingRangeMeasure::SliceLength(subject)
        | RankingRangeMeasure::Field { subject, .. } => expressions.push(subject),
        RankingRangeMeasure::Distance { lower, upper }
        | RankingRangeMeasure::IncreasingTo {
            subject: lower,
            limit: upper,
        } => expressions.extend([lower, upper]),
    }
}

/// Check one state transition under independently established source and target
/// telescopes. Each source uses the selected graph-wide invariant, including
/// reentered roots. Every destination formal receives its exact actual in
/// one simultaneous substitution; graph ownership decides whether strict
/// decrease is additionally required for this edge. `evaluated_prefix` must
/// come from a premise-preserving prefix: live write-frame evidence that no
/// earlier statement writes the path of any slot carrying a premise role
/// (`ranking_range_premise_symbols` through the telescope), so each such
/// mutable parameter still denotes its arrival value at the transition.
pub fn prove_ranking_range_transition(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
    source: RankingRangeState<'_>,
    destination: RankingRangeState<'_>,
    guards: &[(ExpressionHandle, bool)],
    evaluated_prefix: &[ExpressionHandle],
    arguments: &[ExpressionHandle],
) -> Option<RankingRangeEdgeProof> {
    prove_edge(
        program,
        machine,
        source.state,
        range,
        measure,
        premises,
        guards,
        evaluated_prefix,
        Some(arguments),
        EdgeContext::Transition {
            source_parameters: source.entry_parameters,
            destination,
        },
    )
}

enum EdgeContext<'program> {
    Root,
    Transition {
        source_parameters: &'program RankStateBindings,
        destination: RankingRangeState<'program>,
    },
}

/// Separate results prevent a range-membership proof from authorizing descent.
#[derive(Clone, Copy)]
pub struct RankingRangeEdgeProof {
    pub membership_and_pinning: bool,
    pub strictly_decreases: bool,
    pub non_increasing: bool,
}

/// The entry symbols whose current copies the edge judgment holds equal at
/// every arrival: each integer root parameter named by the produced-rank
/// expression, the pinned endpoints, or the active entry premises. Mapping
/// discovery uses this set to decide whether several slots claiming one entry
/// keep their duplicated equality obligation or resolve to a bare forward.
pub fn ranking_range_required_symbols(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
) -> Option<Vec<symbols::SymbolHandle>> {
    let range = program.expression_table.range_node(range)?;
    state_aliases::required_symbols(program, machine, Some(range), measure, premises)
}

/// The root formals, in every carrier, whose arrival value some range judgment
/// on this witness may read: the produced-rank subjects, the range endpoints,
/// every requires fact, and every range-constrained integer formal. A prefix
/// store whose complete write frame is disjoint from each path carrying one of
/// these roles preserves the entry-relative ranking; a store into any of them
/// invalidates it, whatever value it stores. An invalid `range` means the
/// witness authored no range and contributes no endpoint.
pub fn ranking_range_premise_symbols(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
) -> Option<Vec<RankPremiseInput>> {
    let range = if range.is_valid() {
        let range = program.expression_table.range_node(range)?;
        Some(range)
    } else {
        None
    };
    state_aliases::premise_symbols(program, machine, range, measure, true)
}

/// How a ranking judgment reads one carrier: its whole arrival value, or only
/// a collection's extent through `.len`. A store into a collection's elements
/// leaves its extent as it was, so the observation decides which prefix
/// writes a premise survives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RankPremiseObservation {
    Value,
    Length,
}

/// One root formal a ranking judgment reads, and how much of it: the whole
/// formal, or the extent of the collection at `path` beneath it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankPremiseInput {
    pub parameter: symbols::SymbolHandle,
    pub path: Vec<facts::PlaceSegment>,
    pub observation: RankPremiseObservation,
}

/// One state coordinate a ranking judgment reads at arrival, and how much of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankPremiseCoordinate {
    pub coordinate: RankSubjectCoordinate,
    pub observation: RankPremiseObservation,
}

/// A premise carrier in the caller frame vocabulary: what a prefix write must
/// leave unwritten before the judgment's site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankProtectedPath {
    pub path: String,
    pub observation: RankPremiseObservation,
}

/// Spell a premise carrier in the shared frame vocabulary. An extent reads
/// `<path>.len`, the same spelling an authored `.len` store or read carries.
pub fn rank_protected_path(
    program: &dyn crate::CheckingInput,
    parameter: symbols::SymbolHandle,
    path: &[facts::PlaceSegment],
    observation: RankPremiseObservation,
) -> Option<RankProtectedPath> {
    let path = match observation {
        RankPremiseObservation::Value => crate::frame_coordinate_path(program, parameter, path)?,
        RankPremiseObservation::Length => {
            crate::frame_length_coordinate_path(program, parameter, path)?
        }
    };
    Some(RankProtectedPath { path, observation })
}

/// Add a carrier to a protected roster. Two readings of one path keep the
/// stronger: a whole-value reading already covers the extent.
pub fn protect_rank_premise(protected: &mut Vec<RankProtectedPath>, carrier: RankProtectedPath) {
    match protected
        .iter_mut()
        .find(|existing| existing.path == carrier.path)
    {
        Some(existing) => {
            if carrier.observation == RankPremiseObservation::Value {
                existing.observation = RankPremiseObservation::Value;
            }
        }
        None => protected.push(carrier),
    }
}

/// A complete frame preserves a premise carrier when nothing it may write
/// overlaps the carrier. The frame algebra coarsens a store into a
/// collection's elements to the whole collection, which overlaps the
/// collection's extent by spelling alone; `element_store` says the frame is
/// such a store, and an element store never writes an extent.
pub fn frame_preserves_rank_premises(
    frame: facts::NormalizedWriteFrame,
    element_store: bool,
    protected: &[RankProtectedPath],
) -> bool {
    frame.into_complete_paths().is_some_and(|paths| {
        protected.iter().all(|carrier| {
            (element_store && carrier.observation == RankPremiseObservation::Length)
                || paths
                    .iter()
                    .all(|path| !crate::frame_paths_overlap(path, &carrier.path))
        })
    })
}

/// Arrival observations consumed from this state's requirements and constrained
/// integer formals. Both surfaces name premises of the same arithmetic judgment;
/// every observed coordinate must remain unwritten until that judgment's site.
/// A requirement held at arrival, not after a write to one of its operands.
/// These are preservation inputs, not correspondence or equality evidence.
pub fn ranking_range_state_premise_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
) -> Option<Vec<RankPremiseCoordinate>> {
    let mut expressions = Vec::new();
    let mut parameters = Vec::new();
    constrained_parameter_bound_expressions(
        program,
        state,
        &mut expressions,
        Some(&mut parameters),
    );
    let root = program.machine_states(machine).first()?;
    let contracts = if state.symbol == root.symbol {
        program.machine_contracts(machine)
    } else {
        program.state_contracts(state)
    };
    for contract in contracts {
        if contract.kind == SignatureContractKind::Requires {
            expressions.extend(
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .filter_map(|fact| fact.as_expression().copied()),
            );
        }
    }
    let mut coordinates = parameters
        .into_iter()
        .map(|parameter| RankPremiseCoordinate {
            coordinate: RankSubjectCoordinate {
                parameter,
                path: Vec::new(),
            },
            observation: RankPremiseObservation::Value,
        })
        .collect::<Vec<_>>();
    let mut pending = expressions
        .into_iter()
        .map(|expression| (expression, 0))
        .collect::<Vec<_>>();
    while let Some((expression, depth)) = pending.pop() {
        let node = program
            .expression_table
            .expression_at_depth(expression, depth)?;
        // `.len` on a collection reads its extent and nothing else; every
        // other place reads its whole value.
        let (observed, observation) = match crate::value_custody::places::collection_length_receiver(
            program,
            machine,
            Some(state),
            expression,
        ) {
            Some(receiver) => (receiver, RankPremiseObservation::Length),
            None => (expression, RankPremiseObservation::Value),
        };
        if matches!(
            node,
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) && let Some(place) = crate::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            usize::MAX,
            observed,
        ) && let Some(parameter) = place.root.as_symbol()
            && program
                .state_parameter_by_symbol(state, parameter)
                .is_some()
        {
            // Selector values are independent observations even when the
            // existing frame algebra coarsens their storage footprint.
            for segment in &place.segments {
                if let facts::PlaceSegment::Index { expression } = segment {
                    pending.push((*expression, depth + 1));
                }
            }
            let coordinate = RankSubjectCoordinate {
                parameter,
                path: place.segments,
            };
            match coordinates
                .iter_mut()
                .find(|existing| existing.coordinate == coordinate)
            {
                Some(existing) => {
                    if observation == RankPremiseObservation::Value {
                        existing.observation = RankPremiseObservation::Value;
                    }
                }
                None => coordinates.push(RankPremiseCoordinate {
                    coordinate,
                    observation,
                }),
            }
            continue;
        }
        crate::expression_traversal::children(program, node, |child| {
            if child.is_valid() {
                pending.push((child, depth + 1));
            }
        });
    }
    Some(coordinates)
}

/// Establish the produced rank at entry, including an acyclic invocation.
/// No backedge guard or actual argument can strengthen this obligation.
pub fn prove_ranking_range_entry(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
) -> bool {
    // Without an authored range, a bounded distance produces the saturating
    // natural distance (stage 05 emits `MAX - lower`), so an entry whose
    // cursor already passed its bound is still a member: such a loop exits on
    // its first guard. The raw `upper - lower >= 0` test is the clamped tier's
    // job, not an entry obligation. Formation still has to succeed, and every
    // arrival keeps proving the raw distance under its own guards.
    let unranged_distance =
        !range.is_valid() && matches!(measure, RankingRangeMeasure::Distance { .. });
    prove_edge(
        program,
        machine,
        state,
        range,
        measure,
        RankingRangePremises::EntryInvariant,
        &[],
        &[],
        None,
        EdgeContext::Root,
    )
    .is_some_and(|proof| proof.membership_and_pinning || unranged_distance)
}

/// The natural coordinates builtin scalar subjects read through member
/// chains: each `record.field` subject resolves against its carrier formal's
/// own declaration, then re-resolves onto the unique formal carrying that
/// role at this state. A bare subject names no coordinate; an authored chain
/// that resolves but cannot arrive -- a duplicated or role-less record slot --
/// fails the edge rather than borrowing a foreign record's lineage.

/// The binary operators the endpoint-input and field-land traversals visit
/// uniformly: both operands carry the dependency, then each site layers its
/// own operation-specific obligation on top.
pub(super) fn dependency_binary(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::BitwiseAnd
    )
}
/// Push every `[minimum, maximum]` bound expression the root's exact-integer
/// parameters carry in their `Constrained` shells into `expressions`;
/// `recorded` also collects each contributing parameter's symbol.
pub(super) fn constrained_parameter_bound_expressions(
    program: &dyn crate::CheckingInput,
    root: &State,
    expressions: &mut Vec<ExpressionHandle>,
    mut recorded: Option<&mut Vec<symbols::SymbolHandle>>,
) {
    for parameter in program
        .state_parameters(root)
        .iter()
        .filter(|parameter| integer_parameter(program, parameter.type_reference).is_some())
    {
        let mut reference = parameter.type_reference;
        while let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        {
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Range {
                    minimum, maximum, ..
                } = constraint
                {
                    if let Some(recorded) = recorded.as_deref_mut()
                        && !recorded.contains(&parameter.symbol)
                    {
                        recorded.push(parameter.symbol);
                    }
                    expressions.extend([*minimum, *maximum]);
                }
            }
            reference = *base_type;
        }
    }
}

fn subject_field_coordinates<'program>(
    program: &'program dyn crate::CheckingInput,
    root: &'program State,
    state: &'program State,
    entry_parameters: Option<&RankStateBindings>,
    subjects: &[ExpressionHandle],
) -> Option<field_coordinates::FieldCoordinates<'program>> {
    let mut coordinates = field_coordinates::FieldCoordinates::empty();
    if let Some(entries) = entry_parameters {
        for selected in &entries.coordinates {
            if selected.entry.path.is_empty()
                && ranking_coordinate_type(program, root, &selected.entry)
                    .and_then(|reference| integer_parameter(program, reference))
                    .is_some()
                && !selected.destination.path.is_empty()
            {
                coordinates.include(fields::FieldCoordinate::at_coordinate(
                    program,
                    state,
                    &selected.destination,
                    coordinate::CoordinateKind::Integer,
                )?);
            }
        }
    }
    for subject in subjects {
        if let Some(coordinate) =
            fields::FieldCoordinate::resolve_field_projection(program, root, *subject)
        {
            let coordinate = coordinate.at_entry_arrival(program, state, entry_parameters)?;
            coordinates.include(coordinate);
            continue;
        }
        // A bare subject can still name a coordinate at this state: its entry
        // role may arrive packed inside a record carrier, which holds the
        // subject's value at the record's unique natural leaf.
        if let Some(entries) = entry_parameters
            && let Some(coordinate) =
                carried_subject_coordinate(program, root, state, entries, *subject)
        {
            coordinates.include(coordinate);
        }
    }
    Some(coordinates)
}

/// A bare entry scalar may arrive at a selected field. The declaration
/// confirms this exact path; it never searches for a matching integer leaf.
fn carried_subject_coordinate<'program>(
    program: &'program dyn crate::CheckingInput,
    root: &'program State,
    state: &'program State,
    entry_parameters: &RankStateBindings,
    subject: ExpressionHandle,
) -> Option<fields::FieldCoordinate<'program>> {
    let formal = fields::parameter(program, root, subject)?;
    let destination = entry_parameters.destination(&RankSubjectCoordinate {
        parameter: formal.symbol,
        path: Vec::new(),
    })?;
    if destination.path.is_empty() {
        return None;
    }
    fields::FieldCoordinate::at_coordinate(
        program,
        state,
        &destination,
        coordinate::CoordinateKind::Integer,
    )
}

/// The declared type of an exact retained coordinate, using ordinary place
/// projection. This checks carrier correspondence; it grants no value fact.
pub fn ranking_coordinate_type(
    program: &dyn crate::CheckingInput,
    state: &State,
    coordinate: &RankSubjectCoordinate,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let parameter = program.state_parameter_by_symbol(state, coordinate.parameter)?;
    if parameter.is_const {
        return None;
    }
    crate::value_custody::places::project_type_reference_from_segments(
        program,
        parameter.type_reference,
        &coordinate.path,
    )
}

fn scalar_coordinate_value(
    program: &dyn crate::CheckingInput,
    state: &State,
    coordinate: &RankSubjectCoordinate,
    bindings: &[StrictArithmeticSymbolBinding],
) -> Option<StrictArithmeticBindingValue> {
    if coordinate.path.is_empty() {
        return bindings
            .iter()
            .find(|binding| binding.symbol == coordinate.parameter)
            .map(|binding| binding.value.clone());
    }
    let field = fields::FieldCoordinate::at_coordinate(
        program,
        state,
        coordinate,
        coordinate::CoordinateKind::Integer,
    )?;
    Some(StrictArithmeticBindingValue::Atom {
        identity: field.identity,
        unsigned: !integer_parameter(program, field.field.type_reference)?.is_signed_integer(),
    })
}

fn arrival_scalar_actual(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    destination_state: &State,
    destination: &RankSubjectCoordinate,
    engine: &mut Engine<'_>,
    actual: ExpressionHandle,
) -> Option<Polynomial> {
    if destination.path.is_empty() {
        if let Some(value) =
            fields::completed_scalar_actual(program, machine, state, engine, actual, &[])
        {
            return Some(value);
        }
        meanings::install_nonpolynomial_terms(program, machine, state, engine, actual, 0)?;
        return engine.normalize(actual);
    }
    let coordinate = fields::FieldCoordinate::at_coordinate(
        program,
        destination_state,
        destination,
        coordinate::CoordinateKind::Integer,
    )?;
    coordinate.field_actual(program, machine, state, engine, actual)
}

fn arrival_length_actual(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    destination_state: &State,
    destination: &RankSubjectCoordinate,
    engine: &mut Engine<'_>,
    actual: ExpressionHandle,
    bindings: &[(symbols::SymbolHandle, String)],
) -> Option<Polynomial> {
    if destination.path.is_empty() {
        return lengths::actual(program, machine, state, actual, bindings, engine);
    }
    let coordinate = lengths::SliceCoordinate::at_coordinate(
        program,
        destination_state,
        destination,
        coordinate::CoordinateKind::Length,
    )?;
    coordinate.arrived_length(
        program,
        machine,
        state,
        engine,
        actual,
        coordinate.borrowed,
        bindings,
    )
}

fn prove_edge(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
    guards: &[(ExpressionHandle, bool)],
    evaluated_prefix: &[ExpressionHandle],
    arguments: Option<&[ExpressionHandle]>,
    context: EdgeContext<'_>,
) -> Option<RankingRangeEdgeProof> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    if !states
        .iter()
        .any(|candidate| candidate.symbol == state.symbol)
        || (matches!(context, EdgeContext::Root) && root.symbol != state.symbol)
    {
        return None;
    }
    // The source state graph never revisits the root: a machine-entry target
    // is a recursive invocation (`internal_state_index` admits subordinate
    // states only), ranked by the call-component judgment with its own entry.
    if matches!(premises, RankingRangePremises::InitialEntry) && state.symbol != root.symbol {
        return None;
    }
    let (entry_parameters, destination) = match &context {
        EdgeContext::Root => (None, None),
        EdgeContext::Transition {
            source_parameters,
            destination,
        } => {
            validate_mapping(program, machine, state, source_parameters)?;
            validate_mapping(
                program,
                machine,
                destination.state,
                destination.entry_parameters,
            )?;
            (Some(*source_parameters), Some(*destination))
        }
    };
    let mut field_rank = match measure {
        RankingRangeMeasure::Field { subject, measure } => {
            // Scalar telescope compatibility says nothing about records.
            // Rebind the selected field only after proving its unique
            // nominal arrival; template spellings cannot supply that proof.
            let coordinate =
                fields::FieldCoordinate::resolve_field(program, root, subject, measure)?;
            let coordinate = coordinate.at_entry_arrival(program, state, entry_parameters)?;
            Some(field_coordinates::FieldCoordinates::new(coordinate))
        }
        // A declared scalar view may read projected storage: the subject's
        // member chain resolves to the exact `u64` coordinate the view body
        // consumes, so `computed_rank` and arrival substitution share the
        // atom a field view would use. A bare subject keeps the bare-name
        // path -- its symbol binding or an unbound member spelling decides
        // normalization -- while the installed set still binds the authored
        // endpoint and premise projections the same edge judgment reads.
        RankingRangeMeasure::Computed { subject, .. } => Some(subject_field_coordinates(
            program,
            root,
            state,
            entry_parameters,
            &[subject],
        )?),
        // A builtin scalar rank may read projected storage directly:
        // `bag.count` names the exact unsigned coordinate rooted at its
        // carrier formal, of whatever unsigned width the declaration gives
        // the leaf. Bare subjects resolve to no coordinate and keep the
        // symbol-binding path; an unresolvable chain stays an unbound
        // spelling and fails normalization below.
        RankingRangeMeasure::Single(subject) => Some(subject_field_coordinates(
            program,
            root,
            state,
            entry_parameters,
            &[subject],
        )?),
        RankingRangeMeasure::IncreasingTo { subject, limit } => Some(subject_field_coordinates(
            program,
            root,
            state,
            entry_parameters,
            &[subject, limit],
        )?),
        RankingRangeMeasure::Distance { lower, upper } => Some(subject_field_coordinates(
            program,
            root,
            state,
            entry_parameters,
            &[lower, upper],
        )?),
        // A produced slice length is a length coordinate, never a field
        // atom, so the subject names no natural coordinate here. The range
        // endpoints and the rest of the read surface still spell authored
        // member projections (`limits.cap`): the installed set binds exactly
        // those projections so entry membership and endpoint pinning read
        // real atoms instead of failing to normalize.
        RankingRangeMeasure::SliceLength(subject) => Some(subject_field_coordinates(
            program,
            root,
            state,
            entry_parameters,
            &[subject],
        )?),
    };
    // A slice over projected storage produces its length from the member
    // chain's exact leaf: `record.field` names one slice coordinate whose
    // produced length the range reads. Bare slice formals keep the
    // `length_bindings` coordinate below.
    let mut slice_rank = match measure {
        RankingRangeMeasure::SliceLength(subject) => {
            slice_rank_coordinate(program, root, state, entry_parameters, subject)?
                .map(lengths::SliceCoordinates::new)
        }
        _ => None,
    };
    let demanded_coordinates = if arguments.is_some() {
        ranking_range_required_coordinates(program, machine, range, measure, premises)?
    } else {
        Vec::new()
    };
    let range = if range.is_valid() {
        Some(program.expression_table.range_node(range)?)
    } else {
        None
    };
    // An endpoint that is not statically formed still owes a carrier-landing
    // proof under this edge's installed hypotheses; it is deferred, not
    // rejected, until that engine exists below. Every measure owes it: the
    // range reads the produced rank, whatever view produced it.
    let mut deferred_endpoints = Vec::new();
    for endpoint in range.into_iter().flat_map(|range| [range.start, range.end]) {
        if !fields::endpoint_statically_formed(program, machine, root, endpoint) {
            deferred_endpoints.push(endpoint);
        }
    }
    let admit_template =
        |expression| meanings::admitted_carrier(program, machine, root, expression, 0);
    for endpoint in range.into_iter().flat_map(|range| [range.start, range.end]) {
        admit_template(endpoint)?;
    }
    match measure {
        RankingRangeMeasure::Single(subject)
        | RankingRangeMeasure::Computed { subject, .. }
        | RankingRangeMeasure::SliceLength(subject)
        | RankingRangeMeasure::Field { subject, .. } => {
            admit_template(subject)?;
        }
        RankingRangeMeasure::Distance { lower, upper }
        | RankingRangeMeasure::IncreasingTo {
            subject: lower,
            limit: upper,
        } => {
            admit_template(lower)?;
            admit_template(upper)?;
        }
    }
    let admit = |expression| meanings::admitted_carrier(program, machine, state, expression, 0);
    for argument in arguments.unwrap_or_default() {
        admit(*argument)?;
    }
    for (guard, _) in guards {
        admit(*guard)?;
    }
    // Prefix guards/initializers may have executed even when they establish no
    // surviving hypothesis. Check their meaning without assuming their truth.
    for expression in evaluated_prefix {
        admit(*expression)?;
    }
    // A mutable parameter's live value equals its arrival value only while no
    // intervening write touches its path. The evaluated-prefix callers prove
    // that per edge with complete write frames disjoint from every premise
    // carrier's path before this judgment runs; the entry query has no prefix
    // at all. A formal outside the premise set may have been written: nothing
    // here assumes its arrival value, and the actual it feeds is read live.
    // Mutability is then a storage capability, not a value distinction, and an
    // integer atom names the same live value it would for an immutable formal.
    if arguments.is_some_and(|arguments| {
        program
            .state_parameters(destination.map_or(state, |destination| destination.state))
            .iter()
            .count()
            != arguments.len()
    }) {
        return None;
    }
    let required_symbols =
        state_aliases::required_symbols(program, machine, range, measure, premises)?;
    let bindings = match entry_parameters {
        Some(entries) => calls::telescoped_bindings(program, machine, state, entries)?,
        None => integer_bindings(program, state)?,
    };
    if required_symbols.iter().any(|symbol| {
        !bindings.iter().any(|binding| binding.symbol == *symbol)
            || destination.is_some_and(|destination| {
                destination
                    .entry_parameters
                    .destination(&RankSubjectCoordinate {
                        parameter: *symbol,
                        path: Vec::new(),
                    })
                    .is_none()
            })
    }) {
        return None;
    }
    let mut engine = Engine::strict(program, machine, &bindings);
    if !engine.strict_symbol_bindings_are_valid() {
        return None;
    }
    let length_bindings = lengths::bindings(program, machine, state, entry_parameters);
    let mut expressions = projections::expressions(
        program,
        machine,
        root,
        range,
        measure,
        premises,
        arguments,
        guards,
        evaluated_prefix,
    );
    constrained_parameter_bound_expressions(program, state, &mut expressions, None);
    lengths::install(
        program,
        machine,
        state,
        root,
        &length_bindings,
        &mut engine,
        &expressions,
    )?;
    if let Some(field) = &mut field_rank {
        field.install(
            program,
            state,
            root,
            entry_parameters,
            &mut engine,
            &expressions,
        )?;
    }
    if let Some(slice) = &mut slice_rank {
        slice.install(
            program,
            machine,
            state,
            root,
            entry_parameters,
            &mut engine,
            &expressions,
        )?;
    }
    // Saved caller observations ride this site's scope: a state-local `let`
    // the saved-observation bar admits denotes the exact value its
    // initializer produced, so a guard or actual spelling the local
    // normalizes to the bound polynomial rather than an unknown atom. A
    // written or exclusively exposed binding never joins the roster.
    saved_arguments::install(program, machine, state, &length_bindings, &mut engine);
    let auxiliary =
        if arguments.is_none() || !matches!(premises, RankingRangePremises::RankInvariant) {
            entry_comparisons(program, machine, root, &mut engine, &bindings)?
        } else {
            Vec::new()
        };
    let mut comparisons = auxiliary.clone();
    // A state's own constrained formals are enforced by ordinary arrival
    // formation. They remain facts of this exact source frame under either
    // rank regime; only transported entry-template hypotheses require the
    // separate invariant-preservation obligation below.
    comparisons.extend(parameter_comparisons(
        program,
        machine,
        state,
        &mut engine,
        &bindings,
    )?);
    if let Some(field) = &field_rank {
        comparisons.extend(field.comparisons(program));
    }
    if let Some(slice) = &slice_rank {
        // A produced length is a natural coordinate: the projected slice's
        // `>= 0` fact is exactly what the bare-formal length bindings add.
        comparisons.extend(slice.comparisons());
    }
    comparisons.extend(length_bound_comparisons(&length_bindings));
    for &(guard, holds) in guards {
        collect_guard(&mut engine, guard, holds, &mut comparisons, 0)?;
    }
    // Arrival `requires` facts hold inside this state: the caller discharged
    // them at every incoming edge, so the relational judgment reads them as
    // hypotheses the way it reads guards. The root state's arrival contract
    // is the machine's own `requires` (installed again at reentry: each
    // incoming machine call re-discharges it); any other state's is its
    // signature's. Membership and proposition facts stay outside this
    // arithmetic projection -- they are neither assumed nor claimed.
    let requires_contracts = if state.symbol == root.symbol {
        program.machine_contracts(machine)
    } else {
        program.state_contracts(state)
    };
    for contract in requires_contracts {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let Some(expression) = fact.as_expression() {
                collect_guard(&mut engine, *expression, true, &mut comparisons, 0)?;
            }
        }
    }

    // Endpoint arithmetic uses the same exact entry aliases and field
    // coordinates as the rank. Bind non-polynomial term meaning after those
    // coordinates exist; declaration/flow formation and every arrival's
    // pinning remain independent obligations below.
    for endpoint in range.into_iter().flat_map(|range| [range.start, range.end]) {
        meanings::install_nonpolynomial_terms(program, machine, root, &mut engine, endpoint, 0)?;
    }
    let floor = match range {
        Some(range) => engine.normalize(range.start)?,
        None => Polynomial::default(),
    };
    let ceiling = match range {
        Some(range) => engine.normalize(range.end)?,
        None => Polynomial::default(),
    };
    let rank = match measure {
        RankingRangeMeasure::Single(subject) => engine.normalize(subject)?,
        RankingRangeMeasure::Computed {
            subject,
            parameter,
            body,
            ..
        } => computed_rank(program, machine, &mut engine, subject, parameter, body)?,
        RankingRangeMeasure::Field { .. } => field_rank.as_ref()?.value()?,
        RankingRangeMeasure::SliceLength(subject) => match &slice_rank {
            // A slice reached through a member chain reads its projected
            // coordinate; a bare slice formal reads its parameter binding.
            Some(slice) => slice.value()?,
            None => {
                let parameter = lengths::parameter(program, root, subject)?;
                let (_, identity) = length_bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter.symbol)?;
                Polynomial::atom(identity.clone())
            }
        },
        RankingRangeMeasure::Distance { lower, upper }
        | RankingRangeMeasure::IncreasingTo {
            subject: lower,
            limit: upper,
        } => engine.normalize(upper)?.sub(&engine.normalize(lower)?),
    };
    let clamped = matches!(measure, RankingRangeMeasure::IncreasingTo { .. });
    if arguments.is_some() {
        // Consume precisely the produced-value invariant established at each
        // complete arrival. IncreasingTo's zero plateau supplies no raw
        // distance >=0 premise; the shared semantic law retains that distinction.
        if let Some(range) = range {
            comparisons.extend(calls::arrival_invariant(
                measure,
                &rank,
                &floor,
                &ceiling,
                range.end_inclusive,
            ));
        } else if !matches!(
            measure,
            RankingRangeMeasure::IncreasingTo { .. } | RankingRangeMeasure::Distance { .. }
        ) {
            // Unranged distance entry formation permits a cursor already past
            // its bound. Its natural rank does not establish raw upper-lower
            // nonnegativity. Assuming that here contradicts an inverted loop's
            // guard and vacuously proves its recursive edge; every such edge
            // must instead establish the raw distance under its own guards.
            comparisons.push((
                BinaryOperator::GreaterOrEqual,
                rank.clone(),
                Polynomial::default(),
            ));
        }
    }
    hypothesis_landed_endpoints(
        &mut engine,
        program,
        machine,
        root,
        range.map_or(Default::default(), |range| range.start),
        &floor,
        &ceiling,
        &deferred_endpoints,
        comparisons,
    )?;
    // For distance views raw subtraction represents the produced natural rank
    // only on this proved branch. The caller retains the separate clamped
    // interval tier for entries where subject <= limit is not established.
    // A computed rank is a value of its carrier only while the body forms
    // there; the same hypotheses that bound the subject must bound the rank.
    let carrier_maximum = match measure {
        RankingRangeMeasure::Computed { carrier, .. } => Some(carrier_maximum(carrier)?),
        _ => None,
    };
    let rank_is_member = |value: &Polynomial| {
        let natural = if let Some(range) = range {
            calls::membership(
                &engine,
                measure,
                value,
                &floor,
                &ceiling,
                range.end_inclusive,
            )
        } else {
            clamped || engine.prove_at_least(&engine.substituted(value), &BigInt::zero())
        };
        natural
            && carrier_maximum.as_ref().is_none_or(|maximum| {
                engine.prove_at_least(&engine.substituted(&maximum.sub(value)), &BigInt::zero())
            })
    };
    let entry_membership = rank_is_member(&rank);
    let Some(arguments) = arguments else {
        return Some(RankingRangeEdgeProof {
            membership_and_pinning: entry_membership || engine.requires_unsatisfiable,
            strictly_decreases: false,
            non_increasing: false,
        });
    };
    let mut substitutions = BTreeMap::new();
    // Actual arguments retain the complete non-self formal ordinal. Filtering
    // numeric bindings must not shift a payload slot onto the next rank input.
    let target_parameters =
        program.state_parameters(destination.map_or(state, |destination| destination.state));
    for (parameter, argument) in target_parameters.iter().zip(arguments) {
        if let Some(field) = &field_rank {
            field.substitute(
                program,
                machine,
                state,
                entry_parameters,
                destination,
                parameter,
                &demanded_coordinates,
                &mut engine,
                parameter.symbol,
                *argument,
                &mut substitutions,
            )?;
        }
        if let Some(slice) = &slice_rank {
            slice.substitute(
                program,
                machine,
                state,
                entry_parameters,
                destination,
                &mut engine,
                parameter.symbol,
                *argument,
                &length_bindings,
                &mut substitutions,
            )?;
        }
        let target_readings = match destination {
            Some(destination) => destination
                .entry_parameters
                .coordinates
                .iter()
                .filter(|selected| selected.destination.parameter == parameter.symbol)
                .cloned()
                .collect::<Vec<_>>(),
            None => vec![RankCoordinateBinding {
                entry: RankSubjectCoordinate {
                    parameter: parameter.symbol,
                    path: Vec::new(),
                },
                destination: RankSubjectCoordinate {
                    parameter: parameter.symbol,
                    path: Vec::new(),
                },
            }],
        };
        for reading in target_readings {
            // Substitute only what the destination witness observes. An
            // unrelated Wrapping accumulator is payload, not integer algebra.
            // Dependencies come from authored terms, preserving even operands
            // that cancel out of a normalized endpoint.
            if !demanded_coordinates.contains(&reading.entry) || !reading.entry.path.is_empty() {
                continue;
            }
            if let Some((_, identity)) = length_bindings
                .iter()
                .find(|(symbol, _)| *symbol == reading.entry.parameter)
            {
                let actual = arrival_length_actual(
                    program,
                    machine,
                    state,
                    destination.map_or(state, |destination| destination.state),
                    &reading.destination,
                    &mut engine,
                    *argument,
                    &length_bindings,
                )?;
                if let Some(existing) = substitutions.get(identity) {
                    if !engine.requires_unsatisfiable
                        && !comparison_proven(&engine, BinaryOperator::Equal, existing, &actual)
                    {
                        return None;
                    }
                } else {
                    substitutions.insert(identity.clone(), actual);
                }
                continue;
            }
            let Some(binding) = bindings
                .iter()
                .find(|binding| binding.symbol == reading.entry.parameter)
            else {
                continue;
            };
            let StrictArithmeticBindingValue::Atom { identity, .. } = &binding.value else {
                return None;
            };
            let actual = arrival_scalar_actual(
                program,
                machine,
                state,
                destination.map_or(state, |destination| destination.state),
                &reading.destination,
                &mut engine,
                *argument,
            )?;
            if let Some(existing) = substitutions.get(identity) {
                if !engine.requires_unsatisfiable
                    && !comparison_proven(&engine, BinaryOperator::Equal, existing, &actual)
                {
                    return None;
                }
            } else {
                substitutions.insert(identity.clone(), actual);
            }
        }
    }
    // Remainder/quotient atoms retain both operand polynomials, so the map
    // cannot reach them by name: each minted term whose operands substitute
    // completely re-mints under both transported operands. An operand leaf the
    // loop above did not cover keeps its atom unmapped and fails closed here.
    engine.extend_argument_map_over_opaque_terms(&mut substitutions);
    let next_rank = inductive_judgment::apply_argument_map(&rank, &substitutions)?;
    let next_floor = inductive_judgment::apply_argument_map(&floor, &substitutions)?;
    let next_ceiling = inductive_judgment::apply_argument_map(&ceiling, &substitutions)?;
    let pinned_view_bound = match measure {
        RankingRangeMeasure::IncreasingTo { limit, .. }
        | RankingRangeMeasure::Distance { upper: limit, .. } => {
            let bound = engine.normalize(limit)?;
            let next = inductive_judgment::apply_argument_map(&bound, &substitutions)?;
            Some((bound, next))
        }
        RankingRangeMeasure::Single(_)
        | RankingRangeMeasure::Computed { .. }
        | RankingRangeMeasure::SliceLength(_)
        | RankingRangeMeasure::Field { .. } => None,
    };
    for (operator, left, right) in auxiliary
        .iter()
        .filter(|_| matches!(premises, RankingRangePremises::EntryInvariant))
    {
        let next_left = inductive_judgment::apply_argument_map(left, &substitutions)?;
        let next_right = inductive_judgment::apply_argument_map(right, &substitutions)?;
        if !engine.requires_unsatisfiable
            && !comparison_proven(&engine, *operator, &next_left, &next_right)
        {
            return None;
        }
    }
    // Even unreachable arrivals retain exact rank-input coverage. Vacuity
    // discharges comparisons, not missing or ambiguous substitutions.
    if engine.requires_unsatisfiable {
        return Some(RankingRangeEdgeProof {
            membership_and_pinning: true,
            strictly_decreases: true,
            non_increasing: true,
        });
    }
    // Normalization may cancel an undefined destination operation. Endpoint
    // equality alone cannot transport its formation, so replay every node
    // with the same simultaneous mapping used by the arrival's value proof.
    for endpoint in range.into_iter().flat_map(|range| [range.start, range.end]) {
        if !fields::endpoint_operations_land_after_arrival(
            &mut engine,
            program,
            machine,
            root,
            endpoint,
            &substitutions,
        ) {
            return None;
        }
    }
    let prove = |difference: Polynomial, minimum: i64| {
        engine.prove_at_least(&engine.substituted(&difference), &BigInt::from_i64(minimum))
    };
    if let Some((bound, next)) = pinned_view_bound
        && (!prove(bound.sub(&next), 0) || !prove(next.sub(&bound), 0))
    {
        return None;
    }
    // Prove equality to the old endpoints, not merely membership in a newly
    // moved interval. Simultaneous substitution cannot telescope n -> n - 1.
    if !prove(next_floor.sub(&floor), 0)
        || !prove(floor.sub(&next_floor), 0)
        || !prove(next_ceiling.sub(&ceiling), 0)
        || !prove(ceiling.sub(&next_ceiling), 0)
    {
        return None;
    }
    let next_membership = if let Some(range) = range {
        calls::membership(
            &engine,
            measure,
            &next_rank,
            &floor,
            &ceiling,
            range.end_inclusive,
        )
    } else {
        clamped || prove(next_rank.clone(), 0)
    };
    let membership_and_pinning = entry_membership
        && next_membership
        && carrier_maximum
            .as_ref()
            .is_none_or(|maximum| prove(maximum.sub(&next_rank), 0));
    let difference = rank.sub(&next_rank);
    let natural_next = clamped || prove(next_rank.clone(), 0);
    let non_increasing = natural_next
        && (prove(difference.clone(), 0)
            || (clamped && prove(Polynomial::default().sub(&next_rank), 0)));
    let strictly_decreases =
        natural_next && (!clamped || prove(rank.clone(), 1)) && prove(difference, 1);
    Some(RankingRangeEdgeProof {
        membership_and_pinning,
        strictly_decreases,
        non_increasing,
    })
}

/// The produced rank of a declared computation view: the measure body,
/// normalized once over a private parameter atom, with that atom replaced by
/// the subject's polynomial in `engine`'s namespace. Substituting instead of
/// binding the parameter symbol lets two members that select one measure --
/// and therefore share its parameter symbol -- each produce their own rank
/// in one engine, and lets destination substitution of the subject reach the
/// rank through the subject's atoms.
/// The hypothesis-install + deferred-endpoint landing tail both callers
/// share: install `comparisons`, refresh opaque intervals, then on a live
/// engine every deferred endpoint's normalized polynomial (floor at the
/// range start, ceiling at the end) must land under the new hypotheses.
#[allow(clippy::too_many_arguments)]
pub(super) fn hypothesis_landed_endpoints(
    engine: &mut Engine<'_>,
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    range_start: ExpressionHandle,
    floor: &Polynomial,
    ceiling: &Polynomial,
    deferred_endpoints: &[ExpressionHandle],
    comparisons: Vec<(BinaryOperator, Polynomial, Polynomial)>,
) -> Option<()> {
    if !engine.install_hypotheses(comparisons) {
        return None;
    }
    engine.refresh_opaque_intervals();
    // A computed endpoint that declaration bounds alone could not place still
    // owes its carrier landing under this edge's hypotheses: a requires or
    // constrained-parameter fact that bounds a leaf reaches the produced
    // polynomial where the declaration-interval owner saw only the leaves'
    // store ranges. Dead edges stay vacuous; a live edge that cannot land its
    // endpoint has no defined range to read.
    if !engine.requires_unsatisfiable {
        for endpoint in deferred_endpoints {
            let polynomial = if *endpoint == range_start {
                floor
            } else {
                ceiling
            };
            if !fields::endpoint_lands_under(engine, program, machine, state, *endpoint, polynomial)
            {
                return None;
            }
        }
    }
    Some(())
}

pub(super) fn computed_rank(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    engine: &mut Engine<'_>,
    subject: ExpressionHandle,
    parameter: symbols::SymbolHandle,
    body: ExpressionHandle,
) -> Option<Polynomial> {
    let subject = engine.normalize(subject)?;
    let body = identity_views::computed_measure_polynomial(program, machine, parameter, body)?;
    inductive_judgment::apply_argument_map(
        &body,
        &BTreeMap::from([(
            identity_views::COMPUTATION_PARAMETER_ATOM.to_owned(),
            subject,
        )]),
    )
}

/// The greatest value of an unsigned carrier, as the formation ceiling of a
/// computed rank.
pub(super) fn carrier_maximum(carrier: symbols::BuiltinTypeAtom) -> Option<Polynomial> {
    let maximum = match carrier {
        symbols::BuiltinTypeAtom::U8 => u64::from(u8::MAX),
        symbols::BuiltinTypeAtom::U16 => u64::from(u16::MAX),
        symbols::BuiltinTypeAtom::U32 => u64::from(u32::MAX),
        symbols::BuiltinTypeAtom::U64 => u64::MAX,
        _ => return None,
    };
    Some(Polynomial::constant(BigInt::from_u64(maximum)))
}

/// The representable range of an exact integer primitive as BigInt endpoints.
/// The shared interval engine cannot hold `u64::MAX` or `i64::MIN`, so a
/// flow-dependent endpoint formation proof bounds the produced polynomial with
/// this exact carrier range instead of a signed-window approximation.
pub(super) fn integer_carrier_bounds(primitive: PrimitiveType) -> Option<(BigInt, BigInt)> {
    let (minimum, maximum) = primitive.integer_bounds()?;
    Some((BigInt::from_i128(minimum), BigInt::from_i128(maximum)))
}

/// Read entry facts in the root template, even when their current aliases
/// belong to a named state. Nothing here derives a fact from a state spelling.
fn entry_comparisons(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    root: &State,
    engine: &mut Engine<'_>,
    bindings: &[StrictArithmeticSymbolBinding],
) -> Option<Vec<Comparison>> {
    let mut comparisons = contract_comparisons(program, machine, root, engine)?;
    comparisons.extend(parameter_comparisons(
        program, machine, root, engine, bindings,
    )?);
    Some(comparisons)
}

/// Requires-fact hypotheses at the entry state. A subordinate call site must
/// not substitute them: only the member's own entry-invariant proof can
/// re-establish a requires clause after an internal arrival.
fn contract_comparisons(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    root: &State,
    engine: &mut Engine<'_>,
) -> Option<Vec<Comparison>> {
    let admit = |expression| meanings::admitted_carrier(program, machine, root, expression, 0);
    let mut comparisons = Vec::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            // Membership and proposition facts remain outside this arithmetic
            // projection. They are neither assumed nor claimed re-established.
            if let Some(expression) = fact.as_expression() {
                admit(*expression)?;
                collect_guard(engine, *expression, true, &mut comparisons, 0)?;
            }
        }
    }
    Some(comparisons)
}

/// Constrained-type hypotheses for a state's own formals. They hold on every
/// arrival and remain valid at any call site inside that state.
fn parameter_comparisons(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    root: &State,
    engine: &mut Engine<'_>,
    bindings: &[StrictArithmeticSymbolBinding],
) -> Option<Vec<Comparison>> {
    let admit = |expression| meanings::admitted_carrier(program, machine, root, expression, 0);
    let mut comparisons = Vec::new();
    for parameter in program.state_parameters(root).iter() {
        let Some(primitive) = integer_parameter(program, parameter.type_reference) else {
            continue;
        };
        // A bound fixed-integer formal holds a value of its carrier at every
        // evaluation, whatever its arithmetic policy: storage enforces the
        // width. Both carrier endpoints are facts of this frame; a Wrapping
        // update's no-wrap proof reads the upper one beside the edge guard.
        if let Some(StrictArithmeticBindingValue::Atom { identity, .. }) = bindings
            .iter()
            .find(|binding| binding.symbol == parameter.symbol)
            .map(|binding| &binding.value)
            && let Some((minimum, maximum)) = integer_carrier_bounds(primitive)
        {
            comparisons.push((
                BinaryOperator::GreaterOrEqual,
                Polynomial::atom(identity.clone()),
                Polynomial::constant(minimum),
            ));
            comparisons.push((
                BinaryOperator::LessOrEqual,
                Polynomial::atom(identity.clone()),
                Polynomial::constant(maximum),
            ));
        }
        for (minimum, maximum, end_inclusive) in
            crate::value_custody::type_shapes::range_constraints(program, parameter.type_reference)
        {
            // Missing or ambiguous aliases cannot contribute an auxiliary
            // invariant, even when another premise makes the edge dead.
            let binding = bindings
                .iter()
                .find(|binding| binding.symbol == parameter.symbol)?;
            let StrictArithmeticBindingValue::Atom { identity, .. } = &binding.value else {
                return None;
            };
            admit(minimum)?;
            admit(maximum)?;
            let maximum = engine.normalize(maximum)?;
            let maximum = if end_inclusive {
                maximum
            } else {
                maximum.sub(&Polynomial::constant(BigInt::from_i64(1)))
            };
            comparisons.push((
                BinaryOperator::GreaterOrEqual,
                Polynomial::atom(identity.clone()),
                engine.normalize(minimum)?,
            ));
            comparisons.push((
                BinaryOperator::LessOrEqual,
                Polynomial::atom(identity.clone()),
                maximum,
            ));
        }
    }
    Some(comparisons)
}

fn comparison_proven(
    engine: &Engine<'_>,
    operator: BinaryOperator,
    left: &Polynomial,
    right: &Polynomial,
) -> bool {
    let prove = |difference: Polynomial, minimum: i64| {
        engine.prove_at_least(&engine.substituted(&difference), &BigInt::from_i64(minimum))
    };
    match operator {
        BinaryOperator::Less => prove(right.sub(left), 1),
        BinaryOperator::LessOrEqual => prove(right.sub(left), 0),
        BinaryOperator::Greater => prove(left.sub(right), 1),
        BinaryOperator::GreaterOrEqual => prove(left.sub(right), 0),
        BinaryOperator::Equal => prove(left.sub(right), 0) && prove(right.sub(left), 0),
        BinaryOperator::NotEqual => prove(left.sub(right), 1) || prove(right.sub(left), 1),
        _ => false,
    }
}

fn validate_mapping(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    entry_parameters: &RankStateBindings,
) -> Option<()> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    if entry_parameters.state != state.symbol
        || !states
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
    {
        return None;
    }
    for (position, selected) in entry_parameters.coordinates.iter().enumerate() {
        if entry_parameters.coordinates[..position]
            .iter()
            .any(|previous| previous.entry == selected.entry)
        {
            return None;
        }
        let entry_type = ranking_coordinate_type(program, root, &selected.entry)?;
        let destination_type = ranking_coordinate_type(program, state, &selected.destination)?;
        if state.symbol == root.symbol && selected.entry != selected.destination {
            return None;
        }
        if integer_parameter(program, entry_type) != integer_parameter(program, destination_type) {
            return None;
        }
        // Ordinary typed arrivals own nonnumeric carrier compatibility. This
        // query cannot infer an arithmetic value from two absent carriers.
        if entry_parameters.destination(&selected.entry)? != selected.destination {
            return None;
        }
    }
    Some(())
}

fn integer_bindings(
    program: &dyn crate::CheckingInput,
    state: &State,
) -> Option<Vec<StrictArithmeticSymbolBinding>> {
    let mut bindings = Vec::new();
    for parameter in program.state_parameters(state).iter() {
        let Some(primitive) = integer_parameter(program, parameter.type_reference) else {
            // Unrelated payloads are never promoted to numeric facts.
            continue;
        };
        if !parameter.symbol.is_valid() {
            return None;
        }
        bindings.push(StrictArithmeticSymbolBinding::unsigned_integer_atom(
            parameter.symbol,
            format!("\0ranking:{:?}", parameter.symbol),
            primitive,
        ));
    }
    Some(bindings)
}

/// The `ProofFact::Expression` bodies every `Requires` contract on `machine`
/// carries — the premise vocabulary a view's read surface always includes.
pub(super) fn required_contract_expressions(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> Vec<ExpressionHandle> {
    program
        .contract_expressions(
            program.machine_contracts(machine).iter(),
            SignatureContractKind::Requires,
        )
        .collect()
}

/// A stored integer is a numeric coordinate under any arithmetic policy.
/// Policy belongs to selected computations over that value; refusing a Wrapping
/// count here loses an ordinary operand of an Exact shift. Normalization and
/// endpoint formation independently keep the operation's policy obligations.
fn integer_parameter(
    program: &dyn crate::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> Option<PrimitiveType> {
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        if program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .any(|constraint| {
                !matches!(
                    constraint,
                    TypeConstraintNode::Range { .. } | TypeConstraintNode::ArithmeticDomain(_)
                )
            })
        {
            return None;
        }
        reference = *base_type;
    }
    let primitive = crate::value_custody::recasts::exact_primitive_type(program, reference)?;
    primitive.is_fixed_integer().then_some(primitive)
}

fn exact_integer_parameter(
    program: &dyn crate::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<PrimitiveType> {
    (program.arithmetic_domain_for_type_reference(reference)
        == numerics::arithmetic::ArithmeticDomain::Exact)
        .then(|| integer_parameter(program, reference))
        .flatten()
}

type Comparison = (BinaryOperator, Polynomial, Polynomial);

/// Project already meaning-checked hypotheses into integer comparisons.
/// Unreadable Boolean facts contribute nothing; they cannot strengthen a rank
/// proof, but need not prevent an independently proven forwarding edge.
/// The result records whether every fact was readable, so inductive contract
/// checking can use the same projection without treating partial facts as a
/// complete context for rejection. False conjunctions and true disjunctions
/// cannot contribute their individual operands.
pub(super) fn collect_guard(
    engine: &mut Engine<'_>,
    expression: ExpressionHandle,
    holds: bool,
    comparisons: &mut Vec<Comparison>,
    depth: usize,
) -> Option<bool> {
    if depth >= 128 {
        return None;
    }
    match engine.program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            collect_guard(engine, unary.operand, !holds, comparisons, depth + 1)
        }
        ExpressionNode::Atomic(atomic) => {
            collect_guard(engine, atomic.value, holds, comparisons, depth + 1)
        }
        ExpressionNode::Binary(binary)
            if (binary.operator == BinaryOperator::And && holds)
                || (binary.operator == BinaryOperator::Or && !holds) =>
        {
            let left = collect_guard(engine, binary.left, holds, comparisons, depth + 1)?;
            let right = collect_guard(engine, binary.right, holds, comparisons, depth + 1)?;
            Some(left && right)
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            for (condition, boolean) in [(binary.left, binary.right), (binary.right, binary.left)] {
                if let Some(polarity) = engine.program.expression_table.boolean_node(boolean) {
                    return collect_guard(
                        engine,
                        condition,
                        holds == (*polarity == (binary.operator == BinaryOperator::Equal)),
                        comparisons,
                        depth + 1,
                    );
                }
            }
            if let Some(comparison) =
                inductive_judgment::guard_arm_comparison(engine, expression, holds)
            {
                comparisons.push(comparison);
                Some(true)
            } else {
                Some(false)
            }
        }
        _ => {
            if let Some(comparison) =
                inductive_judgment::guard_arm_comparison(engine, expression, holds)
            {
                comparisons.push(comparison);
                Some(true)
            } else {
                Some(false)
            }
        }
    }
}
