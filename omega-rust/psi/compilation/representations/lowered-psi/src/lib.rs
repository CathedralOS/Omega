#![forbid(unsafe_code)]

/*
We represent `LoweredPsi`, the unsealed Psi program carrier between checked
lowering (`Psi 06`), lowered optimization (`Psi 07`), and canonical Terminal Psi
publication (`Psi 08`).
*/

/*
We keep producer-private source-to-Terminal correspondence records
(`LoweredSourceCustody`, `LoweredSourceCallOccurrence`,
`LoweredSelectedComparisonOccurrence`, and `LoweredStateBlock`) as explicit
sidecars on `LoweredPsi` so `Psi 07` and `Psi 08` can join source proof and
contract obligations before publication discards them at the `PSITERM\0` wire
boundary.
*/

/*
- Produced by: `Psi 06` (`06_checked-trees-to-lowered-psi`) and transformed in place by `Psi 07` (`07_lowered-psi-to-lowered-psi`).
- Consumed by: `Psi 08` (`08_lowered-psi-to-terminal-psi`) and `terminal-production`.
*/

pub(crate) mod source_custody {
    pub use semantic_vocabulary::InvocationSite as LoweredSourceCallSite;
    use semantic_vocabulary::{BlockId, MachineId, OperationId};
    use terminal_psi::ValueDeclaration;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LoweredStatementCallCoordinate {
        pub state: symbols::SymbolHandle,
        pub statement_index: usize,
        pub call_ordinal: usize,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum LoweredSourceCallOrigin {
        Statement(LoweredStatementCallCoordinate),
        Declaration {
            state: symbols::SymbolHandle,
            application: checked_trees::CheckedRankApplicationFormationHandle,
            call: arena::Handle<checked_trees::CheckedDeclarationCall>,
            observation_block: BlockId,
            observation: u64,
        },
    }

    impl LoweredSourceCallOrigin {
        pub fn state(self) -> symbols::SymbolHandle {
            match self {
                Self::Statement(site) => site.state,
                Self::Declaration { state, .. } => state,
            }
        }

        pub fn statement(self) -> Option<LoweredStatementCallCoordinate> {
            match self {
                Self::Statement(site) => Some(site),
                Self::Declaration { .. } => None,
            }
        }
    }

    impl From<LoweredStatementCallCoordinate> for LoweredSourceCallOrigin {
        fn from(site: LoweredStatementCallCoordinate) -> Self {
            Self::Statement(site)
        }
    }

    impl LoweredSourceCallOccurrence {
        pub fn matches_statement(
            &self,
            state: symbols::SymbolHandle,
            statement_index: usize,
            call_ordinal: usize,
        ) -> bool {
            matches!(self.origin, LoweredSourceCallOrigin::Statement(site)
            if site.state == state && site.statement_index == statement_index && site.call_ordinal == call_ordinal)
        }

        pub fn matches_checked_origin(
            &self,
            checked: &checked_trees::CheckedTrees,
            origin: checked_trees::CheckedValueOrigin,
        ) -> bool {
            let Some(origin) = checked.consistent_terminal_origin(origin) else {
                return false;
            };
            let Some(site) = self.origin.statement() else {
                return false;
            };
            matches!(origin, checked_trees::CheckedValueOrigin::StateStatement {
        state_symbol, statement_index, ..
    } if state_symbol == site.state && statement_index == site.statement_index)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LoweredSelectedComparisonOccurrence {
        pub operator_use: checked_trees::CheckedOperatorUseHandle,
        pub application_site: checked_trees::CheckedBoundaryOperatorApplicationUseSite,
        pub requirement_operator: symbols::SymbolHandle,
        pub terminal_machine: MachineId,
        pub terminal_operation: OperationId,
        pub operand_order: LoweredSelectedComparisonOperandOrder,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum LoweredSelectedComparisonOperandOrder {
        Authored,
        Swapped,
    }

    impl LoweredSelectedComparisonOperandOrder {
        pub const fn terminal_operand_position(
            self,
            authored_ordinal: usize,
            operand_count: usize,
        ) -> Option<usize> {
            match self {
                Self::Authored => {
                    if authored_ordinal < operand_count {
                        Some(authored_ordinal)
                    } else {
                        None
                    }
                }
                Self::Swapped => {
                    if operand_count == 2 && authored_ordinal < 2 {
                        Some(1 - authored_ordinal)
                    } else {
                        None
                    }
                }
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LoweredSourceScalarBinding {
        pub source_position: usize,
        pub value: ValueDeclaration,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LoweredSourceStructuralBinding {
        pub source_position: usize,
        pub argument: terminal_psi::StructuralArgument,
        pub structural_type: semantic_vocabulary::StructuralTypeId,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LoweredSourceLoanCapture {
        pub loan: arena::Handle<checked_trees::BorrowLoanFact>,
        pub formation: OperationId,
        pub carrier: semantic_vocabulary::PlaceId,
        pub source: terminal_psi::StructuralArgument,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct LoweredSourceFrontierBindings {
        pub loan_captures: Vec<LoweredSourceLoanCapture>,
        pub scalars: Vec<LoweredSourceScalarBinding>,
        pub structural: Vec<LoweredSourceStructuralBinding>,
        pub claims: Vec<(
            language_semantics::PermissionClaimIdentity,
            semantic_vocabulary::ClaimId,
        )>,
    }

    impl LoweredSourceFrontierBindings {
        pub fn visit_value_ids(&self, mut visit: impl FnMut(semantic_vocabulary::ValueId)) {
            for binding in &self.scalars {
                visit(binding.value.id);
            }
            for binding in &self.structural {
                for segment in &binding.argument.path {
                    if let terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } = segment
                    {
                        visit(*index);
                    }
                }
            }
            for capture in &self.loan_captures {
                for segment in &capture.source.path {
                    if let terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } = segment
                    {
                        visit(*index);
                    }
                }
            }
        }
    }

    /// The checked source of a call survives separately from the selected code.
    /// Runtime calls retain their evaluated operand and complete signature row.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum LoweredSourceCallTarget {
        Declaration(symbols::SymbolHandle),
        Runtime(checked_trees::typed_trees::callable_values::RuntimeCallBinding),
    }

    impl LoweredSourceCallTarget {
        pub const fn declaration(self) -> Option<symbols::SymbolHandle> {
            match self {
                Self::Declaration(symbol) => Some(symbol),
                Self::Runtime(_) => None,
            }
        }

        pub const fn runtime(
            self,
        ) -> Option<checked_trees::typed_trees::callable_values::RuntimeCallBinding> {
            match self {
                Self::Declaration(_) => None,
                Self::Runtime(binding) => Some(binding),
            }
        }
    }

    impl LoweredSourceCallTarget {
        pub fn matches_flow_call(self, call: &checked_trees::FlowCallFact) -> bool {
            match self {
                Self::Declaration(symbol) => {
                    call.runtime_call.is_none() && symbol == call.target_symbol
                }
                Self::Runtime(binding) => call.runtime_call == Some(binding),
            }
        }
    }

    impl PartialEq<symbols::SymbolHandle> for LoweredSourceCallTarget {
        fn eq(&self, symbol: &symbols::SymbolHandle) -> bool {
            self.declaration() == Some(*symbol)
        }
    }

    impl PartialEq<LoweredSourceCallTarget> for symbols::SymbolHandle {
        fn eq(&self, source: &LoweredSourceCallTarget) -> bool {
            source == self
        }
    }

    impl From<symbols::SymbolHandle> for LoweredSourceCallTarget {
        fn from(symbol: symbols::SymbolHandle) -> Self {
            Self::Declaration(symbol)
        }
    }

    impl From<checked_trees::typed_trees::callable_values::RuntimeCallBinding>
        for LoweredSourceCallTarget
    {
        fn from(binding: checked_trees::typed_trees::callable_values::RuntimeCallBinding) -> Self {
            Self::Runtime(binding)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LoweredSourceCallOccurrence {
        pub source_site: Option<checked_trees::NominalMachineUseSite>,
        pub origin: LoweredSourceCallOrigin,
        pub terminal_site: LoweredSourceCallSite,
        pub source_target: LoweredSourceCallTarget,
        pub frontier_before_call: LoweredSourceFrontierBindings,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LoweredStateBlock {
        pub machine: MachineId,
        pub state: symbols::SymbolHandle,
        pub block: BlockId,
    }
}
pub use source_custody::{
    LoweredSelectedComparisonOccurrence, LoweredSelectedComparisonOperandOrder,
    LoweredSourceCallOccurrence, LoweredSourceCallOrigin, LoweredSourceCallSite,
    LoweredSourceCallTarget, LoweredSourceFrontierBindings, LoweredSourceLoanCapture,
    LoweredSourceScalarBinding, LoweredSourceStructuralBinding, LoweredStateBlock,
    LoweredStatementCallCoordinate,
};
use terminal_psi::{TerminalDebugMap, TerminalModule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredBuildProviderRequirement {
    pub selection: arena::Handle<checked_trees::expression::BuildSelectionRequest>,
    pub source_state: symbols::SymbolHandle,
    pub declaration: terminal_psi::NominalDeclarationIdentity,
    pub application: terminal_psi::TerminalIdentityIndex,
    pub boundaries: Vec<semantic_vocabulary::BoundaryMachineId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSourceCustody {
    pub source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPsi {
    pub semantic_module: TerminalModule,
    pub source_records: Vec<checked_trees::typed_trees::SourceRecordCorrespondence>,
    pub source_boundary_declarations: Vec<(
        symbols::SymbolHandle,
        semantic_vocabulary::BoundaryMachineId,
    )>,
    pub source_machine_declarations: Vec<(symbols::SymbolHandle, semantic_vocabulary::MachineId)>,
    pub source_proof_declarations: Vec<(symbols::SymbolHandle, terminal_psi::ProofDeclarationId)>,
    pub build_provider_requirements: Vec<LoweredBuildProviderRequirement>,
    pub retained_roots: Vec<semantic_vocabulary::MachineId>,
    pub debug_map: Option<TerminalDebugMap>,
    pub source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
    pub selected_comparison_occurrences: Vec<LoweredSelectedComparisonOccurrence>,
    pub state_blocks: Vec<LoweredStateBlock>,
}

impl LoweredPsi {
    pub fn into_parts(
        self,
    ) -> (
        TerminalModule,
        Option<TerminalDebugMap>,
        LoweredSourceCustody,
    ) {
        (
            self.semantic_module,
            self.debug_map,
            LoweredSourceCustody {
                source_call_occurrences: self.source_call_occurrences,
            },
        )
    }
}
