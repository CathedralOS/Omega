//! Exact typed reads behind the range checker's legacy expression labels.
//!
//! Incoming guards measure predecessor storage. Record those reads in their
//! evaluation scope, then transport exact parameter roots and selector operands
//! to the receiving state. Field/index paths survive that join; spelling alone cannot keep a
//! stale predecessor parameter current or authorize an unrelated binding.

use validation::CanonicalPlace;

mod captures;
mod place_aliases;
mod reads;
mod state_transport;

use reads::collect_reads;

use std::collections::{BTreeMap, BTreeSet};

use super::RangeFacts;

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::{machine::Machine, state::State};

/// Bound shared by every walk below: `record_dependencies` and
/// `collect_reads` recurse over expression trees, while the captures.rs
/// chain walks unroll the same bound iteratively. Authored expressions
/// deeper than the bound record the visited prefix and stop rather than
/// recursing to the bottom of the arena; pinned by tests/depth.rs.
pub(super) const EXPRESSION_WALK_DEPTH_BOUND: usize = 128;

#[derive(Clone)]
pub(super) struct ExpressionDependencies {
    expression: ExpressionHandle,
    label: String,
    machine: SymbolHandle,
    state: SymbolHandle,
    /// None is an incomplete read set, not a storage-free computation.
    reads: Option<Vec<CanonicalPlace>>,
    /// Metadata observes descriptor replacement, not writes to its elements.
    /// Selector reads remain ordinary value dependencies.
    descriptor: Option<CanonicalPlace>,
}

/// A captured scalar reads its own storage, independently of its initializer.
/// This is declaration identity for existing facts, not a replayable value.
#[derive(Clone)]
pub(super) struct ScalarBindingDependency {
    label: String,
    machine: SymbolHandle,
    state: SymbolHandle,
    place: CanonicalPlace,
    length: Option<CapturedLength>,
    value: Option<CapturedScalarValue>,
}

/// Equality with a currently unchanged integer observation, captured when the
/// binding is evaluated. Any write to its inputs or destination retires it.
#[derive(Clone)]
struct CapturedScalarValue {
    expression: ExpressionHandle,
    reads: Vec<CanonicalPlace>,
}

/// A scalar snapshot may still measure a live descriptor. Keep that relation
/// at evaluation, not by expanding its initializer when a later guard runs.
/// Descriptor replacement or selector writes retire the relation without
/// changing the captured scalar's own value facts; element writes do not.
#[derive(Clone)]
pub(super) struct CapturedLength {
    collection: ExpressionHandle,
    descriptor: CanonicalPlace,
    selectors: Vec<CanonicalPlace>,
}

/// One row's typed meaning for label-collision judgment: machine, state and
/// read set, `None` when the read set is incomplete.
type RowMeaning<'table> = (
    SymbolHandle,
    SymbolHandle,
    Option<&'table [CanonicalPlace]>,
    Option<&'table CanonicalPlace>,
);

/// Every row of one display label, both tables.
#[derive(Default)]
struct LabelRows<'table> {
    expressions: Vec<&'table ExpressionDependencies>,
    bindings: Vec<&'table ScalarBindingDependency>,
}

/// Labels whose expression rows, or whose scalar-binding rows, carry no
/// single typed meaning; see `RangeFacts::label_collisions`.
#[derive(Default)]
struct LabelCollisions<'table> {
    expression_labels: BTreeSet<&'table str>,
    binding_labels: BTreeSet<&'table str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::checks::ranges) struct ReceiverLength {
    expression: ExpressionHandle,
    label: String,
    place: CanonicalPlace,
    length: i64,
}

impl ReceiverLength {
    /// One exact-length row re-keyed onto a surviving destination place, as
    /// `storage_sub_places` renders argument storage.
    pub(in crate::checks::ranges) fn at_place(
        expression: ExpressionHandle,
        place: CanonicalPlace,
        label: String,
        length: i64,
    ) -> Self {
        Self {
            expression,
            label,
            place,
            length,
        }
    }

    pub(in crate::checks::ranges) fn same_extent(&self, other: &Self) -> bool {
        self.place == other.place && self.length == other.length
    }

