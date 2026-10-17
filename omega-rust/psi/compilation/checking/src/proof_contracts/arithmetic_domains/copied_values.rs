//! Integer facts carried by a value binding and its scalar descendants.
//!
//! Scalar leaves consume the intervals already established by expression
//! checking. Record and array constructors compose those leaves, and ordinary
//! aggregate copies translate their retained descendants. Capture precedes
//! destination invalidation, so overlapping stores cannot erase the new value.
//! The supplying environment has already crossed value-evaluation effects;
//! this owner never replays an initializer or independently summarizes calls.
//! Reference bindings transport observations of live storage, not a frozen
//! referent. Ordinary alias-aware writes must still retire those observations.

use super::value_environment::StoredIntegerRange;
use super::{Interval, ValueEnvironment, ordered_values, place_path};
use crate::value_custody::places::{CanonicalPlace, canonical_place_from_expression};
use std::collections::BTreeMap;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

#[derive(Default)]
pub(crate) struct CopiedIntegerValue {
    recipes: Vec<(
        Vec<facts::PlaceSegment>,
        String,
        checked_trees::IntegerRangeRecipe,
    )>,
    intervals: Vec<(String, Interval)>,
    unsigned_values: Vec<(String, u64)>,
    ordered_relations: Vec<ordered_values::Relation>,
}

impl CopiedIntegerValue {
    pub(crate) fn capture(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        environment: &ValueEnvironment,
        checked_values: &[super::expression_analysis::CheckedIntegerValue],
    ) -> Self {
        let mut value = Self::default();
        value.capture_at(program, expression, "", &[], environment, checked_values);
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
                self.capture_scalar_equality(observed, destination.clone(), &written);
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
                self.capture_scalar_equality(length, destination.clone(), &written);
            }
        }
        if let Some(state) = state {
            self.capture_aggregate_identities(
                program,
                machine,
                state,
                local.initial_value,
                &destination,
                &written,
                environment,
            );
            self.ordered_relations
                .extend(ordered_values::delivered_value_orderings(
                    program,
                    machine,
                    state,
                    local.initial_value,
                    &destination,
                    &written,
                    environment,
                ));
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
        // spelling. A static projection is stable here only because the
        // shared RHS operand admission below recursively refuses effectful
        // calls; a reference-retargeting RHS cannot earn these relations.
        let Some(destination) =
            ordered_values::operand(program, machine, state, assignment.target, None)
        else {
            return;
        };
        if destination.static_path().is_none() {
            return;
        }
        self.capture_aggregate_identities(
            program,
            machine,
            state,
            assignment.value,
            &destination,
            written,
            environment,
        );
        if !primitive.is_some_and(|primitive| primitive.is_fixed_integer()) {
            return;
        }
        if let Some(observed) = self.capture_relations(
            program,
            machine,
            Some(state),
            assignment.value,
            &destination,
            written,
            environment,
        ) {
            self.capture_scalar_equality(observed, destination.clone(), written);
            if let Some(length) = super::slice_lengths::denoted_collection_length(
                program,
                machine,
                state,
                environment,
                assignment.value,
            ) {
                self.capture_scalar_equality(length, destination.clone(), written);
            }
        }
        self.ordered_relations
            .extend(ordered_values::delivered_value_orderings(
                program,
                machine,
                state,
                assignment.value,
                &destination,
                written,
                environment,
            ));
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
        self.ordered_relations =
            ordered_values::rebind_relations(&environment.ordered_values, &bindings);
        Some(observed)
    }

    fn capture_aggregate_identities(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        destination: &ordered_values::Operand,
        written: &[String],
        environment: &ValueEnvironment,
    ) {
        let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(expression)
        else {
            return;
        };
        for field in program.expression_table.struct_fields(literal.fields) {
            let primitive = integer_field_primitive(program, machine, state, field.value);
            let field_operand = ordered_values::Operand::Field {
                receiver: Box::new(destination.clone()),
                field: field.field_symbol,
                name: field.name.as_str().to_owned(),
                primitive,
            };
            if primitive.is_some() {
                if let Some(observed) =
                    ordered_values::operand(program, machine, state, field.value, None)
                {
                    self.capture_scalar_equality(observed, field_operand.clone(), written);
                }
                if let Some(length) = super::slice_lengths::denoted_collection_length(
                    program,
                    machine,
                    state,
                    environment,
                    field.value,
                ) {
                    self.capture_scalar_equality(length, field_operand, written);
                }
            } else {
                self.capture_aggregate_identities(
                    program,
                    machine,
                    state,
                    field.value,
                    &field_operand,
                    written,
                    environment,
                );
            }
        }
    }

    fn capture_scalar_equality(
        &mut self,
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
                self.ordered_relations.push(ordered_values::Relation {
                    left,
                    right,
                    constraint: ordered_values::RelationConstraint::MinimumDifference(0),
                });
            }
        }
    }

    fn capture_at(
        &mut self,
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        suffix: &str,
        segments: &[facts::PlaceSegment],
        environment: &ValueEnvironment,
        checked_values: &[super::expression_analysis::CheckedIntegerValue],
    ) {
        match program.expression_table.expression(expression) {
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    let mut field_segments = segments.to_vec();
                    field_segments.push(facts::PlaceSegment::Field {
                        symbol: field.field_symbol,
                    });
                    self.capture_at(
                        program,
                        field.value,
                        &format!("{suffix}.{}", field.name),
                        &field_segments,
                        environment,
                        checked_values,
                    );
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                for (index, element) in program
                    .expression_table
                    .expression_handles(*elements)
                    .iter()
                    .enumerate()
                {
                    let mut element_segments = segments.to_vec();
                    element_segments.push(facts::PlaceSegment::FixedIndex { index });
                    self.capture_at(
                        program,
                        *element,
                        &format!("{suffix}[{index}]"),
                        &element_segments,
                        environment,
                        checked_values,
                    );
                }
            }
            _ => {
                if let Some(source) = canonical_place_from_expression(program, expression) {
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
                {
                    copy_retained(&environment.intervals, &source, suffix, &mut self.intervals);
                    copy_retained(
                        &environment.known_u64_values,
                        &source,
                        suffix,
                        &mut self.unsigned_values,
                    );
                }
                if !suffix.is_empty()
                    && let Some(interval) = checked_values
                        .iter()
                        .filter_map(|value| {
                            (value.expression == expression).then_some(value.interval)
                        })
                        .reduce(Interval::union)
                {
                    self.intervals.push((suffix.to_owned(), interval));
                }
                if let Some(value) = super::integer_ranges::literal_u64(program, expression) {
                    self.unsigned_values.push((suffix.to_owned(), value));
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
        for relation in &self.ordered_relations {
            if !environment.ordered_values.contains(relation) {
                environment.ordered_values.push(relation.clone());
            }
        }
        for (suffix, interval) in &self.intervals {
            if !suffix.is_empty() {
                environment.set(format!("{path}{suffix}"), *interval);
            }
        }
        for (suffix, value) in &self.unsigned_values {
            environment.mark_known_u64(format!("{path}{suffix}"), *value);
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
        Operand::Integer(..) => {}
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
            Operand::Place { .. } | Operand::Integer(..) => false,
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
    copied: &mut Vec<(String, Value)>,
) {
    copied.extend(
        retained
            .range(source.to_owned()..)
            .take_while(|(path, _)| path.starts_with(source))
            .filter_map(|(path, value)| {
                let suffix = path.strip_prefix(source)?;
                (suffix.is_empty() || suffix.starts_with('.') || suffix.starts_with('['))
                    .then(|| (format!("{destination}{suffix}"), *value))
            }),
    );
}
