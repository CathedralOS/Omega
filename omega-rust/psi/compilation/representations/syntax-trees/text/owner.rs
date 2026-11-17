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
        sources.sort_unstable_by_key(|source| source.generation);
        Ok(Self { sources })
    }
}

#[cfg(test)]
#[path = "../tests/unit/text_owner_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/unit/text_owner_storage_tests.rs"]
mod storage_tests;
