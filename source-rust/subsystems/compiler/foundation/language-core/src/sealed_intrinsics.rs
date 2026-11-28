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
    /// One of core's two equality evidence traits, `EqualImpliesIdentity` and
    /// `IdentityImpliesEqual`. Each inherits `Equatable` and makes its
    /// `equals` admissible in a proof clause
    /// (wiki/spec/proofs/contracts.md, "Equality bridges and comparison facts").
    EqualityEvidence,
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

/// The checked packed-integer instructions. Each one is the contract of one
/// `asm` mnemonic in core `assembly_packed_integer.omg`: shared fixed arrays in,
/// one owned array out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackedIntegerPrimitive {
    WrappingAddI16x8,
    WrappingSubI16x8,
    UnpackLowWords,
    UnpackHighWords,
    UnpackLowDoublewords,
    UnpackHighDoublewords,
    UnpackLowQuadwords,
    UnpackHighQuadwords,
    ShiftRightI16x8,
    WidenU8x8ToI16x8,
    NarrowI16x8ToU8x8Saturating,
    WidenU8x4ToI16x8,
    NarrowI16x8ToU8x4Saturating,
    LoadI16x4ZeroExtend,
    WrappingAddU8x16,
    MinI16x8,
    CmpGtI16x8,
    BitAndI16x8,
    BitAndNotI16x8,
    AbsI16x8,
}

impl PackedIntegerPrimitive {
    pub const ALL: [Self; 20] = [
        Self::WrappingAddI16x8,
        Self::WrappingSubI16x8,
        Self::UnpackLowWords,
        Self::UnpackHighWords,
        Self::UnpackLowDoublewords,
        Self::UnpackHighDoublewords,
        Self::UnpackLowQuadwords,
        Self::UnpackHighQuadwords,
        Self::ShiftRightI16x8,
        Self::WidenU8x8ToI16x8,
        Self::NarrowI16x8ToU8x8Saturating,
        Self::WidenU8x4ToI16x8,
        Self::NarrowI16x8ToU8x4Saturating,
        Self::LoadI16x4ZeroExtend,
        Self::WrappingAddU8x16,
        Self::MinI16x8,
        Self::CmpGtI16x8,
        Self::BitAndI16x8,
        Self::BitAndNotI16x8,
        Self::AbsI16x8,
    ];

