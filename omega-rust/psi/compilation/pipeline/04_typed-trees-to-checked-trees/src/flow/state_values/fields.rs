//! Live attached-field facts at ordinary state edges. No declaration seeds them.
use crate::flow::FlowBuildContext;
use crate::flow::canonical_place_from_semantic_place;
use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
};
use symbols::SymbolHandle;
use validation::field_domains::ByteSequencePredicate;

/// Identifies one incoming edge's contribution to a joined field row. A
/// transition edge keys on its source state and arm target; a call-derived
/// edge keys on its source state and call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BoundsSource {
    pub(super) state: SymbolHandle,
    pub(super) tag: u64,
}

impl BoundsSource {
    pub(super) fn transition(
        state: SymbolHandle,
        target: typed_trees::statement::TransitionTargetHandle,
    ) -> Self {
        Self {
            state,
            tag: target.arena_index() as u64,
        }
    }

    pub(super) fn invocation(state: SymbolHandle, statement: usize, ordinal: usize) -> Self {
        Self {
            state,
            tag: (1 << 40) | ((statement as u64) << 8) | (ordinal as u64 + 1),
        }
    }

    /// Whether this edge is a call's return handoff. Call edges carry no field
    /// rows of their own (the callee's declared transport covers the call's
    /// own effect), so their absence must abstain from the ceiling rather
    /// than refute it; a transition edge that simply lacks the field is a
    /// real path where the carrier saw no fresh evidence and refutes.
    fn is_invocation(self) -> bool {
        self.tag >= (1 << 40)
    }
}

/// One predecessor edge's latest delivery for a field. An edge whose captured
/// facts say nothing about the field records `empty()` -- an unconstrained
/// path -- so every predecessor bounds the join, not just edges that happened
/// to carry evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
struct EdgeDelivery {
    literal: ExpressionHandle,
    predicates: Vec<ByteSequencePredicate>,
    integer_bounds: Option<facts::IntegerRange>,
    /// The greatest predicate set this edge could carry: the delivered
    /// predicates unioned with the byte-class evidence of any element stores
    /// on its path. Carrier classes are loop invariants proved
    /// co-inductively, so what the edge can sustain -- not only what it
    /// currently mints -- bounds the candidate the join verifies.
    potential: Vec<ByteSequencePredicate>,
}

impl FieldValue {
    /// Record the edge this row was first built from, so later meets recompute
    /// the join over per-edge deliveries instead of treating this one as
    /// already joined.
    pub(super) fn seed_delivery(&mut self, source: BoundsSource) {
        self.deliveries = vec![(source, EdgeDelivery::of(self))];
        self.predicate_ceiling = self.edge_potential.clone();
    }

    /// The co-inductive premise an element-store reseed may assume for the
    /// carrier: the greatest predicate claim this state's edges could jointly
    /// sustain.
    pub(super) fn predicate_ceiling(&self) -> &[ByteSequencePredicate] {
        &self.predicate_ceiling
    }

    pub(super) fn segments(&self) -> &[facts::PlaceSegment] {
        &self.segments
    }

    pub(super) fn root(&self) -> facts::PlaceRoot {
        self.root
    }
}

impl EdgeDelivery {
    fn empty() -> Self {
        Self::missing(&[])
    }

    /// An edge that produced no row for this field: no literal, no predicates,
    /// no bound -- but its deliverable claim forwards `potential`, the joined
    /// could-sustain set the destination already knows, so the edge neither
    /// mints evidence it lacks nor refutes a claim it carries no evidence
    /// against.
    fn missing(potential: &[ByteSequencePredicate]) -> Self {
        Self {
            literal: ExpressionHandle::invalid(),
            predicates: Vec::new(),
            integer_bounds: None,
            potential: potential.to_vec(),
        }
    }

