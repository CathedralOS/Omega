//! Bucketed probe index over the hypothesis rewrite list.
//!
//! Resolution probes every rewrite pattern against each reached term; a flat
//! `Vec` paid one deep structural equality per stored pattern per probe. The
//! index keys each pattern on the same discriminating fields structural
//! equality compares (variant plus its primary identity field), so a probe
//! equality-scans only the bucket that could possibly match. Buckets retain
//! insertion order, preserving the original first-match semantics exactly.

use super::StructuralTerm;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Clone, Debug, Default)]
pub(super) struct RewriteTable {
    entries: Vec<(StructuralTerm, StructuralTerm)>,
    index: HashMap<(u8, u64), Vec<u32>>,
    potential_integer_premise_indices: Vec<u32>,
    deferred_collection_measures: Vec<(u32, u32)>,
    integer_premise_revision: usize,
    max_premise_occurrence: usize,
    has_mathematical_binary_pattern: bool,
    has_mathematical_equal_pattern: bool,
}

impl RewriteTable {
    pub(super) fn push(&mut self, entry: (StructuralTerm, StructuralTerm)) {
        let index = self.entries.len() as u32;
        if let StructuralTerm::MathematicalBinary { operator, .. } = entry.0.semantic_value() {
            self.has_mathematical_binary_pattern = true;
            if *operator == typed_trees::expression::BinaryOperator::Equal {
                self.has_mathematical_equal_pattern = true;
            }
        }
        let deferred_measure_occurrence = match (entry.0.semantic_value(), entry.1.semantic_value())
        {
            (StructuralTerm::CollectionMeasure { collection, .. }, StructuralTerm::Integer(_)) => {
                match collection.semantic_value() {
                    StructuralTerm::CapturedValue { occurrence, .. }
                        if self.find(collection.semantic_value()).is_none() =>
                    {
                        Some(*occurrence as u32)
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(occurrence) = deferred_measure_occurrence {
            self.deferred_collection_measures.push((occurrence, index));
        } else {
            if let StructuralTerm::CapturedValue { occurrence, .. } = entry.0.semantic_value() {
                let target_occ = *occurrence as u32;
                let mut promoted = false;
                self.deferred_collection_measures
                    .retain(|&(occ, deferred_idx)| {
                        if occ == target_occ {
                            let insert_pos = self
                                .potential_integer_premise_indices
                                .binary_search(&deferred_idx)
                                .unwrap_or_else(|pos| pos);
                            self.potential_integer_premise_indices
                                .insert(insert_pos, deferred_idx);
                            promoted = true;
                            false
                        } else {
                            true
                        }
                    });
                if promoted {
                    self.max_premise_occurrence = self.max_premise_occurrence.max(*occurrence);
                    self.integer_premise_revision += 1;
                }
            }
            if is_potential_integer_premise(&entry.0, &entry.1) {
                self.potential_integer_premise_indices.push(index);
                self.max_premise_occurrence = self
                    .max_premise_occurrence
                    .max(max_captured_occurrence(&entry.0))
                    .max(max_captured_occurrence(&entry.1));
                self.integer_premise_revision += 1;
            } else if !self.potential_integer_premise_indices.is_empty() {
                let left_occ = max_captured_occurrence(&entry.0);
                let is_fresh_capture = match entry.0.semantic_value() {
                    StructuralTerm::CapturedValue { occurrence, .. } => {
                        *occurrence > self.max_premise_occurrence
                    }
                    StructuralTerm::Projection { .. }
                    | StructuralTerm::IndexedValue { .. }
                    | StructuralTerm::CollectionMeasure { .. }
                    | StructuralTerm::ScalarBinary { .. }
                    | StructuralTerm::Application { .. }
                    | StructuralTerm::CallProjection { .. } => {
                        left_occ > self.max_premise_occurrence
                    }
                    _ => false,
                };
                if !is_fresh_capture {
                    self.max_premise_occurrence = self
                        .max_premise_occurrence
                        .max(left_occ)
                        .max(max_captured_occurrence(&entry.1));
                    self.integer_premise_revision += 1;
                }
            }
        }
        let key = rewrite_key(&entry.0);
        self.index.entry(key).or_default().push(index);
        self.entries.push(entry);
    }

    pub(super) fn has_potential_integer_premises(&self) -> bool {
        !self.potential_integer_premise_indices.is_empty()
    }

    pub(super) fn potential_integer_premises_len(&self) -> usize {
        self.potential_integer_premise_indices.len()
    }

    pub(super) fn max_premise_occurrence(&self) -> usize {
        self.max_premise_occurrence
    }

    pub(super) fn bump_integer_premise_revision_for_occurrences(&mut self, max_occurrence: usize) {
        self.max_premise_occurrence = self.max_premise_occurrence.max(max_occurrence);
        if !self.potential_integer_premise_indices.is_empty() {
            self.integer_premise_revision += 1;
        }
    }

    pub(super) fn integer_premise_revision(&self) -> usize {
        self.integer_premise_revision
    }

    pub(super) fn has_mathematical_binary_pattern(&self) -> bool {
        self.has_mathematical_binary_pattern
    }

    pub(super) fn has_mathematical_equal_pattern(&self) -> bool {
        self.has_mathematical_equal_pattern
    }

    pub(super) fn potential_integer_premises(
        &self,
    ) -> impl Iterator<Item = &(StructuralTerm, StructuralTerm)> {
        self.potential_integer_premise_indices
            .iter()
            .map(|&index| &self.entries[index as usize])
    }

    /// The first stored pattern equal to `term`, in the same insertion order
    /// the flat scan observed.
    pub(super) fn find(&self, term: &StructuralTerm) -> Option<&(StructuralTerm, StructuralTerm)> {
        let indices = self.index.get(&rewrite_key(term))?;
        for &index in indices {
            let entry = &self.entries[index as usize];
            if entry.0 == *term {
                return Some(entry);
            }
        }
        None
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &(StructuralTerm, StructuralTerm)> {
        self.entries.iter()
    }

    pub(super) fn entries(&self) -> &[(StructuralTerm, StructuralTerm)] {
        &self.entries
    }
}

fn string_key(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn handle_key<T>(handle: arena::Handle<T>) -> u64 {
    handle.arena_index() as u64 | (handle.generation() as u64) << 32
}

fn discriminant_key<T>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    std::mem::discriminant(value).hash(&mut hasher);
    hasher.finish()
}

/// The bucket discriminant is drawn only from fields structural equality
/// compares, so equal terms always share a bucket. Different terms may share
/// one freely; `find` still confirms every candidate with full equality.
fn rewrite_key(term: &StructuralTerm) -> (u8, u64) {
    match term.semantic_value() {
        StructuralTerm::Variable(name) => (1, string_key(name)),
        StructuralTerm::BoundValue(symbol) => (2, handle_key(*symbol)),
        StructuralTerm::CapturedValue { expression, .. } => (3, handle_key(*expression)),
        StructuralTerm::BoundProjection { subject, .. } => (4, handle_key(*subject)),
        StructuralTerm::Integer(_) => (5, 0),
        StructuralTerm::Float(_) => (6, 0),
        StructuralTerm::Constructor { data, case, .. } => {
            (7, string_key(data) ^ string_key(case).rotate_left(32))
        }
        StructuralTerm::Application { target, .. } => (8, handle_key(*target)),
        StructuralTerm::CallProjection { target, field, .. } => {
            (9, handle_key(*target) ^ handle_key(*field).rotate_left(32))
        }
        StructuralTerm::IndexedValue { .. } => (10, 0),
        StructuralTerm::Projection { .. } => (11, 0),
        StructuralTerm::ScalarBinary { operator, .. } => (12, discriminant_key(operator)),
        StructuralTerm::Opaque(text) => (13, string_key(text)),
        // `semantic_value` already unwrapped every observed layer; a stray
        // observed pattern lands in its own bucket and still compares
        // exactly, only without index help.
        StructuralTerm::ObservedValue { .. } => (14, 0),
        StructuralTerm::IntegerEmbedding { .. } => (15, 0),
        StructuralTerm::MathematicalBinary { operator, .. } => (16, discriminant_key(operator)),
        StructuralTerm::CollectionMeasure { measure, .. } => (17, discriminant_key(measure)),
    }
}

fn is_potential_integer_premise(predicate: &StructuralTerm, truth: &StructuralTerm) -> bool {
    match truth.semantic_value() {
        StructuralTerm::Constructor { data, fields, .. } => {
            data == "bool"
                && fields.is_empty()
                && predicate_may_unwrap_to_integer_comparison(predicate, 0)
        }
        StructuralTerm::Float(_)
        | StructuralTerm::Opaque(_)
        | StructuralTerm::BoundProjection { .. } => false,
        StructuralTerm::BoundValue(_) | StructuralTerm::CapturedValue { .. }
            if matches!(
                predicate.semantic_value(),
                StructuralTerm::CapturedValue { .. }
            ) =>
        {
            false
        }
        other => {
            if matches!(
                predicate.semantic_value(),
                StructuralTerm::CapturedValue { .. }
                    | StructuralTerm::Projection { .. }
                    | StructuralTerm::IndexedValue { .. }
                    | StructuralTerm::Application { .. }
                    | StructuralTerm::CallProjection { .. }
            ) && !term_has_mathematical_or_integer_embedding(other)
            {
                return false;
            }
            term_may_produce_mathematical_value(predicate)
                && term_may_produce_mathematical_value(other)
        }
    }
}

fn term_has_mathematical_or_integer_embedding(term: &StructuralTerm) -> bool {
    match term {
        StructuralTerm::MathematicalBinary { .. }
        | StructuralTerm::IntegerEmbedding { .. }
        | StructuralTerm::CollectionMeasure { .. } => true,
        StructuralTerm::ObservedValue {
            value, descriptor, ..
        } => {
            term_has_mathematical_or_integer_embedding(value)
                || descriptor
                    .as_deref()
                    .is_some_and(term_has_mathematical_or_integer_embedding)
        }
        StructuralTerm::Constructor { fields, .. } => fields
            .iter()
            .any(|(_, value)| term_has_mathematical_or_integer_embedding(value)),
        StructuralTerm::Application { arguments, .. }
        | StructuralTerm::CallProjection { arguments, .. } => arguments
            .iter()
            .any(term_has_mathematical_or_integer_embedding),
        StructuralTerm::IndexedValue {
            collection, index, ..
        } => {
            term_has_mathematical_or_integer_embedding(collection)
                || term_has_mathematical_or_integer_embedding(index)
        }
        StructuralTerm::Projection { subject, .. } => {
            term_has_mathematical_or_integer_embedding(subject)
        }
        StructuralTerm::ScalarBinary { left, right, .. } => {
            term_has_mathematical_or_integer_embedding(left)
                || term_has_mathematical_or_integer_embedding(right)
        }
        StructuralTerm::Variable(_)
        | StructuralTerm::BoundValue(_)
        | StructuralTerm::CapturedValue { .. }
        | StructuralTerm::BoundProjection { .. }
        | StructuralTerm::Integer(_)
        | StructuralTerm::Float(_)
        | StructuralTerm::Opaque(_) => false,
    }
}

fn max_captured_occurrence(term: &StructuralTerm) -> usize {
    match term {
        StructuralTerm::CapturedValue { occurrence, .. } => *occurrence,
        StructuralTerm::ObservedValue {
            value, descriptor, ..
        } => max_captured_occurrence(value)
            .max(descriptor.as_deref().map_or(0, max_captured_occurrence)),
        StructuralTerm::Constructor { fields, .. } => fields
            .iter()
            .map(|(_, value)| max_captured_occurrence(value))
            .max()
            .unwrap_or(0),
        StructuralTerm::Application { arguments, .. }
        | StructuralTerm::CallProjection { arguments, .. } => arguments
            .iter()
            .map(max_captured_occurrence)
            .max()
            .unwrap_or(0),
        StructuralTerm::IndexedValue {
            collection, index, ..
        } => max_captured_occurrence(collection).max(max_captured_occurrence(index)),
        StructuralTerm::CollectionMeasure { collection, .. } => max_captured_occurrence(collection),
        StructuralTerm::Projection { subject, .. } => max_captured_occurrence(subject),
        StructuralTerm::ScalarBinary { left, right, .. }
        | StructuralTerm::MathematicalBinary { left, right, .. } => {
            max_captured_occurrence(left).max(max_captured_occurrence(right))
        }
        StructuralTerm::IntegerEmbedding { value, .. } => max_captured_occurrence(value),
        StructuralTerm::Variable(_)
        | StructuralTerm::BoundValue(_)
        | StructuralTerm::BoundProjection { .. }
        | StructuralTerm::Integer(_)
        | StructuralTerm::Float(_)
        | StructuralTerm::Opaque(_) => 0,
    }
}

fn term_may_produce_mathematical_value(term: &StructuralTerm) -> bool {
    !matches!(
        term.semantic_value(),
        StructuralTerm::Float(_)
            | StructuralTerm::Opaque(_)
            | StructuralTerm::ScalarBinary { .. }
            | StructuralTerm::Constructor { .. }
            | StructuralTerm::BoundProjection { .. }
    )
}

fn predicate_may_unwrap_to_integer_comparison(predicate: &StructuralTerm, depth: usize) -> bool {
    if depth >= 64 {
        return true;
    }
    match predicate.semantic_value() {
        StructuralTerm::MathematicalBinary { operator, .. } => operator.is_comparison(),
        StructuralTerm::ScalarBinary {
            operator,
            meaning,
            left,
            right,
            ..
        } => {
            if !operator.is_comparison() {
                return false;
            }
            if meaning
                .iter()
                .all(|(primitive, _)| primitive.accepts_integer_literal())
            {
                return true;
            }
            if meaning
                .iter()
                .all(|(primitive, _)| *primitive == typed_trees::types::PrimitiveType::Bool)
                && matches!(
                    operator,
                    typed_trees::expression::BinaryOperator::Equal
                        | typed_trees::expression::BinaryOperator::NotEqual
                )
            {
                if right.boolean_value().is_some() {
                    return predicate_may_unwrap_to_integer_comparison(left, depth + 1);
                }
                if left.boolean_value().is_some() {
                    return predicate_may_unwrap_to_integer_comparison(right, depth + 1);
                }
            }
            false
        }
        _ => false,
    }
}
