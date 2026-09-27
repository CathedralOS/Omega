use arena::{Arena, HandleSpan, OwnedIndex};
use symbols::SymbolHandle;

/// Lazily built `symbol -> machine index` map owned by the plan it
/// describes. The slot pairs the map with the `machines` arena revision it
/// was built against; a query rebuilds when the arena moved, so no
/// caller-side freshness key is needed and a dropped plan can never serve a
/// replacement its index.
pub type OperationalMachineIndex = OwnedIndex<symbols::SymbolKeyMap<SymbolHandle, usize>>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperationalPlan {
    pub root_machines: HandleSpan<MachineOperational>,
    pub machines: Arena<MachineOperational>,
    pub states: Arena<StateOperational>,
    pub calls: Arena<CallOperational>,
    pub static_machine_bindings: Arena<StaticMachineCallBinding>,
    /// Machine-symbol lookup owned by the plan; see
    /// [`OperationalMachineIndex`].
    pub machine_index: OperationalMachineIndex,
}

/// Exact static arguments on an ordinary generic call. Nested applications
/// retain their own substitution spans; names and type/const arguments do not
/// become nominal service-row variables.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StaticMachineCallBinding {
    pub parameter: SymbolHandle,
    pub selected: SymbolHandle,
    pub arguments: HandleSpan<StaticMachineCallBinding>,
}

impl OperationalPlan {
    pub fn machines(&self) -> &[MachineOperational] {
        self.machines.span_or_empty(self.root_machines)
    }

    /// Run `run` against the plan-owned `symbol -> machine index` map,
    /// rebuilding it when the machines arena's revision moved since the
    /// stored map was built.
    pub fn with_machine_index<R>(
        &self,
        run: impl FnOnce(&symbols::SymbolKeyMap<SymbolHandle, usize>) -> R,
    ) -> R {
        self.machine_index.with(
            self.machines.revision(),
            || {
                self.machines()
                    .iter()
                    .enumerate()
                    .map(|(index, summary)| (summary.symbol, index))
                    .collect()
            },
            run,
        )
    }

    /// Position of `symbol` in `machines()`, through the plan-owned index.
    pub fn machine_index_by_symbol(&self, symbol: SymbolHandle) -> Option<usize> {
        self.with_machine_index(|index| index.get(&symbol).copied())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineOperational {
    pub symbol: SymbolHandle,
    /// Authored ceilings. For checked bodies these gate admission; for
    /// requirements and boundaries they are the pinned caller summary.
    pub published_may_suspend: bool,
    pub published_may_block: bool,
    /// Effective recursive summary consumed by callers.
    pub transitive_may_suspend: bool,
    pub transitive_may_block: bool,
    /// Declaration-free recursive body summary used to validate ceilings.
    pub body_may_suspend: bool,
    pub body_may_block: bool,
    pub states: HandleSpan<StateOperational>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateOperational {
    pub symbol: SymbolHandle,
    pub direct_may_suspend: bool,
    pub direct_may_block: bool,
    pub transitive_may_suspend: bool,
    pub transitive_may_block: bool,
    pub calls: HandleSpan<CallOperational>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallOperational {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_name: String,
    pub target_state_symbol: SymbolHandle,
    /// Original static binder; its requirement remains independent of the
    /// executable target after specialization.
    pub static_machine_parameter: SymbolHandle,
    pub static_machine_bindings: HandleSpan<StaticMachineCallBinding>,
    pub target_machine_symbol: SymbolHandle,
    /// Exact named-operator declaration selected from typed path and arity
    /// when this call is not a machine/state invocation.
    pub target_operator_symbol: SymbolHandle,
    pub direct_may_suspend: bool,
    pub direct_may_block: bool,
    pub transitive_may_suspend: bool,
    pub transitive_may_block: bool,
    pub acknowledgement: language_semantics::CallOperationalAcknowledgement,
}
