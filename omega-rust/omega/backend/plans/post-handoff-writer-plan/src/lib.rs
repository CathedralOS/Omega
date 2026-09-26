#![forbid(unsafe_code)]

//! Post-handoff writer plans: the reusable fragment ABI, generated fragment
//! plans, invocation plans, and the step/plan carriers a provider replays
//! after control leaves the loader.
//!
//! This is the Omega consumer derivation of the portable materialization
//! description in `layout-plans`. `derive_post_handoff_writer` turns a
//! `SymbolicMaterializationPlan`'s portable actions into a writer program,
//! `PostHandoffWriterPlan::lower_reusable_fragment` separates static fragment
//! geometry from invocation evidence, and the invocation carrier replays its
//! own structure and bound source values before any consumer resolves an
//! address. Byte application against a concrete destination belongs to the
//! executable-installation execution owner; the interpreter consumes the
//! portable actions through its own path and never lowers these fragments.

mod post_handoff_writer;

pub use post_handoff_writer::{
    GeneratedPostHandoffWriterFragmentPlan, GeneratedPostHandoffWriterStep,
    POST_HANDOFF_WRITER_CONTEXT_ABI_V1, POST_HANDOFF_WRITER_DESTINATION_OFFSET,
    POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH, POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET,
    PostHandoffWriterFitConstraint, PostHandoffWriterInvocationPlan, PostHandoffWriterPlan,
    PostHandoffWriterSource, PostHandoffWriterSourceSlot, PostHandoffWriterStep,
    derive_post_handoff_writer, post_handoff_writer_context_byte_len,
};
