//! The source providers a Build or default precedence selected for one compilation.

use crate::source_state::{TypedTrees, trait_definition::TraitApplicationEnvironment};
use arena::Handle;
use semantic_vocabulary::source_selections::CompositionMode;
use symbols::SymbolHandle;

#[derive(Debug, Clone)]
pub enum ProviderUse {
    Selection {
        target: String,
        composition_mode: CompositionMode,
    },
    Export,
}

#[derive(Debug, Clone)]
pub struct ProviderSelectionInput {
    pub use_as: ProviderUse,
    pub schema: TraitApplicationEnvironment,
    pub provider: SymbolHandle,
    pub provider_application:
        Option<Handle<crate::source_state::product_operand::ProductOperandApplication>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderSelections<'source> {
    pub source: &'source TypedTrees,
    pub choices: &'source [ProviderSelectionInput],
}
