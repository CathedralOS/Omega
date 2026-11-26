/*
The core vocabulary catalog lists bundled core declarations that checking must
recognize by identity: the services, tokens, domains and private primitives
that typed rules are written against. Recognition is all a row grants. A
vocabulary row never gives compiler custody; that comes only from
`sealed_intrinsics`, and the two tables share no variant, so neither can be
read as the other.

Rows match exactly like sealed intrinsic rows: the package-relative file, the
declaration path and the exact bundled source bytes must all agree, and the
source consumer first establishes toolchain origin through
`source_symbol_table::source_symbols::core_vocabulary_at` or `SymbolTable::core_vocabulary`. A source-free
receiver instead joins the retained exact declaration owner to the catalog's
source commitment. A lookalike user declaration, or an edited toolchain file,
is therefore ordinary code to every consumer.
*/

use crate::bundled_files::BundledFile;
use crate::sealed_intrinsics::{BundledDeclaration, SealedDeclaration};
use std::path::Path;

/// A core declaration that typed rules recognize by identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoreVocabulary {
    /// The owner-defined resource projection trait in core `content.omg`.
    Content,
    /// The two proof algebras the bundled Content trait admits.
    IntervalSet,
    CountedQuantity,
    /// The proof-natural constructor family in core nat.omg.
    ProofNatural,
    /// The ordinary `Never` data declaration in core `never.omg`.
    /// This identifies a nominal ABI result; it proves no inhabitedness.
    Never,
    /// The proof-only meaning, format type and two IEEE format constants.
    FloatMeaning,
    FloatFormat,
    FloatBinary32,
    FloatBinary64,
    OpaqueRepresentation,
    PrivateCallbackSlot,
    AccessCustody,
    FixedLayoutPlan,
    /// The `MachineControl` service in core `assembly.omg`.
    MachineControlService,
    /// The `PortIo` service in core `assembly.omg`.
    PortIoService,
    InterruptControlService,
    InterruptEntryService,
    RootMemoryAccessService,
    StandardInputService,
    StandardOutputService,
    ProcessExitService,
    NetworkService,
    UnsafeService,
    UnsafeAtom(UnsafeAtom),
    NativeImageLoadService,
    /// One of core `filesystem_authority.omg`'s capability-only services.
    FilesystemService(FilesystemService),
    /// A private segment-selector primitive in core `code_segment.omg` or
    /// `data_segment.omg`.
    SegmentSelector(SegmentSelectorOperation),
    /// A private task-register primitive in core `task_register.omg`.
    TaskRegister(TaskRegisterOperation),
    /// Core `interrupt.omg`'s linear `InterruptAcknowledgement` token.
    InterruptAcknowledgement,
    /// Core `interrupt.omg`'s `InterruptAcknowledgement::Pending` domain.
    InterruptAcknowledgementPending,
    /// Core `interrupt.omg`'s `InterruptAcknowledgementCompletion` trait.
    InterruptAcknowledgementCompletion,
    /// Its `complete` requirement.
    InterruptAcknowledgementCompletionComplete,
    /// Core `static_interrupt.omg`'s `StaticInterruptCompletionPermit` token.
    StaticInterruptCompletionPermit,
    /// A reflection operation or result type in core `reflect.omg`.
    Reflection(ReflectionDeclaration),
    /// The semantic access policy requirement and its result vocabulary.
    AccessPolicy,
    AccessPolicyPlan,
    AccessPlan,
    /// Core `extent.omg`'s linear `Extent` storage data.
    Extent,
    /// One of `Extent`'s declared domains in `extent.omg`.
    ExtentDomain(ExtentDomain),
    /// The `ProgramStorageEntry` arrival trait in `extent.omg`.
    ProgramStorageEntry,
    /// Core `fixed_vec.omg`'s `FixedVec<T, N>` data.
    FixedVec,
    /// A primitive instruction-effect proposition in core `assembly_model.omg`.
    AssemblyEffect(AssemblyEffect),
    /// A native-binding vocabulary type in core `external_binding.omg`.
    ExternalBinding(ExternalBindingVocabulary),
    /// Core `integer.omg`'s carrier metadata query or its result type.
    IntegerCarrier(IntegerCarrierDeclaration),
}

