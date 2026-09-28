//! Admission of general relocations on the mutable object lane.
//!
//! A `RelocationPlan` is caller-authored data. Admission proves once what
//! every downstream patcher would otherwise have to re-check record by
//! record: the plan's target is the object's exact target, each kind is in
//! that architecture's closed vocabulary and carries its fixed field width,
//! each patched window lands inside a materialized section, every symbolic
//! target and origin custody handle resolves, and no two patch windows in
//! one section overlap.
//!
//! Final symbol addresses cannot exist at admission time, so checks that
//! depend on a chosen layout — the rel32 delta range, the absolute value —
//! stay with the patcher and the byte envelopes, not here.

use crate::{
    ObjectPlan, RelocationKind, RelocationOrigin, RelocationPlan, SectionKind, SymbolKind,
};
use std::collections::BTreeMap;

/// A relocation plan admitted against its object plan.
///
/// The wrapper retains the plan's custody: consumers read the proven rows
/// back through [`Self::plan`] or take the plan back with [`Self::into_plan`].
#[derive(Debug, Clone)]
#[must_use = "an admitted relocation plan retains the rows it proved"]
pub struct ValidatedRelocationPlan {
    plan: RelocationPlan,
    section_record_counts: BTreeMap<SectionKindOrdinal, usize>,
}

/// `SectionKind` has no `Ord`; admission tallies by this stable ordinal so
/// the map ordering is not the enum's presentation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SectionKindOrdinal(u8);

const fn section_ordinal(section: SectionKind) -> SectionKindOrdinal {
    match section {
        SectionKind::Text => SectionKindOrdinal(0),
        SectionKind::Data => SectionKindOrdinal(1),
        SectionKind::Bss => SectionKindOrdinal(2),
    }
}

impl ValidatedRelocationPlan {
    /// The admitted plan, unchanged from what the caller supplied.
    pub const fn plan(&self) -> &RelocationPlan {
        &self.plan
    }

    pub fn record_count(&self) -> usize {
        self.plan.record_count()
    }

    /// Records patching each section kind, in the stable ordinal order.
    pub fn section_record_counts(&self) -> impl Iterator<Item = (SectionKind, usize)> + '_ {
        self.section_record_counts
            .iter()
            .map(|(ordinal, count)| (ordinal.section_kind(), *count))
    }

    pub fn records_in_section(&self, section: SectionKind) -> usize {
        self.section_record_counts
            .get(&section_ordinal(section))
            .copied()
            .unwrap_or(0)
    }

    /// Returns the admitted plan, ending the wrapper's custody.
    pub fn into_plan(self) -> RelocationPlan {
        self.plan
    }
}

impl SectionKindOrdinal {
    const fn section_kind(self) -> SectionKind {
        match self {
            Self(0) => SectionKind::Text,
            Self(1) => SectionKind::Data,
            _ => SectionKind::Bss,
        }
    }
}

/// One reason admission refused a plan. Typed so rejections distinguish
/// custody failures (unknown symbols/origins) from shape failures
/// (vocabulary, width, bounds, overlap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationAdmissionError {
    /// The plan's target is not the object plan's exact target.
    TargetMismatch,
    /// The record's kind is outside the architecture's closed vocabulary.
    KindOutsideTargetVocabulary { kind: RelocationKind },
    /// The record's byte width differs from the kind's fixed field width.
    WrongFieldWidth {
        kind: RelocationKind,
        byte_width: usize,
    },
    /// The record's offset lacks the alignment the kind's field requires.
    MisalignedField { kind: RelocationKind, offset: usize },
    /// No section of the record's kind exists in the object layout.
    MissingSection { section: SectionKind },
    /// The record patches a section that carries no bytes to patch.
    UnmaterializedSection { section: SectionKind },
    /// `offset + byte_width` overflows or exceeds the section's size.
    FieldOutsideSection {
        section: SectionKind,
        offset: usize,
        byte_width: usize,
    },
    /// The record's symbolic target does not resolve in the symbol table.
    UnknownSymbol,
    /// An origin's custody handle does not resolve in the symbol table.
    UnknownOriginSymbol,
    /// An instruction or semantic origin names a non-function symbol.
    OriginSymbolNotFunction,
    /// A materialization origin names an import; imports own no local bytes.
    MaterializationOriginIsImport,
    /// Two records patch overlapping windows of the same section.
    OverlappingRecords { section: SectionKind, offset: usize },
}

/// A rejected plan retains its custody beside the reason, so a caller can
/// inspect or discard the exact rows admission refused.
#[derive(Debug)]
#[must_use = "a refused relocation plan retains the rows that failed"]
pub struct RelocationAdmissionFailure {
    plan: RelocationPlan,
    error: RelocationAdmissionError,
}

