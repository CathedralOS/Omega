//! Exact Fused routed-service establishment for a selected `ProgramEntry`
//! receiver.
//!
//! Checking records each routed `Binding<R>` erasure and settlement selects
//! the Fused provider plan it names; `root.rs` derives the entry's
//! establishments from those two results. Nothing here re-joins the erasure
//! receipts to the plans: both come from the same compile, and the
//! verification-placement rule in `omega-rust/pipeline.md` keeps a stage from
//! re-validating fresh output it was just handed.

mod root;

pub use root::{ program_entry_service_requirements};
pub(crate) use root::{derive_fused_program_entry_establishments};

use checked_trees::{CheckedTrees, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape};
use diagnostics::Diagnostic;
use provider_planning::CompositionMode;
use provider_planning::SelectedProviderReviewProvenance;
use typed_trees::data::{DataDefinition, DataField};

fn data_field_identity(field: &DataField) -> String {
    field
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| field.name.as_str().to_owned())
}
