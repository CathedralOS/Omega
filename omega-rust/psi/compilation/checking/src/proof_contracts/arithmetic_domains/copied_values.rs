//! Integer facts carried by a value binding and its scalar descendants.
//!
//! Scalar leaves consume the intervals already established by expression
//! checking. Record and array constructors compose those leaves, and ordinary
//! aggregate copies translate their retained descendants. Capture precedes
//! destination invalidation, so overlapping stores cannot erase the new value.
//! Checked visits supply each leaf's captured facts. Complete write frames expire
//! observations before later operands; this owner never reevaluates an initializer.
//! Reference bindings transport observations of live storage, not a frozen
//! referent. Ordinary alias-aware writes must still retire those observations.

use super::value_environment::StoredIntegerRange;
use super::{Interval, ValueEnvironment, ordered_values, place_path};
use crate::value_custody::places::{CanonicalPlace, canonical_place_from_expression};
use std::collections::BTreeMap;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

mod captured_observations;
mod delivery;
use captured_observations::CapturedValue;
mod source_observations;
use source_observations::SourceObservations;

struct CaptureContext<'a> {
    source: SourceObservations<'a>,
    environment: &'a ValueEnvironment,
    checked_values: &'a [super::expression_analysis::CheckedIntegerValue],
}

#[derive(Default)]
pub(crate) struct CopiedIntegerValue {
    recipes: Vec<(
        Vec<facts::PlaceSegment>,
        String,
        checked_trees::IntegerRangeRecipe,
    )>,
    intervals: Vec<CapturedScalarObservation<Interval>>,
    unsigned_values: Vec<CapturedScalarObservation<u64>>,
    float_intervals: Vec<CapturedScalarObservation<super::value_environment::FloatInterval>>,
    non_nan_paths: Vec<CapturedScalarObservation<()>>,
    float_aliases: Vec<CapturedScalarObservation<String>>,
    ordered_relations: Vec<ordered_values::Relation>,
    // These associations already survived the complete normal evaluation.
    // Source-derived observations above still retire in operand order; replaying
    // that prefix over final-prefix facts would erase valid post-call relations.
    completed_relations: Vec<ordered_values::Relation>,
}

struct CapturedScalarObservation<Value> {
    // None means only a legacy text-keyed descendant was available. It may
    // survive an empty frame, but cannot establish separation from writes.
    segments: Option<Vec<facts::PlaceSegment>>,
    suffix: String,
    value: Value,
}

impl CopiedIntegerValue {
    /// Completed associations are consumed only at a delivered value and
    /// its supplied record fields, not at every expression evaluated inside it.
    /// Keep numeric observation requests on the same delivery-child owner.
    pub(crate) fn visit_delivered_values(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        visitor: &mut impl FnMut(ExpressionHandle),
    ) {
        delivery::visit_values(program, expression, visitor);
    }

    pub(crate) fn capture(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: Option<&typed_trees::state::State>,
        expression: ExpressionHandle,
        receiving_type: typed_trees::types::TypeReferenceHandle,
        environment: &ValueEnvironment,
        checked_values: &[super::expression_analysis::CheckedIntegerValue],
    ) -> Self {
        let mut value = Self::default();
        let context = CaptureContext {
            source: SourceObservations::new(program, machine, state, expression),
            environment,
            checked_values,
        };
        value.capture_at(&context, receiving_type, expression, "", &[], Some(&[]));
        value
    }