/// Specific Product opt-ins admitted by the memberless core Unsafe parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum UnsafeAtom {
    RawMemoryRead,
    RawMemoryWrite,
    BorrowMutabilityUpgrade,
    ForeignCodeExecution,
}

impl UnsafeAtom {
    pub const fn name(self) -> &'static str {
        match self {
            Self::RawMemoryRead => "RawMemoryRead",
            Self::RawMemoryWrite => "RawMemoryWrite",
            Self::BorrowMutabilityUpgrade => "BorrowMutabilityUpgrade",
            Self::ForeignCodeExecution => "ForeignCodeExecution",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FilesystemService {
    ContentRead,
    ContentWrite,
    MetadataQuery,
    MetadataMutation,
    DirectoryEnumeration,
    NamespaceMutation,
}

impl FilesystemService {
    pub const ALL: [Self; 6] = [
        Self::ContentRead,
        Self::ContentWrite,
        Self::MetadataQuery,
        Self::MetadataMutation,
        Self::DirectoryEnumeration,
        Self::NamespaceMutation,
    ];
}

/// The `integer.omg` declarations the carrier-family classification is read
/// through (`wiki/spec/language/numeric_values.md`, "Integer carrier
/// families"). The four family queries are ordinary machines over `Query`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerCarrierDeclaration {
    /// `carrier<T>()`, answered from the type argument's carrier identity.
    Query,
    /// `Carrier`, the record `carrier<T>()` describes a type with.
    Carrier,
}

/// The primitive propositions `assembly_model.omg` declares for instruction effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssemblyEffect {
    PortWritten,
    ModelSpecificWritten,
    AtomicReleaseWritten,
}

/// The native-binding vocabulary types in `external_binding.omg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExternalBindingVocabulary {
    FunctionFieldBinding,
    ForeignBinding,
    DllImport,
}

/// The domains `extent.omg` declares over `Extent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExtentDomain {
    Vacant,
    Resident,
    Granted,
}

impl ExtentDomain {
    /// The domain's declared member name, as checking spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vacant => "Vacant",
            Self::Resident => "Resident",
            Self::Granted => "Granted",
        }
    }
}

/// The `reflect.omg` declarations reflection elaboration is written against.
/// They live in module `reflect`, so their package paths carry that prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReflectionDeclaration {
    /// `schema<T>()`.
    Schema,
    /// `callable(declaration)`, an uninvoked declaration selector.
    Callable,
    /// `field(declaration)`, an uninvoked field selector.
    Field,
    /// `parameter(declaration)`, an uninvoked boundary-formal selector.
    Parameter,
    /// `CallableInfo`, the descriptive result of `callable`.
    CallableInfo,
    /// `TypeSchema`, the result of `schema`.
    TypeSchema,
    /// `CaseInfo`, active-case metadata.
    CaseInfo,
    /// `FieldInfo`, field metadata.
    FieldInfo,
    ParameterInfo,
    VisitRuntimeFields,
    VisitRuntimeFieldsMut,
    InspectRecordFields,
    VisitActiveFields,
    VisitActiveFieldsMut,
}

impl CoreVocabulary {
    /// Whether this is the `extent.omg` domain declared with member `name`.
    /// Names `extent.omg` does not declare never match.
    pub fn is_extent_domain_named(self, name: &str) -> bool {
        matches!(self, Self::ExtentDomain(domain) if domain.name() == name)
    }
}

/// The selector primitives of the two segment files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SegmentSelectorOperation {
    ReloadCode,
    ReadCode,
    ReloadData,
    ReadData,
    ReadExtra,
    ReadStack,
    ReadFs,
    ReadGs,
}

/// The task-register primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskRegisterOperation {
    Activate,
    Read,
}

struct CoreVocabularyRow {
    file: &'static str,
    declaration: &'static str,
    source: BundledFile,
    vocabulary: CoreVocabulary,
}

const fn row(
    file: &'static str,
    declaration: &'static str,
    source: BundledFile,
    vocabulary: CoreVocabulary,
) -> CoreVocabularyRow {
    CoreVocabularyRow {
        file,
        declaration,
        source,
        vocabulary,
    }
}

