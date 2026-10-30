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

// Source-text tags are process-wide allocation identities. A record's old
// tags may already belong to unrelated live forests in this process. Allocate
// one fresh tag per retained page and relocate only this storage domain's name
// handles; semantic arena generations and source coordinates remain unchanged.
impl RetainedSource {
    fn read_storage<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<(u32, Self), compiler_storage::RecordError> {
        let original: u32 = compiler_storage::Record::read(input)?;
        if original == 0 {
            return Err(input.error("retained syntax source has a zero generation"));
        }
        let text: Arc<str> = compiler_storage::Record::read(input)?;
        let generation = input.define_generation::<Self, _>(original, &text, || {
            NEXT_SOURCE_GENERATION
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    current.checked_add(1)
                })
                .map_err(|_| "retained syntax source generation exhausted")
        })?;
        Ok((original, Self { generation, text }))
    }
}
impl compiler_storage::Record for RetainedSource {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        compiler_storage::Record::write(&self.generation, output)?;
        compiler_storage::Record::write(&self.text, output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        Self::read_storage(input).map(|(_, source)| source)
    }
}
impl compiler_storage::Record for SyntaxTextOwner {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        compiler_storage::Record::write(&self.sources, output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        input.remap_generations::<RetainedSource>();
        let mut sources = input.record(|input| {
            let length = input.sequence::<RetainedSource>()?;
            let mut sources = Vec::with_capacity(length);
            let mut previous = 0;
            for _ in 0..length {
                let (original, source) = RetainedSource::read_storage(input)?;
                if original <= previous {
                    return Err(
                        input.error("retained syntax sources are not in unique generation order")
                    );
                }
                previous = original;
                sources.push(source);
            }
            Ok(sources)
        })?;
        // Overlapping forests may first define their shared pages in another
        // order. Restore the lookup owner's order after applying the one map.
        sources.sort_unstable_by_key(|source| source.generation);
        Ok(Self { sources })
    }
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    use compiler_storage::{RecordLimits, decode, encode};

    #[test]
    fn restored_forests_keep_shared_pages_without_colliding_with_live_names() {
        let mut first = SyntaxTextOwner::default();
        let first_name = first.retain(Arc::from("first"));
        let mut second = SyntaxTextOwner::default();
        let second_name = second.retain(Arc::from("second"));
        let mut both = first.clone();
        both.retain_from(&second);
        // Reading second first defines its newer old generation before first.
        let bytes = encode(
            &(second, both, first_name, second_name),
            RecordLimits::default(),
        )
        .unwrap();
        let mut live = SyntaxTextOwner::default();
        let live_name = live.retain(Arc::from("live"));
        let (second, both, restored_first, restored_second): (
            SyntaxTextOwner,
            SyntaxTextOwner,
            Handle<RetainedSource>,
            Handle<RetainedSource>,
        ) = decode(&bytes, RecordLimits::default()).unwrap();
        assert_ne!(first_name, restored_first);
        assert_ne!(second_name, restored_second);
        assert_eq!(both.source_text(restored_first), "first");
        assert_eq!(both.source_text(restored_second), "second");
        assert_eq!(second.source_text(restored_second), "second");
        live.retain_from(&both);
        assert_eq!(live.source_text(live_name), "live");
        assert_eq!(live.source_text(restored_first), "first");
        assert_eq!(live.source_text(restored_second), "second");
    }

    #[test]
    fn duplicate_page_identity_cannot_change_its_retained_text() {
        let mut first = SyntaxTextOwner::default();
        let handle = first.retain(Arc::from("original"));
        let conflicting = SyntaxTextOwner {
            sources: vec![RetainedSource {
                generation: handle.generation(),
                text: Arc::from("substituted"),
            }],
        };
        let bytes = encode(&(first, conflicting), RecordLimits::default()).unwrap();
        assert!(
            decode::<(SyntaxTextOwner, SyntaxTextOwner)>(&bytes, RecordLimits::default()).is_err()
        );
    }
}
