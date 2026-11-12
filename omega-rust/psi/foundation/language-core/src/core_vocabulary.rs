/*
The core vocabulary catalog lists bundled core declarations that checking must
recognize by identity: the services, tokens, domains and private primitives
that typed rules are written against. Recognition is all a row grants. A
vocabulary row never gives compiler custody; that comes only from
`sealed_intrinsics`, and the two tables share no variant, so neither can be
read as the other.

Rows match exactly like sealed intrinsic rows: the package-relative file, the
declaration path and the exact bundled source bytes must all agree, and the
caller first establishes toolchain origin through `symbols::core_vocabulary_at`
or `SymbolTable::core_vocabulary`. A lookalike user declaration, or an edited
toolchain file, is therefore ordinary code to every consumer.
*/

use crate::sealed_intrinsics::{SealedDeclaration, SealedDeclarationSite};
use std::path::Path;

/// A core declaration that typed rules recognize by identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoreVocabulary {
    /// The `MachineControl` service in core `assembly.omg`.
    MachineControlService,
    /// The `PortIo` service in core `assembly.omg`.
    PortIoService,
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
    source: &'static [u8],
    vocabulary: CoreVocabulary,
}

macro_rules! core_source {
    ($file:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../source/library/core/",
            $file
        ))
    };
}

const ASSEMBLY: &[u8] = core_source!("assembly.omg");
const CODE_SEGMENT: &[u8] = core_source!("code_segment.omg");
const DATA_SEGMENT: &[u8] = core_source!("data_segment.omg");
const TASK_REGISTER: &[u8] = core_source!("task_register.omg");
const INTERRUPT: &[u8] = core_source!("interrupt.omg");
const STATIC_INTERRUPT: &[u8] = crate::static_interrupts::CORE_DECLARATIONS;

const fn row(
    file: &'static str,
    declaration: &'static str,
    source: &'static [u8],
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
    source: &'static [u8],
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
        "assembly.omg",
        "MachineControl",
        ASSEMBLY,
        CoreVocabulary::MachineControlService,
    ),
    row(
        "assembly.omg",
        "PortIo",
        ASSEMBLY,
        CoreVocabulary::PortIoService,
    ),
    selector(
        "code_segment.omg",
        "reload_code_segment",
        CODE_SEGMENT,
        SegmentSelectorOperation::ReloadCode,
    ),
    selector(
        "code_segment.omg",
        "read_code_selector",
        CODE_SEGMENT,
        SegmentSelectorOperation::ReadCode,
    ),
    selector(
        "data_segment.omg",
        "reload_data_segments",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReloadData,
    ),
    selector(
        "data_segment.omg",
        "read_data_selector",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReadData,
    ),
    selector(
        "data_segment.omg",
        "read_extra_selector",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReadExtra,
    ),
    selector(
        "data_segment.omg",
        "read_stack_selector",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReadStack,
    ),
    selector(
        "data_segment.omg",
        "read_fs_selector",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReadFs,
    ),
    selector(
        "data_segment.omg",
        "read_gs_selector",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReadGs,
    ),
    row(
        "task_register.omg",
        "activate_task_register",
        TASK_REGISTER,
        CoreVocabulary::TaskRegister(TaskRegisterOperation::Activate),
    ),
    row(
        "task_register.omg",
        "read_task_register",
        TASK_REGISTER,
        CoreVocabulary::TaskRegister(TaskRegisterOperation::Read),
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgement",
        INTERRUPT,
        CoreVocabulary::InterruptAcknowledgement,
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgement::Pending",
        INTERRUPT,
        CoreVocabulary::InterruptAcknowledgementPending,
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgementCompletion",
        INTERRUPT,
        CoreVocabulary::InterruptAcknowledgementCompletion,
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgementCompletion::complete",
        INTERRUPT,
        CoreVocabulary::InterruptAcknowledgementCompletionComplete,
    ),
    row(
        "static_interrupt.omg",
        "StaticInterruptCompletionPermit",
        STATIC_INTERRUPT,
        CoreVocabulary::StaticInterruptCompletionPermit,
    ),
];

pub struct CoreVocabularyCatalog;

impl CoreVocabularyCatalog {
    /// Returns the vocabulary the site's declaration is, or `None` when no row
    /// matches every part of the site.
    pub fn classify(site: SealedDeclarationSite<'_>) -> Option<CoreVocabulary> {
        ROWS.iter()
            .find(|row| {
                site.file == Path::new(row.file)
                    && SealedDeclaration::Exact(row.declaration).matches(site.declaration)
                    && row.source == site.source
            })
            .map(|row| row.vocabulary)
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
            CoreVocabularyCatalog::classify(site("assembly.omg", "MachineControl", ASSEMBLY)),
            Some(CoreVocabulary::MachineControlService)
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "data_segment.omg",
                "read_gs_selector",
                DATA_SEGMENT
            )),
            Some(CoreVocabulary::SegmentSelector(
                SegmentSelectorOperation::ReadGs
            ))
        );
        // A selector name in the other segment file is not that primitive.
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "code_segment.omg",
                "read_gs_selector",
                CODE_SEGMENT
            )),
            None
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site("assembly.omg", "MachineControl", b"edited")),
            None
        );
    }
}
