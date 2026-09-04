//! Relocation records and their origins, including the two semantic origins kept
//! apart because their identity namespaces collide as raw integers.

use crate::{ObjectSymbolHandle, SectionKind};
use omega_target::NativeTarget;
use psi_arena::{Arena, Handle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationPlan {
    pub target: NativeTarget,
    pub record_set: RelocationRecordSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationRecordSet {
    pub records: Arena<RelocationRecord>,
}

impl RelocationRecordSet {
    pub fn with_roots(records: Arena<RelocationRecord>) -> Self {
        Self { records }
    }

    pub fn with_capacity(record_capacity: usize) -> Self {
        Self::with_roots(Arena::with_capacity(record_capacity))
    }
}

impl Default for RelocationPlan {
    fn default() -> Self {
        Self::with_target(NativeTarget::host())
    }
}

impl RelocationPlan {
    pub fn with_target(target: NativeTarget) -> Self {
        Self::with_record_capacity(target, 0)
    }

    pub fn with_roots(target: NativeTarget, record_set: RelocationRecordSet) -> Self {
        Self { target, record_set }
    }

    pub fn with_record_capacity(target: NativeTarget, record_capacity: usize) -> Self {
        Self::with_roots(target, RelocationRecordSet::with_capacity(record_capacity))
    }

    pub fn push_record(&mut self, record: RelocationRecord) -> Handle<RelocationRecord> {
        self.record_set.records.insert(record)
    }

    pub fn record_count(&self) -> usize {
        self.record_set.records.len()
    }

    pub fn records(&self) -> impl Iterator<Item = (Handle<RelocationRecord>, &RelocationRecord)> {
        self.record_set.records.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationRecord {
    pub origin: RelocationOrigin,
    pub section: SectionKind,
    pub offset: usize,
    pub byte_width: usize,
    pub symbol_handle: ObjectSymbolHandle,
    /// Signed semantic addend applied to the resolved symbol before the
    /// target-specific relocation transform.
    pub addend: i64,
    pub kind: RelocationKind,
}

impl Default for RelocationRecord {
    fn default() -> Self {
        Self {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 0,
            },
            section: SectionKind::Text,
            offset: 0,
            byte_width: 0,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Aarch64Branch26,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationOrigin {
    Instruction {
        function_symbol_handle: ObjectSymbolHandle,
        selected_instruction_index: u32,
    },
    /// A source-independent semantic operation that directly owns the
    /// relocation. The identity is representation-defined and deliberately
    /// remains a full-width stable integer rather than being recast as a
    /// selected legacy instruction index.
    SemanticOperation {
        function_symbol_handle: ObjectSymbolHandle,
        operation_identity: u64,
    },
    /// A source-independent semantic control/ownership edge that directly
    /// owns the relocation. Edge identity is a namespace distinct from
    /// `SemanticOperation`, even when their raw integers happen to match.
    SemanticEdge {
        function_symbol_handle: ObjectSymbolHandle,
        edge_identity: u64,
    },
    Materialization {
        object_symbol_handle: ObjectSymbolHandle,
    },
}

impl RelocationOrigin {
    pub const fn symbol_handle(self) -> ObjectSymbolHandle {
        match self {
            Self::Instruction {
                function_symbol_handle,
                ..
            } => function_symbol_handle,
            Self::SemanticOperation {
                function_symbol_handle,
                ..
            } => function_symbol_handle,
            Self::SemanticEdge {
                function_symbol_handle,
                ..
            } => function_symbol_handle,
            Self::Materialization {
                object_symbol_handle,
            } => object_symbol_handle,
        }
    }

    pub const fn selected_instruction_index(self) -> Option<u32> {
        match self {
            Self::Instruction {
                selected_instruction_index,
                ..
            } => Some(selected_instruction_index),
            Self::SemanticOperation { .. }
            | Self::SemanticEdge { .. }
            | Self::Materialization { .. } => None,
        }
    }

    pub const fn semantic_operation_identity(self) -> Option<u64> {
        match self {
            Self::SemanticOperation {
                operation_identity, ..
            } => Some(operation_identity),
            Self::Instruction { .. } | Self::SemanticEdge { .. } | Self::Materialization { .. } => {
                None
            }
        }
    }

    pub const fn semantic_edge_identity(self) -> Option<u64> {
        match self {
            Self::SemanticEdge { edge_identity, .. } => Some(edge_identity),
            Self::Instruction { .. }
            | Self::SemanticOperation { .. }
            | Self::Materialization { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationKind {
    Aarch64Page21,
    Aarch64PageOffset12,
    Aarch64Branch26,
    Absolute64,
    X86_64Relative32,
}

#[cfg(test)]
mod tests {
    use crate::{RelocationOrigin, RelocationPlan, RelocationRecord, RelocationRecordSet};
    use omega_target::NativeTarget;
    use psi_arena::{Arena, Handle};

    #[test]
    fn relocation_record_set_constructor_keeps_record_root_explicit() {
        let records = Arena::<RelocationRecord>::with_capacity(3);

        let record_set = RelocationRecordSet::with_roots(records.clone());

        assert_eq!(record_set.records, records);
    }

    #[test]
    fn relocation_plan_constructor_keeps_target_and_record_roots_explicit() {
        let target = NativeTarget::linux_arm64();
        let record_set = RelocationRecordSet::with_capacity(2);

        let plan = RelocationPlan::with_roots(target, record_set.clone());

        assert_eq!(plan.target, target);
        assert_eq!(plan.record_set, record_set);
    }

    #[test]
    fn semantic_operation_and_edge_origins_keep_disjoint_identities() {
        let function_symbol_handle = Handle::invalid();
        let operation = RelocationOrigin::SemanticOperation {
            function_symbol_handle,
            operation_identity: 7,
        };
        let edge = RelocationOrigin::SemanticEdge {
            function_symbol_handle,
            edge_identity: 7,
        };

        assert_eq!(operation.symbol_handle(), function_symbol_handle);
        assert_eq!(operation.semantic_operation_identity(), Some(7));
        assert_eq!(operation.semantic_edge_identity(), None);
        assert_eq!(edge.symbol_handle(), function_symbol_handle);
        assert_eq!(edge.semantic_operation_identity(), None);
        assert_eq!(edge.semantic_edge_identity(), Some(7));
        assert_ne!(operation, edge);
    }
}
