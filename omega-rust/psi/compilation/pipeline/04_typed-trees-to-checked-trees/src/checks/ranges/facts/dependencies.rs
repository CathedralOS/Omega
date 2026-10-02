//! Exact typed reads behind the range checker's legacy expression labels.
//!
//! Incoming guards measure predecessor storage. Record those reads in their
//! evaluation scope, then transport exact parameter roots to the receiving
//! state. Field/index paths survive that join; spelling alone cannot keep a
//! stale predecessor parameter current or authorize an unrelated binding.

mod captures;
mod reads;
pub(crate) use reads::enter_root_currency_scope;
#[cfg(test)]
mod tests;

use reads::collect_reads;

use std::collections::{BTreeMap, BTreeSet};

use super::RangeFacts;
use crate::flow::CanonicalPlace;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::{TypedTrees, machine::Machine, state::State};

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
}

/// A captured scalar reads its own storage, independently of its initializer.
/// This is declaration identity for existing facts, not a replayable value.
#[derive(Clone)]
pub(super) struct ScalarBindingDependency {
    label: String,
    machine: SymbolHandle,
    state: SymbolHandle,
    place: CanonicalPlace,
}

/// One row's typed meaning for label-collision judgment: machine, state and
/// read set, `None` when the read set is incomplete.
type RowMeaning<'table> = (SymbolHandle, SymbolHandle, Option<&'table [CanonicalPlace]>);

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
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<i64> {
        if self.exact_lengths.is_empty() {
            return None;
        }
        let mut reads = Vec::new();
        let bound_lookup = self.bound_lookup().clone();
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
            &bound_lookup,
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
        program: &TypedTrees,
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
                    && crate::flow::normalized_event_place_root(program, place.root)
                        == facts::PlaceRoot::Symbol(machine.symbol)
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
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        storages: &[(CanonicalPlace, SymbolHandle)],
    ) -> Vec<(String, ExpressionHandle, CanonicalPlace)> {
        self.expression_dependencies
            .iter()
            .filter(|row| row.machine == machine.symbol && row.state == state.symbol)
            .filter_map(|row| {
                let [place] = row.reads.as_deref()? else {
                    return None;
                };
                let mut place = crate::flow::rebase_exact_local_place(
                    program,
                    state.symbol,
                    self.statement_index,
                    place.clone(),
                    self.checked_calls.and_then(|context| context.call_frames()),
                )?;
                crate::flow::normalize_attached_place_root(
                    program,
                    machine.symbol,
                    state.symbol,
                    &mut place,
                );
                place.root = crate::flow::normalized_event_place_root(program, place.root);
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
        });
    }

    pub(in crate::checks::ranges) fn record_scalar_binding_dependency(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        source: ExpressionHandle,
        symbol: SymbolHandle,
        label: &str,
    ) {
        use typed_trees::{statement::StatementNode, types::PrimitiveType};
        let Some(StatementNode::LocalData(local)) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(self.statement_index)
        else {
            return;
        };
        if !symbol.is_valid()
            || local.symbol != symbol
            || local.initial_value != source
            || local.name.as_str() != label
            || program.symbols.get(symbol).kind != symbols::SymbolKind::Local
            || program.symbols.get(symbol).parent != state.symbol
            || !matches!(
                program.primitive_type_reference(local.type_reference),
                Some(
                    PrimitiveType::I8
                        | PrimitiveType::I16
                        | PrimitiveType::I32
                        | PrimitiveType::I64
                        | PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                )
            )
        {
            return;
        }
        let Some(place) = crate::flow::canonical_place_from_symbol(symbol) else {
            return;
        };
        if self.scalar_binding_dependencies.iter().any(|row| {
            row.machine == machine.symbol
                && row.state == state.symbol
                && row.label == label
                && row.place == place
        }) {
            return;
        }
        self.scalar_binding_dependencies
            .push(ScalarBindingDependency {
                label: label.to_owned(),
                machine: machine.symbol,
                state: state.symbol,
                place,
            });
    }

    pub(in crate::checks::ranges) fn record_expression_dependencies(
        &mut self,
        program: &TypedTrees,
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
        program: &TypedTrees,
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
        program: &TypedTrees,
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
            let bound_lookup = self.bound_lookup().clone();
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
                &bound_lookup,
            )
        };
        if complete && scope.evaluation.symbol != state.symbol {
            for read in &mut reads {
                if read.root == facts::PlaceRoot::Symbol(machine.symbol) {
                    continue;
                }
                let Some((_, destination)) = scope
                    .parameter_roots
                    .iter()
                    .find(|(source, _)| read.root == facts::PlaceRoot::Symbol(*source))
                else {
                    complete = false;
                    break;
                };
                read.root = facts::PlaceRoot::Symbol(*destination);
            }
        }
        self.recorded_expressions
            .insert((expression, machine.symbol, state.symbol));
        self.expression_dependencies.push(ExpressionDependencies {
            expression,
            label: program.expression_table.display_name(expression),
            machine: machine.symbol,
            state: state.symbol,
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
    fn label_collisions(&self, program: &TypedTrees) -> LabelCollisions<'_> {
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
                .map(|row| (row.machine, row.state, row.reads.as_deref()))
                .chain(rows.bindings.iter().map(|row| {
                    (
                        row.machine,
                        row.state,
                        Some(std::slice::from_ref(&row.place)),
                    )
                }));
            let first = meanings.next().expect("a label group holds one row");
            let agree = |left: RowMeaning<'_>, right: RowMeaning<'_>| {
                left.0 == right.0 && left.1 == right.1 && same_reads(program, left.2, right.2)
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
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        writes: Option<&[CanonicalPlace]>,
    ) -> Vec<String> {
        let Some(writes) = writes else {
            return Vec::new();
        };
        let mut writes = writes.to_vec();
        for write in &mut writes {
            crate::flow::normalize_attached_place_root(
                program,
                machine.symbol,
                state.symbol,
                write,
            );
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
                        writes
                            .iter()
                            .all(|write| !places_overlap(program, read, write))
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
        program: &TypedTrees,
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
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        writes: Option<&[CanonicalPlace]>,
    ) -> bool {
        self.preserved_expression_labels(program, machine, state, writes)
            .contains(&program.expression_table.display_name(expression))
    }
}

fn places_overlap(program: &TypedTrees, left: &CanonicalPlace, right: &CanonicalPlace) -> bool {
    crate::flow::normalized_event_place_root(program, left.root)
        == crate::flow::normalized_event_place_root(program, right.root)
        && crate::flow::canonical_place_segments_may_overlap(
            program,
            &left.segments,
            &right.segments,
        )
}

fn same_reads(
    program: &TypedTrees,
    left: Option<&[CanonicalPlace]>,
    right: Option<&[CanonicalPlace]>,
) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    let equal = |left: &CanonicalPlace, right: &CanonicalPlace| {
        crate::flow::normalized_event_place_root(program, left.root)
            == crate::flow::normalized_event_place_root(program, right.root)
            && left.segments.len() == right.segments.len()
            && left
                .segments
                .iter()
                .zip(&right.segments)
                .all(|(left, right)| match (*left, *right) {
                    (
                        facts::PlaceSegment::Index { expression: left },
                        facts::PlaceSegment::Index { expression: right },
                    ) => program
                        .expression_table
                        .expressions_structurally_equal(left, right),
                    (left, right) => crate::flow::canonical_place_segments_equal(left, right),
                })
    };
    // Separate guard occurrences may copy the same typed selector tree into
    // different arena slots. Compare their meaning, not those allocation slots.
    left.iter()
        .all(|read| right.iter().any(|other| equal(read, other)))
        && right
            .iter()
            .all(|read| left.iter().any(|other| equal(read, other)))
}
