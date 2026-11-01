//! Authored component-assumption acceptances retained by Build results.
//!
//! These data records preserve exact digest and declaration provenance. They
//! do not harvest source, execute Build, or accept a component themselves.

use symbols::SymbolHandle;

/// One authored `accept_component_assumption` declaration: the exact
/// assumption digest the build accepts, bound to the machine and span that
/// declared it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredComponentAssumptionAcceptance {
    pub digest: [u8; 32],
    pub selecting_machine: SymbolHandle,
    pub source_span: source::SourceSpan,
}
