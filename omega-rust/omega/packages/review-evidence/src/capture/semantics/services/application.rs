//! Exact source application, independent of native/conformance realization.
//! A reached native receipt supplies a checked requirement scope; it does not
//! invent a named implementation or grant a new calling application.
use provider_planning::ResolvedNativeApplication;
use symbols::SymbolHandle;
#[derive(Clone, Copy)]
pub(super) enum ApplicationSource<'a> {
    Provider(Option<SymbolHandle>),
    Requirement(&'a ResolvedNativeApplication),
}
impl ApplicationSource<'_> {
    pub(super) fn provider(self) -> Option<SymbolHandle> {
        match self {
            Self::Provider(provider) => provider,
            Self::Requirement(_) => None,
        }
    }
}