    fn of(field: &FieldValue) -> Self {
        Self {
            literal: field.literal,
            predicates: field.predicates.clone(),
            integer_bounds: field.integer_bounds.clone(),
            potential: field.edge_potential.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FieldValue {
    /// The destination-side identity of the carrier storage: the machine
    /// symbol for an attached `self` field, or the destination parameter's
    /// own symbol for a `&mut`/shared transition argument. `&mut self` is an
    /// ordinary `&mut T` argument's receiver spelling; the two roots name the
    /// same kind of evidence at one hop apart.
    root: facts::PlaceRoot,
    segments: Vec<facts::PlaceSegment>,
    literal: ExpressionHandle,
    predicates: Vec<ByteSequencePredicate>,
    integer_bounds: Option<facts::IntegerRange>,
    /// Each predecessor edge's latest delivery. Every joined channel is
    /// recomputed from these rows, so an edge that later proves richer
    /// evidence retightens the join; meeting directly into the stored channels
    /// could only shrink them, and a mid-fixpoint under-approximation (a bound
    /// still unconstrained on one pass) would wedge the row forever.
    deliveries: Vec<(BoundsSource, EdgeDelivery)>,
    /// What this field's edge is able to deliver for its carrier predicates:
    /// the captured predicates plus, for an edge leaving a state that stored
    /// into the carrier, the stores' accumulated byte-class evidence. Only
    /// meaningful on a captured edge record; a stored row never consults it.
    edge_potential: Vec<ByteSequencePredicate>,
    /// The greatest predicate claim the current edges could jointly sustain:
    /// the intersection over each delivery's potential. An element store
    /// re-proves the carrier's byte classes under the co-inductive hypothesis
    /// that this candidate stands -- it is the loop invariant the edges are
    /// collectively maintaining. A weak mid-fixpoint delivery (a bound still
    /// unconstrained on one pass) shrinks the stored join the next premise
    /// reads; the ceiling still answers what the edges CAN deliver, so the
    /// premise recovers when the byte-side evidence does. An edge whose
    /// evidence is genuinely absent refutes the candidate by itself, so the
    /// join can never ratify a predicate some path disproves.
    predicate_ceiling: Vec<ByteSequencePredicate>,
    /// Joins that extended the accumulated interval. Repeated growth widens
    /// to authored thresholds or the primitive carrier. Unchanged deliveries
    /// preserve that widening; genuinely changed evidence can retighten it.
    bounds_growth: u8,
    /// Joins that narrowed the accumulated interval. Repeated shrinking on a
    /// bound that is being recomputed one step per join -- a loop clamping a
    /// counter feeds each pass a slightly tighter endpoint -- holds the
    /// shrunken axis so the slide cannot take one pass per integer.
    bounds_shrink: u8,
}

const BOUNDS_WIDENING_DELAY: u8 = 4;
const BOUNDS_NARROWING_LIMIT: u8 = 8;

pub(super) fn height(fields: &[FieldValue], threshold_count: usize) -> usize {
    // Either endpoint can traverse the finite threshold set, then reach its
    // primitive carrier. Narrowing can revisit those thresholds, so reserve
    // a traversal for every permitted narrowing and the final outward pass.
    // This remains a finite work allowance; exhaustion still discards all
    // provisional evidence in the flow builder.
    let bounds_height = threshold_count
        .saturating_add(1)
        .saturating_mul(2)
        .saturating_mul(usize::from(BOUNDS_NARROWING_LIMIT) + 1)
        .saturating_add(usize::from(BOUNDS_WIDENING_DELAY))
        .saturating_add(usize::from(BOUNDS_NARROWING_LIMIT));
    fields
        .iter()
        .map(|field| {
            1 + field.predicates.len()
                + if field.integer_bounds.is_some() {
                    bounds_height
                } else {
                    0
                }
        })
        .sum()
}

/// The data definition a captured row's root denotes: the attached record for
/// the machine root, or the readable referent type behind the destination
/// parameter's own symbol for a transition argument. `&mut self` and a named
/// `&mut T` parameter are the same carrier here, one root spelling apart.
fn root_data_definition<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    root: facts::PlaceRoot,
) -> Option<&'program typed_trees::data::DataDefinition> {
    let symbol = root.as_symbol()?;
    if symbol == machine.symbol {
        let attached = machine.attached_data.as_ref()?;
        return program.data_definition_by_name_str(attached.as_str());
    }
    let parameter = program.state_parameter_by_symbol(state, symbol)?;
    validation::field_domains::readable_nominal_definition(program, parameter.type_reference)
}

/// The declared primitive extent of a field path, when it resolves to a
/// primitive scalar: the widest interval a widening join can need.
fn carrier_integer_range(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    root: facts::PlaceRoot,
    segments: &[facts::PlaceSegment],
) -> Option<facts::IntegerRange> {
    let mut data = root_data_definition(program, machine, state, root)?;
    let mut reference = None;
    for (index, segment) in segments.iter().enumerate() {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        let next = program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == *symbol => field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference),
                _ => None,
            })?;
        reference = Some(next);
        if index + 1 < segments.len() {
            data = validation::field_domains::data_definition_for_field_type(program, next)?;
        }
    }
    let primitive = program.primitive_type_reference(reference?)?;
    validation::carrier_integer_range(primitive)
}