    /// Capture live scalar exclusions before the destination is invalidated.
    /// A dynamic indexed read may become a hoisted local; that local freezes
    /// this evaluated observation, not any future read of its old storage.
    /// Other live storage identities remain in this environment: copying a
    /// selector must also translate indexed observations using that selector.
    pub(crate) fn capture_local_relations(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: Option<&typed_trees::state::State>,
        local: &typed_trees::statement::TableLocalData,
        environment: &ValueEnvironment,
    ) {
        let destination = ordered_values::Operand::Place {
            root: local.symbol,
            path: local.name.as_str().to_owned(),
        };
        let written = [local.name.as_str().to_owned()];
        let observed = self.capture_relations(
            program,
            machine,
            state,
            local.initial_value,
            &destination,
            &written,
            environment,
        );
        if program
            .primitive_type_reference(local.type_reference)
            .is_some_and(|primitive| primitive.is_fixed_integer())
        {
            if let Some(observed) = observed {
                Self::capture_scalar_equality(
                    &mut self.ordered_relations,
                    observed,
                    destination.clone(),
                    &written,
                );
            }
            if let Some(state) = state
                && let Some(length) = super::slice_lengths::denoted_collection_length(
                    program,
                    machine,
                    state,
                    environment,
                    local.initial_value,
                )
            {
                Self::capture_scalar_equality(
                    &mut self.ordered_relations,
                    length,
                    destination.clone(),
                    &written,
                );
            }
        }
        if let Some(state) = state {
            self.capture_descendant_observations(
                program,
                machine,
                state,
                local.initial_value,
                &destination,
                &written,
                environment,
                local.type_reference,
            );
            self.retain_completed_observations(
                program,
                machine,
                state,
                local.initial_value,
                &destination,
            );
        }
    }

    /// Stores capture the same scalar value as local bindings. Other operands
    /// retain identity only when the complete write footprint preserves them;
    /// the new destination must never revive facts about its old contents.
    pub(crate) fn capture_assignment_relations(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        assignment: &typed_trees::statement::TableAssignment,
        written: &[String],
        environment: &ValueEnvironment,
    ) {
        let primitive =
            super::declared_place_type_raw(program, machine, Some(state), assignment.target)
                .map(|reference| {
                    crate::value_custody::places::assignment_value_type(program, reference)
                })
                .and_then(|reference| program.primitive_type_reference(reference));
        // Dynamic destinations require a captured selector, not its later
        // spelling. Effectful RHS values additionally cross their complete
        // evaluation frame below; a static spelling does not freeze its inputs.
        let Some(destination) =
            ordered_values::operand(program, machine, state, assignment.target, None)
        else {
            return;
        };
        if destination.static_path().is_none() {
            return;
        }
        self.capture_descendant_observations(
            program,
            machine,
            state,
            assignment.value,
            &destination,
            written,
            environment,
            crate::assignment_stored_value_type(program, machine, state, assignment)
                .unwrap_or_default(),
        );
        if primitive.is_some_and(|primitive| primitive.is_fixed_integer())
            && let Some(observed) = self.capture_relations(
                program,
                machine,
                Some(state),
                assignment.value,
                &destination,
                written,
                environment,
            )
        {
            Self::capture_scalar_equality(
                &mut self.ordered_relations,
                observed,
                destination.clone(),
                written,
            );
            if let Some(length) = super::slice_lengths::denoted_collection_length(
                program,
                machine,
                state,
                environment,
                assignment.value,
            ) {
                Self::capture_scalar_equality(
                    &mut self.ordered_relations,
                    length,
                    destination.clone(),
                    written,
                );
            }
        }
        self.retain_completed_observations(program, machine, state, assignment.value, &destination);
    }

    fn retain_completed_observations(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        destination: &ordered_values::Operand,
    ) {
        if self.ordered_relations.is_empty() {
            return;
        }
        // A constructor captures children in order. Later operands may change
        // an earlier child's source home; the end environment cannot stand in
        // for that earlier observation. Retain only independent inputs across
        // the complete evaluation frame, plus this completed destination.
        let writes = if crate::machine_calls::calls::expression_has_calls(program, expression) {
            crate::CallFrameResolver::new(program)
                .and_then(|frames| frames.expression_may_write_paths(machine, expression))
        } else {
            Some(Vec::new())
        };
        self.ordered_relations.retain(|relation| {
            ordered_values::survives_delivery(
                program,
                state,
                &relation.left,
                destination,
                writes.as_deref(),
            ) && ordered_values::survives_delivery(
                program,
                state,
                &relation.right,
                destination,
                writes.as_deref(),
            )
        });
    }

