//! Source storage belongs to the syntax forest, not to each name occurrence.
//!
//! Source pages have independently retained lifetimes and generational identities.
//! Copying between forests retains their pages once; names keep their identities
//! without rebuilding strings or incrementing a counter for every occurrence.
//! The page identity is allocated at source registration, outside grammar loops.

use arena::Handle;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

static NEXT_SOURCE_GENERATION: AtomicU32 = AtomicU32::new(1);

#[derive(Clone)]
pub struct RetainedSource {
    generation: u32,
    text: Arc<str>,
}

pub trait SyntaxTextContext {
    fn source_text(&self, source: Handle<RetainedSource>) -> &str;
}

#[derive(Clone, Default)]
pub struct SyntaxTextOwner {
    sources: Vec<RetainedSource>,
}

impl SyntaxTextOwner {
    pub fn retain(&mut self, text: Arc<str>) -> Handle<RetainedSource> {
        let generation = NEXT_SOURCE_GENERATION
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("retained syntax source generation exhausted");
        // Registration identities increase across all forests. Every page
        // already retained here was registered before this one, including pages
        // adopted from another forest, so the new row belongs at the end.
        self.sources.push(RetainedSource { generation, text });
        Handle::from_parts(1, generation)
    }

    pub fn retain_from(&mut self, other: &Self) {
        for source in &other.sources {
            if let Err(position) = self
                .sources
                .binary_search_by_key(&source.generation, |source| source.generation)
            {
                self.sources.insert(position, source.clone());
            }
        }
    }

    pub fn adopt(&mut self, other: Self) {
        for source in other.sources {
            if let Err(position) = self
                .sources
                .binary_search_by_key(&source.generation, |source| source.generation)
            {
                self.sources.insert(position, source);
            }
        }
    }
}

impl SyntaxTextContext for SyntaxTextOwner {
    #[inline]
    fn source_text(&self, source: Handle<RetainedSource>) -> &str {
        assert_eq!(
            source.arena_index(),
            1,
            "invalid retained syntax source handle"
        );
        if let Some(last) = self.sources.last()
            && last.generation == source.generation()
        {
            return &last.text;
        }
        let position = self
            .sources
            .binary_search_by_key(&source.generation(), |source| source.generation)
            .expect("retained syntax source does not belong to this forest");
        &self.sources[position].text
    }
}

impl std::fmt::Debug for SyntaxTextOwner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SyntaxTextOwner")
            .field("source_count", &self.sources.len())
            .finish()
    }
}
impl std::fmt::Debug for RetainedSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedSource")
            .field("generation", &self.generation)
            .field("bytes", &self.text.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{SyntaxTextContext, SyntaxTextOwner};
    use std::sync::Arc;

    #[test]
    fn adoption_keeps_independent_page_identities_and_moves_source_ownership() {
        let text: Arc<str> = Arc::from("retained");
        let mut destination = SyntaxTextOwner::default();
        let first = destination.retain(Arc::from("original"));
        let mut source = SyntaxTextOwner::default();
        let second = source.retain(text.clone());
        destination.adopt(source);
        assert_eq!(Arc::strong_count(&text), 2, "adoption moves the owner");
        assert_eq!(destination.source_text(first), "original");
        assert_eq!(destination.source_text(second), "retained");
    }

    #[test]
    #[should_panic(expected = "does not belong to this forest")]
    fn unrelated_forest_cannot_resolve_a_name_page() {
        let mut source = SyntaxTextOwner::default();
        let handle = source.retain(Arc::from("owned"));
        SyntaxTextOwner::default().source_text(handle);
    }
}
