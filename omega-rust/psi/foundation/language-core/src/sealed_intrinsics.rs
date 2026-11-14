/*
The sealed intrinsic catalog is the one list of bundled declarations that may
receive compiler custody: the compiler, not an authored body or foreign symbol,
supplies their behaviour. Every stage that asks "is this declaration compiler
owned?" reads this table instead of keeping its own file-name or member-name
recognizer, so adding or removing compiler custody is one row here.

A row names the file relative to its bundled package root, the declaration
path inside the package and the bundled source bytes the row was written
against. A declaration matches only when all three agree; the exact bytes also
tell core and std files with the same relative name apart. A user file with
the same name, a lookalike member, or a locally edited toolchain file never
receives custody. Callers establish the remaining precondition,
that the file was loaded from the toolchain rather than from a user package,
through `symbols::sealed_intrinsic_at`; every compiler consumer reaches the
catalog through that gate or through `SymbolTable::sealed_intrinsic`.

Matching a row grants identity only. Each consumer still authenticates the
declaration's signature, supply mode and contracts against the operation it
realizes before it uses the row.
*/

use crate::static_interrupts::StaticInterruptOperation;
use std::path::Path;

/// The compiler-supplied operation a catalog row identifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SealedIntrinsic {
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
    /// Core `layout.omg`'s `Access` template, the only data from which
    /// placed scalar accessors are generated.
    PlacedAccessorTemplate,
    /// std `console.omg`'s `ByteRead`, the result type of `Console::read_byte`.
    HostedByteRead,
    /// A hosted console or process-exit requirement in bundled source.
    HostedRequirement(HostedOperation),
    /// A target's bundled realization of a hosted requirement.
    HostedRealization {
        operation: HostedOperation,
        target: &'static str,
    },
    /// A UEFI fixed-image requirement in bundled core source.
    FixedImageRequirement(FixedImageOperation),
    /// The bundled `uefi_x86_64` realization of a fixed-image requirement.
    FixedImageRealization(FixedImageOperation),
}

/// Catalog identity only; the complete machine declaration owns its signature
/// and contract, and each execution consumer still checks the supplied operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerPrimitive {
    AtomicFence,
    FloatProjection,
    FloatSemantics,
    NaturalRanking,
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
    /// std `Console::exit_process`.
    ConsoleExitProcess,
    /// std `Console::write_byte`.
    ConsoleWriteByte,
    /// std `Console::read_byte`.
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
    source: &'static [u8],
    intrinsic: SealedIntrinsic,
}

const CORE_FLOAT_PROVIDER: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/core/float_provider.omg"
));

const CORE_TASK: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/core/task.omg"
));

const CORE_STATIC_INTERRUPT: &[u8] = crate::static_interrupts::CORE_DECLARATIONS;

macro_rules! bundled {
    ($($path:tt)+) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../source/library/",
            $($path)+
        ))
    };
}

const CORE_LAYOUT: &[u8] = bundled!("core/layout.omg");
const CORE_RUN: &[u8] = bundled!("core/run.omg");
const CORE_FLOAT_OPERATIONS: &[u8] = bundled!("core/float_operations.omg");
const CORE_EXTENT: &[u8] = bundled!("core/extent.omg");
const CORE_PROGRAM_IMAGE: &[u8] = bundled!("core/program_image.omg");
const CORE_PROCESS_EXIT: &[u8] = bundled!("core/process_exit.omg");
const STD_CONSOLE: &[u8] = bundled!("std/console.omg");

const fn row(
    file: &'static str,
    declaration: &'static str,
    source: &'static [u8],
    intrinsic: SealedIntrinsic,
) -> SealedIntrinsicRow {
    SealedIntrinsicRow {
        file,
        declaration: SealedDeclaration::Exact(declaration),
        source,
        intrinsic,
    }
}

