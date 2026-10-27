//! Arena records keep storage identity; their payload owners admit meaning.
use super::value::{RecordError, RecordReader, RecordValue, RecordWriter};
use arena::{Arena, ArenaSlotStorage, Handle, HandleSpan};

impl<T> RecordValue for Handle<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.arena_index().write(output)?;
        self.generation().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let index = u32::read(input)?;
        let generation = u32::read(input)?;
        if (index == 0) != (generation == 0) {
            return Err(input.error("handle has a partial absence identity"));
        }
        Ok(Self::from_parts(index, generation))
    }
}

impl<T> RecordValue for HandleSpan<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.start().write(output)?;
        self.count().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let start = Handle::read(input)?;
        let count = u32::read(input)?;
        if (count != 0 && !start.is_valid())
            || (count != 0 && start.arena_index().checked_add(count - 1).is_none())
        {
            return Err(input.error("span has an invalid handle range"));
        }
        Ok(Self::from_parts(start, count))
    }
}

impl<T: RecordValue> RecordValue for Arena<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        let storage = self.storage_view();
        storage.dummy.write(output)?;
        output.sequence(storage.items.len())?;
        for item in storage.items {
            item.write(output)?;
        }
        storage.fresh_generation.write(output)?;
        storage.fresh.write(output)?;
        storage.slots.is_some().write(output)?;
        if let Some(slots) = storage.slots {
            slots.generations.write(output)?;
            slots.occupied.write(output)?;
            slots.free_indices.write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            let dummy = T::read(input)?;
            let items = Vec::<T>::read(input)?;
            let generation = u32::read(input)?;
            let fresh = bool::read(input)?;
            let slots = if bool::read(input)? {
                let generations = Vec::<u32>::read(input)?;
                let occupied = Vec::<bool>::read(input)?;
                let free_indices = Vec::<u32>::read(input)?;
                // Recovery checks membership using a temporary vacancy bitmap.
                input.allocate(occupied.len())?;
                Some(ArenaSlotStorage {
                    generations,
                    occupied,
                    free_indices,
                })
            } else {
                None
            };
            Arena::from_storage(dummy, items, slots, generation, fresh)
                .map_err(|reason| input.error(reason))
        })
    }
}
