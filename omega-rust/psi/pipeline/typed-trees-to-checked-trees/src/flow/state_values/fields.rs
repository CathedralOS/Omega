//! Live attached-field facts at ordinary state edges. No declaration seeds them.
use crate::facts::field_domain::ByteSequencePredicate;
use crate::flow::FlowBuildContext;
use crate::flow::canonical_place_from_semantic_place;
use crate::flow::normalize_attached_place_root;
use crate::flow::normalized_event_place_root;
use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
};
use symbols::SymbolHandle;

#[cfg(test)]
mod tests;

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
    /// Joins that extended the accumulated interval. A bound that keeps
    /// growing -- an incrementing counter's backedge -- is widened to its
    /// primitive carrier so joins converge; an edge set that stabilizes keeps
    /// the exact union.
    bounds_growth: u8,
}

pub(super) fn height(fields: &[FieldValue]) -> usize {
    fields
        .iter()
        .map(|field| 1 + field.predicates.len() + usize::from(field.integer_bounds.is_some()))
        .sum()
}

/// The declared primitive extent of a field path, when it resolves to a
/// primitive scalar: the widest interval a widening join can need.
fn carrier_integer_range(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    segments: &[facts::PlaceSegment],
) -> Option<facts::IntegerRange> {
    let attached = machine.attached_data.as_ref()?;
    let mut data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
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
            data = crate::facts::field_domain::data_definition_for_field_type(program, next)?;
        }
    }
    let primitive = program.primitive_type_reference(reference?)?;
    crate::values::bounds::primitive_range(primitive)
}

/// Integer literals authored anywhere in the program, sorted ascending. They
/// are the widening thresholds: the constants the program itself compares
/// against, which is where a still-extending bound is most likely to settle.
fn integer_literal_thresholds(program: &typed_trees::TypedTrees) -> Vec<numerics::bignum::BigInt> {
    let mut literals: Vec<numerics::bignum::BigInt> = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Integer(literal) => literal.value_bignum(),
            _ => None,
        })
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
/// one each pass -- widens to the declared primitive carrier, which cannot
/// grow again; an edge set that stabilizes keeps the exact union.
pub(super) fn meet(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    previous: &mut Vec<FieldValue>,
    incoming: &[FieldValue],
    source: BoundsSource,
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
            .find(|next| next.segments == field.segments)
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
            slot.1 = delivery;
        } else {
            field.deliveries.push((source, delivery));
        }
        changed |= field.rejoin(program, machine);
        // Keep the row even when the join is empty: its delivery map remembers
        // which edges carry no evidence, so a re-delivered field still
        // intersects over those edges instead of claiming a fresh start.
        true
    });
    for next in incoming {
        if previous.iter().any(|field| field.segments == next.segments) {
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
            segments: next.segments.clone(),
            literal: ExpressionHandle::invalid(),
            predicates: Vec::new(),
            integer_bounds: None,
            deliveries,
            edge_potential: Vec::new(),
            predicate_ceiling: Vec::new(),
            bounds_growth: 0,
        };
        field.rejoin(program, machine);
        previous.push(field);
        changed = true;
    }
    changed
}

