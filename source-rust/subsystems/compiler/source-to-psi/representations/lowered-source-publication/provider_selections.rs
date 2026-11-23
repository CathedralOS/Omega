//! The source providers a Build or default precedence selected for one compilation.

use crate::source_state::{TypedTrees, trait_definition::TraitApplicationEnvironment};
use arena::Handle;
use semantic_vocabulary::source_selections::CompositionMode;
use symbols::SymbolHandle;

#[derive(Debug, Clone)]
pub struct ProviderSelectionInput {
    pub target: String,
    pub schema: TraitApplicationEnvironment,
    pub provider: SymbolHandle,
    pub provider_application:
        Option<Handle<crate::source_state::product_operand::ProductOperandApplication>>,
    pub composition_mode: CompositionMode,
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderSelections<'source> {
    pub source: &'source TypedTrees,
    pub choices: &'source [ProviderSelectionInput],
}