/*
    Each hosted target bundles one console and one process-exit realization
    file in std. The rows name the target so a provider plan for one target
    can never take custody through another target's realization.
*/
macro_rules! hosted_target_rows {
    ($target:literal) => {
        [
            row(
                concat!("targets/", $target, "/console_impl.omg"),
                "ConsoleNativeProvider::exit_process",
                bundled!(concat!("std/targets/", $target, "/console_impl.omg")),
                SealedIntrinsic::HostedRealization {
                    operation: HostedOperation::ConsoleExitProcess,
                    target: $target,
                },
            ),
            row(
                concat!("targets/", $target, "/console_impl.omg"),
                "ConsoleNativeProvider::write_byte",
                bundled!(concat!("std/targets/", $target, "/console_impl.omg")),
                SealedIntrinsic::HostedRealization {
                    operation: HostedOperation::ConsoleWriteByte,
                    target: $target,
                },
            ),
            row(
                concat!("targets/", $target, "/console_impl.omg"),
                "ConsoleNativeProvider::read_byte",
                bundled!(concat!("std/targets/", $target, "/console_impl.omg")),
                SealedIntrinsic::HostedRealization {
                    operation: HostedOperation::ConsoleReadByte,
                    target: $target,
                },
            ),
            row(
                concat!("targets/", $target, "/process_exit_impl.omg"),
                "ProcessExitNativeProvider::exit_process",
                bundled!(concat!("std/targets/", $target, "/process_exit_impl.omg")),
                SealedIntrinsic::HostedRealization {
                    operation: HostedOperation::ProcessExit,
                    target: $target,
                },
            ),
        ]
    };
}

const HOSTED_LINUX_ARM64: [SealedIntrinsicRow; 4] = hosted_target_rows!("linux_arm64");
const HOSTED_LINUX_X86_64: [SealedIntrinsicRow; 4] = hosted_target_rows!("linux_x86_64");
const HOSTED_MACOS_ARM64: [SealedIntrinsicRow; 4] = hosted_target_rows!("macos_arm64");
const HOSTED_MACOS_X86_64: [SealedIntrinsicRow; 4] = hosted_target_rows!("macos_x86_64");
const HOSTED_REALIZATIONS: [&[SealedIntrinsicRow]; 4] = [
    &HOSTED_LINUX_ARM64,
    &HOSTED_LINUX_X86_64,
    &HOSTED_MACOS_ARM64,
    &HOSTED_MACOS_X86_64,
];