impl RelocationAdmissionFailure {
    pub const fn error(&self) -> &RelocationAdmissionError {
        &self.error
    }

    pub const fn plan(&self) -> &RelocationPlan {
        &self.plan
    }

    pub fn into_parts(self) -> (RelocationPlan, RelocationAdmissionError) {
        (self.plan, self.error)
    }
}

impl RelocationKind {
    /// The fixed byte width of the field this kind patches.
    pub const fn field_width(self) -> usize {
        match self {
            Self::Absolute64 => 8,
            Self::X86_64Relative32
            | Self::Aarch64Page21
            | Self::Aarch64PageOffset12
            | Self::Aarch64Branch26 => 4,
        }
    }

    /// The alignment the patched field's offset must satisfy: absolute
    /// slots are naturally aligned, AArch64 kinds patch instruction words.
    pub const fn field_alignment(self) -> usize {
        match self {
            Self::Absolute64 => 8,
            Self::Aarch64Page21 | Self::Aarch64PageOffset12 | Self::Aarch64Branch26 => 4,
            Self::X86_64Relative32 => 1,
        }
    }

    /// The architecture's closed relocation vocabulary.
    pub const fn admitted_on(self, architecture: target::Architecture) -> bool {
        match architecture {
            target::Architecture::X86_64 => {
                matches!(self, Self::Absolute64 | Self::X86_64Relative32)
            }
            target::Architecture::Aarch64 => matches!(
                self,
                Self::Aarch64Page21
                    | Self::Aarch64PageOffset12
                    | Self::Aarch64Branch26
                    | Self::Absolute64
            ),
        }
    }
}

/// Admit a general relocation plan against the object it patches.
pub fn admit_relocation_plan(
    plan: RelocationPlan,
    object: &ObjectPlan,
) -> Result<ValidatedRelocationPlan, RelocationAdmissionFailure> {
    macro_rules! refuse {
        ($error:expr) => {
            return Err(RelocationAdmissionFailure {
                plan,
                error: $error,
            })
        };
    }

    if plan.target != object.target {
        refuse!(RelocationAdmissionError::TargetMismatch);
    }
    let architecture = object.target.architecture;

    let mut section_windows = BTreeMap::<SectionKindOrdinal, Vec<(usize, usize)>>::new();
    let mut section_record_counts = BTreeMap::<SectionKindOrdinal, usize>::new();

    for record in plan
        .records()
        .map(|(_, record)| record.clone())
        .collect::<Vec<_>>()
    {
        let record = &record;
        if !record.kind.admitted_on(architecture) {
            refuse!(RelocationAdmissionError::KindOutsideTargetVocabulary { kind: record.kind });
        }
        if record.byte_width != record.kind.field_width() {
            refuse!(RelocationAdmissionError::WrongFieldWidth {
                kind: record.kind,
                byte_width: record.byte_width,
            });
        }
        let alignment = record.kind.field_alignment();
        if record.offset % alignment != 0 {
            refuse!(RelocationAdmissionError::MisalignedField {
                kind: record.kind,
                offset: record.offset,
            });
        }

        let Some((_, section_plan)) = object
            .layout
            .sections
            .iter()
            .find(|(_, section)| section.kind == record.section)
        else {
            refuse!(RelocationAdmissionError::MissingSection {
                section: record.section,
            });
        };
        if record.section == SectionKind::Bss {
            refuse!(RelocationAdmissionError::UnmaterializedSection {
                section: record.section,
            });
        }
        let Some(field_end) = record
            .offset
            .checked_add(record.byte_width)
            .filter(|end| *end <= section_plan.size)
        else {
            refuse!(RelocationAdmissionError::FieldOutsideSection {
                section: record.section,
                offset: record.offset,
                byte_width: record.byte_width,
            });
        };

        if !object.layout.symbols.is_valid(record.symbol_handle) {
            refuse!(RelocationAdmissionError::UnknownSymbol);
        }
        match record.origin {
            RelocationOrigin::Instruction {
                function_symbol_handle,
                ..
            }
            | RelocationOrigin::SemanticOperation {
                function_symbol_handle,
                ..
            }
            | RelocationOrigin::SemanticEdge {
                function_symbol_handle,
                ..
            } => {
                if !object.layout.symbols.is_valid(function_symbol_handle) {
                    refuse!(RelocationAdmissionError::UnknownOriginSymbol);
                }
                if object.layout.symbols.get(function_symbol_handle).kind != SymbolKind::Function {
                    refuse!(RelocationAdmissionError::OriginSymbolNotFunction);
                }
            }
            RelocationOrigin::Materialization {
                object_symbol_handle,
            } => {
                if !object.layout.symbols.is_valid(object_symbol_handle) {
                    refuse!(RelocationAdmissionError::UnknownOriginSymbol);
                }
                if object.layout.symbols.get(object_symbol_handle).kind == SymbolKind::Import {
                    refuse!(RelocationAdmissionError::MaterializationOriginIsImport);
                }
            }
        }

        let ordinal = section_ordinal(record.section);
        let window = (record.offset, field_end);
        if section_windows.get(&ordinal).is_some_and(|windows| {
            windows
                .iter()
                .any(|(start, end)| window.0 < *end && *start < window.1)
        }) {
            refuse!(RelocationAdmissionError::OverlappingRecords {
                section: record.section,
                offset: record.offset,
            });
        }
        section_windows.entry(ordinal).or_default().push(window);
        *section_record_counts.entry(ordinal).or_insert(0) += 1;
    }

    Ok(ValidatedRelocationPlan {
        plan,
        section_record_counts,
    })
}