    /// Both rows seed the same destination facts: the same extent under the
    /// same label. Only the observing expression differs, and the seeded
    /// dependency row reads the place, not that expression.
    pub(in crate::checks::ranges) fn same_seed(&self, other: &Self) -> bool {
        self.same_extent(other) && self.label == other.label
    }
}

struct ExpressionDependencyScope<'scope> {
    evaluation: &'scope State,
    destination: &'scope State,
    parameter_roots: &'scope [(SymbolHandle, SymbolHandle)],
}

impl<'field> RangeFacts<'field> {
    pub(in crate::checks::ranges) fn forget_collection_expression(&mut self, label: &str) {
        self.forget_collection_facts(label);
        // A later state or binding can reuse the display label for different
        // storage. New extent evidence must not attach to its old typed reads.
        self.expression_dependencies
            .retain(|row| row.label != label);
        self.recorded_expressions = self
            .expression_dependencies
            .iter()
            .map(|row| (row.expression, row.machine, row.state))
            .collect();
    }

    pub(in crate::checks::ranges) fn expression_exact_length(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<i64> {
        if self.exact_lengths.is_empty() {
            return None;
        }
        let mut reads = Vec::new();
        let bound_lookup = self.bound_lookup();
        if !collect_reads(
            program,
            machine,
            state,
            self.statement_index,
            expression,
            self.checked_calls,
            self.checked_operators,
            &mut reads,
            0,
            bound_lookup,
        ) || reads.len() != 1
        {
            return None;
        }
        reads[0] = crate::flow::rebase_exact_local_place(
            program,
            state.symbol,
            self.statement_index,
            reads[0].clone(),
            self.checked_calls.and_then(|context| context.call_frames()),
        )?;
        let mut length = None;
        for row in &self.expression_dependencies {
            if row.machine != machine.symbol || row.state != state.symbol {
                continue;
            }
            let Some(candidate) = self.exact_length(&row.label) else {
                continue;
            };
            let row_reads = row.reads.as_ref().and_then(|row_reads| {
                let [place] = row_reads.as_slice() else {
                    return None;
                };
                crate::flow::rebase_exact_local_place(
                    program,
                    state.symbol,
                    self.statement_index,
                    place.clone(),
                    self.checked_calls.and_then(|context| context.call_frames()),
                )
                .map(|place| vec![place])
            });
            if row.machine == machine.symbol
                && row.state == state.symbol
                && same_reads(program, row_reads.as_deref(), Some(&reads))
            {
                if length.is_some_and(|known| known != candidate) {
                    return None;
                }
                length = Some(candidate);
            }
        }
        length
    }

    pub(in crate::checks::ranges) fn receiver_lengths(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
    ) -> Vec<ReceiverLength> {
        self.expression_dependencies
            .iter()
            .filter_map(|row| {
                let [place] = row.reads.as_deref()? else {
                    return None;
                };
                let place = crate::flow::rebase_exact_local_place(
                    program,
                    state.symbol,
                    self.statement_index,
                    place.clone(),
                    self.checked_calls.and_then(|context| context.call_frames()),
                )?;
                (row.machine == machine.symbol
                    && row.state == state.symbol
                    && (place.root) == facts::PlaceRoot::Symbol(machine.symbol)
                    && !place.segments.is_empty()
                    && place
                        .segments
                        .iter()
                        .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. })))
                .then(|| {
                    Some(ReceiverLength {
                        expression: row.expression,
                        label: row.label.clone(),
                        place: CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(machine.symbol),
                            segments: place.segments.clone(),
                        },
                        length: self.exact_length(&row.label)?,
                    })
                })?
            })
            .collect()
    }

    /// The expression-dependency rows whose single read place sits under one
    /// of `storages`, re-keyed to that storage's destination place.
    ///
    /// `storages` pairs a caller-side canonical place with the destination
    /// root the edge binds it to: `(machine root + prefix, machine symbol)`
    /// for attached `self` storage (a no-op re-root; `self.line` stays
    /// `self.line`), or `(argument place, parameter symbol)` for a `&mut`
    /// transition argument (`buf.line` arrives as `param.line`). Each
    /// element is `(source_label, source_expression, destination_place)`;
    /// the caller re-spells the destination label from the place.
    pub(in crate::checks::ranges) fn storage_sub_places(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        storages: &[(CanonicalPlace, SymbolHandle)],
    ) -> Vec<(String, ExpressionHandle, CanonicalPlace)> {
        self.expression_dependencies
            .iter()
            .filter(|row| row.machine == machine.symbol && row.state == state.symbol)
            // A descriptor observation is not the value stored at the
            // descriptor's place. Re-keying `view.len` as `view` would attach
            // scalar bounds and value-write dependencies to a collection.
            // Extents travel through the collection-length fact channels.
            .filter(|row| row.descriptor.is_none())
            .filter_map(|row| {
                let [place] = row.reads.as_deref()? else {
                    return None;
                };
                let place = crate::flow::rebase_exact_local_place(
                    program,
                    state.symbol,
                    self.statement_index,
                    place.clone(),
                    self.checked_calls.and_then(|context| context.call_frames()),
                )?;

                storages.iter().find_map(|(storage, destination)| {
                    if place.root != storage.root || !place.segments.starts_with(&storage.segments)
                    {
                        return None;
                    }
                    let suffix = &place.segments[storage.segments.len()..];
                    if !suffix.iter().all(|segment| {
                        matches!(
                            segment,
                            facts::PlaceSegment::Field { .. }
                                | facts::PlaceSegment::Case { .. }
                                | facts::PlaceSegment::FixedIndex { .. }
                        )
                    }) {
                        return None;
                    }
                    Some((
                        row.label.clone(),
                        row.expression,
                        CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(*destination),
                            segments: suffix.to_vec(),
                        },
                    ))
                })
            })
            .collect()
    }

    pub(in crate::checks::ranges) fn seed_receiver_lengths(
        &mut self,
        machine: SymbolHandle,
        state: SymbolHandle,
        lengths: &[ReceiverLength],
    ) {
        for length in lengths {
            self.prove_exact_length(length.label.clone(), length.length);
            self.seed_storage_dependency(
                machine,
                state,
                length.expression,
                &length.label,
                &length.place,
            );
        }
    }

    /// Rebind a transported fact's read dependency to destination storage.
    /// The original expression identifies the observation, not a source-state
    /// expression to reevaluate. Mutation and later edges use the exact place.
    pub(in crate::checks::ranges) fn seed_storage_dependency(
        &mut self,
        machine: SymbolHandle,
        state: SymbolHandle,
        expression: ExpressionHandle,
        label: &str,
        place: &CanonicalPlace,
    ) {
        self.recorded_expressions
            .insert((expression, machine, state));
        self.expression_dependencies.push(ExpressionDependencies {
            expression,
            label: label.to_owned(),
            machine,
            state,
            reads: Some(vec![place.clone()]),
            descriptor: None,
        });
    }

    pub(in crate::checks::ranges) fn record_scalar_binding_dependency(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        source: ExpressionHandle,
        symbol: SymbolHandle,
        label: &str,
    ) {
        use typed_trees::statement::StatementNode;
        let reference = match program
            .statement_table
            .statement_at(state, self.statement_index)
        {
            Some(StatementNode::LocalData(local))
                if local.symbol == symbol
                    && local.initial_value == source
                    && local.name.as_str() == label =>
            {
                Some(local.type_reference)
            }
            Some(StatementNode::Assignment(assignment))
                if assignment.value == source
                    && validation::expression_traversal::single_segment_name_symbol(
                        program,
                        assignment.target,
                    ) == Some(symbol) =>
            {
                super::super::types::expression_type_reference(
                    program,
                    machine,
                    state,
                    assignment.target,
                )
            }
            _ => None,
        };
        if !symbol.is_valid()
            || program.symbols.get(symbol).kind != symbols::SymbolKind::Local
            || program.symbols.get(symbol).parent != state.symbol
            || !reference
                .and_then(|reference| program.primitive_type_reference(reference))
                .is_some_and(|primitive| primitive.is_integer())
        {
            return;
        }
        let Some(place) = crate::flow::canonical_place_from_symbol(symbol) else {
            return;
        };
        let length = self.capture_length(program, machine, state, source);
        let mut reads = Vec::new();
        let value = (captures::has_scalar_snapshot_meaning(program, machine, state, source, 0)
            && collect_reads(
                program,
                machine,
                state,
                self.statement_index,
                source,
                self.checked_calls,
                self.checked_operators,
                &mut reads,
                0,
                self.bound_lookup(),
            )
            && !reads
                .iter()
                .any(|read| places_overlap(program, read, &place)))
        .then_some(CapturedScalarValue {
            expression: source,
            reads,
        });
        if let Some(row) = self.scalar_binding_dependencies.iter_mut().find(|row| {
            row.machine == machine.symbol
                && row.state == state.symbol
                && row.label == label
                && row.place == place
        }) {
            row.length = length;
            row.value = value;
            return;
        }
        self.scalar_binding_dependencies
            .push(ScalarBindingDependency {
                label: label.to_owned(),
                machine: machine.symbol,
                state: state.symbol,
                place,
                length,
                value,
            });
    }

    /// All length comparisons consume the same live measurement, whether
    /// authored directly or retained by a scalar binding/copy. Symbols select
    /// snapshots; expression labels only address the existing fact tables.
    pub(in crate::checks::ranges) fn length_collection(
        &self,
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        if let Some(member) = validation::length_member(program, expression) {
            return Some(member.receiver);
        }
        if let Some((_, length)) = self
            .measured_results
            .iter()
            .find(|(result, _)| *result == expression)
        {
            return Some(length.collection);
        }
        self.captured_length_collection(program, expression)
    }

    /// A retained measurement names its descriptor only while the scalar,
    /// descriptor and captured selectors have not been replaced. This is the
    /// same snapshot edge consumed by guards, not a replay of an initializer.
    pub(in crate::checks::ranges) fn captured_length_collection(
        &self,
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let symbol =
            validation::expression_traversal::single_segment_name_symbol(program, expression)?;
        self.scalar_binding_dependencies.iter().find_map(|row| {
            (row.place.root == facts::PlaceRoot::Symbol(symbol))
                .then(|| row.length.as_ref().map(|length| length.collection))
                .flatten()
        })
    }

    fn capture_length(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        source: ExpressionHandle,
    ) -> Option<CapturedLength> {
        if let Some(symbol) =
            validation::expression_traversal::single_segment_name_symbol(program, source)
        {
            return self.scalar_binding_dependencies.iter().find_map(|row| {
                (row.machine == machine.symbol
                    && row.state == state.symbol
                    && row.place.root == facts::PlaceRoot::Symbol(symbol))
                .then(|| row.length.clone())
                .flatten()
            });
        }
        if let Some((_, length)) = self
            .measured_results
            .iter()
            .find(|(result, _)| *result == source)
        {
            return Some(length.clone());
        }
        let collection = validation::length_member(program, source)?.receiver;
        self.capture_collection_length(program, machine, state, collection)
    }

    fn capture_collection_length(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        collection: ExpressionHandle,
    ) -> Option<CapturedLength> {
        let reference =
            super::super::types::expression_type_reference(program, machine, state, collection)?;
        let reference = validation::unwrapped_type_reference(program, reference)?;
        // A record field merely named `len` is an ordinary scalar. Only the
        // compiler-owned array/slice metadata operation establishes this edge.
        if !matches!(
            program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::FixedArray { .. }
                | typed_trees::types::TypeReferenceNode::Slice { .. }
        ) {
            return None;
        }
        let descriptor = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            self.statement_index,
            collection,
        )?;
        let mut reads = Vec::new();
        if !collect_reads(
            program,
            machine,
            state,
            self.statement_index,
            collection,
            self.checked_calls,
            self.checked_operators,
            &mut reads,
            0,
            self.bound_lookup(),
        ) || !reads
            .iter()
            .any(|read| crate::flow::canonical_places_agree(program, read, &descriptor))
        {
            return None;
        }
        reads.retain(|read| !crate::flow::canonical_places_agree(program, read, &descriptor));
        Some(CapturedLength {
            collection,
            descriptor,
            selectors: reads,
        })
    }

    pub(super) fn retire_captured_observations(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        writes: Option<&[CanonicalPlace]>,
    ) {
        self.measured_results.retain(|(_, length)| {
            !writes.is_none_or(|writes| {
                writes.iter().any(|write| {
                    super::invalidation::write_covers_place(program, write, &length.descriptor)
                        || length
                            .selectors
                            .iter()
                            .any(|read| places_overlap(program, read, write))
                })
            })
        });
        for row in &mut self.scalar_binding_dependencies {
            if row.value.as_ref().is_some_and(|value| {
                writes.is_none_or(|writes| {
                    writes.iter().any(|write| {
                        places_overlap(program, &row.place, write)
                            || value
                                .reads
                                .iter()
                                .any(|read| places_overlap(program, read, write))
                    })
                })
            }) {
                row.value = None;
            }
            let Some(length) = &row.length else {
                continue;
            };
            if writes.is_none_or(|writes| {
                writes.iter().any(|write| {
                    places_overlap(program, &row.place, write)
                        || super::invalidation::write_covers_place(
                            program,
                            write,
                            &length.descriptor,
                        )
                        || length
                            .selectors
                            .iter()
                            .any(|read| places_overlap(program, read, write))
                })
            }) {
                row.length = None;
            }
        }
    }

    pub(in crate::checks::ranges) fn begin_result_measurement(
        &mut self,
        expression: ExpressionHandle,
    ) {
        self.measured_results
            .retain(|(result, _)| *result != expression);
    }

    /// Capture when the measured argument finishes, before the next operand
    /// executes. Subsequent argument/callee writes use ordinary invalidation.
    pub(in crate::checks::ranges) fn capture_result_measurement(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        argument: ExpressionHandle,
    ) {
        let Some(call) = program.expression_table.call_node(expression) else {
            return;
        };
        let Some(collection) = validation::normal_return_length_collection(program, call) else {
            return;
        };
        let actual = match program.expression_table.expression(argument) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => argument,
        };
        if actual != collection {
            return;
        }
        if let Some(length) = self.capture_collection_length(program, machine, state, collection) {
            self.measured_results.push((expression, length));
        }
    }

    pub(in crate::checks::ranges) fn record_expression_dependencies(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) {
        self.record_dependencies(
            program,
            machine,
            &ExpressionDependencyScope {
                evaluation: state,
                destination: state,
                parameter_roots: &[],
            },
            expression,
            0,
        );
    }

    /// The caller supplies only parameter bindings already proved label-stable
    /// by the incoming-edge analysis. Incomplete or unmapped reads remain
    /// incomplete; they never become storage-free premises.
    pub(in crate::checks::ranges) fn record_incoming_guard_dependencies(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        evaluation: &State,
        destination: &State,
        parameter_roots: &[(SymbolHandle, SymbolHandle)],
        expression: ExpressionHandle,
    ) {
        self.record_dependencies(
            program,
            machine,
            &ExpressionDependencyScope {
                evaluation,
                destination,
                parameter_roots,
            },
            expression,
            0,
        );
    }

    fn record_dependencies(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        scope: &ExpressionDependencyScope<'_>,
        expression: ExpressionHandle,
        depth: usize,
    ) {
        let state = scope.destination;
        if depth >= EXPRESSION_WALK_DEPTH_BOUND
            || !program.expression_table.expression_is_valid(expression)
        {
            return;
        }
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Name(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
        ) {
            return;
        }
        if self
            .recorded_expressions
            .contains(&(expression, machine.symbol, state.symbol))
        {
            return;
        }
        let mut reads = Vec::new();
        let mut complete = validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(scope.evaluation),
            expression,
        ) && {
            let bound_lookup = self.bound_lookup();
            collect_reads(
                program,
                machine,
                scope.evaluation,
                self.statement_index,
                expression,
                self.checked_calls,
                self.checked_operators,
                &mut reads,
                0,
                bound_lookup,
            )
        };
        let descriptor_read = validation::collection_length_receiver(
            program,
            machine,
            Some(scope.evaluation),
            expression,
        )
        .and_then(|receiver| {
            validation::canonical_place_from_expression_in_state(
                program,
                scope.evaluation.symbol,
                self.statement_index,
                receiver,
            )
        })
        .and_then(|descriptor| {
            reads
                .iter()
                .position(|read| crate::flow::canonical_places_agree(program, read, &descriptor))
        });
        if complete && scope.evaluation.symbol != state.symbol {
            complete = state_transport::transport_reads(
                program,
                machine,
                scope,
                self.statement_index,
                &mut reads,
            );
        }
        self.recorded_expressions
            .insert((expression, machine.symbol, state.symbol));
        self.expression_dependencies.push(ExpressionDependencies {
            expression,
            label: program.expression_table.display_name(expression),
            machine: machine.symbol,
            state: state.symbol,
            descriptor: descriptor_read
                .filter(|_| complete)
                .map(|position| reads[position].clone()),
            reads: complete.then_some(reads),
        });
        let children = match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => [binary.left, binary.right],
            ExpressionNode::Unary(unary) => [unary.operand, ExpressionHandle::invalid()],
            ExpressionNode::Cast(cast) => [cast.value, ExpressionHandle::invalid()],
            _ => return,
        };
        for child in children {
            self.record_dependencies(program, machine, scope, child, depth + 1);
        }
    }

    /// The display labels whose rows cannot select one typed meaning, judged
    /// over the whole table once per query instead of once per row.
    ///
    /// Two rows agree when they share machine and state and their read sets
    /// match (`same_reads`). That relation is symmetric and transitive but not
    /// reflexive: an incomplete read set, or a place with an unresolved
    /// segment, agrees with nothing, itself included. For such a relation the
    /// label's first row decides the whole label: when every row agrees with
    /// it, all rows agree pairwise; otherwise every row disagrees with some
    /// row. A row's own label therefore collides exactly when the per-row
    /// scan over every same-label row would find a disagreement. A scalar
    /// binding enters that comparison as its single place, and bindings
    /// sharing a label must also name one exact place among themselves.
    fn label_collisions(&self, program: &dyn ::validation::CheckingInput) -> LabelCollisions<'_> {
        let mut groups = BTreeMap::<&str, LabelRows<'_>>::new();
        for row in &self.expression_dependencies {
            groups
                .entry(row.label.as_str())
                .or_default()
                .expressions
                .push(row);
        }
        for row in &self.scalar_binding_dependencies {
            groups
                .entry(row.label.as_str())
                .or_default()
                .bindings
                .push(row);
        }
        let mut collisions = LabelCollisions::default();
        for (label, rows) in groups {
            let mut meanings = rows
                .expressions
                .iter()
                .map(|row| {
                    (
                        row.machine,
                        row.state,
                        row.reads.as_deref(),
                        row.descriptor.as_ref(),
                    )
                })
                .chain(rows.bindings.iter().map(|row| {
                    (
                        row.machine,
                        row.state,
                        Some(std::slice::from_ref(&row.place)),
                        None,
                    )
                }));
            let first = meanings.next().expect("a label group holds one row");
            let agree = |left: RowMeaning<'_>, right: RowMeaning<'_>| {
                left.0 == right.0
                    && left.1 == right.1
                    && same_reads(program, left.2, right.2)
                    && match (left.3, right.3) {
                        (None, None) => true,
                        (Some(left), Some(right)) => {
                            crate::flow::canonical_places_agree(program, left, right)
                        }
                        _ => false,
                    }
            };
            let meanings_agree = agree(first, first) && meanings.all(|other| agree(first, other));
            if !meanings_agree && !rows.expressions.is_empty() {
                collisions.expression_labels.insert(label);
            }
            let bindings_identical = rows.bindings.split_first().is_none_or(|(first, rest)| {
                rest.iter().all(|other| {
                    other.machine == first.machine
                        && other.state == first.state
                        && other.place == first.place
                })
            });
            if !bindings_identical || (!meanings_agree && !rows.expressions.is_empty()) {
                collisions.binding_labels.insert(label);
            }
        }
        collisions
    }

    pub(super) fn preserved_expression_labels(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        writes: Option<&[CanonicalPlace]>,
    ) -> Vec<String> {
        let Some(writes) = writes else {
            return Vec::new();
        };
        let mut writes = writes.to_vec();
        for write in &mut writes {
            if !matches!(write.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid() && program.symbols.get(symbol).kind != symbols::SymbolKind::Field)
            {
                return Vec::new();
            }
        }
        let collisions = self.label_collisions(program);
        let mut preserved = Vec::new();
        for row in &self.expression_dependencies {
            if !row.reads.as_ref().is_some_and(|reads| {
                row.machine == machine.symbol
                    && row.state == state.symbol
                    && program.expression_table.expression_is_valid(row.expression)
                    && reads.iter().all(|read| {
                        writes.iter().all(|write| {
                            if row.descriptor.as_ref().is_some_and(|descriptor| {
                                crate::flow::canonical_places_agree(program, descriptor, read)
                            }) {
                                !super::invalidation::write_covers_place(program, write, read)
                            } else {
                                !places_overlap(program, read, write)
                            }
                        })
                    })
            }) {
                continue;
            }
            // Equal display text is only a lookup key for the old fact tables.
            // It cannot select one of several incompatible typed meanings.
            if collisions.expression_labels.contains(row.label.as_str()) {
                continue;
            }
            if !preserved.contains(&row.label) {
                preserved.push(row.label.clone());
            }
        }
        for row in &self.scalar_binding_dependencies {
            if row.machine != machine.symbol
                || row.state != state.symbol
                || writes
                    .iter()
                    .any(|write| places_overlap(program, &row.place, write))
                || collisions.binding_labels.contains(row.label.as_str())
            {
                continue;
            }
            if !preserved.contains(&row.label) {
                preserved.push(row.label.clone());
            }
        }
        preserved
    }

    /// An owned integer slot has no referent: complete storage writes suffice
    /// even when other locals prevent alias expansion. Keep all the ordinary
    /// identity and same-label collision checks before selecting these rows.
    pub(super) fn preserved_scalar_binding_labels(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        storage_writes: Option<&[CanonicalPlace]>,
    ) -> Vec<String> {
        let mut preserved =
            self.preserved_expression_labels(program, machine, state, storage_writes);
        preserved.retain(|label| {
            self.scalar_binding_dependencies.iter().any(|row| {
                row.machine == machine.symbol && row.state == state.symbol && row.label == *label
            })
        });
        preserved
    }

    /// The labels this machine state's rows carry outside `preserved`, the
    /// caller's `preserved_expression_labels` answer for the same writes.
    pub(super) fn affected_expression_labels(
        &self,
        machine: &Machine,
        state: &State,
        preserved: &[String],
    ) -> Vec<String> {
        self.expression_dependencies
            .iter()
            .filter(|row| {
                row.machine == machine.symbol
                    && row.state == state.symbol
                    && !preserved.contains(&row.label)
            })
            .map(|row| row.label.clone())
            .chain(
                self.scalar_binding_dependencies
                    .iter()
                    .filter(|row| {
                        row.machine == machine.symbol
                            && row.state == state.symbol
                            && !preserved.contains(&row.label)
                    })
                    .map(|row| row.label.clone()),
            )
            .collect()
    }

    pub(in crate::checks::ranges) fn expression_is_disjoint_from_writes(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        writes: Option<&[CanonicalPlace]>,
    ) -> bool {
        self.preserved_expression_labels(program, machine, state, writes)
            .contains(&program.expression_table.display_name(expression))
    }
}

fn places_overlap(
    program: &dyn ::validation::CheckingInput,
    left: &CanonicalPlace,
    right: &CanonicalPlace,
) -> bool {
    (left.root) == (right.root)
        && crate::flow::canonical_place_segments_may_overlap(
            program,
            &left.segments,
            &right.segments,
        )
}

fn same_reads(
    program: &dyn ::validation::CheckingInput,
    left: Option<&[CanonicalPlace]>,
    right: Option<&[CanonicalPlace]>,
) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    // Separate guard occurrences may copy the same typed selector tree into
    // different arena slots. Compare their meaning, not those allocation slots.
    let equal = |left: &CanonicalPlace, right: &CanonicalPlace| {
        crate::flow::canonical_places_agree(program, left, right)
    };
    left.iter()
        .all(|read| right.iter().any(|other| equal(read, other)))
        && right
            .iter()
            .all(|read| left.iter().any(|other| equal(read, other)))
}
