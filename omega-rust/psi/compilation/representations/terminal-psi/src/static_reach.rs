//! A generic call's service reach is its fixed services plus the published
//! reach of the callbacks it invokes. These checking records keep an open
//! callback distinct from an empty service set; they are not executable rows.

use semantic_vocabulary::{InvocationSite, MachineId, ServiceId};

/// The parameter's originating requirement in one checked generic body.
/// Positions include every static parameter, including types and const values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StaticReachParameter {
    pub owner: MachineId,
    pub parameter: u32,
    pub requirement: MachineId,
}

/// One actual callback supplied to the exact generic invocation below.
/// `conformance` is the zero-based index of its complete refinement question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticReachArgument {
    pub parameter: u32,
    pub requirement: MachineId,
    pub selected: MachineId,
    pub arguments: Vec<StaticReachArgument>,
    pub conformance: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticReachApplication {
    pub caller: MachineId,
    pub site: InvocationSite,
    pub callee: MachineId,
    pub arguments: Vec<StaticReachArgument>,
}

/// Requirement IDs name exact callback projections, including their applied
/// headers. Both rosters are strictly increasing and union is idempotent.
pub type StaticReachTerm = crate::ContractDependency<Vec<ServiceId>, Vec<MachineId>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticReachSummary {
    pub machine: MachineId,
    pub term: StaticReachTerm,
}
