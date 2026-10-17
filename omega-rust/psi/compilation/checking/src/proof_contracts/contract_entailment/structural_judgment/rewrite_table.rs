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
}

impl RewriteTable {
    pub(super) fn push(&mut self, entry: (StructuralTerm, StructuralTerm)) {
        let key = rewrite_key(&entry.0);
        self.index
            .entry(key)
            .or_default()
            .push(self.entries.len() as u32);
        self.entries.push(entry);
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
