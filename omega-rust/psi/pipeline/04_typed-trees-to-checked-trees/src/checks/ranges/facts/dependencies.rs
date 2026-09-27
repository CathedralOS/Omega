//! Exact typed reads behind the range checker's legacy expression labels.

mod captures;
mod reads;
pub(crate) use reads::enter_root_currency_scope;
#[cfg(test)]
mod tests;

use reads::collect_reads;

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
            self.recorded_expressions
                .insert((length.expression, machine, state));
            self.expression_dependencies.push(ExpressionDependencies {
                expression: length.expression,
                label: length.label.clone(),
                machine,
                state,
                reads: Some(vec![length.place.clone()]),
            });
        }
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
        self.record_dependencies(program, machine, state, expression, 0);
    }

    fn record_dependencies(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        depth: usize,
    ) {
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
        let complete = validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(state),
            expression,
        ) && {
            let bound_lookup = self.bound_lookup().clone();
            collect_reads(
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
            )
        };
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
            self.record_dependencies(program, machine, state, child, depth + 1);
        }
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
            if self.expression_dependencies.iter().any(|other| {
                other.label == row.label
                    && (other.machine != row.machine
                        || other.state != row.state
                        || !same_reads(program, other.reads.as_deref(), row.reads.as_deref()))
            }) {
                continue;
            }
            if self.scalar_binding_dependencies.iter().any(|binding| {
                binding.label == row.label
                    && (binding.machine != row.machine
                        || binding.state != row.state
                        || !same_reads(
                            program,
                            row.reads.as_deref(),
                            Some(std::slice::from_ref(&binding.place)),
                        ))
            }) {
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
                || self.scalar_binding_dependencies.iter().any(|other| {
                    other.label == row.label
                        && (other.machine != row.machine
                            || other.state != row.state
                            || other.place != row.place)
                })
                || self.expression_dependencies.iter().any(|other| {
                    other.label == row.label
                        && (other.machine != row.machine
                            || other.state != row.state
                            || !same_reads(
                                program,
                                other.reads.as_deref(),
                                Some(std::slice::from_ref(&row.place)),
                            ))
                })
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

    pub(super) fn affected_expression_labels(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        writes: Option<&[CanonicalPlace]>,
    ) -> Vec<String> {
        let preserved = self.preserved_expression_labels(program, machine, state, writes);
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