const fn member_row(
    file: &'static str,
    owner: &'static str,
    source: &'static [u8],
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
        "atomic.omg",
        "Atomic::fence",
        bundled!("core/atomic.omg"),
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::AtomicFence),
    ),
    row(
        "float_operations.omg",
        "Float::meaning32",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::FloatProjection),
    ),
    row(
        "float_operations.omg",
        "Float::meaning64",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::FloatProjection),
    ),
    member_row(
        "float_operations.omg",
        "FloatSemantics",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::FloatSemantics),
    ),
    row(
        "nat.omg",
        "Nat::Descending",
        bundled!("core/nat.omg"),
        SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::NaturalRanking),
    ),
    row(
        "float_provider.omg",
        "NativeFloats",
        CORE_FLOAT_PROVIDER,
        SealedIntrinsic::NativeFloatMap,
    ),
    row(
        "float_provider.omg",
        "FloatNativeOperations",
        CORE_FLOAT_PROVIDER,
        SealedIntrinsic::NativeFloatOperationsTrait,
    ),
    member_row(
        "float_operations.omg",
        "Float",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "F32",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "F64",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "I8",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "I16",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "I32",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "I64",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "U8",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "U16",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "U32",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    member_row(
        "float_operations.omg",
        "U64",
        CORE_FLOAT_OPERATIONS,
        SealedIntrinsic::FloatOperationRequirement,
    ),
    row(
        "layout.omg",
        "LayoutTarget",
        CORE_LAYOUT,
        SealedIntrinsic::LayoutTargetData,
    ),
    row(
        "layout.omg",
        "LayoutStorage",
        CORE_LAYOUT,
        SealedIntrinsic::LayoutStorageData,
    ),
    row(
        "layout.omg",
        "layout_target_facts",
        CORE_LAYOUT,
        SealedIntrinsic::LayoutTargetFacts,
    ),
    row(
        "layout.omg",
        "read",
        CORE_LAYOUT,
        SealedIntrinsic::PlacedPrimitive(PlacedPrimitiveOperation::Read),
    ),
    row(
        "layout.omg",
        "write",
        CORE_LAYOUT,
        SealedIntrinsic::PlacedPrimitive(PlacedPrimitiveOperation::Write),
    ),
    row(
        "layout.omg",
        "Access::initialize_from_profile",
        CORE_LAYOUT,
        lifecycle(PlacementOperation::Initialize, PlacementContent::Resident),
    ),
    row(
        "layout.omg",
        "Access::extract_resident",
        CORE_LAYOUT,
        lifecycle(PlacementOperation::MoveOut, PlacementContent::Resident),
    ),
    row(
        "layout.omg",
        "Access::view_from_profile",
        CORE_LAYOUT,
        lifecycle(PlacementOperation::View, PlacementContent::Resident),
    ),
    row(
        "layout.omg",
        "Access::view_mut_from_profile",
        CORE_LAYOUT,
        lifecycle(PlacementOperation::View, PlacementContent::Resident),
    ),
    row(
        "run.omg",
        "Run::initialize_from_profile",
        CORE_RUN,
        lifecycle(PlacementOperation::Initialize, PlacementContent::Run),
    ),
    row(
        "run.omg",
        "Run::extract_from_profile",
        CORE_RUN,
        lifecycle(PlacementOperation::MoveOut, PlacementContent::Run),
    ),
    row(
        "run.omg",
        "Run::relocate_from_profiles",
        CORE_RUN,
        lifecycle(PlacementOperation::Relocate, PlacementContent::Run),
    ),
    row(
        "layout.omg",
        "Access",
        CORE_LAYOUT,
        SealedIntrinsic::PlacedAccessorTemplate,
    ),
    row(
        "console.omg",
        "ByteRead",
        STD_CONSOLE,
        SealedIntrinsic::HostedByteRead,
    ),
    row(
        "console.omg",
        "Console::exit_process",
        STD_CONSOLE,
        SealedIntrinsic::HostedRequirement(HostedOperation::ConsoleExitProcess),
    ),
    row(
        "console.omg",
        "Console::write_byte",
        STD_CONSOLE,
        SealedIntrinsic::HostedRequirement(HostedOperation::ConsoleWriteByte),
    ),
    row(
        "console.omg",
        "Console::read_byte",
        STD_CONSOLE,
        SealedIntrinsic::HostedRequirement(HostedOperation::ConsoleReadByte),
    ),
    row(
        "process_exit.omg",
        "ProcessExit::exit_process",
        CORE_PROCESS_EXIT,
        SealedIntrinsic::HostedRequirement(HostedOperation::ProcessExit),
    ),
    row(
        "extent.omg",
        "ProgramStorageEntry::enter",
        CORE_EXTENT,
        SealedIntrinsic::FixedImageRequirement(FixedImageOperation::ProgramStorageEntry),
    ),
    row(
        "extent.omg",
        "ProgramStorageEntryNativeProvider::enter",
        CORE_EXTENT,
        SealedIntrinsic::FixedImageRealization(FixedImageOperation::ProgramStorageEntry),
    ),
    row(
        "program_image.omg",
        "ProgramImage::retained_suffix_start",
        CORE_PROGRAM_IMAGE,
        SealedIntrinsic::FixedImageRequirement(
            FixedImageOperation::ProgramImageRetainedSuffixStart,
        ),
    ),
    row(
        "program_image.omg",
        "ProgramImageNativeProvider::retained_suffix_start",
        CORE_PROGRAM_IMAGE,
        SealedIntrinsic::FixedImageRealization(
            FixedImageOperation::ProgramImageRetainedSuffixStart,
        ),
    ),
    SealedIntrinsicRow {
        file: "float_provider.omg",
        declaration: SealedDeclaration::MemberOf("NativeFloats"),
        source: CORE_FLOAT_PROVIDER,
        intrinsic: SealedIntrinsic::NativeFloatOperation,
    },
    SealedIntrinsicRow {
        file: "static_interrupt.omg",
        declaration: SealedDeclaration::Exact("X86StaticInterruptScope::run"),
        source: CORE_STATIC_INTERRUPT,
        intrinsic: SealedIntrinsic::StaticInterrupt(StaticInterruptOperation::Install),
    },
    SealedIntrinsicRow {
        file: "static_interrupt.omg",
        declaration: SealedDeclaration::Exact("view_static_interrupt_context"),
        source: CORE_STATIC_INTERRUPT,
        intrinsic: SealedIntrinsic::StaticInterrupt(StaticInterruptOperation::Context),
    },
    SealedIntrinsicRow {
        file: "static_interrupt.omg",
        declaration: SealedDeclaration::Exact("begin_static_interrupt_completion"),
        source: CORE_STATIC_INTERRUPT,
        intrinsic: SealedIntrinsic::StaticInterrupt(StaticInterruptOperation::BeginCompletion),
    },
    SealedIntrinsicRow {
        file: "static_interrupt.omg",
        declaration: SealedDeclaration::Exact("finish_static_interrupt_completion"),
        source: CORE_STATIC_INTERRUPT,
        intrinsic: SealedIntrinsic::StaticInterrupt(StaticInterruptOperation::FinishCompletion),
    },
    SealedIntrinsicRow {
        file: "task.omg",
        declaration: SealedDeclaration::Exact("TaskRuntime::start"),
        source: CORE_TASK,
        intrinsic: SealedIntrinsic::TaskRuntimeStart,
    },
    SealedIntrinsicRow {
        file: "task.omg",
        declaration: SealedDeclaration::Exact("TaskRuntime::try_start"),
        source: CORE_TASK,
        intrinsic: SealedIntrinsic::TaskRuntimeTryStart,
    },
];

