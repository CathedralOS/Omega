//! Final-image fixtures shared by this crate's encoding witnesses.
//!
//! `FinalImage::with_capacity` takes three arena capacities. They only
//! preallocate; no capacity reaches the emitted bytes, the symbol table or the
//! relocation table. Each witness below starts with an invalid entry symbol and
//! empty tables, so the counts are stated once here instead of at every site.

use arena::Handle;
use image::{FinalImage, FinalImageMemory};

pub(crate) fn image(target: target::NativeTarget, memory: FinalImageMemory) -> FinalImage {
    FinalImage::with_capacity(target, memory, Handle::invalid(), 0, 0, 0)
}
