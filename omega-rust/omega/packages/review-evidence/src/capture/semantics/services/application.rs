//! Exact source application, independent of native/conformance realization.
//! A reached native receipt supplies a checked requirement scope; it does not
//! invent a named implementation or grant a new calling application.
use build_time_evaluation::native_bindings::NativeRequirementApplication;
use symbols::SymbolHandle;
#[derive(Clone, Copy)]
pub(super) enum ApplicationSource<'a> {
    Provider(Option<SymbolHandle>),
    Requirement(&'a NativeRequirementApplication),
}
impl ApplicationSource<'_> {
    pub(super) fn provider(self) -> Option<SymbolHandle> {
        match self {
            Self::Provider(provider) => provider,
            Self::Requirement(_) => None,
        }
    }
}
