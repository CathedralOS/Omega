use source_text::SourceSpan;

use symbols::SymbolHandle;

pub use symbols::root_grants::{AuthoredRootGrant, RootGrantSubject};

pub use behavior_exclusion_requests::{AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind};

pub use semantic_vocabulary::source_selections::{
    CompositionMode, NativeBindingProducer, ProductTarget,
};

pub use source_declarations::source_selections::AssumedBindingContract;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub slot: String,
    pub implementation: String,
    pub implementation_symbol: SymbolHandle,
    pub source_span: Option<SourceSpan>,
}
