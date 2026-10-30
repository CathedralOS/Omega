//! Invocation-local direct-call declarations, before any realization or reach.
//!
//! Positions belong to this immutable roster. Declaration identities, not the
//! diagnostic names, join entries, provider rows and source calls. This derived
//! inventory is neither a portable program nor a realizability certificate.

pub use source::DependencyScope;

pub struct DirectCallDeclarations<Declaration> {
    pub scope: DependencyScope,
    pub machines: Vec<DirectCallMachine<Declaration>>,
    machine_positions: std::collections::HashMap<Declaration, usize>,
}

pub struct DirectCallMachine<Declaration> {
    pub declaration: Declaration,
    pub name: String,
    pub in_scope: bool,
    pub states: Vec<DirectCallState<Declaration>>,
}

pub struct DirectCallState<Declaration> {
    pub declaration: Declaration,
    pub name: String,
    /// First-encounter order, deduplicated within this state only.
    pub calls: Vec<DirectCall<Declaration>>,
}

pub struct DirectCall<Declaration> {
    pub declaration: Declaration,
    /// No authored machine is a provider dependency, not an absent body.
    pub machine: Option<usize>,
}

impl<Declaration: Eq + std::hash::Hash> DirectCallDeclarations<Declaration> {
    /// Valid machine aliases precede state aliases in the producer's input.
    /// Retain that precedence and first occurrence. Repeated row/entry lookups
    /// use this invocation-local index rather than rescanning every machine.
    pub fn new(
        scope: DependencyScope,
        machines: Vec<DirectCallMachine<Declaration>>,
        machine_positions: Vec<(Declaration, usize)>,
    ) -> Self {
        let mut positions = std::collections::HashMap::with_capacity(machine_positions.len());
        for (declaration, position) in machine_positions {
            positions.entry(declaration).or_insert(position);
        }
        Self {
            scope,
            machines,
            machine_positions: positions,
        }
    }

    /// Both machine names and exact state names can be invocation targets.
    pub fn called_machine_position(&self, declaration: &Declaration) -> Option<usize> {
        self.machine_positions.get(declaration).copied()
    }
}