    fn capture_relations(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: Option<&typed_trees::state::State>,
        expression: ExpressionHandle,
        destination: &ordered_values::Operand,
        written: &[String],
        environment: &ValueEnvironment,
    ) -> Option<ordered_values::Operand> {
        let observed = state
            .and_then(|state| ordered_values::operand(program, machine, state, expression, None))?;
        let mut bindings = Vec::new();
        for relation in &environment.ordered_values {
            retain_places(&relation.left, &observed, &mut bindings);
            retain_places(&relation.right, &observed, &mut bindings);
        }
        bindings.retain(|(source, _)| source.survives(written));
        bindings.push((observed.clone(), destination.clone()));
        self.ordered_relations
            .extend(ordered_values::rebind_relations(
                &environment.ordered_values,
                &bindings,
            ));
        Some(observed)
    }

    fn capture_descendant_observations(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        destination: &ordered_values::Operand,
        written: &[String],
        environment: &ValueEnvironment,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) {
        let captured = CapturedValue {
            operand: destination,
            type_reference,
        };
        let source = SourceObservations::new(program, machine, Some(state), expression);
        self.capture_descendants(
            program,
            machine,
            state,
            expression,
            destination,
            written,
            environment,
            &captured,
            &source,
            Some(&[]),
        );
    }

    fn capture_descendants(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        destination: &ordered_values::Operand,
        written: &[String],
        environment: &ValueEnvironment,
        captured: &CapturedValue<'_>,
        source: &SourceObservations<'_>,
        later_writes: Option<&[String]>,
    ) {
        let fields = delivery::fields(program, expression);
        if fields.is_none() {
            // Children are evaluated in authored initializer order. Expire earlier
            // borrowed-content observations before this operand runs, then
            // capture this operand's completed guarantees. Applying the whole
            // constructor frame afterward cannot recover that ordering.
            let writes = source.effects(expression);
            self.ordered_relations.retain(|relation| {
                captured.observation_survives(program, state, &relation.left, writes.as_deref())
                    && captured.observation_survives(
                        program,
                        state,
                        &relation.right,
                        writes.as_deref(),
                    )
            });
        }
        // A value binding transports integer observations of that value,
        // whether the observation is the scalar itself, a field, or a view's
        // extent. Nested constructor operands use the same delivery route as
        // a whole local/store; payload equality is never inferred from length.
        self.capture_completed_associations(state, expression, destination, written, environment);
        if later_writes.is_some_and(|written| source.permits(expression, written))
            && super::declared_place_type_raw(program, machine, Some(state), expression)
                .is_some_and(|reference| crate::slice_view_shell(program, reference, &[]).is_some())
            && let Some(observed) =
                ordered_values::operand(program, machine, state, expression, None)
        {
            Self::capture_scalar_equality(
                &mut self.ordered_relations,
                ordered_values::Operand::CollectionLength(Box::new(observed)),
                ordered_values::Operand::CollectionLength(Box::new(destination.clone())),
                written,
            );
        }
        let Some(fields) = fields else {
            return;
        };
        // Source equalities read the completed environment; payload guarantees
        // belong to their authored occurrence. Carry suffix effects separately
        // so computing the former never reverses retirement of the latter.
        let following =
            source.following_writes(fields.iter().map(|field| field.value), later_writes);
        for (field, following) in fields.iter().zip(&following) {
            let primitive = integer_field_primitive(program, machine, state, field.value);
            let field_operand = ordered_values::Operand::Field {
                receiver: Box::new(destination.clone()),
                field: field.field_symbol,
                name: field.name.as_str().to_owned(),
                primitive,
            };
            self.capture_descendants(
                program,
                machine,
                state,
                field.value,
                &field_operand,
                written,
                environment,
                captured,
                source,
                following.as_deref(),
            );
            if primitive.is_some()
                && following
                    .as_deref()
                    .is_some_and(|written| source.permits(field.value, written))
            {
                if let Some(observed) =
                    ordered_values::operand(program, machine, state, field.value, None)
                {
                    Self::capture_scalar_equality(
                        &mut self.ordered_relations,
                        observed,
                        field_operand.clone(),
                        written,
                    );
                }
                if let Some(length) = super::slice_lengths::denoted_collection_length(
                    program,
                    machine,
                    state,
                    environment,
                    field.value,
                ) {
                    Self::capture_scalar_equality(
                        &mut self.ordered_relations,
                        length,
                        field_operand,
                        written,
                    );
                }
            }
        }
    }

