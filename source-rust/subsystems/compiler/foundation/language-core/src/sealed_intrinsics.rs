/*
The sealed intrinsic catalog is the one list of bundled declarations that may
receive compiler custody: the compiler, not an authored body or foreign symbol,
supplies their behaviour. Every stage that asks "is this declaration compiler
owned?" reads this table instead of keeping its own file-name or member-name
recognizer, so adding or removing compiler custody is one row here.

Each row identifies a bundled file and a declaration inside it. During source
compilation, source_symbols checks toolchain origin and bundled-declarations
compares every byte. A saved-program consumer already has the exact owner
revision and declaration in Psi. It compares that revision with its installed
catalog before realizing the operation.

Matching a row grants identity only. Each consumer still authenticates the
declaration's signature, supply mode and contracts against the operation it
realizes before it uses the row.
*/

use crate::bundled_files::BundledFile;
use crate::byte_predicates::ByteSequencePredicate;
use crate::static_interrupts::StaticInterruptOperation;
use std::path::Path;

/// The compiler-supplied operation a catalog row identifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SealedIntrinsic {
    /// Core's structural equality synthesis protocol; not an executable primitive.
    EqualityProtocol,
    /// A bodyless machine supplied by a closed portable compiler catalog.
    CompilerPrimitive(CompilerPrimitive),
    /// A member of core's `NativeFloats` realization of `FloatNativeOperations`.
    NativeFloatOperation,
    /// Core's `NativeFloats` map itself.
    NativeFloatMap,
    /// The private `FloatNativeOperations` trait `NativeFloats` realizes.
    NativeFloatOperationsTrait,
    /// A public `Float`, `F32`, `F64` or integer-from-float conversion
    /// operation in core `float_operations.omg`.
    FloatOperationRequirement,
    /// Core `layout.omg`'s opaque `LayoutTarget` data.
    LayoutTargetData,
    /// Core `layout.omg`'s opaque `LayoutStorage` data.
    LayoutStorageData,
    /// Core `layout.omg`'s `layout_target_facts` snapshot query.
    LayoutTargetFacts,
    /// Core's ordinary identity body also records an exact private-slot placement.
    PrivateLayoutPlacement,
    /// Core `layout.omg`'s free placed `read` or `write` primitive.
    PlacedPrimitive(PlacedPrimitiveOperation),
    /// A sealed placement-lifecycle operation over an `Access` or `Run` profile.
    PlacementLifecycle {
        operation: PlacementOperation,
        content: PlacementContent,
    },
    /// One of the four static-interrupt operations in core `static_interrupt.omg`.
    StaticInterrupt(StaticInterruptOperation),
    /// `TaskRuntime::start` in core `task.omg`.
    TaskRuntimeStart,
    /// `TaskRuntime::try_start` in core `task.omg`.
    TaskRuntimeTryStart,
    /// Core `layout.omg`'s existing restricted scalar accessor template.
    PlacedAccessorTemplate,
    /// The source requirement a compiler-owned hosted primitive must satisfy.
    HostedRequirement(HostedOperation),
    /// An exact compiler-owned primitive, separate from ordinary provider bodies.
    HostedPrimitive(HostedOperation),
    /// A UEFI fixed-image requirement in bundled core source.
    FixedImageRequirement(FixedImageOperation),
    /// The bundled `uefi_x86_64` realization of a fixed-image requirement.
    FixedImageRealization(FixedImageOperation),
}

/// Catalog identity only; the complete machine declaration owns its signature
/// and contract, and each execution consumer still checks the supplied operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerPrimitive {
    PackedInteger(PackedIntegerPrimitive),
    AtomicFence,
    /// An exact pure observation of one borrowed byte sequence.
    ByteObservation(ByteSequencePredicate),
    FloatProjection,
    FloatSemantics,
    NaturalRanking,
    Collection(CollectionPrimitive),
}

/// Source declarations that accept shared fixed arrays and produce an owned array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackedIntegerPrimitive {
    WrappingAddI16x8,
    WrappingSubI16x8,
    ShuffleI16x8,
    ShiftRightI16x8,
    WidenU8x8ToI16x8,
    NarrowI16x8ToU8x8Saturating,
    LoadI16x4ZeroExtend,
    WidenU8x4ToI16x8,
    NarrowI16x8ToU8x4Saturating,
    WrappingAddU8x16,
    MinI16x8,
    CmpGtI16x8,
    BitAndI16x8,
    BitAndNotI16x8,
    AbsI16x8,
}

