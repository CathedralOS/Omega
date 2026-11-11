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

const ROWS: &[SealedIntrinsicRow] = &[
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
