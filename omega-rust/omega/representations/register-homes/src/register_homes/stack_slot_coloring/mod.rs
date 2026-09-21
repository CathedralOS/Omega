//! Durable stack-slot coloring record: the versioned transport for
//! target-neutral spill-area-relative assignments keyed by canonical identity.
//! Computation, validation, and replay-against-roots live in the producing
//! transform; decoding grants no slot, frame, or emission authority.

mod codec;
mod identity;
mod model;

pub use identity::stack_slot_coloring_identity;
pub use model::*;