impl PackedIntegerPrimitive {
    pub fn inputs(self) -> usize {
        match self {
            Self::WrappingAddI16x8
            | Self::WrappingSubI16x8
            | Self::ShuffleI16x8
            | Self::WrappingAddU8x16
            | Self::MinI16x8
            | Self::CmpGtI16x8
            | Self::BitAndI16x8
            | Self::BitAndNotI16x8 => 2,
            Self::ShiftRightI16x8
            | Self::WidenU8x8ToI16x8
            | Self::NarrowI16x8ToU8x8Saturating
            | Self::LoadI16x4ZeroExtend
            | Self::WidenU8x4ToI16x8
            | Self::NarrowI16x8ToU8x4Saturating
            | Self::AbsI16x8 => 1,
        }
    }

    pub fn immediates(self) -> usize {
        match self {
            Self::ShuffleI16x8 => 8,
            Self::ShiftRightI16x8 => 1,
            Self::WrappingAddI16x8
            | Self::WrappingSubI16x8
            | Self::WidenU8x8ToI16x8
            | Self::NarrowI16x8ToU8x8Saturating
            | Self::LoadI16x4ZeroExtend
            | Self::WidenU8x4ToI16x8
            | Self::NarrowI16x8ToU8x4Saturating
            | Self::WrappingAddU8x16
            | Self::MinI16x8
            | Self::CmpGtI16x8
            | Self::BitAndI16x8
            | Self::BitAndNotI16x8
            | Self::AbsI16x8 => 0,
        }
    }
}

/// The operation supplied by one exact generic core collection declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CollectionPrimitive {
    Read,
    Range,
    ArrayView,
}

/// The free placed scalar primitives in core `layout.omg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlacedPrimitiveOperation {
    Read,
    Write,
}

/// What a placement-lifecycle operation does to its profile's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlacementOperation {
    Initialize,
    MoveOut,
    View,
    Relocate,
}

/// Whether the profile holds one resident (`Access`) or a run (`Run`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlacementContent {
    Resident,
    Run,
}

/// The hosted operations the compiler realizes for every hosted target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostedOperation {
    /// system `Console::write_byte`.
    ConsoleWriteByte,
    /// system `Console::read_byte`.
    ConsoleReadByte,
    /// core `ProcessExit::exit_process`.
    ProcessExit,
}

/// The fixed-image operations the compiler realizes for `uefi_x86_64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FixedImageOperation {
    /// core `ProgramStorageEntry::enter`.
    ProgramStorageEntry,
    /// core `ProgramImage::retained_suffix_start`.
    ProgramImageRetainedSuffixStart,
}