/// Integer literals the program compares against, sorted ascending. They are
/// the widening thresholds: a still-extending bound settles where a guard or
/// contract compares it with a constant (named constants are substituted by
/// now, so `i < LIMIT` reads as a literal comparison). A growing bound climbs
/// one threshold per flow pass, so every other literal -- an increment, an
/// index, a byte value -- only added passes: std's 193 literals made one
/// loop counter take 183 passes, where its 81 compared literals suffice.
/// The collection is program-pure; `FlowBuildContext` memoizes it so the
/// arena scan runs at most once per flow build rather than per widening
/// rejoin.
pub(in crate::flow) fn integer_literal_thresholds(
    program: &dyn ::validation::CheckingInput,
) -> Vec<numerics::bignum::BigInt> {
    use typed_trees::expression::BinaryOperator;
    let mut literals: Vec<numerics::bignum::BigInt> = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                        | BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                ) =>
            {
                Some([binary.left, binary.right])
            }
            _ => None,
        })
        .flatten()
        .filter_map(
            |operand| match program.expression_table.expression(operand) {
                ExpressionNode::Integer(literal) => literal.value_bignum(),
                _ => None,
            },
        )
        .collect();
    literals.sort();
    literals.dedup();
    literals
}

/// Missing evidence is absorbing for a field, but only per edge: each join
/// records what THIS edge delivered and recomputes the row from every edge's
/// latest delivery, so a field an edge stops mentioning loses that edge's
/// evidence yet can be re-delivered by a later pass.
///
/// Channels join differently over the deliveries. Integer bounds union: a
/// value entering the state may come from any edge, so only the union of the
/// edges' ranges covers it; an edge that captured no bound contributes the
/// declared carrier. Byte predicates intersect: a predicate holds only when
/// every edge proves it, so an edge that delivered none empties the join.
/// A literal survives only when every edge delivered that same literal.
/// A bound that keeps extending across joins -- a counter backedge that adds
/// one each pass -- widens to authored thresholds or the declared primitive
/// carrier. Identical arrivals retain that widening rather than shrinking
/// back to the raw union without new evidence.
pub(super) fn meet(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    destination: &typed_trees::state::State,
    previous: &mut Vec<FieldValue>,
    incoming: &[FieldValue],
    source: BoundsSource,
    thresholds: &[numerics::bignum::BigInt],
) -> bool {
    let mut changed = false;
    // Every edge that has ever arrived at this destination. A field first
    // delivered on a later edge still records the earlier edges as carrying no
    // evidence for it.
    let mut known_sources: Vec<BoundsSource> = Vec::new();
    for field in previous.iter() {
        for (key, _) in &field.deliveries {
            if !known_sources.contains(key) {
                known_sources.push(*key);
            }
        }
    }
    previous.retain_mut(|field| {
        let delivery = incoming
            .iter()
            .find(|next| next.root == field.root && next.segments == field.segments)
            .map(EdgeDelivery::of)
            // A call's return edge never captures field rows, so its absence
            // forwards the running ceiling rather than refuting it -- the
            // callee's own premise check is what limits that handoff. A
            // transition edge that lacks the field carries no claim of its
            // own and refutes outright.
            .unwrap_or_else(|| {
                if source.is_invocation() {
                    EdgeDelivery::missing(&field.predicate_ceiling)
                } else {
                    EdgeDelivery::empty()
                }
            });
        if let Some(slot) = field.deliveries.iter_mut().find(|(key, _)| *key == source) {
            // Replaying the same evidence must preserve any sound widening.
            // Rejoining the raw deliveries would shrink that bound without
            // learning anything new and unnecessarily dirty the destination.
            // Compare the full delivery, including a missing call edge's
            // freshly derived potential, so changed ceilings still propagate.
            if slot.1 == delivery {
                return true;
            }
            slot.1 = delivery;
        } else {
            field.deliveries.push((source, delivery));
        }
        changed |= field.rejoin(program, machine, destination, thresholds);
        // Keep the row even when the join is empty: its delivery map remembers
        // which edges carry no evidence, so a re-delivered field still
        // intersects over those edges instead of claiming a fresh start.
        true
    });
    for next in incoming {
        if previous
            .iter()
            .any(|field| field.root == next.root && field.segments == next.segments)
        {
            continue;
        }
        let mut deliveries: Vec<(BoundsSource, EdgeDelivery)> = known_sources
            .iter()
            .map(|key| {
                (
                    *key,
                    if key.is_invocation() {
                        EdgeDelivery::missing(&next.edge_potential)
                    } else {
                        EdgeDelivery::empty()
                    },
                )
            })
            .collect();
        deliveries.push((source, EdgeDelivery::of(next)));
        let mut field = FieldValue {
            root: next.root,
            segments: next.segments.clone(),
            literal: ExpressionHandle::invalid(),
            predicates: Vec::new(),
            integer_bounds: None,
            deliveries,
            edge_potential: Vec::new(),
            predicate_ceiling: Vec::new(),
            bounds_growth: 0,
            bounds_shrink: 0,
        };
        field.rejoin(program, machine, destination, thresholds);
        previous.push(field);
        changed = true;
    }
    changed
}

