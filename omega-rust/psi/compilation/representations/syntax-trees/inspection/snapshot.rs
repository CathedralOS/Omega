pub use expression_and_type_snapshots::{
    ExpressionSnapshot, FixedArrayLengthSnapshot, IdentifierSnapshot, MatchArmSnapshot,
    MatchPatternSnapshot, StaticArgumentSnapshot, StructLiteralFieldSnapshot,
    TypeConstraintSnapshot, TypeReferenceSnapshot,
};
pub use item_snapshots::{
    CapabilityContractKindSnapshot, CapabilityContractSnapshot, CapabilityMemberSnapshot,
    CarryPolicySnapshot, ConformanceMemberSnapshot, DataMemberSnapshot, DataPayloadFieldSnapshot,
    DataPropertiesSnapshot, ExternalBindingSnapshot, GenericConformanceBoundSnapshot, ItemSnapshot,
    OperatorSnapshot, ProofFactSnapshot, PropositionBodySnapshot,
    PropositionParameterSignatureSnapshot, QuotientEquivalenceSelectionSnapshot, QuotientSnapshot,
    SatisfiesClauseSnapshot, TypeParameterSnapshot,
};
pub use mathematical::{
    MathematicalDefinitionBodySnapshot, MathematicalDefinitionSnapshot,
    MathematicalParameterSnapshot, MathematicalTypeSnapshot,
};
pub use state_and_statement_snapshots::{
    AssemblyFactKindSnapshot, NativeCallbackParameterSnapshot, StateParameterSnapshot,
    StateSignatureSnapshot, StateSnapshot, StatementSnapshot, TransitionGuardSnapshot,
    TransitionTargetSnapshot,
};

use crate::SyntaxTrees;
use crate::snapshot::item_snapshots::snapshot_item;
use diagnostics::PhaseSnapshot;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConstantExtentProbeSnapshot {
    pub expression: ExpressionSnapshot,
    pub is_public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyntaxTreesSnapshot {
    pub source_id: usize,
    pub root_items: Vec<ItemSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mathematical_definitions: Vec<MathematicalDefinitionSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constant_extent_probes: Vec<ConstantExtentProbeSnapshot>,
}

impl SyntaxTreesSnapshot {
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

impl SyntaxTrees {
    pub fn snapshot(&self) -> SyntaxTreesSnapshot {
        SyntaxTreesSnapshot {
            mathematical_definitions: self
                .root_mathematical_definition_handles()
                .iter()
                .map(|handle| {
                    mathematical::snapshot_definition(
                        self,
                        self.root_mathematical_definition(*handle),
                    )
                })
                .collect(),
            constant_extent_probes: self
                .roots
                .constant_extent_probes
                .iter()
                .map(|(handle, public)| ConstantExtentProbeSnapshot {
                    expression: expression_and_type_snapshots::snapshot_expression_handle(
                        self, *handle,
                    ),
                    is_public: *public,
                })
                .collect(),
            source_id: self.source_id.0,
            root_items: self
                .root_item_handles()
                .iter()
                .map(|handle| snapshot_item(self, self.items.item(*handle)))
                .collect(),
        }
    }

    pub fn snapshot_json_pretty(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json_pretty()
    }

    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json()
    }
}

impl PhaseSnapshot for SyntaxTrees {
    type Snapshot = SyntaxTreesSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        SyntaxTrees::snapshot(self)
    }
}

#[path = "expressions_and_types.rs"]
pub(crate) mod expression_and_type_snapshots;

#[path = "mathematical.rs"]
mod mathematical;

#[path = "items.rs"]
pub(crate) mod item_snapshots;

#[path = "statements.rs"]
pub(crate) mod state_and_statement_snapshots;

#[cfg(test)]
#[path = "../tests/unit/snapshot_tests.rs"]
mod tests;