impl FixedImageOperation {
    /// The target with the physical fixed-image entry and residency contract.
    pub const TARGET: &'static str = "uefi_x86_64";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SealedDeclaration {
    /// Exactly this package-relative declaration path.
    Exact(&'static str),
    /// Any single-segment member directly under this owner path.
    MemberOf(&'static str),
}

impl SealedDeclaration {
    pub(crate) fn matches(self, declaration: &str) -> bool {
        match self {
            Self::Exact(path) => declaration == path,
            Self::MemberOf(owner) => declaration
                .strip_prefix(owner)
                .and_then(|rest| rest.strip_prefix("::"))
                .is_some_and(|member| !member.is_empty() && !member.contains("::")),
        }
    }
}

struct SealedIntrinsicRow {
    file: &'static str,
    declaration: SealedDeclaration,
    source: BundledFile,
    intrinsic: SealedIntrinsic,
}

const fn row(
    file: &'static str,
    declaration: &'static str,
    source: BundledFile,
    intrinsic: SealedIntrinsic,
) -> SealedIntrinsicRow {
    SealedIntrinsicRow {
        file,
        declaration: SealedDeclaration::Exact(declaration),
        source,
        intrinsic,
    }
}

const fn member_row(
    file: &'static str,
    owner: &'static str,
    source: BundledFile,
    intrinsic: SealedIntrinsic,
) -> SealedIntrinsicRow {
    SealedIntrinsicRow {
        file,
        declaration: SealedDeclaration::MemberOf(owner),
        source,
        intrinsic,
    }
}

const fn lifecycle(operation: PlacementOperation, content: PlacementContent) -> SealedIntrinsic {
    SealedIntrinsic::PlacementLifecycle { operation, content }
}

const ROWS: &[SealedIntrinsicRow] = &[
    row(
        "equality.omg",
        "Equatable",
        BundledFile::CoreEquality,
        SealedIntrinsic::EqualityProtocol,
    ),
    row(
        "indexing/indexing.omg",
        "read",
        BundledFile::CoreIndexing,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::Collection(
            CollectionPrimitive::Read,
        )),
    ),
    row(
        "indexing/indexing.omg",
        "range",
        BundledFile::CoreIndexing,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::Collection(
            CollectionPrimitive::Range,
        )),
    ),
    row(
        "indexing/indexing.omg",
        "view",
        BundledFile::CoreIndexing,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::Collection(
            CollectionPrimitive::ArrayView,
        )),
    ),
    row(
        "Atomic/atomic.omg",
        "fence",
        BundledFile::CoreAtomic,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::AtomicFence),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "wrapping_add_i16x8",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::WrappingAddI16x8,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "wrapping_sub_i16x8",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::WrappingSubI16x8,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "shuffle_i16x8",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::ShuffleI16x8,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "shift_right_i16x8",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::ShiftRightI16x8,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "widen_u8x8_to_i16x8",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::WidenU8x8ToI16x8,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "narrow_i16x8_to_u8x8_saturating",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::NarrowI16x8ToU8x8Saturating,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "load_i16x4_zero_extend",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::LoadI16x4ZeroExtend,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "widen_u8x4_to_i16x8",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::WidenU8x4ToI16x8,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "narrow_i16x8_to_u8x4_saturating",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::NarrowI16x8ToU8x4Saturating,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "wrapping_add_u8x16",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::WrappingAddU8x16,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "min_i16x8",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::MinI16x8,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "cmpgt_i16x8",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::CmpGtI16x8,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "bit_and_i16x8",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::BitAndI16x8,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "bit_andnot_i16x8",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::BitAndNotI16x8,
        )),
    ),
    row(
        "packed_integer/packed_integer.omg",
        "abs_i16x8",
        BundledFile::CorePackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::AbsI16x8,
        )),
    ),
    row(
        "byte_observation/byte_observation.omg",
        "valid_utf8",
        BundledFile::CoreByteObservation,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::ByteObservation(
            ByteSequencePredicate::ValidUtf8,
        )),
    ),
    row(
        "byte_observation/byte_observation.omg",
        "no_nul",
        BundledFile::CoreByteObservation,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::ByteObservation(
            ByteSequencePredicate::NoNul,
        )),
    ),
    row(
        "byte_observation/byte_observation.omg",
        "ascii_only",
        BundledFile::CoreByteObservation,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::ByteObservation(
            ByteSequencePredicate::AsciiOnly,
        )),
    ),
    row(
        "byte_observation/byte_observation.omg",
        "non_empty",
        BundledFile::CoreByteObservation,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::ByteObservation(
            ByteSequencePredicate::NonEmpty,
        )),
    ),
    row(
        "float_operations.omg",
        "Float::meaning32",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::FloatProjection),
    ),
    row(
        "float_operations.omg",
        "Float::meaning64",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::FloatProjection),
    ),
    member_row(
        "float_operations.omg",
        "FloatSemantics",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::FloatSemantics),
    ),
    row(
        "nat.omg",
        "Nat::Descending",
        BundledFile::CoreNat,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::NaturalRanking),
    ),
    row(
        "nat.omg",
        "Nat::BoundedDistance",
        BundledFile::CoreNat,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::NaturalRanking),
    ),
    row(
        "nat.omg",
        "Nat::IncreasingTo",
        BundledFile::CoreNat,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::NaturalRanking),
    ),
    row(
        "float_provider.omg",
        "NativeFloats",
        BundledFile::CoreFloatProvider,
        SealedIntrinsic::NativeFloatMap,
    ),
    row(
        "float_provider.omg",
        "FloatNativeOperations",
        BundledFile::CoreFloatProvider,
        SealedIntrinsic::NativeFloatOperationsTrait,
    ),
    member_row(
        "float_operations.omg",
        "Float",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "F32",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "F64",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "I8",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "I16",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "I32",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "I64",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "U8",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "U16",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "U32",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "U64",
        BundledFile::CoreFloatOperations,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    row(
        "layout.omg",
        "LayoutTarget",
        BundledFile::CoreLayout,
        SealedIntrinsic::LayoutTargetData,
    ),
    row(
        "layout.omg",
        "LayoutStorage",
        BundledFile::CoreLayout,
        SealedIntrinsic::LayoutStorageData,
    ),
    row(
        "layout.omg",
        "layout_target_facts",
        BundledFile::CoreLayout,
        SealedIntrinsic::LayoutTargetFacts,
    ),
    row(
        "layout.omg",
        "layout_plan_place_private",
        BundledFile::CoreLayout,
        SealedIntrinsic::PrivateLayoutPlacement,
    ),
    row(
        "layout.omg",
        "layout_plan_place_private_field",
        BundledFile::CoreLayout,
        SealedIntrinsic::PrivateLayoutPlacement,
    ),
    row(
        "layout.omg",
        "read",
        BundledFile::CoreLayout,
        SealedIntrinsic::PlacedPrimitive(PlacedPrimitiveOperation::Read),
    ),
    row(
        "layout.omg",
        "write",
        BundledFile::CoreLayout,
        SealedIntrinsic::PlacedPrimitive(PlacedPrimitiveOperation::Write),
    ),
    row(
        "access/access.omg",
        "access::view",
        BundledFile::CoreAccess,
        lifecycle(PlacementOperation::View, PlacementContent::Resident),
    ),
    row(
        "access/access.omg",
        "access::view_mut",
        BundledFile::CoreAccess,
        lifecycle(PlacementOperation::View, PlacementContent::Resident),
    ),
    row(
        "access/access.omg",
        "access::view_resident_mut",
        BundledFile::CoreAccess,
        lifecycle(PlacementOperation::View, PlacementContent::Resident),
    ),
    row(
        "access/access.omg",
        "access::initialize_from_profile",
        BundledFile::CoreAccess,
        lifecycle(PlacementOperation::Initialize, PlacementContent::Resident),
    ),
    row(
        "access/access.omg",
        "access::extract_resident",
        BundledFile::CoreAccess,
        lifecycle(PlacementOperation::MoveOut, PlacementContent::Resident),
    ),
    row(
        "access/access.omg",
        "access::view_from_profile",
        BundledFile::CoreAccess,
        lifecycle(PlacementOperation::View, PlacementContent::Resident),
    ),
    row(
        "access/access.omg",
        "access::view_mut_from_profile",
        BundledFile::CoreAccess,
        lifecycle(PlacementOperation::View, PlacementContent::Resident),
    ),
    row(
        "run.omg",
        "Run::initialize_from_profile",
        BundledFile::CoreRun,
        lifecycle(PlacementOperation::Initialize, PlacementContent::Run),
    ),
    row(
        "run.omg",
        "Run::extract_from_profile",
        BundledFile::CoreRun,
        lifecycle(PlacementOperation::MoveOut, PlacementContent::Run),
    ),
    row(
        "run.omg",
        "Run::relocate_from_profiles",
        BundledFile::CoreRun,
        lifecycle(PlacementOperation::Relocate, PlacementContent::Run),
    ),
    row(
        "layout.omg",
        "Accessor",
        BundledFile::CoreLayout,
        SealedIntrinsic::PlacedAccessorTemplate,
    ),
    row(
        "process_exit_native.omg",
        "ProcessExitPrimitive",
        BundledFile::CoreProcessExitNative,
        SealedIntrinsic::HostedPrimitive(HostedOperation::ProcessExit),
    ),
    row(
        "process_exit.omg",
        "ProcessExit::exit_process",
        BundledFile::CoreProcessExit,
        SealedIntrinsic::HostedRequirement(HostedOperation::ProcessExit),
    ),
    row(
        "extent.omg",
        "ProgramStorageEntry::enter",
        BundledFile::CoreExtent,
        SealedIntrinsic::FixedImageRequirement(FixedImageOperation::ProgramStorageEntry),
    ),
    row(
        "extent.omg",
        "ProgramStorageEntryNativeProvider::ProgramStorageEntry::enter",
        BundledFile::CoreExtent,
        SealedIntrinsic::FixedImageRealization(FixedImageOperation::ProgramStorageEntry),
    ),
    row(
        "program_image.omg",
        "ProgramImage::retained_suffix_start",
        BundledFile::CoreProgramImage,
        SealedIntrinsic::FixedImageRequirement(
            FixedImageOperation::ProgramImageRetainedSuffixStart,
        ),
    ),
    row(
        "program_image.omg",
        "ProgramImageNativeProvider::ProgramImage::retained_suffix_start",
        BundledFile::CoreProgramImage,
        SealedIntrinsic::FixedImageRealization(
            FixedImageOperation::ProgramImageRetainedSuffixStart,
        ),
    ),
    SealedIntrinsicRow {
        file: "float_provider.omg",
        declaration: SealedDeclaration::MemberOf("NativeFloats"),
        source: BundledFile::CoreFloatProvider,
        intrinsic: SealedIntrinsic::NativeFloatOperation,
    },
    SealedIntrinsicRow {
        file: "static_interrupt.omg",
        declaration: SealedDeclaration::Exact("X86StaticInterruptScope::run"),
        source: BundledFile::CoreStaticInterrupt,
        intrinsic: SealedIntrinsic::StaticInterrupt(StaticInterruptOperation::Install),
    },
    SealedIntrinsicRow {
        file: "static_interrupt.omg",
        declaration: SealedDeclaration::Exact("view_static_interrupt_context"),
        source: BundledFile::CoreStaticInterrupt,
        intrinsic: SealedIntrinsic::StaticInterrupt(StaticInterruptOperation::Context),
    },
    SealedIntrinsicRow {
        file: "static_interrupt.omg",
        declaration: SealedDeclaration::Exact("begin_static_interrupt_completion"),
        source: BundledFile::CoreStaticInterrupt,
        intrinsic: SealedIntrinsic::StaticInterrupt(StaticInterruptOperation::BeginCompletion),
    },
    SealedIntrinsicRow {
        file: "static_interrupt.omg",
        declaration: SealedDeclaration::Exact("finish_static_interrupt_completion"),
        source: BundledFile::CoreStaticInterrupt,
        intrinsic: SealedIntrinsic::StaticInterrupt(StaticInterruptOperation::FinishCompletion),
    },
    SealedIntrinsicRow {
        file: "task.omg",
        declaration: SealedDeclaration::Exact("TaskRuntime::start"),
        source: BundledFile::CoreTask,
        intrinsic: SealedIntrinsic::TaskRuntimeStart,
    },
    SealedIntrinsicRow {
        file: "task.omg",
        declaration: SealedDeclaration::Exact("TaskRuntime::try_start"),
        source: BundledFile::CoreTask,
        intrinsic: SealedIntrinsic::TaskRuntimeTryStart,
    },
];

