//! Non-authorizing join between one checked Atomic result contract and one
//! already-specialized provider-backed resident access request.
//!
//! This carrier neither establishes a source placed value nor attempts an
//! Atomic operation. It only retains the exact checked contract beside the
//! sealed runtime request whose placement authority already owns resident
//! custody.

use access_plans::AtomicPrimitiveAccessRequest;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::typed_trees::{
    PlacedAtomicObservingResultContract, PlacedAtomicResidentContract,
};


/// One exact checked observing-result contract joined to the sealed Atomic
/// request that retains runtime resident custody.
#[derive(Debug)]
#[must_use = "checked Atomic resident access retains a non-Clone runtime request"]
pub struct CheckedAtomicResidentAccess<'checked, 'view, 'extent> {
    program: &'checked TypedTrees,
    view_symbol: SymbolHandle,
    field_symbol: SymbolHandle,
    contract: &'checked PlacedAtomicResidentContract,
    result: &'checked PlacedAtomicObservingResultContract,
    access: AtomicPrimitiveAccessRequest<'view, 'extent>,
}

impl<'checked, 'view, 'extent> CheckedAtomicResidentAccess<'checked, 'view, 'extent> {
    pub const fn atomic_access(&self) -> &AtomicPrimitiveAccessRequest<'view, 'extent> {
        &self.access
    }

    pub const fn resident_contract(&self) -> &'checked PlacedAtomicResidentContract {
        self.contract
    }

    pub const fn observing_result(&self) -> &'checked PlacedAtomicObservingResultContract {
        self.result
    }


    /// Remove only this checked-contract join. The original specialized
    /// request and all provider-backed resident custody remain intact.
    pub fn into_atomic_access(self) -> AtomicPrimitiveAccessRequest<'view, 'extent> {
        self.access
    }
}

/// Failed checked/runtime binding returns the exact non-Clone Atomic request.
#[derive(Debug)]
pub struct CheckedAtomicResidentAccessRejection<'view, 'extent> {
    access: AtomicPrimitiveAccessRequest<'view, 'extent>,
    diagnostics: Vec<Diagnostic>,
}

impl<'view, 'extent> CheckedAtomicResidentAccessRejection<'view, 'extent> {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_parts(
        self,
    ) -> (
        AtomicPrimitiveAccessRequest<'view, 'extent>,
        Vec<Diagnostic>,
    ) {
        (self.access, self.diagnostics)
    }
}