/// A declaration already known to come from a toolchain source file.
#[derive(Debug, Clone, Copy)]
pub struct SealedDeclarationSite<'a> {
    /// The file path relative to its package root.
    pub file: &'a Path,
    /// The declaration path inside the package, segments joined by `::`.
    pub declaration: &'a str,
    /// The complete source bytes of the file as loaded.
    pub source: &'a [u8],
}

pub struct SealedIntrinsicCatalog;

impl SealedIntrinsicCatalog {
    /// Returns the operation the site's declaration is sealed to, or `None`
    /// when no row matches every part of the site.
    pub fn classify(site: SealedDeclarationSite<'_>) -> Option<SealedIntrinsic> {
        ROWS.iter()
            .chain(HOSTED_REALIZATIONS.into_iter().flatten())
            .find(|row| {
                site.file == Path::new(row.file)
                    && row.declaration.matches(site.declaration)
                    && row.source == site.source
            })
            .map(|row| row.intrinsic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site<'a>(
        file: &'a str,
        declaration: &'a str,
        source: &'a [u8],
    ) -> SealedDeclarationSite<'a> {
        SealedDeclarationSite {
            file: Path::new(file),
            declaration,
            source,
        }
    }

    #[test]
    fn rows_match_only_their_exact_file_declaration_and_source() {
        assert_eq!(
            SealedIntrinsicCatalog::classify(site("task.omg", "TaskRuntime::start", CORE_TASK)),
            Some(SealedIntrinsic::TaskRuntimeStart)
        );
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "float_provider.omg",
                "NativeFloats::float_add_f32_f32",
                CORE_FLOAT_PROVIDER
            )),
            Some(SealedIntrinsic::NativeFloatOperation)
        );
        // A lookalike file, an edited source, a nested member, or a bare
        // owner name supplies no custody.
        let linux = bundled!("std/targets/linux_x86_64/console_impl.omg");
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "targets/linux_x86_64/console_impl.omg",
                "ConsoleNativeProvider::write_byte",
                linux
            )),
            Some(SealedIntrinsic::HostedRealization {
                operation: HostedOperation::ConsoleWriteByte,
                target: "linux_x86_64",
            })
        );
        // One target's realization bytes never stand in for another target's file.
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "targets/linux_arm64/console_impl.omg",
                "ConsoleNativeProvider::write_byte",
                linux
            )),
            None
        );
        assert_eq!(
            SealedIntrinsicCatalog::classify(site("tasks.omg", "TaskRuntime::start", CORE_TASK)),
            None
        );
        assert_eq!(
            SealedIntrinsicCatalog::classify(site("task.omg", "TaskRuntime::start", b"edited")),
            None
        );
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "float_provider.omg",
                "NativeFloats::a::b",
                CORE_FLOAT_PROVIDER
            )),
            None
        );
        // The map itself is its own row; a bare member owner is not.
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "float_provider.omg",
                "NativeFloats",
                CORE_FLOAT_PROVIDER
            )),
            Some(SealedIntrinsic::NativeFloatMap)
        );
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "float_operations.omg",
                "Float",
                CORE_FLOAT_OPERATIONS
            )),
            None
        );
    }
}
