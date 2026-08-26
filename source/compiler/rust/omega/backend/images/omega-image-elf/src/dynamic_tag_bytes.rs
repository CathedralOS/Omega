//! Canonical ELF64-LSB serialization of the semantic `.dynamic` table.
//!
//! The primary System V ABI defines [`Elf64_Dyn`] as one signed eight-byte tag
//! followed by an eight-byte value/address union, while the generic ELF [data
//! encoding] defines least-significant-byte-first serialization. This module
//! preserves all seven address fields as exact zero placeholders with typed
//! byte-coordinate fixups; it assigns no address or section index.
//!
//! [`Elf64_Dyn`]: https://gabi.xinuos.com/elf/08-dynamic.html#dynamic-section
//! [data encoding]: https://gabi.xinuos.com/elf/02-eheader.html#data-encoding

use crate::bytes::write_u64;
use crate::dynamic_tags::{
    ElfDynamicAddressTarget, ElfDynamicTagContents, ElfDynamicValue, ValidatedElfDynamicTagPlan,
};
use psi_diagnostics::Diagnostic;

const ELF64_DYNAMIC_ROW_SIZE: usize = 16;
const ELF64_DYNAMIC_VALUE_OFFSET: usize = 8;
const ELF64_DYNAMIC_VALUE_SIZE: u8 = 8;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Independently decoded and replayed ELF64-LSB `.dynamic` bytes.
///
/// The exact semantic tag plan remains owned by this non-clone carrier. Every
/// address union is still zero and named by a typed fixup; these bytes have no
/// descriptor, placement, program header, image mutation, or runnable-image
/// authority.
#[derive(Debug)]
#[must_use = "validated ELF dynamic bytes retain the exact semantic tag plan"]
pub struct ValidatedElfDynamicTablePayload {
    plan: ValidatedElfDynamicTagPlan,
    contents: ElfDynamicTablePayloadContents,
    payload_identity: u64,
}

impl ValidatedElfDynamicTablePayload {
    pub const fn plan(&self) -> &ValidatedElfDynamicTagPlan {
        &self.plan
    }

    pub fn row_count(&self) -> usize {
        self.plan.row_count()
    }

    pub fn byte_count(&self) -> usize {
        self.contents.bytes.len()
    }

    pub fn address_fixup_count(&self) -> usize {
        self.contents.address_fixups.len()
    }

    /// Compatibility fingerprint of the exact semantic tag identity,
    /// ELF64-LSB bytes, and typed byte-coordinate fixups. This is not final-
    /// byte, placement, loader, or publication identity.
    pub const fn payload_identity(&self) -> u64 {
        self.payload_identity
    }

    #[allow(dead_code)]
    pub(crate) const fn contents(&self) -> &ElfDynamicTablePayloadContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(self) -> (ValidatedElfDynamicTagPlan, ElfDynamicTablePayloadContents) {
        (self.plan, self.contents)
    }
}

/// Rejected `.dynamic` serialization with exact semantic-plan custody.
#[derive(Debug)]
#[must_use = "ELF dynamic serialization rejection retains the semantic plan"]
pub struct ElfDynamicTableSerializationError {
    plan: ValidatedElfDynamicTagPlan,
    diagnostic: Diagnostic,
}