#[cfg(test)]
mod tests {
    use super::{RelocationAdmissionError, SectionKindOrdinal, admit_relocation_plan};
    use crate::{
        ObjectFileLayout, ObjectPlan, RelocationKind, RelocationOrigin, RelocationPlan,
        RelocationRecord, SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection,
    };
    use arena::{Arena, Handle};
    use target::NativeTarget;

    fn object(target: NativeTarget, section_sizes: &[(SectionKind, usize)]) -> ObjectPlan {
        let mut sections = Arena::<SectionPlan>::new();
        for (kind, size) in section_sizes {
            sections.insert(SectionPlan {
                kind: *kind,
                size: *size,
                alignment: 1,
            });
        }
        ObjectPlan::with_layout(
            target,
            ObjectFileLayout::with_roots(
                sections,
                Arena::new(),
                Arena::new(),
                Vec::new(),
                Handle::invalid(),
            ),
        )
    }

    fn symbol(
        object: &mut ObjectPlan,
        kind: SymbolKind,
        section: SymbolSection,
    ) -> Handle<SymbolPlan> {
        object.layout.symbols.insert(SymbolPlan {
            name: "s".to_string(),
            section,
            offset: 0,
            size: 0,
            kind,
            import_library: String::new(),
        })
    }

    fn record(
        section: SectionKind,
        offset: usize,
        symbol_handle: Handle<SymbolPlan>,
        kind: RelocationKind,
    ) -> RelocationRecord {
        RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: symbol_handle,
            },
            section,
            offset,
            byte_width: kind.field_width(),
            symbol_handle,
            addend: 0,
            kind,
        }
    }

    #[test]
    fn empty_plan_admits_with_zero_counts() {
        let object = object(
            NativeTarget::linux_x64(),
            &[(SectionKind::Text, 16), (SectionKind::Data, 16)],
        );
        let admitted = admit_relocation_plan(
            RelocationPlan::with_target(NativeTarget::linux_x64()),
            &object,
        )
        .expect("empty plan admits");
        assert_eq!(admitted.record_count(), 0);
        assert_eq!(admitted.records_in_section(SectionKind::Text), 0);
    }

    #[test]
    fn target_mismatch_refuses_and_retains_the_plan() {
        let object = object(NativeTarget::linux_x64(), &[(SectionKind::Text, 16)]);
        let plan = RelocationPlan::with_target(NativeTarget::linux_arm64());
        let failure = admit_relocation_plan(plan, &object).unwrap_err();
        assert_eq!(*failure.error(), RelocationAdmissionError::TargetMismatch);
        let (plan, _) = failure.into_parts();
        assert_eq!(plan.target, NativeTarget::linux_arm64());
    }

    #[test]
    fn kind_outside_target_vocabulary_refuses() {
        let mut object = object(NativeTarget::linux_x64(), &[(SectionKind::Data, 16)]);
        let symbol = symbol(
            &mut object,
            SymbolKind::Object,
            SymbolSection::Section(SectionKind::Data),
        );
        let mut plan = RelocationPlan::with_target(NativeTarget::linux_x64());
        plan.push_record(record(
            SectionKind::Data,
            0,
            symbol,
            RelocationKind::Aarch64Branch26,
        ));
        let failure = admit_relocation_plan(plan, &object).unwrap_err();
        assert_eq!(
            *failure.error(),
            RelocationAdmissionError::KindOutsideTargetVocabulary {
                kind: RelocationKind::Aarch64Branch26
            }
        );
    }

    #[test]
    fn aarch64_record_admits_on_aarch64_object() {
        let mut object = object(NativeTarget::linux_arm64(), &[(SectionKind::Text, 16)]);
        let function = symbol(
            &mut object,
            SymbolKind::Function,
            SymbolSection::Section(SectionKind::Text),
        );
        let mut plan = RelocationPlan::with_target(NativeTarget::linux_arm64());
        let mut row = record(
            SectionKind::Text,
            4,
            function,
            RelocationKind::Aarch64Branch26,
        );
        row.origin = RelocationOrigin::Instruction {
            function_symbol_handle: function,
            selected_instruction_index: 1,
        };
        plan.push_record(row);
        let admitted = admit_relocation_plan(plan, &object).expect("admits");
        assert_eq!(admitted.records_in_section(SectionKind::Text), 1);
        assert_eq!(
            admitted
                .section_record_counts()
                .collect::<Vec<(SectionKind, usize)>>(),
            vec![(SectionKind::Text, 1)]
        );
    }

    #[test]
    fn wrong_width_and_misalignment_refuse() {
        let mut object = object(NativeTarget::linux_x64(), &[(SectionKind::Data, 32)]);
        let symbol = symbol(
            &mut object,
            SymbolKind::Object,
            SymbolSection::Section(SectionKind::Data),
        );

        let mut narrow = RelocationPlan::with_target(NativeTarget::linux_x64());
        let mut row = record(SectionKind::Data, 0, symbol, RelocationKind::Absolute64);
        row.byte_width = 4;
        narrow.push_record(row);
        assert_eq!(
            *admit_relocation_plan(narrow, &object).unwrap_err().error(),
            RelocationAdmissionError::WrongFieldWidth {
                kind: RelocationKind::Absolute64,
                byte_width: 4
            }
        );

        let mut skewed = RelocationPlan::with_target(NativeTarget::linux_x64());
        skewed.push_record(record(
            SectionKind::Data,
            4,
            symbol,
            RelocationKind::Absolute64,
        ));
        assert_eq!(
            *admit_relocation_plan(skewed, &object).unwrap_err().error(),
            RelocationAdmissionError::MisalignedField {
                kind: RelocationKind::Absolute64,
                offset: 4
            }
        );
    }

    #[test]
    fn missing_unmaterialized_and_out_of_bounds_sections_refuse() {
        let mut object = object(
            NativeTarget::linux_x64(),
            &[(SectionKind::Data, 8), (SectionKind::Bss, 8)],
        );
        let symbol = symbol(
            &mut object,
            SymbolKind::Object,
            SymbolSection::Section(SectionKind::Data),
        );

        let mut text_record = RelocationPlan::with_target(NativeTarget::linux_x64());
        text_record.push_record(record(
            SectionKind::Text,
            0,
            symbol,
            RelocationKind::Absolute64,
        ));
        assert_eq!(
            *admit_relocation_plan(text_record, &object)
                .unwrap_err()
                .error(),
            RelocationAdmissionError::MissingSection {
                section: SectionKind::Text
            }
        );

        let mut bss_record = RelocationPlan::with_target(NativeTarget::linux_x64());
        bss_record.push_record(record(
            SectionKind::Bss,
            0,
            symbol,
            RelocationKind::Absolute64,
        ));
        assert_eq!(
            *admit_relocation_plan(bss_record, &object)
                .unwrap_err()
                .error(),
            RelocationAdmissionError::UnmaterializedSection {
                section: SectionKind::Bss
            }
        );

        let mut past_end = RelocationPlan::with_target(NativeTarget::linux_x64());
        past_end.push_record(record(
            SectionKind::Data,
            8,
            symbol,
            RelocationKind::Absolute64,
        ));
        assert_eq!(
            *admit_relocation_plan(past_end, &object)
                .unwrap_err()
                .error(),
            RelocationAdmissionError::FieldOutsideSection {
                section: SectionKind::Data,
                offset: 8,
                byte_width: 8
            }
        );
    }

    #[test]
    fn unresolved_symbol_and_origin_custody_refuse() {
        let mut object = object(
            NativeTarget::linux_x64(),
            &[(SectionKind::Data, 16), (SectionKind::Text, 16)],
        );
        let object_symbol = symbol(
            &mut object,
            SymbolKind::Object,
            SymbolSection::Section(SectionKind::Data),
        );
        let import_symbol = symbol(&mut object, SymbolKind::Import, SymbolSection::None);
        let data_symbol = symbol(
            &mut object,
            SymbolKind::Object,
            SymbolSection::Section(SectionKind::Data),
        );

        let mut dead_symbol = RelocationPlan::with_target(NativeTarget::linux_x64());
        let mut row = record(
            SectionKind::Data,
            0,
            data_symbol,
            RelocationKind::Absolute64,
        );
        row.symbol_handle = Handle::invalid();
        dead_symbol.push_record(row);
        assert_eq!(
            *admit_relocation_plan(dead_symbol, &object)
                .unwrap_err()
                .error(),
            RelocationAdmissionError::UnknownSymbol
        );

        let mut non_function_origin = RelocationPlan::with_target(NativeTarget::linux_x64());
        let mut row = record(
            SectionKind::Text,
            0,
            data_symbol,
            RelocationKind::X86_64Relative32,
        );
        row.origin = RelocationOrigin::Instruction {
            function_symbol_handle: object_symbol,
            selected_instruction_index: 0,
        };
        non_function_origin.push_record(row);
        assert_eq!(
            *admit_relocation_plan(non_function_origin, &object)
                .unwrap_err()
                .error(),
            RelocationAdmissionError::OriginSymbolNotFunction
        );

        let mut import_origin = RelocationPlan::with_target(NativeTarget::linux_x64());
        let mut row = record(
            SectionKind::Data,
            8,
            data_symbol,
            RelocationKind::Absolute64,
        );
        row.origin = RelocationOrigin::Materialization {
            object_symbol_handle: import_symbol,
        };
        import_origin.push_record(row);
        assert_eq!(
            *admit_relocation_plan(import_origin, &object)
                .unwrap_err()
                .error(),
            RelocationAdmissionError::MaterializationOriginIsImport
        );
    }

    #[test]
    fn overlapping_windows_refuse_and_disjoint_windows_admit() {
        let mut object = object(NativeTarget::linux_x64(), &[(SectionKind::Data, 32)]);
        let symbol = symbol(
            &mut object,
            SymbolKind::Object,
            SymbolSection::Section(SectionKind::Data),
        );

        let mut overlapping = RelocationPlan::with_target(NativeTarget::linux_x64());
        overlapping.push_record(record(
            SectionKind::Data,
            0,
            symbol,
            RelocationKind::Absolute64,
        ));
        overlapping.push_record(record(
            SectionKind::Data,
            8,
            symbol,
            RelocationKind::Absolute64,
        ));
        overlapping.push_record(record(
            SectionKind::Data,
            8,
            symbol,
            RelocationKind::Absolute64,
        ));
        assert_eq!(
            *admit_relocation_plan(overlapping, &object)
                .unwrap_err()
                .error(),
            RelocationAdmissionError::OverlappingRecords {
                section: SectionKind::Data,
                offset: 8
            }
        );

        let mut disjoint = RelocationPlan::with_target(NativeTarget::linux_x64());
        disjoint.push_record(record(
            SectionKind::Data,
            0,
            symbol,
            RelocationKind::Absolute64,
        ));
        disjoint.push_record(record(
            SectionKind::Data,
            8,
            symbol,
            RelocationKind::Absolute64,
        ));
        disjoint.push_record(record(
            SectionKind::Data,
            16,
            symbol,
            RelocationKind::Absolute64,
        ));
        let admitted = admit_relocation_plan(disjoint, &object).expect("admits");
        assert_eq!(admitted.records_in_section(SectionKind::Data), 3);
        let plan = admitted.into_plan();
        assert_eq!(plan.record_count(), 3);
    }

    #[test]
    fn field_metadata_matches_the_closed_vocabulary() {
        assert_eq!(RelocationKind::Absolute64.field_width(), 8);
        assert_eq!(RelocationKind::Absolute64.field_alignment(), 8);
        assert_eq!(RelocationKind::X86_64Relative32.field_width(), 4);
        assert_eq!(RelocationKind::X86_64Relative32.field_alignment(), 1);
        assert_eq!(RelocationKind::Aarch64Branch26.field_width(), 4);
        assert_eq!(RelocationKind::Aarch64Branch26.field_alignment(), 4);
        assert!(RelocationKind::Absolute64.admitted_on(target::Architecture::X86_64));
        assert!(RelocationKind::Absolute64.admitted_on(target::Architecture::Aarch64));
        assert!(!RelocationKind::Aarch64Page21.admitted_on(target::Architecture::X86_64));
        assert!(!RelocationKind::X86_64Relative32.admitted_on(target::Architecture::Aarch64));
        assert_eq!(SectionKindOrdinal(0).section_kind(), SectionKind::Text);
    }
}
