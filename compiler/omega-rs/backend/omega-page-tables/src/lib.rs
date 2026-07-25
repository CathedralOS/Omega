//! Target page-table materializers over normalized Extent mapping plans.
//!
//! This crate is provider-side engineering, not a source-language escape
//! hatch. It consumes only inert projections from [`omega_extents::PageTableDraft`]
//! and returns bytes. Installation authority and translation receipts remain
//! separate.

mod aarch64;
mod x86_64;

pub use aarch64::*;
pub use x86_64::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageTableMaterializationDiagnostic(pub String);

impl std::fmt::Display for PageTableMaterializationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PageTableMaterializationDiagnostic {}