    fn capture_scalar_equality(
        relations: &mut Vec<ordered_values::Relation>,
        observed: ordered_values::Operand,
        destination: ordered_values::Operand,
        written: &[String],
    ) {
        // A successfully evaluated scalar initializer and its new binding
        // denote the same value. Retain that relationship while the observed
        // inputs remain current, so hoisting arithmetic does not discard its
        // correlations. Ordinary write invalidation retires either side; a
        // shadowing destination must not revive its old storage observation.
        if observed.survives(written) {
            for (left, right) in [
                (destination.clone(), observed.clone()),
                (observed, destination),
            ] {
                relations.push(ordered_values::Relation {
                    left,
                    right,
                    constraint: ordered_values::RelationConstraint::MinimumDifference(0),
                });
            }
        }
    }

    fn capture_completed_associations(
        &mut self,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        destination: &ordered_values::Operand,
        written: &[String],
        environment: &ValueEnvironment,
    ) {
        for association in &environment.numeric_observations.scalar_associations {
            if association.state == state.symbol && association.expression == expression {
                Self::capture_scalar_equality(
                    &mut self.completed_relations,
                    association.current.clone(),
                    destination.clone(),
                    written,
                );
            }
        }
    }

    fn capture_at(
        &mut self,
        context: &CaptureContext<'_>,
        receiving_type: typed_trees::types::TypeReferenceHandle,
        expression: ExpressionHandle,
        suffix: &str,
        segments: &[facts::PlaceSegment],
        later_writes: Option<&[String]>,
    ) {
        let program = context.source.program;
        let environment = context.environment;
        let checked_values = context.checked_values;
        match program.expression_table.expression(expression) {
            ExpressionNode::StructLiteral(literal) => {
                let fields = program.expression_table.struct_fields(literal.fields);
                let following = context
                    .source
                    .following_writes(fields.iter().map(|field| field.value), later_writes);
                for (field, following) in fields.iter().zip(&following) {
                    let mut field_segments = segments.to_vec();
                    field_segments.push(facts::PlaceSegment::Field {
                        symbol: field.field_symbol,
                    });
                    self.capture_at(
                        context,
                        receiving_type,
                        field.value,
                        &format!("{suffix}.{}", field.name),
                        &field_segments,
                        following.as_deref(),
                    );
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let elements = program.expression_table.expression_handles(*elements);
                let following = context
                    .source
                    .following_writes(elements.iter().copied(), later_writes);
                for ((index, element), following) in elements.iter().enumerate().zip(&following) {
                    let mut element_segments = segments.to_vec();
                    element_segments.push(facts::PlaceSegment::FixedIndex { index });
                    self.capture_at(
                        context,
                        receiving_type,
                        *element,
                        &format!("{suffix}[{index}]"),
                        &element_segments,
                        following.as_deref(),
                    );
                }
            }
            _ => {
                // Retire *every* earlier payload before this authored operand
                // executes, then capture its completed facts. An interval or
                // recipe is no more detached from borrowed backing than an
                // ordered relation; the receiving type decides what was copied.
                let writes = context.source.effects(expression);
                let survives = |path: Option<&[facts::PlaceSegment]>| {
                    captured_observations::scalar_path_survives(
                        program,
                        receiving_type,
                        path,
                        writes.as_deref(),
                    )
                };
                self.recipes.retain(|(path, _, _)| survives(Some(path)));
                self.intervals
                    .retain(|observed| survives(observed.segments.as_deref()));
                self.unsigned_values
                    .retain(|observed| survives(observed.segments.as_deref()));
                self.float_intervals
                    .retain(|observed| survives(observed.segments.as_deref()));
                self.non_nan_paths
                    .retain(|observed| survives(observed.segments.as_deref()));
                self.float_aliases.retain(|observed| {
                    survives(observed.segments.as_deref())
                        && writes.as_ref().is_some_and(|writes| {
                            writes.iter().all(|written| {
                                !super::place_paths::place_paths_overlap(&observed.value, written)
                            })
                        })
                });
                // Post-environment metadata is not an occurrence snapshot.
                // Keep authored-order retirement above, but admit source reads
                // only if their own expression and every following sibling
                // (including enclosing siblings) left that observation current.
                let source_is_current =
                    later_writes.is_some_and(|written| context.source.permits(expression, written));
                if let Some(source) = canonical_place_from_expression(program, expression)
                    && source_is_current
                {
                    for stored in &environment.integer_ranges {
                        if stored.place.root == source.root
                            && stored.place.segments.starts_with(&source.segments)
                        {
                            let mut destination = segments.to_vec();
                            destination
                                .extend_from_slice(&stored.place.segments[source.segments.len()..]);
                            let source_label = source.label(program);
                            if let Some(tail) = stored.path.strip_prefix(&source_label) {
                                self.recipes.push((
                                    destination,
                                    format!("{suffix}{tail}"),
                                    stored.recipe.clone(),
                                ));
                            }
                        }
                    }
                }
                // Every checked visit participates. A missing snapshot cannot
                // be replaced by a stronger visit of the same expression.
                let mut visits = checked_values
                    .iter()
                    .filter(|value| value.expression == expression)
                    .map(|value| &value.recipe);
                if let Some(Some(first)) = visits.next()
                    && visits.all(|recipe| recipe.as_ref() == Some(first))
                {
                    self.recipes.retain(|(existing, _, _)| existing != segments);
                    self.recipes
                        .push((segments.to_vec(), suffix.to_owned(), first.clone()));
                }
                if let Some(source) = place_path(program, expression)
                    .or_else(|| super::element_place_path(program, expression))
                    .filter(|_| source_is_current)
                {
                    copy_retained(
                        &environment.intervals,
                        &source,
                        suffix,
                        segments,
                        &mut self.intervals,
                    );
                    copy_retained(
                        &environment.known_u64_values,
                        &source,
                        suffix,
                        segments,
                        &mut self.unsigned_values,
                    );
                    copy_retained(
                        &environment.float_intervals,
                        &source,
                        suffix,
                        segments,
                        &mut self.float_intervals,
                    );
                    for path in environment
                        .non_nan
                        .range(source.clone()..)
                        .take_while(|path| path.starts_with(&source))
                    {
                        if let Some(tail) = path.strip_prefix(&source)
                            && (tail.is_empty() || tail.starts_with('.') || tail.starts_with('['))
                        {
                            self.non_nan_paths.push(CapturedScalarObservation {
                                segments: tail.is_empty().then(|| segments.to_vec()),
                                suffix: format!("{suffix}{tail}"),
                                value: (),
                            });
                        }
                    }
                    if !suffix.is_empty() {
                        self.float_aliases.push(CapturedScalarObservation {
                            segments: Some(segments.to_vec()),
                            suffix: suffix.to_owned(),
                            value: source.clone(),
                        });
                    }
                    for (left, right) in &environment.float_aliases {
                        for (candidate, partner) in [(left, right), (right, left)] {
                            if let Some(tail) = candidate.strip_prefix(&source)
                                && (tail.is_empty()
                                    || tail.starts_with('.')
                                    || tail.starts_with('['))
                            {
                                self.float_aliases.push(CapturedScalarObservation {
                                    segments: tail.is_empty().then(|| segments.to_vec()),
                                    suffix: format!("{suffix}{tail}"),
                                    value: partner.clone(),
                                });
                            }
                        }
                    }
                } else if !suffix.is_empty()
                    && source_is_current
                    && let Some(source) =
                        super::assignments::float_source_place_path(program, expression)
                {
                    self.float_aliases.push(CapturedScalarObservation {
                        segments: Some(segments.to_vec()),
                        suffix: suffix.to_owned(),
                        value: source,
                    });
                }
                if !suffix.is_empty()
                    && let Some(value) =
                        super::float_arithmetic::float_literal_value(program, expression)
                    && !value.is_nan()
                {
                    self.float_intervals.push(CapturedScalarObservation {
                        segments: Some(segments.to_vec()),
                        suffix: suffix.to_owned(),
                        value: super::value_environment::FloatInterval {
                            low: Some(value),
                            high: Some(value),
                        },
                    });
                    self.non_nan_paths.push(CapturedScalarObservation {
                        segments: Some(segments.to_vec()),
                        suffix: suffix.to_owned(),
                        value: (),
                    });
                }
                if !suffix.is_empty()
                    && let Some(interval) = checked_values
                        .iter()
                        .filter_map(|value| {
                            (value.expression == expression).then_some(value.interval)
                        })
                        .reduce(Interval::union)
                {
                    self.intervals.push(CapturedScalarObservation {
                        segments: Some(segments.to_vec()),
                        suffix: suffix.to_owned(),
                        value: interval,
                    });
                }
                if let Some(value) = super::integer_ranges::literal_u64(program, expression) {
                    self.unsigned_values.push(CapturedScalarObservation {
                        segments: Some(segments.to_vec()),
                        suffix: suffix.to_owned(),
                        value,
                    });
                }
            }
        }
    }

    /// The destination's old facts must already have been retired by the
    /// ordinary store owner. A copied root interval is recorded there with its
    /// enforced declared range; this owner supplies the composite descendants.
    pub(crate) fn record(
        &self,
        environment: &mut ValueEnvironment,
        path: Option<String>,
        destination: Option<CanonicalPlace>,
    ) {
        let Some(path) = path else { return };
        if let Some(destination) = destination
            && matches!(destination.root, facts::PlaceRoot::Symbol(_))
        {
            for (segments, suffix, recipe) in &self.recipes {
                let mut place = destination.clone();
                place.extend_segments(segments);
                if place.segments.iter().any(|segment| {
                    !matches!(segment, facts::PlaceSegment::Field { symbol } if symbol.is_valid())
                        && !matches!(segment, facts::PlaceSegment::FixedIndex { .. })
                }) {
                    continue;
                }
                environment
                    .integer_ranges
                    .retain(|stored| stored.place != place);
                environment.integer_ranges.push(StoredIntegerRange {
                    place,
                    path: format!("{path}{suffix}"),
                    recipe: recipe.clone(),
                });
            }
        }
        for relation in self
            .ordered_relations
            .iter()
            .chain(&self.completed_relations)
        {
            if !environment.ordered_values.contains(relation) {
                environment.ordered_values.push(relation.clone());
            }
        }
        for observed in &self.intervals {
            if !observed.suffix.is_empty() {
                environment.set(format!("{path}{}", observed.suffix), observed.value);
            }
        }
        for observed in &self.unsigned_values {
            environment.mark_known_u64(format!("{path}{}", observed.suffix), observed.value);
        }
        for observed in &self.float_intervals {
            environment.narrow_float(format!("{path}{}", observed.suffix), observed.value);
        }
        for observed in &self.non_nan_paths {
            environment.mark_non_nan(format!("{path}{}", observed.suffix));
        }
        for observed in &self.float_aliases {
            environment
                .mark_float_alias(format!("{path}{}", observed.suffix), observed.value.clone());
        }
    }
}

fn integer_field_primitive(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    if crate::value_custody::places::collection_length_receiver(
        program,
        machine,
        Some(state),
        expression,
    )
    .is_some()
    {
        return Some(typed_trees::types::PrimitiveType::U64);
    }
    super::declared_place_type_raw(program, machine, Some(state), expression)
        .and_then(|reference| program.primitive_type_reference(reference))
        .filter(|primitive| primitive.is_fixed_integer())
}

fn retain_places(
    operand: &ordered_values::Operand,
    observed: &ordered_values::Operand,
    bindings: &mut Vec<(ordered_values::Operand, ordered_values::Operand)>,
) {
    use ordered_values::Operand;
    if operand == observed {
        return;
    }
    match operand {
        Operand::Place { .. } => {
            if !bindings.iter().any(|(source, _)| source == operand) {
                bindings.push((operand.clone(), operand.clone()));
            }
        }
        Operand::Integer(..) | Operand::CompletedCall { .. } => {}
        Operand::CollectionLength(collection) => retain_places(collection, observed, bindings),
        Operand::Field { receiver, .. } => {
            // A sibling field can survive even when its containing record's
            // whole-value identity does not. Keep that exact observation,
            // unless it contains the copied selector: then rebinding must
            // descend so the captured selector replaces its old live read.
            if !depends_on(receiver, observed)
                && !bindings.iter().any(|(source, _)| source == operand)
            {
                bindings.push((operand.clone(), operand.clone()));
            } else {
                retain_places(receiver, observed, bindings);
            }
        }
        Operand::Indexed {
            collection, index, ..
        } => {
            retain_places(collection, observed, bindings);
            retain_places(index, observed, bindings);
        }
        Operand::Call { arguments, .. }
        | Operand::Binary {
            operands: arguments,
            ..
        } => {
            for argument in arguments {
                retain_places(argument, observed, bindings);
            }
        }
    }
}

fn depends_on(operand: &ordered_values::Operand, observed: &ordered_values::Operand) -> bool {
    use ordered_values::Operand;
    operand == observed
        || match operand {
            Operand::Place { .. } | Operand::Integer(..) | Operand::CompletedCall { .. } => false,
            Operand::Field { receiver, .. } => depends_on(receiver, observed),
            Operand::CollectionLength(collection) => depends_on(collection, observed),
            Operand::Indexed {
                collection, index, ..
            } => depends_on(collection, observed) || depends_on(index, observed),
            Operand::Call { arguments, .. }
            | Operand::Binary {
                operands: arguments,
                ..
            } => arguments
                .iter()
                .any(|argument| depends_on(argument, observed)),
        }
}

/// Environment maps are ordered by storage path, so a copy visits only the
/// source root's contiguous prefix rather than every fact in the state.
fn copy_retained<Value: Copy>(
    retained: &BTreeMap<String, Value>,
    source: &str,
    destination: &str,
    segments: &[facts::PlaceSegment],
    copied: &mut Vec<CapturedScalarObservation<Value>>,
) {
    copied.extend(
        retained
            .range(source.to_owned()..)
            .take_while(|(path, _)| path.starts_with(source))
            .filter_map(|(path, value)| {
                let suffix = path.strip_prefix(source)?;
                (suffix.is_empty() || suffix.starts_with('.') || suffix.starts_with('[')).then(
                    || CapturedScalarObservation {
                        segments: suffix.is_empty().then(|| segments.to_vec()),
                        suffix: format!("{destination}{suffix}"),
                        value: *value,
                    },
                )
            }),
    );
}