/// Coordinates of a declaration known to the shared catalog.
pub struct BundledDeclaration {
    pub file: &'static Path,
    pub declaration: &'static str,
    pub source: BundledFile,
}

pub struct SealedIntrinsicCatalog;

impl SealedIntrinsicCatalog {
    /// Retrieve the unique exact member that supplies this catalog operation.
    /// Families of member names never establish an exact canonical declaration.
    pub fn exact_declaration(intrinsic: SealedIntrinsic) -> Option<BundledDeclaration> {
        let mut matches = ROWS.iter().filter(|row| row.intrinsic == intrinsic);
        let (Some(row), None) = (matches.next(), matches.next()) else {
            return None;
        };
        let SealedDeclaration::Exact(declaration) = row.declaration else {
            return None;
        };
        Some(BundledDeclaration {
            file: Path::new(row.file),
            declaration,
            source: row.source,
        })
    }

    /// Returns the operation the site's declaration is sealed to, or `None`
    /// when no row matches every part of the site.
    pub fn match_declaration(
        file: &Path,
        declaration: &str,
        matches_source: impl Fn(BundledFile) -> bool,
    ) -> Option<SealedIntrinsic> {
        ROWS.iter()
            .find(|row| {
                file == Path::new(row.file)
                    && row.declaration.matches(declaration)
                    && matches_source(row.source)
            })
            .map(|row| row.intrinsic)
    }
}