impl FieldValue {
    /// Recompute the joined literal, predicate set, and integer bound from the
    /// per-edge deliveries. `thresholds` is the memoized program literal set
    /// widening consults when a bound keeps extending.
    fn rejoin(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        thresholds: &[numerics::bignum::BigInt],
    ) -> bool {
        let mut changed = false;
        let literal = self
            .deliveries
            .first()
            .filter(|(_, first)| {
                first.literal.is_valid()
                    && self
                        .deliveries
                        .iter()
                        .all(|(_, delivery)| delivery.literal == first.literal)
            })
            .map(|(_, first)| first.literal)
            .unwrap_or_else(ExpressionHandle::invalid);
        // Two invalid sentinels differ in bits but denote the same absence.
        let literal_unchanged = match (self.literal.is_valid(), literal.is_valid()) {
            (true, true) => self.literal == literal,
            (false, false) => true,
            _ => false,
        };
        if !literal_unchanged {
            self.literal = literal;
            changed = true;
        }
        let predicates: Vec<ByteSequencePredicate> = ByteSequencePredicate::ALL
            .into_iter()
            .filter(|predicate| {
                self.deliveries
                    .iter()
                    .all(|(_, delivery)| delivery.predicates.contains(predicate))
            })
            .collect();
        if self.predicates != predicates {
            self.predicates = predicates;
            changed = true;
        }
        let ceiling: Vec<ByteSequencePredicate> = ByteSequencePredicate::ALL
            .into_iter()
            .filter(|predicate| {
                self.deliveries
                    .iter()
                    .all(|(_, delivery)| delivery.potential.contains(predicate))
            })
            .collect();
        if self.predicate_ceiling != ceiling {
            self.predicate_ceiling = ceiling;
            changed = true;
        }
        let mut joined: Option<facts::IntegerRange> = None;
        let mut unbounded_edge = false;
        for (_, delivery) in &self.deliveries {
            match &delivery.integer_bounds {
                Some(bound) => {
                    joined = Some(match joined {
                        Some(accumulated) => facts::IntegerRange {
                            minimum: accumulated.minimum.min(bound.minimum.clone()),
                            maximum: accumulated.maximum.max(bound.maximum.clone()),
                        },
                        None => bound.clone(),
                    });
                }
                // An edge that captured no bound contributes the declared
                // carrier: the join must still cover what arrives on it. A
                // field with no resolvable carrier simply carries no bound.
                None => {
                    unbounded_edge = true;
                    break;
                }
            }
        }
        let mut joined = if unbounded_edge {
            carrier_integer_range(program, machine, state, self.root, &self.segments)
        } else {
            joined
        };
        if let (Some(stored), Some(fresh)) = (&self.integer_bounds, &joined) {
            let extends_minimum = fresh.minimum < stored.minimum;
            let extends_maximum = fresh.maximum > stored.maximum;
            if extends_minimum || extends_maximum {
                self.bounds_growth = self.bounds_growth.saturating_add(1);
                if self.bounds_growth >= BOUNDS_WIDENING_DELAY
                    && let Some(carrier) =
                        carrier_integer_range(program, machine, state, self.root, &self.segments)
                    && let Some(widened) = joined.as_mut()
                {
                    // Widening jumps to the smallest program literal that
                    // still covers the fresh bound rather than straight to
                    // the carrier: loop counters keep re-extending by their
                    // step until the bound is useless, but the authored
                    // comparisons name the values that actually occur.
                    // An update count cannot refute a later threshold: an
                    // outer loop's bound may grow many times before reaching
                    // its guard, while an inner loop merely preserves it.
                    // Skip to the carrier only beyond the finite threshold
                    // set. `height` accounts for traversal and narrowing;
                    // the builder still fails closed if it cannot settle.
                    if extends_minimum {
                        widened.minimum = thresholds
                            .iter()
                            .rev()
                            .find(|literal| **literal <= widened.minimum)
                            .cloned()
                            .unwrap_or_else(|| carrier.minimum.clone());
                    }
                    if extends_maximum {
                        widened.maximum = thresholds
                            .iter()
                            .find(|literal| **literal >= widened.maximum)
                            .cloned()
                            .unwrap_or_else(|| carrier.maximum.clone());
                    }
                }
            }
        }
        if let (Some(stored), Some(fresh)) = (&self.integer_bounds, &mut joined)
            && (fresh.minimum > stored.minimum || fresh.maximum < stored.maximum)
        {
            self.bounds_shrink = self.bounds_shrink.saturating_add(1);
            if self.bounds_shrink >= BOUNDS_NARROWING_LIMIT {
                // Widening's mirror: a bound that keeps shrinking on the
                // same axis is re-deriving the fixed point one endpoint at
                // a time -- a counter clamped by a runtime value nudges the
                // held bound by its step each pass, so taking every step
                // costs one pass per integer of the climb. Holding the
                // shrunken axis keeps a bound that still covers every
                // delivery, while the starved drift stops producing fresh
                // endpoints for the next join.
                if fresh.minimum > stored.minimum {
                    fresh.minimum = stored.minimum.clone();
                }
                if fresh.maximum < stored.maximum {
                    fresh.maximum = stored.maximum.clone();
                }
            }
        }
        if self.integer_bounds != joined {
            self.integer_bounds = joined;
            changed = true;
        }
        changed
    }
}

