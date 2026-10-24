#![forbid(unsafe_code)]

//! Post-handoff writer plans: the reusable fragment ABI, generated fragment
//! plans, invocation plans, and the step/plan carriers a provider replays
//! after control leaves the loader.
//!
//! This is the Omega consumer derivation of the portable materialization
//! description in `layout-plans`. `derive_post_handoff_writer` turns a
//! `SymbolicMaterializationPlan`'s portable actions into a writer program,
//! `PostHandoffWriterPlan::lower_reusable_fragment` separates static fragment
//! geometry from invocation evidence, and the invocation carrier admits one
//! concrete destination and replays its own structure and bound source values
//! before any consumer resolves an address. Byte application against a
//! concrete destination belongs to the executable-installation execution
//! owner; the interpreter consumes the portable actions through its own path
//! and never lowers these fragments.

mod post_handoff_writer;

pub use post_handoff_writer::{
    GeneratedPostHandoffWriterFragmentPlan, POST_HANDOFF_WRITER_CONTEXT_ABI_V1,
    POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH, POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET,
    PostHandoffWriterInvocationPlan, PostHandoffWriterPlan, PostHandoffWriterSource,
    PostHandoffWriterSourceSlot, PostHandoffWriterStep, derive_post_handoff_writer,
    post_handoff_writer_context_byte_len, validate_step,
};
/// Shared test fixtures for the writer-plan consumer tests.
#[cfg(feature = "test-support")]
pub mod test_support {
    use crate::post_handoff_writer::{
        PostHandoffWriterPlan, PostHandoffWriterSource, PostHandoffWriterStep,
    };
    use layout_plans::{
        ByteOrder, MaterializationWrite, PlacementConstraints, PlacementPhase, RelocationTarget,
    };

    /// The canonical single-step `"address"` writer every consumer test
    /// builds: one full 64-bit container write of one target's bits.
    pub fn single_address_writer(
        target: RelocationTarget,
        source: PostHandoffWriterSource,
    ) -> PostHandoffWriterPlan {
        PostHandoffWriterPlan {
            byte_len: 8,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "address".into(),
                    target,
                    container_byte_offset: 0,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 64,
                    stored_integer_fit: None,
                },
                source,
            }],
        }
    }
}
