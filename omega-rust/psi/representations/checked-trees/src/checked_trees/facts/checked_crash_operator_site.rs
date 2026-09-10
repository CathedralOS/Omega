use arena::Handle;
use symbols::SymbolHandle;

use crate::{CheckedOperatorUseFact, CrashRouteBucket, FlowOperatorInvocationFact};

/// One selected operator's crash obligations at its exact captured invocation.
///
/// Operator uses have their own source occurrence identity, including individual
/// Match arms. They are not machine calls and must not borrow a call ordinal.
/// Published buckets retain the selected requirement in formal coordinates;
/// surviving buckets use proven caller-entry values or conservative Truth.
/// An empty surviving set records a proved discharge, not a missing analysis.
/// These checked-source rows are not yet portable Terminal crash evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCrashOperatorSite {
    pub operator_use: Handle<CheckedOperatorUseFact>,
    pub invocation: Handle<FlowOperatorInvocationFact>,
    pub selected_operator: SymbolHandle,
    pub published: Vec<CrashRouteBucket>,
    pub surviving: Vec<CrashRouteBucket>,
}