/// A Named edge's explicit arguments bind the destination's non-self
/// parameters. Each argument's canonical place names the caller storage the
/// parameter aliases, so a captured fact rooted at that place -- with the
/// argument's own segment prefix stripped -- is evidence about the
/// parameter's referent at entry. `&mut self` rides the machine root through
/// the same join; a named `&mut T` argument is no different one symbol over.
#[allow(clippy::too_many_arguments)]
/// The (value handle, byte predicates, integer bounds) row an integer point
/// carries: no place, no predicates, and the degenerate min == max interval.
fn point_range_point(
    value: numerics::bignum::BigInt,
) -> (
    ExpressionHandle,
    Vec<ByteSequencePredicate>,
    Option<facts::IntegerRange>,
) {
    (
        ExpressionHandle::invalid(),
        Vec::new(),
        Some(facts::IntegerRange {
            minimum: value.clone(),
            maximum: value,
        }),
    )
}

pub(super) fn capture(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &typed_trees::state::State,
    statement_index: usize,
    arguments: &[typed_trees::expression::ExpressionHandle],
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> Vec<FieldValue> {
    // (argument storage root, argument's own prefix, destination parameter).
    // `put(&mut buf)` and `put(buf)` both resolve to the caller's `buf`
    // storage; `put(&mut self.holder)` strips the `holder` field prefix.
    let mut argument_roots: Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>, SymbolHandle)> =
        Vec::new();
    let mut ordinal = 0;
    for parameter in program.state_parameters(destination) {
        let Some(&argument) = arguments.get(ordinal) else {
            break;
        };
        ordinal += 1;
        let Some(place) =
            build.canonical_place_at(program, state.symbol, statement_index, argument)
        else {
            continue;
        };

        if let Some(_) = place.root.as_symbol() {
            argument_roots.push((place.root, place.segments, parameter.symbol));
        }
    }
    if argument_roots.is_empty() {
        return Vec::new();
    }
    // A bound minted at a later point refines what came before it: an arm's
    // guard bound already encodes the state-entry bound it was intersected
    // with, so the newest point's interval is the tightest claim. Payloads
    // sharing the newest point are still unioned across.
    fn bound_point_rank(point: ProgramPoint) -> (usize, u8) {
        match point {
            ProgramPoint::Statement {
                statement_index, ..
            } => (statement_index + 1, 0),
            facts::call_point_triple_pattern!(statement_index, ..) => (statement_index + 1, 1),
            ProgramPoint::TransitionArm {
                statement_index, ..
            }
            | ProgramPoint::Exit {
                statement_index, ..
            } => (statement_index + 1, 2),
            _ => (0, 0),
        }
    }
    let mut fields: Vec<FieldValue> = Vec::new();
    let mut bounds_ranks: Vec<(usize, u8)> = Vec::new();
    for reference in build.contexts.semantic_context_refs.span_or_empty(contexts) {
        for fact in semantic
            .context_view(semantic.contexts.get(reference.context))
            .facts()
        {
            let (literal, predicates, integer_bounds) = match fact.payload {
                FactPayload::AssignedValue { value }
                    if program.expression_table.expression_is_valid(value) =>
                {
                    if let Some(bytes) = typed_trees::byte_predicates::literal_bytes(program, value)
                    {
                        (
                            value,
                            ByteSequencePredicate::ALL
                                .into_iter()
                                .filter(|predicate| predicate.holds_for(&bytes))
                                .collect::<Vec<_>>(),
                            None,
                        )
                    } else {
                        match program.expression_table.expression(value) {
                            ExpressionNode::Integer(literal) => {
                                let Some(value) = literal.value_bignum() else {
                                    continue;
                                };
                                point_range_point(value.clone())
                            }
                            _ if crate::values::is_assignment_literal(program, value) => {
                                (value, Vec::new(), None)
                            }
                            _ => continue,
                        }
                    }
                }
                FactPayload::AssignedScalarValue { value } => {
                    let facts::ScalarValue::Integer(value) = semantic.scalar_values.get(value)
                    else {
                        continue;
                    };
                    point_range_point(value.clone())
                }
                FactPayload::BytePredicate { predicate: proved } => (
                    ExpressionHandle::invalid(),
                    ByteSequencePredicate::ALL
                        .into_iter()
                        .filter(|predicate| proved.implies(*predicate))
                        .collect(),
                    None,
                ),
                FactPayload::AssignedIntegerBounds { bounds }
                    if semantic.integer_ranges.is_valid(bounds) =>
                {
                    let bounds = semantic.integer_ranges.get(bounds);
                    if bounds.minimum > bounds.maximum {
                        continue;
                    }
                    (
                        ExpressionHandle::invalid(),
                        Vec::new(),
                        Some(bounds.clone()),
                    )
                }
                _ => continue,
            };
            // A field value captured inside a subslice-preserving byte class
            // keeps that class at the edge: byte-class evidence survives a
            // join that drops differing exact bounds, so a byte written from
            // two converging computations still re-seeds the carrier class.
            let mut predicates = predicates;
            if let Some(bounds) = &integer_bounds
                && let (Some(minimum), Some(maximum)) =
                    (bounds.minimum.to_i64(), bounds.maximum.to_i64())
                && let (Ok(minimum), Ok(maximum)) = (u8::try_from(minimum), u8::try_from(maximum))
            {
                for predicate in ByteSequencePredicate::ALL.into_iter().filter(|predicate| {
                    predicate.is_subslice_preserving() && predicate.holds_for(&[minimum, maximum])
                }) {
                    if !predicates.contains(&predicate) {
                        predicates.push(predicate);
                    }
                }
            }
            let Some(place) = fact.place.as_place() else {
                continue;
            };
            let Some(place) =
                canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
            else {
                continue;
            };

            // Each destination identity this evidence serves: the shared
            // machine root for an attached `self` field, and every transition
            // argument whose storage prefix the fact's place extends. A
            // `&mut self.holder` actual joins both -- `self.holder.line` for
            // the receiver and `param.line` for the bound parameter.
            let field_segments = |segments: &[facts::PlaceSegment]| {
                segments.iter().all(|segment| match segment {
                    facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
                    facts::PlaceSegment::Case { variant } => variant.is_valid(),
                    facts::PlaceSegment::FixedIndex { .. } => true,
                    _ => false,
                })
            };
            let mut targets: Vec<(facts::PlaceRoot, &[facts::PlaceSegment])> = Vec::new();
            for (root, prefix, parameter) in &argument_roots {
                if place.root != *root
                    || !place.segments.starts_with(prefix)
                    || !field_segments(&place.segments[prefix.len()..])
                {
                    continue;
                }
                targets.push((
                    facts::PlaceRoot::Symbol(*parameter),
                    &place.segments[prefix.len()..],
                ));
            }
            for (root, segments) in targets {
                if let Some(index) = fields
                    .iter()
                    .position(|field| field.root == root && field.segments == segments)
                {
                    let field = &mut fields[index];
                    if literal.is_valid() {
                        field.literal = literal;
                    }
                    if let Some(incoming) = integer_bounds.clone() {
                        let rank = bound_point_rank(fact.point);
                        field.integer_bounds =
                            match (bounds_ranks[index], field.integer_bounds.take()) {
                                (stored, Some(previous)) if stored > rank => Some(previous),
                                (stored, Some(previous)) if stored == rank => {
                                    Some(facts::IntegerRange {
                                        minimum: previous.minimum.min(incoming.minimum),
                                        maximum: previous.maximum.max(incoming.maximum),
                                    })
                                }
                                _ => {
                                    bounds_ranks[index] = rank;
                                    Some(incoming)
                                }
                            };
                    }
                    for predicate in predicates.iter().copied() {
                        if !field.predicates.contains(&predicate) {
                            field.predicates.push(predicate);
                        }
                    }
                } else {
                    bounds_ranks.push(if integer_bounds.is_some() {
                        bound_point_rank(fact.point)
                    } else {
                        (0, 0)
                    });
                    fields.push(FieldValue {
                        root,
                        segments: segments.to_vec(),
                        literal,
                        predicates: predicates.clone(),
                        integer_bounds: integer_bounds.clone(),
                        deliveries: Vec::new(),
                        edge_potential: Vec::new(),
                        predicate_ceiling: Vec::new(),
                        bounds_growth: 0,
                        bounds_shrink: 0,
                    });
                }
            }
        }
    }
    for field in &mut fields {
        // The edge's deliverable claim: the captured predicates, the source
        // state's own predicate ceiling (what ITS incoming edges could
        // jointly sustain), plus the byte-class evidence of this pass's
        // element stores into the carrier. A store mints only what its
        // carrier premise supports; the potential records what the edge
        // could carry if the candidate holds. Carrying the source ceiling is
        // what lets a collapsed mid-fixpoint join recover: a store-free state
        // still forwards "my edges could deliver p" downstream instead of
        // refuting the candidate merely because its own join is currently
        // low. New claims enter only through real evidence -- minted
        // predicates or byte-side store classes -- so a carrier whose every
        // store leaves the class still refutes itself.
        field.edge_potential = field.predicates.clone();
        for predicate in
            super::field_predicate_ceiling(build, state.symbol, field.root, &field.segments)
        {
            if !field.edge_potential.contains(predicate) {
                field.edge_potential.push(*predicate);
            }
        }
        // Element-store potentials are recorded only for attached machine
        // storage (byte_sequences' carrier_ceiling gate); a parameter carrier
        // never minted one, so it must not inherit a same-segments row.
        if field.root == facts::PlaceRoot::Symbol(machine.symbol)
            && let Some((_, _, byteok)) =
                build
                    .element_store_potentials
                    .iter()
                    .find(|(stored_state, segments, _)| {
                        *stored_state == state.symbol && *segments == field.segments
                    })
        {
            for predicate in byteok {
                if !field.edge_potential.contains(predicate) {
                    field.edge_potential.push(*predicate);
                }
            }
        }
    }
    fields
}

