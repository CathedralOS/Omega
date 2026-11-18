//! Completed native binding inputs and prepared application coordinates.
//! Authored provider choices and their retained source origins stay inside
//! source-compiler; this leaf carries no source files or preparation state.

#![forbid(unsafe_code)]

use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;

/// Exact prepared application coordinate. Only the source owner can join it
/// to its private arena; native consumers use the full canonical identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderApplicationId {
    index: u32,
    generation: u32,
}

impl ProviderApplicationId {
    pub const fn from_parts(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
    pub const fn arena_index(self) -> u32 {
        self.index
    }
    pub const fn generation(self) -> u32 {
        self.generation
    }
    pub const fn is_valid(self) -> bool {
        self.index != 0
    }
}

impl Default for ProviderApplicationId {
    fn default() -> Self {
        Self::from_parts(0, 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBindingSelection {
    pub application: Option<ProviderApplicationId>,
    pub subject: SymbolHandle,
    pub requirement: SymbolHandle,
    pub target: declaration_contracts::selection::ProductTarget,
    pub locator: semantic_vocabulary::NativeBindingLocator,
    pub producer: declaration_contracts::selection::NativeBindingProducer,
    pub source_occurrence_digest: [u8; 32],
    pub supplier_package: PackageKeyIdentity,
    pub source_span: source_text::SourceSpan,
}

/// One completed Build invocation and the exact binding it produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBuildSupply {
    pub selection: NativeBindingSelection,
    pub evaluation: std::sync::Arc<evaluation_receipts::NativeBuildEvaluation>,
}

impl NativeBuildSupply {
    pub fn materialization(&self) -> evaluation_receipts::NativeBindingMaterialization {
        evaluation_receipts::NativeBindingMaterialization {
            target: self.selection.target.canonical_name().to_owned(),
            locator: self.selection.locator.clone(),
            producer_identity: self.selection.producer.callable_identity.clone(),
            supplier_package: self.selection.supplier_package,
            source_occurrence_digest: self.selection.source_occurrence_digest,
            evaluation: std::sync::Arc::clone(&self.evaluation),
        }
    }
}
