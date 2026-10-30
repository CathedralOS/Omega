use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum InvocationTarget {
    /// Positional identity in the callable's non-`self` entry parameters.
    Parameter(u32),
    /// A statically selected boundary-service binding with no parameter path.
    Service(SymbolHandle),
}

/// Actual checked call edges, separate from a public permission ceiling.
/// Requirement symbols retain their complete application through preparation.
/// An abstract edge is explicit when a bodyless contract supplies no concrete
/// call; selection must not guess an application when that edge is ambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum RealizedInvocationTarget {
    Requirement(SymbolHandle),
    Abstract(InvocationTarget),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInvocationInference {
    pub machine: SymbolHandle,
    pub published: Vec<InvocationTarget>,
    pub inferred_direct: Vec<InvocationTarget>,
    pub inferred_transitive: Vec<InvocationTarget>,
    pub effective: Vec<InvocationTarget>,
    pub realized: Vec<RealizedInvocationTarget>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvocationInferencePlan {
    pub machines: Vec<MachineInvocationInference>,
}

impl InvocationInferencePlan {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineInvocationInference> {
        self.machines
            .iter()
            .find(|summary| summary.machine == machine)
    }
}