const fn selector(
    file: &'static str,
    declaration: &'static str,
    source: BundledFile,
    operation: SegmentSelectorOperation,
) -> CoreVocabularyRow {
    row(
        file,
        declaration,
        source,
        CoreVocabulary::SegmentSelector(operation),
    )
}

const ROWS: &[CoreVocabularyRow] = &[
    row(
        "content.omg",
        "Content",
        BundledFile::CoreContent,
        CoreVocabulary::Content,
    ),
    row(
        "content.omg",
        "IntervalSet",
        BundledFile::CoreContent,
        CoreVocabulary::IntervalSet,
    ),
    row(
        "content.omg",
        "CountedQuantity",
        BundledFile::CoreContent,
        CoreVocabulary::CountedQuantity,
    ),
    row(
        "nat.omg",
        "Nat",
        BundledFile::CoreNat,
        CoreVocabulary::ProofNatural,
    ),
    row(
        "native_execution_authority.omg",
        "NativeImageLoad",
        BundledFile::CoreNativeExecutionAuthority,
        CoreVocabulary::NativeImageLoadService,
    ),
    row(
        "unsafe.omg",
        "Unsafe",
        BundledFile::CoreUnsafe,
        CoreVocabulary::UnsafeService,
    ),
    row(
        "unsafe.omg",
        UnsafeAtom::RawMemoryRead.name(),
        BundledFile::CoreUnsafe,
        CoreVocabulary::UnsafeAtom(UnsafeAtom::RawMemoryRead),
    ),
    row(
        "unsafe.omg",
        UnsafeAtom::RawMemoryWrite.name(),
        BundledFile::CoreUnsafe,
        CoreVocabulary::UnsafeAtom(UnsafeAtom::RawMemoryWrite),
    ),
    row(
        "unsafe.omg",
        UnsafeAtom::BorrowMutabilityUpgrade.name(),
        BundledFile::CoreUnsafe,
        CoreVocabulary::UnsafeAtom(UnsafeAtom::BorrowMutabilityUpgrade),
    ),
    row(
        "unsafe.omg",
        UnsafeAtom::ForeignCodeExecution.name(),
        BundledFile::CoreUnsafe,
        CoreVocabulary::UnsafeAtom(UnsafeAtom::ForeignCodeExecution),
    ),
    row(
        "network_authority.omg",
        "Network",
        BundledFile::CoreNetworkAuthority,
        CoreVocabulary::NetworkService,
    ),
    row(
        "filesystem_authority.omg",
        "FilesystemContentRead",
        BundledFile::CoreFilesystemAuthority,
        CoreVocabulary::FilesystemService(FilesystemService::ContentRead),
    ),
    row(
        "filesystem_authority.omg",
        "FilesystemContentWrite",
        BundledFile::CoreFilesystemAuthority,
        CoreVocabulary::FilesystemService(FilesystemService::ContentWrite),
    ),
    row(
        "filesystem_authority.omg",
        "FilesystemMetadataQuery",
        BundledFile::CoreFilesystemAuthority,
        CoreVocabulary::FilesystemService(FilesystemService::MetadataQuery),
    ),
    row(
        "filesystem_authority.omg",
        "FilesystemMetadataMutation",
        BundledFile::CoreFilesystemAuthority,
        CoreVocabulary::FilesystemService(FilesystemService::MetadataMutation),
    ),
    row(
        "filesystem_authority.omg",
        "DirectoryEnumeration",
        BundledFile::CoreFilesystemAuthority,
        CoreVocabulary::FilesystemService(FilesystemService::DirectoryEnumeration),
    ),
    row(
        "filesystem_authority.omg",
        "FilesystemNamespaceMutation",
        BundledFile::CoreFilesystemAuthority,
        CoreVocabulary::FilesystemService(FilesystemService::NamespaceMutation),
    ),
    row(
        "float_meaning.omg",
        "FloatMeaning",
        BundledFile::CoreFloatMeaning,
        CoreVocabulary::FloatMeaning,
    ),
    row(
        "float_format.omg",
        "FloatFormat",
        BundledFile::CoreFloatFormat,
        CoreVocabulary::FloatFormat,
    ),
    row(
        "float_format.omg",
        "FloatFormat::BINARY32",
        BundledFile::CoreFloatFormat,
        CoreVocabulary::FloatBinary32,
    ),
    row(
        "float_format.omg",
        "FloatFormat::BINARY64",
        BundledFile::CoreFloatFormat,
        CoreVocabulary::FloatBinary64,
    ),
    row(
        "never.omg",
        "Never",
        BundledFile::CoreNever,
        CoreVocabulary::Never,
    ),
    row(
        "representation.omg",
        "OpaqueRepresentation",
        BundledFile::CoreRepresentation,
        CoreVocabulary::OpaqueRepresentation,
    ),
    row(
        "layout.omg",
        "PrivateCallbackSlot",
        BundledFile::CoreLayout,
        CoreVocabulary::PrivateCallbackSlot,
    ),
    row(
        "layout.omg",
        "AccessCustody",
        BundledFile::CoreLayout,
        CoreVocabulary::AccessCustody,
    ),
    row(
        "layout.omg",
        "FixedLayout::plan",
        BundledFile::CoreLayout,
        CoreVocabulary::FixedLayoutPlan,
    ),
    row(
        "layout.omg",
        "AccessPolicy",
        BundledFile::CoreLayout,
        CoreVocabulary::AccessPolicy,
    ),
    row(
        "layout.omg",
        "AccessPolicy::plan",
        BundledFile::CoreLayout,
        CoreVocabulary::AccessPolicyPlan,
    ),
    row(
        "layout.omg",
        "AccessPlan",
        BundledFile::CoreLayout,
        CoreVocabulary::AccessPlan,
    ),
    row(
        "assembly.omg",
        "InterruptControl",
        BundledFile::CoreAssembly,
        CoreVocabulary::InterruptControlService,
    ),
    row(
        "assembly.omg",
        "RootMemoryAccess",
        BundledFile::CoreAssembly,
        CoreVocabulary::RootMemoryAccessService,
    ),
    row(
        "interrupt.omg",
        "InterruptEntry",
        BundledFile::CoreInterrupt,
        CoreVocabulary::InterruptEntryService,
    ),
    row(
        "process_authority.omg",
        "StandardInput",
        BundledFile::CoreProcessAuthority,
        CoreVocabulary::StandardInputService,
    ),
    row(
        "process_authority.omg",
        "StandardOutput",
        BundledFile::CoreProcessAuthority,
        CoreVocabulary::StandardOutputService,
    ),
    row(
        "process_exit.omg",
        "ProcessExit",
        BundledFile::CoreProcessExit,
        CoreVocabulary::ProcessExitService,
    ),
    row(
        "assembly.omg",
        "MachineControl",
        BundledFile::CoreAssembly,
        CoreVocabulary::MachineControlService,
    ),
    row(
        "assembly.omg",
        "PortIo",
        BundledFile::CoreAssembly,
        CoreVocabulary::PortIoService,
    ),
    selector(
        "code_segment.omg",
        "reload_code_segment",
        BundledFile::CoreCodeSegment,
        SegmentSelectorOperation::ReloadCode,
    ),
    selector(
        "code_segment.omg",
        "read_code_selector",
        BundledFile::CoreCodeSegment,
        SegmentSelectorOperation::ReadCode,
    ),
    selector(
        "data_segment.omg",
        "reload_data_segments",
        BundledFile::CoreDataSegment,
        SegmentSelectorOperation::ReloadData,
    ),
    selector(
        "data_segment.omg",
        "read_data_selector",
        BundledFile::CoreDataSegment,
        SegmentSelectorOperation::ReadData,
    ),
    selector(
        "data_segment.omg",
        "read_extra_selector",
        BundledFile::CoreDataSegment,
        SegmentSelectorOperation::ReadExtra,
    ),
    selector(
        "data_segment.omg",
        "read_stack_selector",
        BundledFile::CoreDataSegment,
        SegmentSelectorOperation::ReadStack,
    ),
    selector(
        "data_segment.omg",
        "read_fs_selector",
        BundledFile::CoreDataSegment,
        SegmentSelectorOperation::ReadFs,
    ),
    selector(
        "data_segment.omg",
        "read_gs_selector",
        BundledFile::CoreDataSegment,
        SegmentSelectorOperation::ReadGs,
    ),
    row(
        "task_register.omg",
        "activate_task_register",
        BundledFile::CoreTaskRegister,
        CoreVocabulary::TaskRegister(TaskRegisterOperation::Activate),
    ),
    row(
        "task_register.omg",
        "read_task_register",
        BundledFile::CoreTaskRegister,
        CoreVocabulary::TaskRegister(TaskRegisterOperation::Read),
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgement",
        BundledFile::CoreInterrupt,
        CoreVocabulary::InterruptAcknowledgement,
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgement::Pending",
        BundledFile::CoreInterrupt,
        CoreVocabulary::InterruptAcknowledgementPending,
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgementCompletion",
        BundledFile::CoreInterrupt,
        CoreVocabulary::InterruptAcknowledgementCompletion,
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgementCompletion::complete",
        BundledFile::CoreInterrupt,
        CoreVocabulary::InterruptAcknowledgementCompletionComplete,
    ),
    row(
        "static_interrupt.omg",
        "StaticInterruptCompletionPermit",
        BundledFile::CoreStaticInterrupt,
        CoreVocabulary::StaticInterruptCompletionPermit,
    ),
    row(
        "reflect/reflect.omg",
        "reflect::schema",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::Schema),
    ),
    row(
        "integer/integer.omg",
        "integer::carrier",
        BundledFile::CoreInteger,
        CoreVocabulary::IntegerCarrier(IntegerCarrierDeclaration::Query),
    ),
    row(
        "integer/integer.omg",
        "integer::Carrier",
        BundledFile::CoreInteger,
        CoreVocabulary::IntegerCarrier(IntegerCarrierDeclaration::Carrier),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::TypeSchema",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::TypeSchema),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::callable",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::Callable),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::field",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::Field),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::parameter",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::Parameter),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::ParameterInfo",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::ParameterInfo),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::CallableInfo",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::CallableInfo),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::CaseInfo",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::CaseInfo),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::FieldInfo",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::FieldInfo),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::visit_runtime_fields",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitRuntimeFields),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::visit_runtime_fields_mut",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitRuntimeFieldsMut),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::inspect_record_fields",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::InspectRecordFields),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::visit_active_fields",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitActiveFields),
    ),
    row(
        "reflect/reflect.omg",
        "reflect::visit_active_fields_mut",
        BundledFile::CoreReflect,
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitActiveFieldsMut),
    ),
    row(
        "extent.omg",
        "Extent",
        BundledFile::CoreExtent,
        CoreVocabulary::Extent,
    ),
    row(
        "extent.omg",
        "Extent::Vacant",
        BundledFile::CoreExtent,
        CoreVocabulary::ExtentDomain(ExtentDomain::Vacant),
    ),
    // The generic domain `Extent::Resident<P, T>` has the fixed carrier
    // `Extent`, so its symbol's path keeps the carrier like the others.
    row(
        "extent.omg",
        "Extent::Resident",
        BundledFile::CoreExtent,
        CoreVocabulary::ExtentDomain(ExtentDomain::Resident),
    ),
    row(
        "extent.omg",
        "Extent::Granted",
        BundledFile::CoreExtent,
        CoreVocabulary::ExtentDomain(ExtentDomain::Granted),
    ),
    row(
        "extent.omg",
        "ProgramStorageEntry",
        BundledFile::CoreExtent,
        CoreVocabulary::ProgramStorageEntry,
    ),
    row(
        "fixed_vec.omg",
        "FixedVec",
        BundledFile::CoreFixedVec,
        CoreVocabulary::FixedVec,
    ),
    row(
        "assembly_model.omg",
        "port_written",
        BundledFile::CoreAssemblyModel,
        CoreVocabulary::AssemblyEffect(AssemblyEffect::PortWritten),
    ),
    row(
        "assembly_model.omg",
        "model_specific_written",
        BundledFile::CoreAssemblyModel,
        CoreVocabulary::AssemblyEffect(AssemblyEffect::ModelSpecificWritten),
    ),
    row(
        "assembly_model.omg",
        "atomic_release_written",
        BundledFile::CoreAssemblyModel,
        CoreVocabulary::AssemblyEffect(AssemblyEffect::AtomicReleaseWritten),
    ),
    row(
        "external_binding.omg",
        "FunctionFieldBinding",
        BundledFile::CoreExternalBinding,
        CoreVocabulary::ExternalBinding(ExternalBindingVocabulary::FunctionFieldBinding),
    ),
    row(
        "external_binding.omg",
        "ForeignBinding",
        BundledFile::CoreExternalBinding,
        CoreVocabulary::ExternalBinding(ExternalBindingVocabulary::ForeignBinding),
    ),
    row(
        "external_binding.omg",
        "DllImport",
        BundledFile::CoreExternalBinding,
        CoreVocabulary::ExternalBinding(ExternalBindingVocabulary::DllImport),
    ),
];