    /// The x86-64 mnemonic that spells this instruction in `asm`; it is also
    /// the name of its contract declaration.
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::WrappingAddI16x8 => "vpaddw",
            Self::WrappingSubI16x8 => "vpsubw",
            Self::UnpackLowWords => "vpunpcklwd",
            Self::UnpackHighWords => "vpunpckhwd",
            Self::UnpackLowDoublewords => "vpunpckldq",
            Self::UnpackHighDoublewords => "vpunpckhdq",
            Self::UnpackLowQuadwords => "vpunpcklqdq",
            Self::UnpackHighQuadwords => "vpunpckhqdq",
            Self::ShiftRightI16x8 => "vpsraw",
            Self::WidenU8x8ToI16x8 => "vpmovzxbw",
            Self::NarrowI16x8ToU8x8Saturating => "vpackuswb",
            // The four-byte forms are the x86-64 pairs `vmovd` + `vpmovzxbw`
            // and `vpackuswb` + `vmovd`, which read or write exactly four
            // bytes; the eight-byte mnemonics would touch eight.
            Self::WidenU8x4ToI16x8 => "vpmovzxbw_low4",
            Self::NarrowI16x8ToU8x4Saturating => "vpackuswb_low4",
            Self::LoadI16x4ZeroExtend => "vmovq",
            Self::WrappingAddU8x16 => "vpaddb",
            Self::MinI16x8 => "vpminsw",
            Self::CmpGtI16x8 => "vpcmpgtw",
            Self::BitAndI16x8 => "vpand",
            Self::BitAndNotI16x8 => "vpandn",
            Self::AbsI16x8 => "vpabsw",
        }
    }

    pub fn from_mnemonic(mnemonic: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|primitive| primitive.mnemonic() == mnemonic)
    }

    pub fn inputs(self) -> usize {
        match self {
            Self::WrappingAddI16x8
            | Self::WrappingSubI16x8
            | Self::UnpackLowWords
            | Self::UnpackHighWords
            | Self::UnpackLowDoublewords
            | Self::UnpackHighDoublewords
            | Self::UnpackLowQuadwords
            | Self::UnpackHighQuadwords
            | Self::WrappingAddU8x16
            | Self::MinI16x8
            | Self::CmpGtI16x8
            | Self::BitAndI16x8
            | Self::BitAndNotI16x8 => 2,
            Self::ShiftRightI16x8
            | Self::WidenU8x8ToI16x8
            | Self::NarrowI16x8ToU8x8Saturating
            | Self::WidenU8x4ToI16x8
            | Self::NarrowI16x8ToU8x4Saturating
            | Self::LoadI16x4ZeroExtend
            | Self::AbsI16x8 => 1,
        }
    }

    /// Constant operands after the arrays, such as the shift count.
    pub fn immediates(self) -> usize {
        match self {
            Self::ShiftRightI16x8 => 1,
            _ => 0,
        }
    }

    /// The fixed lane selection an interleave realizes. Lanes 0 to 7 name the
    /// left input and 8 to 15 the right input.
    pub const fn interleaved_lanes(self) -> Option<[u8; 8]> {
        Some(match self {
            Self::UnpackLowWords => [0, 8, 1, 9, 2, 10, 3, 11],
            Self::UnpackHighWords => [4, 12, 5, 13, 6, 14, 7, 15],
            Self::UnpackLowDoublewords => [0, 1, 8, 9, 2, 3, 10, 11],
            Self::UnpackHighDoublewords => [4, 5, 12, 13, 6, 7, 14, 15],
            Self::UnpackLowQuadwords => [0, 1, 2, 3, 8, 9, 10, 11],
            Self::UnpackHighQuadwords => [4, 5, 6, 7, 12, 13, 14, 15],
            _ => return None,
        })
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
        "equality.omg",
        "EqualImpliesIdentity",
        BundledFile::CoreEquality,
        SealedIntrinsic::EqualityEvidence,
    ),
    row(
        "equality.omg",
        "IdentityImpliesEqual",
        BundledFile::CoreEquality,
        SealedIntrinsic::EqualityEvidence,
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
        "assembly_packed_integer.omg",
        "vpaddw",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::WrappingAddI16x8,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpsubw",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::WrappingSubI16x8,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpunpcklwd",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::UnpackLowWords,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpunpckhwd",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::UnpackHighWords,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpunpckldq",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::UnpackLowDoublewords,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpunpckhdq",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::UnpackHighDoublewords,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpunpcklqdq",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::UnpackLowQuadwords,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpunpckhqdq",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::UnpackHighQuadwords,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpsraw",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::ShiftRightI16x8,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpmovzxbw",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::WidenU8x8ToI16x8,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpackuswb",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::NarrowI16x8ToU8x8Saturating,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpmovzxbw_low4",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::WidenU8x4ToI16x8,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpackuswb_low4",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::NarrowI16x8ToU8x4Saturating,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vmovq",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::LoadI16x4ZeroExtend,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpaddb",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::WrappingAddU8x16,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpminsw",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::MinI16x8,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpcmpgtw",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::CmpGtI16x8,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpand",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::BitAndI16x8,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpandn",
        BundledFile::CoreAssemblyPackedInteger,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::PackedInteger(
            PackedIntegerPrimitive::BitAndNotI16x8,
        )),
    ),
    row(
        "assembly_packed_integer.omg",
        "vpabsw",
        BundledFile::CoreAssemblyPackedInteger,
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

    /// Returns the operation for a file already authenticated against its
    /// exact bundled core path and bytes at source admission.
    pub fn classify_bundled(file: BundledFile, declaration: &str) -> Option<SealedIntrinsic> {
        ROWS.iter()
            .find(|row| row.source == file && row.declaration.matches(declaration))
            .map(|row| row.intrinsic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_classification_matches_row_paths() {
        for row in ROWS {
            assert_eq!(
                Path::new(row.file),
                Path::new(row.source.core_relative_path()),
                "sealed row `{:?}` path must match its BundledFile path",
                row.intrinsic
            );
            let sample = match row.declaration {
                SealedDeclaration::Exact(path) => path.to_owned(),
                SealedDeclaration::MemberOf(owner) => format!("{owner}::member"),
            };
            assert_eq!(
                SealedIntrinsicCatalog::classify_bundled(row.source, &sample),
                Some(row.intrinsic)
            );
        }
    }
}