impl ElfDynamicTableSerializationError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicTagPlan, Diagnostic) {
        (self.plan, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicTableSerializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicTableSerializationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfDynamicTablePayloadContents {
    pub(crate) bytes: Vec<u8>,
    pub(crate) address_fixups: Vec<ElfDynamicPayloadAddressFixup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfDynamicPayloadFixupKind {
    Elf64AbsoluteAddress = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfDynamicPayloadAddressFixup {
    pub(crate) row_ordinal: u32,
    pub(crate) byte_offset: usize,
    pub(crate) byte_width: u8,
    pub(crate) kind: ElfDynamicPayloadFixupKind,
    pub(crate) target: ElfDynamicAddressTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DecodedElfDynamicRow {
    tag: i64,
    value: u64,
}

struct Candidate {
    plan: ValidatedElfDynamicTagPlan,
    contents: ElfDynamicTablePayloadContents,
    payload_identity: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Serialize the exact semantic tag plan as ELF64-LSB `Elf64_Dyn` rows and
/// independently decode every row and address-fixup coordinate before sealing
/// success.
///
/// This does not append a `.dynamic` name, create its descriptor, assign a
/// section index/address, resolve a pointer, place bytes, or mutate the image.
pub fn serialize_elf_dynamic_table(
    plan: ValidatedElfDynamicTagPlan,
) -> Result<ValidatedElfDynamicTablePayload, Box<ElfDynamicTableSerializationError>> {
    let contents = match encode_contents(&plan) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicTableSerializationError {
                plan,
                diagnostic,
            }));
        }
    };
    let payload_identity = payload_identity(&plan, &contents);
    let candidate = Candidate {
        plan,
        contents,
        payload_identity,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfDynamicTableSerializationError {
            plan: error.candidate.plan,
            diagnostic: error.diagnostic,
        })),
    }
}

