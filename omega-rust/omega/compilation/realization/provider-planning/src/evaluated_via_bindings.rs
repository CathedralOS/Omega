// Retained locator receipts and source diagnostic coordinates. Source `via`
// resolution/evaluation has no product route, so no source producer lives here.

use source::SourceSpan;
use symbols::SymbolHandle;
use target::TargetProfile;

pub use effects::provider_plan::EvaluatedNativeBinding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedViaBindingRow {
    realization_machine: SymbolHandle,
    satisfied_owner: SymbolHandle,
    requirement: SymbolHandle,
    producer_machine: SymbolHandle,
    producer_entry_state: SymbolHandle,
    via_source_span: SourceSpan,
    evaluated: EvaluatedNativeBinding,
}

impl EvaluatedViaBindingRow {
    pub const fn realization_machine(&self) -> SymbolHandle {
        self.realization_machine
    }
    pub const fn satisfied_owner(&self) -> SymbolHandle {
        self.satisfied_owner
    }
    pub const fn requirement(&self) -> SymbolHandle {
        self.requirement
    }
    pub const fn producer_machine(&self) -> SymbolHandle {
        self.producer_machine
    }
    pub const fn producer_entry_state(&self) -> SymbolHandle {
        self.producer_entry_state
    }
    pub const fn via_source_span(&self) -> SourceSpan {
        self.via_source_span
    }
    pub const fn evaluated(&self) -> &EvaluatedNativeBinding {
        &self.evaluated
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedViaBindingTable {
    scope: source::DependencyScope,
    target: Option<TargetProfile>,
    identities: terminal_psi::TerminalIdentityTable, // Resolves receipt-carried `TerminalIdentityIndex` values.
    rows: Vec<EvaluatedViaBindingRow>,
}

impl EvaluatedViaBindingTable {
    pub const fn scope(&self) -> source::DependencyScope {
        self.scope
    }
    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }
    pub const fn target(&self) -> Option<TargetProfile> {
        self.target
    }
    pub fn rows(&self) -> &[EvaluatedViaBindingRow] {
        &self.rows
    }

    /// The table of a program that authors no ordinary `via` leaf.
    pub fn without_bindings(scope: source::DependencyScope, target: Option<TargetProfile>) -> Self {
        Self {
            scope,
            target,
            identities: terminal_psi::TerminalIdentityTable::from(vec![String::new()]),
            rows: Vec::new(),
        }
    }

    pub fn exact(
        &self,
        realization_machine: SymbolHandle,
        satisfied_owner: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Option<&EvaluatedViaBindingRow> {
        self.rows.iter().find(|row| {
            row.realization_machine == realization_machine
                && row.satisfied_owner == satisfied_owner
                && row.requirement == requirement
        })
    }
}
