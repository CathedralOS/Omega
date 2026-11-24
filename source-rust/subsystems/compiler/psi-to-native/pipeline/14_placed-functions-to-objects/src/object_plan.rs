/*
We define the mutable arena-backed object plan (`ObjectPlan`, `ObjectFileLayout`)
and relocation plan (`RelocationPlan`, `RelocationRecord`, `RelocationOrigin`)
consumed by the image builders, along with the object output record
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
    /// The frame each compiled function establishes, for container unwind records.
    pub function_unwinds: Vec<FunctionUnwindPlan>,
}

/// One defined function symbol and the frame its prologue and epilogues establish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionUnwindPlan {
    pub symbol: ObjectSymbolHandle,
    pub frame: calling_conventions::frame_unwind::FunctionFrameUnwind,
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
            function_unwinds: Vec::new(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportRelocationError {
    Mismatch(&'static str),
    Overflow,
}

pub struct PlacedImportCall<'a> {
    pub field: &'a machine_code::PlacedNormalizedForeignCallResolution,
    pub caller_symbol: ObjectSymbolHandle,
    pub locator: target::NormalizedImportLocatorRef<'a>,
    pub callbacks: &'a [machine_code::CallbackAddressMaterialization],
}

impl ObjectPlan {
    pub fn with_import_relocations(
        mut self,
        calls: &[PlacedImportCall<'_>],
    ) -> Result<(Self, RelocationPlan), ImportRelocationError> {
        let callback_count = calls
            .iter()
            .flat_map(|call| call.callbacks)
            .map(|callback| match callback.encoding {
                machine_code::CallbackAddressEncoding::X86_64Relative32 { .. } => 1,
                machine_code::CallbackAddressEncoding::Aarch64PageAddress { .. } => 2,
            })
            .sum::<usize>();
        let mut relocations =
            RelocationPlan::with_record_capacity(self.target, calls.len() + callback_count);
        for call in calls {
            self.record_import_call(&mut relocations, call)?;
        }
        Ok((self, relocations))
    }

    /// An imported call and its callback arguments share one invocation site.
    /// Resolve their symbols and record all instruction fields together.
    fn record_import_call(
        &mut self,
        relocations: &mut RelocationPlan,
        call: &PlacedImportCall<'_>,
    ) -> Result<(), ImportRelocationError> {
        use machine_code::CallbackAddressEncoding;
        let PlacedImportCall {
            field,
            caller_symbol,
            locator,
            callbacks,
        } = *call;
        let caller_text_offset = self
            .layout
            .symbols
            .try_get(caller_symbol)
            .filter(|symbol| {
                symbol.kind == SymbolKind::Function
                    && symbol.section == SymbolSection::Section(SectionKind::Text)
            })
            .ok_or(ImportRelocationError::Mismatch(
                "fragment import caller is absent",
            ))?
            .offset;
        let origin = invocation_relocation_origin(caller_symbol, field.invocation_site);
        for callback in callbacks {
            let Some((callback_symbol, _)) =
                crate::object_function_reference(self, callback.target.callback_function)
            else {
                return Err(ImportRelocationError::Mismatch(
                    "fragment import lacks callback private function",
                ));
            };
            let mut push_callback_relocation = |relocation_offset: usize,
                                                kind: RelocationKind|
             -> Result<(), ImportRelocationError> {
                let offset = caller_text_offset
                    .checked_add(relocation_offset)
                    .ok_or(ImportRelocationError::Overflow)?;
                relocations.push_record(RelocationRecord {
                    origin,
                    section: SectionKind::Text,
                    offset,
                    byte_width: 4,
                    symbol_handle: callback_symbol,
                    addend: 0,
                    kind,
                });
                Ok(())
            };
            match callback.encoding {
                CallbackAddressEncoding::X86_64Relative32 { relocation_offset } => {
                    push_callback_relocation(relocation_offset, RelocationKind::X86_64Relative32)?
                }
                CallbackAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                } => {
                    push_callback_relocation(
                        page_relocation_offset,
                        RelocationKind::Aarch64Page21,
                    )?;
                    push_callback_relocation(
                        page_offset_relocation_offset,
                        RelocationKind::Aarch64PageOffset12,
                    )?;
                }
            }
        }
        let symbol = if let Some(import) = self
            .layout
            .normalized_imports
            .iter()
            .find(|import| import.locator.as_ref() == locator)
        {
            import.symbol
        } else {
            let name = crate::normalized_import_symbol_name(locator);
            if self
                .layout
                .symbols
                .iter()
                .any(|(_, symbol)| symbol.name == name)
            {
                return Err(ImportRelocationError::Mismatch(
                    "fragment import symbol name collision",
                ));
            }
            self.layout.insert_normalized_import(name, locator)
        };
        relocations.push_record(RelocationRecord {
            origin,
            section: SectionKind::Text,
            offset: usize::try_from(field.field_section_offset)
                .map_err(|_| ImportRelocationError::Overflow)?,
            byte_width: usize::from(field.field_byte_width),
            symbol_handle: symbol,
            addend: field.addend,
            kind: RelocationKind::from(field.kind),
        });
        Ok(())
    }

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

impl From<machine_code::NormalizedForeignCallResolutionKind> for RelocationKind {
    fn from(kind: machine_code::NormalizedForeignCallResolutionKind) -> Self {
        use machine_code::NormalizedForeignCallResolutionKind;
        match kind {
            NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => RelocationKind::X86_64Relative32,
            NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => RelocationKind::Aarch64Branch26,
        }
    }
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
        | semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
        | semantic_vocabulary::InvocationSite::Cleanup { edge, .. } => {
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