fn encode_contents(
    plan: &ValidatedElfDynamicTagPlan,
) -> Result<ElfDynamicTablePayloadContents, Diagnostic> {
    let semantic = plan.contents();
    let mut bytes = Vec::with_capacity(checked_product(
        semantic.rows.len(),
        ELF64_DYNAMIC_ROW_SIZE,
        "ELF64 dynamic payload size",
    )?);
    for row in &semantic.rows {
        write_u64(&mut bytes, (row.tag as i64) as u64);
        write_u64(&mut bytes, encoded_value(row.value));
    }
    let address_fixups = semantic
        .address_obligations
        .iter()
        .map(|obligation| {
            let row_offset = checked_product(
                obligation.row_ordinal as usize,
                ELF64_DYNAMIC_ROW_SIZE,
                "ELF64 dynamic fixup row offset",
            )?;
            Ok(ElfDynamicPayloadAddressFixup {
                row_ordinal: obligation.row_ordinal,
                byte_offset: checked_sum(
                    row_offset,
                    ELF64_DYNAMIC_VALUE_OFFSET,
                    "ELF64 dynamic fixup value offset",
                )?,
                byte_width: obligation.byte_width,
                kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                target: obligation.target,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(ElfDynamicTablePayloadContents {
        bytes,
        address_fixups,
    })
}

const fn encoded_value(value: ElfDynamicValue) -> u64 {
    match value {
        ElfDynamicValue::NeededStringOffset(offset) => offset as u64,
        ElfDynamicValue::ProcedureRelocationByteCount(count)
        | ElfDynamicValue::DynamicStringByteCount(count)
        | ElfDynamicValue::DynamicSymbolEntryByteCount(count)
        | ElfDynamicValue::VersionRequirementRecordCount(count) => count,
        ElfDynamicValue::RelocationTag(tag) => (tag as i64) as u64,
        ElfDynamicValue::AddressPlaceholder | ElfDynamicValue::Null => 0,
    }
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfDynamicTablePayload, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.plan, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.payload_identity != payload_identity(&candidate.plan, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error("ELF dynamic payload identity does not replay"),
        });
    }
    Ok(ValidatedElfDynamicTablePayload {
        plan: candidate.plan,
        contents: candidate.contents,
        payload_identity: candidate.payload_identity,
    })
}

fn validate_contents(
    plan: &ValidatedElfDynamicTagPlan,
    contents: &ElfDynamicTablePayloadContents,
) -> Result<(), Diagnostic> {
    let semantic = plan.contents();
    let decoded = decode_rows(&contents.bytes, semantic.rows.len())?;
    require(
        decoded.len() == semantic.rows.len(),
        "decoded Elf64_Dyn row count drifted from the semantic plan",
    )?;
    for (decoded, semantic) in decoded.iter().zip(&semantic.rows) {
        require(
            decoded.tag == semantic.tag as i64 && decoded.value == encoded_value(semantic.value),
            "decoded Elf64_Dyn tag or value drifted from the semantic row",
        )?;
    }
    validate_fixups(&contents.bytes, semantic, &contents.address_fixups)
}

fn decode_rows(
    bytes: &[u8],
    expected_count: usize,
) -> Result<Vec<DecodedElfDynamicRow>, Diagnostic> {
    let expected_size = checked_product(
        expected_count,
        ELF64_DYNAMIC_ROW_SIZE,
        "decoded Elf64_Dyn payload size",
    )?;
    require(
        bytes.len() == expected_size,
        "Elf64_Dyn payload has a truncated row or trailing bytes",
    )?;
    let mut rows = Vec::with_capacity(expected_count);
    for index in 0..expected_count {
        let offset = checked_product(index, ELF64_DYNAMIC_ROW_SIZE, "decoded Elf64_Dyn row")?;
        rows.push(DecodedElfDynamicRow {
            tag: read_i64(bytes, offset, "Elf64_Dyn.d_tag")?,
            value: read_u64(
                bytes,
                checked_sum(offset, ELF64_DYNAMIC_VALUE_OFFSET, "Elf64_Dyn.d_un offset")?,
                "Elf64_Dyn.d_un",
            )?,
        });
    }
    Ok(rows)
}

fn validate_fixups(
    bytes: &[u8],
    semantic: &ElfDynamicTagContents,
    fixups: &[ElfDynamicPayloadAddressFixup],
) -> Result<(), Diagnostic> {
    require(
        fixups.len() == semantic.address_obligations.len(),
        "serialized ELF dynamic address-fixup count is not exact",
    )?;
    for (fixup, obligation) in fixups.iter().zip(&semantic.address_obligations) {
        let row_offset = checked_product(
            obligation.row_ordinal as usize,
            ELF64_DYNAMIC_ROW_SIZE,
            "replayed Elf64_Dyn fixup row",
        )?;
        let expected_offset = checked_sum(
            row_offset,
            ELF64_DYNAMIC_VALUE_OFFSET,
            "replayed Elf64_Dyn fixup value",
        )?;
        require(
            fixup.row_ordinal == obligation.row_ordinal
                && fixup.byte_offset == expected_offset
                && fixup.byte_width == ELF64_DYNAMIC_VALUE_SIZE
                && fixup.byte_width == obligation.byte_width
                && fixup.kind == ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress
                && fixup.target == obligation.target,
            "serialized ELF dynamic address fixup drifted from its semantic obligation",
        )?;
        require(
            read_u64(bytes, fixup.byte_offset, "Elf64_Dyn address placeholder")? == 0,
            "serialized Elf64_Dyn address field is not an exact zero placeholder",
        )?;
    }
    for (index, row) in semantic.rows.iter().enumerate() {
        let fixup_count = fixups
            .iter()
            .filter(|fixup| fixup.row_ordinal as usize == index)
            .count();
        require(
            fixup_count == usize::from(matches!(row.value, ElfDynamicValue::AddressPlaceholder)),
            "serialized Elf64_Dyn row has a missing, duplicate, or orphan address fixup",
        )?;
    }
    validate_fixup_coverage(fixups)
}

fn validate_fixup_coverage(fixups: &[ElfDynamicPayloadAddressFixup]) -> Result<(), Diagnostic> {
    for (index, fixup) in fixups.iter().enumerate() {
        let end = checked_sum(
            fixup.byte_offset,
            usize::from(fixup.byte_width),
            "Elf64_Dyn fixup end",
        )?;
        for other in &fixups[index + 1..] {
            let other_end = checked_sum(
                other.byte_offset,
                usize::from(other.byte_width),
                "Elf64_Dyn fixup end",
            )?;
            require(
                end <= other.byte_offset || other_end <= fixup.byte_offset,
                "serialized Elf64_Dyn address fixups overlap or duplicate one field",
            )?;
        }
    }
    Ok(())
}

fn read_i64(bytes: &[u8], offset: usize, context: &'static str) -> Result<i64, Diagnostic> {
    let end = checked_sum(offset, 8, context)?;
    let value = bytes
        .get(offset..end)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(i64::from_le_bytes(value))
}

fn read_u64(bytes: &[u8], offset: usize, context: &'static str) -> Result<u64, Diagnostic> {
    let end = checked_sum(offset, 8, context)?;
    let value = bytes
        .get(offset..end)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(u64::from_le_bytes(value))
}

fn checked_product(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn checked_sum(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

fn payload_identity(
    plan: &ValidatedElfDynamicTagPlan,
    contents: &ElfDynamicTablePayloadContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-dynamic-table-payload.v1");
    hash.bytes(&plan.tag_identity().to_le_bytes());
    hash.bytes(&contents.bytes);
    hash.bytes(&(contents.address_fixups.len() as u64).to_le_bytes());
    for fixup in &contents.address_fixups {
        hash.bytes(&fixup.row_ordinal.to_le_bytes());
        hash.bytes(&(fixup.byte_offset as u64).to_le_bytes());
        hash.byte(fixup.byte_width);
        hash.byte(fixup.kind as u8);
        hash.byte(fixup.target as u8);
    }
    hash.finish()
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            self.byte(byte);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors,
        plan_elf_dynamic_sections, plan_elf_dynamic_tags, plan_elf_procedure_linkage_relocations,
        plan_elf_procedure_linkage_section_descriptors, plan_elf_procedure_linkage_templates,
        serialize_elf_dynamic_sections,
    };
    use omega_image::{
        FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSection, FinalImageSymbol,
    };
    use omega_object_file::{RelocationKind, SymbolKind};
    use omega_target::{
        ForeignLocatorCandidate, TargetProfile, normalize_elf_interpreter_plan,
        normalize_foreign_locator,
    };
    use psi_arena::Handle;

    #[derive(Clone, Copy)]
    struct ImportFixture {
        object: &'static [u8],
        symbol: &'static [u8],
        version: &'static [u8],
        instruction_offsets: &'static [usize],
    }

    const FIRST_SITES: &[usize] = &[0, 32];
    const SECOND_SITES: &[usize] = &[16];
    const IMPORTS: [ImportFixture; 2] = [
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"alpha\xfe",
            version: b"V1\xfd",
            instruction_offsets: FIRST_SITES,
        },
        ImportFixture {
            object: b"libb.so",
            symbol: b"beta",
            version: b"V2",
            instruction_offsets: SECOND_SITES,
        },
    ];

    fn interpreter_path(target: TargetProfile) -> &'static [u8] {
        match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
            _ => unreachable!("dynamic-byte fixture uses a Linux target"),
        }
    }

    fn tag_plan(target: TargetProfile, imports: &[ImportFixture]) -> ValidatedElfDynamicTagPlan {
        let relocation_count = imports
            .iter()
            .map(|fixture| fixture.instruction_offsets.len())
            .sum();
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 64],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            imports.len(),
            imports.len(),
            relocation_count,
        );
        for (index, fixture) in imports.iter().enumerate() {
            let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: format!("__omega_dynamic_byte_import_{index}"),
                section: FinalImageSection::None,
                offset: 0,
                size: 0,
                kind: SymbolKind::Import,
            });
            image.symbol_table.imports.insert(FinalImageImport {
                symbol_handle,
                import: FinalImageImportPlan::Normalized(
                    normalize_foreign_locator(
                        ForeignLocatorCandidate::ElfVersioned {
                            object: fixture.object.to_vec(),
                            symbol: fixture.symbol.to_vec(),
                            version: fixture.version.to_vec(),
                        },
                        target,
                    )
                    .expect("valid dynamic-byte locator"),
                ),
            });
            for instruction_offset in fixture.instruction_offsets {
                let (relocation_offset, kind) = match target {
                    TargetProfile::LinuxX64 => {
                        image.memory.text[*instruction_offset] = 0xe8;
                        (instruction_offset + 1, RelocationKind::X86_64Relative32)
                    }
                    TargetProfile::LinuxArm64 => {
                        image.memory.text[*instruction_offset..*instruction_offset + 4]
                            .copy_from_slice(&[0, 0, 0, 0x94]);
                        (*instruction_offset, RelocationKind::Aarch64Branch26)
                    }
                    _ => unreachable!(),
                };
                image
                    .relocation_table
                    .relocations
                    .insert(FinalImageRelocation {
                        section: FinalImageSection::Text,
                        offset: relocation_offset,
                        byte_width: 4,
                        symbol_handle,
                        addend: 0,
                        kind,
                    });
            }
        }
        let interpreter = normalize_elf_interpreter_plan(interpreter_path(target).to_vec(), target)
            .expect("valid dynamic-byte interpreter");
        let inputs =
            plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link inputs");
        let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
        let payloads = serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
        let base = plan_elf_dynamic_section_descriptors(payloads).expect("valid base descriptors");
        let linkage =
            plan_elf_procedure_linkage_relocations(base).expect("valid procedure linkage");
        let templates =
            plan_elf_procedure_linkage_templates(linkage).expect("valid linkage templates");
        let descriptors = plan_elf_procedure_linkage_section_descriptors(templates)
            .expect("valid linkage descriptors");
        plan_elf_dynamic_tags(descriptors).expect("valid semantic dynamic tags")
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let plan = tag_plan(target, &IMPORTS);
        let contents = encode_contents(&plan).expect("encoded dynamic bytes");
        let payload_identity = payload_identity(&plan, &contents);
        Candidate {
            plan,
            contents,
            payload_identity,
        }
    }

    fn row(bytes: &[u8], ordinal: usize) -> &[u8] {
        let start = ordinal * ELF64_DYNAMIC_ROW_SIZE;
        &bytes[start..start + ELF64_DYNAMIC_ROW_SIZE]
    }

    #[test]
    fn both_targets_serialize_exact_elf64_lsb_rows_and_fixup_coordinates() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let payload = serialize_elf_dynamic_table(tag_plan(target, &IMPORTS))
                .expect("validated dynamic payload");
            assert_eq!(payload.row_count(), 15);
            assert_eq!(payload.byte_count(), 240);
            assert_eq!(payload.address_fixup_count(), 7);
            assert_ne!(payload.payload_identity(), 0);

            let bytes = &payload.contents.bytes;
            let first_needed = match payload.plan.contents().rows[0].value {
                ElfDynamicValue::NeededStringOffset(offset) => offset,
                _ => panic!("first row must be DT_NEEDED"),
            };
            assert_eq!(&row(bytes, 0)[..8], &[1, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(
                &row(bytes, 0)[8..],
                &(u64::from(first_needed)).to_le_bytes(),
            );
            assert_eq!(&row(bytes, 9)[..8], &[20, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(&row(bytes, 9)[8..], &[7, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(&row(bytes, 11)[..8], &[0xf0, 0xff, 0xff, 0x6f, 0, 0, 0, 0],);
            assert_eq!(row(bytes, 14), &[0; 16]);
            assert_eq!(
                payload.contents.address_fixups,
                [
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 3,
                        byte_offset: 56,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::ProcedureGot,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 4,
                        byte_offset: 72,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::SystemVHash,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 5,
                        byte_offset: 88,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::DynamicString,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 6,
                        byte_offset: 104,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::DynamicSymbol,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 10,
                        byte_offset: 168,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::ProcedureRelocation,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 11,
                        byte_offset: 184,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::GnuSymbolVersion,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 12,
                        byte_offset: 200,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::GnuVersionRequirement,
                    },
                ],
            );
            validate_contents(payload.plan(), &payload.contents)
                .expect("independent dynamic-byte replay");
        }
    }

    #[test]
    fn encoded_needed_offsets_still_select_exact_raw_non_utf8_names() {
        let payload =
            serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
        let dynstr = &payload
            .plan
            .descriptors()
            .templates()
            .linkage()
            .descriptors()
            .payloads()
            .payloads()
            .dynstr;
        let names = (0..payload.plan.needed_row_count())
            .map(|ordinal| {
                let offset = read_u64(
                    &payload.contents.bytes,
                    ordinal * ELF64_DYNAMIC_ROW_SIZE + ELF64_DYNAMIC_VALUE_OFFSET,
                    "DT_NEEDED offset",
                )
                .unwrap() as usize;
                let tail = &dynstr[offset..];
                &tail[..tail.iter().position(|byte| *byte == 0).unwrap()]
            })
            .collect::<Vec<_>>();
        assert_eq!(names, [b"liba\xff.so".as_slice(), b"libb.so".as_slice()]);
    }

    #[test]
    fn import_permutation_preserves_bytes_while_target_remains_identity_bound() {
        let forward =
            serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
        let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
        let reverse =
            serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxX64, &reverse_imports))
                .unwrap();
        let arm =
            serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxArm64, &IMPORTS)).unwrap();
        assert_eq!(forward.contents, reverse.contents);
        assert_eq!(forward.payload_identity(), reverse.payload_identity());
        assert_eq!(forward.contents.bytes, arm.contents.bytes);
        assert_ne!(forward.payload_identity(), arm.payload_identity());
    }

    #[test]
    fn independent_decoder_rejects_every_tag_value_placeholder_and_boundary_corruption() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| candidate.contents.bytes[0] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[2 * 16 + 8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[3 * 16 + 8] = 1),
            Box::new(|candidate| candidate.contents.bytes[7 * 16 + 8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[8 * 16 + 8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[9 * 16 + 8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[13 * 16 + 8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[14 * 16] = 1),
            Box::new(|candidate| candidate.contents.bytes[14 * 16 + 8] = 1),
            Box::new(|candidate| candidate.contents.bytes[11 * 16..11 * 16 + 8].reverse()),
            Box::new(|candidate| {
                candidate.contents.bytes.pop();
            }),
            Box::new(|candidate| candidate.contents.bytes.push(0)),
            Box::new(|candidate| candidate.payload_identity ^= 1),
        ];
        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxX64);
            let expected_identity = candidate.plan.tag_identity();
            corrupt(&mut candidate);
            let error =
                validate_candidate(candidate).expect_err("corrupt Elf64_Dyn payload must reject");
            assert_eq!(
                error.candidate.plan.tag_identity(),
                expected_identity,
                "dynamic-byte rejection retains exact semantic-plan custody",
            );
        }
    }

    #[test]
    fn independent_replay_rejects_every_fixup_coordinate_kind_and_target_corruption() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| {
                candidate.contents.address_fixups.pop();
            }),
            Box::new(|candidate| {
                candidate
                    .contents
                    .address_fixups
                    .push(candidate.contents.address_fixups[0])
            }),
            Box::new(|candidate| candidate.contents.address_fixups.swap(0, 1)),
            Box::new(|candidate| candidate.contents.address_fixups[0].row_ordinal = u32::MAX),
            Box::new(|candidate| candidate.contents.address_fixups[0].byte_offset += 1),
            Box::new(|candidate| candidate.contents.address_fixups[0].byte_width = 4),
            Box::new(|candidate| {
                candidate.contents.address_fixups[0].target = ElfDynamicAddressTarget::DynamicString
            }),
        ];
        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxArm64);
            let expected_identity = candidate.plan.tag_identity();
            corrupt(&mut candidate);
            let error = validate_candidate(candidate)
                .expect_err("corrupt Elf64_Dyn address fixup must reject");
            assert_eq!(error.candidate.plan.tag_identity(), expected_identity);
        }
    }

    #[test]
    fn decoder_and_fixup_checks_reject_overflow_and_malformed_inputs_without_panicking() {
        assert!(checked_product(usize::MAX, 16, "product").is_err());
        assert!(checked_sum(usize::MAX, 8, "sum").is_err());
        assert!(decode_rows(&[], usize::MAX).is_err());
        assert!(decode_rows(&[0; 15], 1).is_err());
        assert!(decode_rows(&[0; 17], 1).is_err());
        assert!(read_i64(&[], usize::MAX, "tag").is_err());
        assert!(read_u64(&[0; 7], 0, "value").is_err());

        let overflow = ElfDynamicPayloadAddressFixup {
            row_ordinal: u32::MAX,
            byte_offset: usize::MAX,
            byte_width: 8,
            kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
            target: ElfDynamicAddressTarget::ProcedureGot,
        };
        assert!(validate_fixup_coverage(&[overflow, overflow]).is_err());
    }
}