pub(super) fn append(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    fields: &[FieldValue],
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    point: ProgramPoint,
) {
    for field in fields {
        let mut segments = HandleSpan::empty();
        for segment in &field.segments {
            semantic
                .place_segments
                .append_to_span(&mut segments, *segment);
        }
        // The row's stored root is already destination-side: the machine
        // symbol for an attached `self` field, the parameter's own symbol
        // for a named `&mut` argument's storage.
        let place = semantic.append_place(facts::Place {
            root: field.root,
            segments,
        });
        let mut references = HandleSpan::empty();
        // A bound that covers its entire carrier states nothing the declared
        // type doesn't already say; minting it anyway would only union the
        // full extent back into tighter live facts at the place.
        let bounds_payload = field
            .integer_bounds
            .as_ref()
            .filter(|bounds| {
                carrier_integer_range(program, machine, state, field.root, &field.segments)
                    .is_none_or(|carrier| !validation::integer_range_contains(bounds, &carrier))
            })
            .map(|bounds| FactPayload::AssignedIntegerBounds {
                bounds: semantic.integer_ranges.append(bounds.clone()),
            });
        let payloads = field
            .literal
            .is_valid()
            .then_some(FactPayload::AssignedValue {
                value: field.literal,
            })
            .into_iter()
            .chain(
                field
                    .predicates
                    .iter()
                    .map(|predicate| FactPayload::BytePredicate {
                        predicate: *predicate,
                    }),
            )
            .chain(bounds_payload);
        for payload in payloads {
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::default(),
                payload,
            });
            semantic.append_ref(&mut references, fact);
        }
        semantic.append_context(point, references);
    }
}