/*
    Bundled core files whose declarations checking recognizes by their own
    hermetic identity rather than by a per-declaration row: the consumer still
    compares the exact identity, and the catalog supplies only that the symbol
    was declared in this exact bundled file. Like every row, a source matches
    only with its exact bytes behind the toolchain-origin gate.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoreSource {
    Activation,
    Content,
    Atomic,
    Relation,
    FloatOperations,
    FloatFormat,
    FloatMeaning,
    Layout,
    Run,
    CodeLayout,
    Foundation,
}

const CORE_SOURCES: &[(CoreSource, &str, BundledFile)] = &[
    (
        CoreSource::Activation,
        "activation.omg",
        BundledFile::CoreActivation,
    ),
    (CoreSource::Atomic, "Atomic/atomic.omg", BundledFile::CoreAtomic),
    (CoreSource::Content, "content.omg", BundledFile::CoreContent),
    (
        CoreSource::Relation,
        "relation.omg",
        BundledFile::CoreRelation,
    ),
    (
        CoreSource::FloatOperations,
        "float_operations.omg",
        BundledFile::CoreFloatOperations,
    ),
    (
        CoreSource::FloatFormat,
        "float_format.omg",
        BundledFile::CoreFloatFormat,
    ),
    (
        CoreSource::FloatMeaning,
        "float_meaning.omg",
        BundledFile::CoreFloatMeaning,
    ),
    (
        CoreSource::Foundation,
        "foundation.omg",
        BundledFile::CoreFoundation,
    ),
    (CoreSource::Layout, "layout.omg", BundledFile::CoreLayout),
    (CoreSource::Run, "run.omg", BundledFile::CoreRun),
    (
        CoreSource::CodeLayout,
        "code_layout.omg",
        BundledFile::CoreCodeLayout,
    ),
];

impl CoreSource {
    /// The bundled file's path relative to the core package root.
    pub fn file(self) -> &'static str {
        CORE_SOURCES
            .iter()
            .find(|(source, _, _)| *source == self)
            .map(|(_, file, _)| *file)
            .expect("every core source has a row")
    }

    /// The core source bundled at `file`, if any.
    pub fn from_file(file: &str) -> Option<Self> {
        CORE_SOURCES
            .iter()
            .find(|(_, candidate, _)| *candidate == file)
            .map(|(source, _, _)| *source)
    }
}

pub struct CoreVocabularyCatalog;

impl CoreVocabularyCatalog {
    /// The receiver's bundled declaration, without reopening source. These
    /// coordinates identify a catalog row; they grant no execution authority.
    pub fn declaration(vocabulary: CoreVocabulary) -> Option<BundledDeclaration> {
        let row = ROWS.iter().find(|row| row.vocabulary == vocabulary)?;
        Some(BundledDeclaration {
            file: Path::new(row.file),
            declaration: row.declaration,
            source: row.source,
        })
    }

    /// Returns the vocabulary the site's declaration is, or `None` when no row
    /// matches every part of the site.
    pub fn match_declaration(
        file: &Path,
        declaration: &str,
        matches_source: impl Fn(BundledFile) -> bool,
    ) -> Option<CoreVocabulary> {
        ROWS.iter()
            .find(|row| {
                file == Path::new(row.file)
                    && SealedDeclaration::Exact(row.declaration).matches(declaration)
                    && matches_source(row.source)
            })
            .map(|row| row.vocabulary)
    }

    /// The core source the site's file is, matching path and exact bytes.
    pub fn match_source(
        file: &Path,
        matches_source: impl Fn(BundledFile) -> bool,
    ) -> Option<CoreSource> {
        CORE_SOURCES
            .iter()
            .find(|(_, candidate, bundled_file)| {
                file == Path::new(candidate) && matches_source(*bundled_file)
            })
            .map(|(core, _, _)| *core)
    }
}
