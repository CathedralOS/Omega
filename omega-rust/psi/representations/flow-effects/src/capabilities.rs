use arena::Arena;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CapabilityFlowKind {
    #[default]
    Uses,
    Returns,
    Acquires,
    Stores,
    Derives,
}

impl CapabilityFlowKind {
    pub const ALL: [Self; 5] = [
        Self::Uses,
        Self::Returns,
        Self::Acquires,
        Self::Stores,
        Self::Derives,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uses => "uses",
            Self::Returns => "returns",
            Self::Acquires => "acquires",
            Self::Stores => "stores",
            Self::Derives => "derives",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CapabilityFlowFact {
    pub kind: CapabilityFlowKind,
    pub capability_symbol: SymbolHandle,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    /// For a verb propagated up from a nested call, the helper state the
    /// authority flowed through on its way to this state; invalid for a direct
    /// boundary call.
    pub via_state_symbol: SymbolHandle,
}

impl CapabilityFlowFact {
    /// Whether this fact was propagated up through a nested helper call rather
    /// than recorded at a direct boundary call.
    pub fn is_propagated(&self) -> bool {
        self.via_state_symbol.is_valid()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilityFlowPlan {
    pub flows: Arena<CapabilityFlowFact>,
}

impl CapabilityFlowPlan {
    pub fn with_roots(flows: Arena<CapabilityFlowFact>) -> Self {
        Self { flows }
    }

    pub fn empty() -> Self {
        Self::with_roots(Arena::default())
    }

    pub fn len(&self) -> usize {
        self.flows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flows.is_empty()
    }

    pub fn flows(&self) -> impl Iterator<Item = &CapabilityFlowFact> {
        self.flows.iter().map(|(_, flow)| flow)
    }

    pub fn count_by_kind(&self, kind: CapabilityFlowKind) -> usize {
        self.flows().filter(|flow| flow.kind == kind).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_flow_kind_names_are_stable() {
        let names = CapabilityFlowKind::ALL
            .into_iter()
            .map(CapabilityFlowKind::as_str)
            .collect::<Vec<_>>();

        assert_eq!(names, ["uses", "returns", "acquires", "stores", "derives"]);
    }

    #[test]
    fn capability_flow_plan_keeps_flow_root_explicit() {
        let mut flows = Arena::with_capacity(1);
        flows.append(CapabilityFlowFact {
            kind: CapabilityFlowKind::Acquires,
            capability_symbol: SymbolHandle::from_arena_index(1),
            machine_symbol: SymbolHandle::from_arena_index(2),
            state_symbol: SymbolHandle::from_arena_index(3),
            statement_index: 4,
            call_ordinal: 5,
            via_state_symbol: SymbolHandle::invalid(),
        });

        let plan = CapabilityFlowPlan::with_roots(flows.clone());

        assert_eq!(plan.flows, flows);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan.count_by_kind(CapabilityFlowKind::Acquires), 1);
        assert_eq!(plan.count_by_kind(CapabilityFlowKind::Uses), 0);
    }
}
