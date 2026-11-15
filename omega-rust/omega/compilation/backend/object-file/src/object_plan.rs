/*
We define the mutable arena-backed object plan (`ObjectPlan`, `ObjectFileLayout`)
and relocation plan (`RelocationPlan`, `RelocationRecord`, `RelocationOrigin`)
consumed by the image builders, along with the `OMGOBJ` container output shape
(`ObjectContainerOutput`), section vocabulary (`SectionPlan`, `SectionKind`),
and symbol/import side tables (`SymbolPlan`, `NormalizedImportPlan`,
`FunctionSymbolPlan`). We keep `RelocationOrigin::SemanticOperation` and
`RelocationOrigin::SemanticEdge` as separate variants so raw integer collisions
between operation and edge identity namespaces never conflate relocation origins.
*/

use arena::{Arena, Handle};
use function_identity::MachineFunctionIdentity;
use target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectFileLayout {
    pub sections: Arena<SectionPlan>,
    pub symbols: Arena<SymbolPlan>,
    pub function_symbols: Arena<FunctionSymbolPlan>,
    pub normalized_imports: Vec<NormalizedImportPlan>,
    pub entry_symbol: ObjectSymbolHandle,
}

impl ObjectFileLayout {
    pub fn with_roots(
        sections: Arena<SectionPlan>,
        symbols: Arena<SymbolPlan>,
        function_symbols: Arena<FunctionSymbolPlan>,
        normalized_imports: Vec<NormalizedImportPlan>,
        entry_symbol: ObjectSymbolHandle,
    ) -> Self {
        Self {
            sections,
            symbols,
            function_symbols,
            normalized_imports,
            entry_symbol,
        }
    }

    pub fn insert_import(
        &mut self,
        name: String,
        locator: &target::NormalizedForeignLocator,
    ) -> ObjectSymbolHandle {
        self.insert_normalized_import(name, target::NormalizedImportLocatorRef::Foreign(locator))
    }

    pub fn insert_normalized_import(
        &mut self,
        name: String,
        locator: target::NormalizedImportLocatorRef<'_>,
    ) -> ObjectSymbolHandle {
        let symbol = self.symbols.insert(SymbolPlan {
            name,
            section: SymbolSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
            import_library: String::new(),
        });
        self.normalized_imports.push(NormalizedImportPlan {
            symbol,
            locator: locator.to_owned(),
        });
        symbol
    }

    pub fn with_capacity(section_capacity: usize, symbol_capacity: usize) -> Self {
        Self::with_roots(
            Arena::with_capacity(section_capacity),
            Arena::with_capacity(symbol_capacity),
            Arena::new(),
            Vec::new(),
            ObjectSymbolHandle::invalid(),
        )
    }

    pub fn with_capacities(
        section_capacity: usize,
        symbol_capacity: usize,
        function_symbol_capacity: usize,
    ) -> Self {
        Self::with_roots(
            Arena::with_capacity(section_capacity),
            Arena::with_capacity(symbol_capacity),
            Arena::with_capacity(function_symbol_capacity),
            Vec::new(),
            ObjectSymbolHandle::invalid(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectPlan {
    pub target: NativeTarget,
    pub layout: ObjectFileLayout,
}

impl ObjectPlan {
    pub(crate) fn with_layout(target: NativeTarget, layout: ObjectFileLayout) -> Self {
        Self { target, layout }
    }

    pub fn with_capacity(
        target: NativeTarget,
        section_capacity: usize,
        symbol_capacity: usize,
    ) -> Self {
        Self::with_layout(
            target,
            ObjectFileLayout::with_capacity(section_capacity, symbol_capacity),
        )
    }

    pub fn with_capacities(
        target: NativeTarget,
        section_capacity: usize,
        symbol_capacity: usize,
        function_symbol_capacity: usize,
    ) -> Self {
        Self::with_layout(
            target,
            ObjectFileLayout::with_capacities(
                section_capacity,
                symbol_capacity,
                function_symbol_capacity,
            ),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectContainerOutput {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub relocations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionPlan {
    pub kind: SectionKind,
    pub size: usize,
    pub alignment: usize,
}

impl Default for SectionPlan {
    fn default() -> Self {
        Self {
            kind: SectionKind::Text,
            size: 0,
            alignment: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Text,
    Data,
    Bss,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolPlan {
    pub name: String,
    pub section: SymbolSection,
    pub offset: usize,
    pub size: usize,
    pub kind: SymbolKind,
    pub import_library: String,
}

pub type ObjectSymbolHandle = Handle<SymbolPlan>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedImportPlan {
    pub symbol: ObjectSymbolHandle,
    pub locator: target::NormalizedImportLocator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSymbolPlan {
    pub identity: MachineFunctionIdentity,
    pub symbol: ObjectSymbolHandle,
}

impl Default for FunctionSymbolPlan {
    fn default() -> Self {
        Self {
            identity: MachineFunctionIdentity::default(),
            symbol: ObjectSymbolHandle::invalid(),
        }
    }
}

impl Default for SymbolPlan {
    fn default() -> Self {
        Self {
            name: String::new(),
            section: SymbolSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Object,
            import_library: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SymbolSection {
    #[default]
    None,
    Section(SectionKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Import,
    Object,
}

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
    SemanticOperation {
        function_symbol_handle: ObjectSymbolHandle,
        operation_identity: u64,
    },
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

pub fn invocation_relocation_origin(
    symbol: ObjectSymbolHandle,
    site: semantic_vocabulary::InvocationSite,
) -> RelocationOrigin {
    match site {
        semantic_vocabulary::InvocationSite::Operation(operation) => {
            RelocationOrigin::SemanticOperation {
                function_symbol_handle: symbol,
                operation_identity: operation.get(),
            }
        }
        semantic_vocabulary::InvocationSite::InvokeNoNormal(edge)
        | semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
            RelocationOrigin::SemanticEdge {
                function_symbol_handle: symbol,
                edge_identity: edge.get(),
            }
        }
    }
}

pub const fn relocation_kind_tag(kind: RelocationKind) -> u8 {
    match kind {
        RelocationKind::Aarch64Page21 => 1,
        RelocationKind::Aarch64PageOffset12 => 2,
        RelocationKind::Aarch64Branch26 => 3,
        RelocationKind::Absolute64 => 4,
        RelocationKind::X86_64Relative32 => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::RelocationOrigin;
    use arena::Handle;

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
