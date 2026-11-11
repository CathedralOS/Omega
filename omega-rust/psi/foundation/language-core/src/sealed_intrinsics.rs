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
    /// A member of core's `NativeFloats` realization of `FloatNativeOperations`.
    NativeFloatOperation,
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
enum SealedDeclaration {
    /// Exactly this package-relative declaration path.
    Exact(&'static str),
    /// Any single-segment member directly under this owner path.
    MemberOf(&'static str),
}

impl SealedDeclaration {
    fn matches(self, declaration: &str) -> bool {
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

const ROWS: &[SealedIntrinsicRow] = &[
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
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "float_provider.omg",
                "NativeFloats",
                CORE_FLOAT_PROVIDER
            )),
            None
        );
    }
}