impl FieldValue {
    /// Recompute the joined literal, predicate set, and integer bound from the
    /// per-edge deliveries.
    fn rejoin(
        &mut self,
        program: &typed_trees::TypedTrees,
        machine: &typed_trees::machine::Machine,
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
            carrier_integer_range(program, machine, &self.segments)
        } else {
            joined
        };
        if let (Some(stored), Some(fresh)) = (&self.integer_bounds, &joined) {
            let extends_minimum = fresh.minimum < stored.minimum;
            let extends_maximum = fresh.maximum > stored.maximum;
            if extends_minimum || extends_maximum {
                self.bounds_growth = self.bounds_growth.saturating_add(1);
                if self.bounds_growth >= 4
                    && let Some(carrier) = carrier_integer_range(program, machine, &self.segments)
                    && let Some(widened) = joined.as_mut()
                {
                    // Widening jumps to the smallest program literal that
                    // still covers the fresh bound rather than straight to
                    // the carrier: loop counters keep re-extending by their
                    // step until the bound is useless, but the authored
                    // comparisons name the values that actually occur, and
                    // the sequence stays monotone and bounded by the literal
                    // count before it can reach the carrier.
                    let thresholds = integer_literal_thresholds(program);
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
        if self.integer_bounds != joined {
            self.integer_bounds = joined;
            changed = true;
        }
        changed
    }
}

fn has_self(program: &typed_trees::TypedTrees, state: &typed_trees::state::State) -> bool {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_self)
        .count()
        == 1
}

pub(super) fn capture(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    ctx: &FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &typed_trees::state::State,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> Vec<FieldValue> {
    if !has_self(program, state) || !has_self(program, destination) {
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
            ProgramPoint::Call {
                statement_index, ..
            }
            | ProgramPoint::CallRequires {
                statement_index, ..
            }
            | ProgramPoint::CallEnsures {
                statement_index, ..
            } => (statement_index + 1, 1),
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
    for reference in ctx.contexts.semantic_context_refs.span_or_empty(contexts) {
        for fact in semantic
            .context_view(semantic.contexts.get(reference.context))
            .facts()
        {
            let (literal, predicates, integer_bounds) = match fact.payload {
                FactPayload::AssignedValue { value }
                    if program.expression_table.expression_is_valid(value) =>
                {
                    match program.expression_table.expression(value) {
                        ExpressionNode::String(bytes) => (
                            value,
                            ByteSequencePredicate::ALL
                                .into_iter()
                                .filter(|predicate| predicate.holds_for(bytes))
                                .collect::<Vec<_>>(),
                            None,
                        ),
                        ExpressionNode::Integer(literal) => {
                            let Some(value) = literal.value_bignum() else {
                                continue;
                            };
                            (
                                ExpressionHandle::invalid(),
                                Vec::new(),
                                Some(facts::IntegerRange {
                                    minimum: value.clone(),
                                    maximum: value,
                                }),
                            )
                        }
                        _ => continue,
                    }
                }
                FactPayload::AssignedScalarValue { value } => {
                    let facts::ScalarValue::Integer(value) = semantic.scalar_values.get(value)
                    else {
                        continue;
                    };
                    (
                        ExpressionHandle::invalid(),
                        Vec::new(),
                        Some(facts::IntegerRange {
                            minimum: value.clone(),
                            maximum: value.clone(),
                        }),
                    )
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
            let FactPlace::Place(place) = fact.place else {
                continue;
            };
            let Some(mut place) =
                canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
            else {
                continue;
            };
            normalize_attached_place_root(program, machine.symbol, state.symbol, &mut place);
            place.root = normalized_event_place_root(program, place.root);
            if place.root != facts::PlaceRoot::Symbol(machine.symbol)
                || !matches!(place.segments.first(), Some(facts::PlaceSegment::Field { symbol }) if symbol.is_valid())
                || !place.segments.iter().all(|segment| match segment {
                    facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
                    facts::PlaceSegment::Case { variant } => variant.is_valid(),
                    facts::PlaceSegment::FixedIndex { .. } => true,
                    _ => false,
                })
            {
                continue;
            }
            if let Some(index) = fields
                .iter()
                .position(|field| field.segments == place.segments)
            {
                let field = &mut fields[index];
                if literal.is_valid() {
                    field.literal = literal;
                }
                if let Some(incoming) = integer_bounds {
                    let rank = bound_point_rank(fact.point);
                    field.integer_bounds = match (bounds_ranks[index], field.integer_bounds.take())
                    {
                        (stored, Some(previous)) if stored > rank => Some(previous),
                        (stored, Some(previous)) if stored == rank => Some(facts::IntegerRange {
                            minimum: previous.minimum.min(incoming.minimum),
                            maximum: previous.maximum.max(incoming.maximum),
                        }),
                        _ => {
                            bounds_ranks[index] = rank;
                            Some(incoming)
                        }
                    };
                }
                for predicate in predicates {
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
                    segments: place.segments,
                    literal,
                    predicates,
                    integer_bounds,
                    deliveries: Vec::new(),
                    edge_potential: Vec::new(),
                    predicate_ceiling: Vec::new(),
                    bounds_growth: 0,
                });
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
        for predicate in super::field_predicate_ceiling(ctx, state.symbol, &field.segments) {
            if !field.edge_potential.contains(predicate) {
                field.edge_potential.push(*predicate);
            }
        }
        if let Some((_, _, byteok)) =
            ctx.element_store_potentials
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
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    fields: &[FieldValue],
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    point: ProgramPoint,
) {
    if !has_self(program, state) {
        return;
    }
    for field in fields {
        let mut segments = HandleSpan::empty();
        for segment in &field.segments {
            semantic
                .place_segments
                .append_to_span(&mut segments, *segment);
        }
        let place = semantic.append_place(facts::Place {
            root: facts::PlaceRoot::Symbol(machine.symbol),
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
                carrier_integer_range(program, machine, &field.segments)
                    .is_none_or(|carrier| !crate::values::bounds::contains(bounds, &carrier))
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
