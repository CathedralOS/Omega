//! The OMGTRO container and the four-of-six target admission that decides whether
//! a relocation-free plan is canonical at all.

use std::collections::BTreeSet;

mod publication;
mod text_section;
pub use publication::*;
pub use text_section::*;

use crate::{SectionKind, section_name};
use optimization_core::{
    OptimizationSelectionIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::{Architecture, NativeTarget, ObjectFormat};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

const PLAN_SCHEMA: &[u8] = b"omega.terminal.relocation-free-object-plan.v1\0";
const CONTAINER_MAGIC: &[u8; 8] = b"OMGTRO\0\0";
const CONTAINER_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectLocalSymbolId(u64);

impl ObjectLocalSymbolId {
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationFreeObjectSymbolPolicy {
    PrivateSemanticMachineSymbolsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationFreeObjectSymbolLinkage {
    ObjectLocalV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationFreeObjectSymbolRole {
    SemanticEntryV1,
    PrivateFunctionV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationFreeObjectRelocationRequirements {
    ProvenNoneForFullyResolvedInternalControlV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationFreeObjectTextSection {
    pub name: String,
    pub alignment: u64,
    pub byte_count: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationFreeFunctionSymbol {
    pub symbol: ObjectLocalSymbolId,
    pub source_function_index: u64,
    pub machine: MachineId,
    pub name: String,
    pub section_offset: u64,
    pub byte_count: u64,
    pub linkage: RelocationFreeObjectSymbolLinkage,
    pub role: RelocationFreeObjectSymbolRole,
}

/// Clean object-owned representation of one fully resolved optimizer text section.
///
/// This value deliberately owns no native-image, installation, process-entry, export, or
/// publication authority. Its semantic entry remains an object-local function symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationFreeObjectPlan {
    pub identity: RelocationFreeObjectPlanIdentity,
    pub source_text_section: TerminalRelocationFreeTextSectionIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub text_section: RelocationFreeObjectTextSection,
    pub symbol_policy: RelocationFreeObjectSymbolPolicy,
    pub symbols: Vec<RelocationFreeFunctionSymbol>,
    pub semantic_entry: MachineId,
    pub semantic_entry_symbol: ObjectLocalSymbolId,
    pub relocation_record_count: u64,
    pub relocation_requirements: RelocationFreeObjectRelocationRequirements,
}

impl RelocationFreeObjectPlan {
    pub fn recomputed_identity(
        &self,
    ) -> Result<RelocationFreeObjectPlanIdentity, RelocationFreeObjectError> {
        let mut canonical = PLAN_SCHEMA.to_vec();
        canonical.extend_from_slice(&encode_plan_content(self)?);
        Ok(RelocationFreeObjectPlanIdentity::from_canonical_bytes(
            &canonical,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationFreeObjectContainer {
    pub identity: RelocationFreeObjectContainerIdentity,
    pub object: RelocationFreeObjectPlanIdentity,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeObjectError {
    StaleObjectIdentity,
    NonCanonicalTarget,
    WrongTextSectionName,
    WrongTextSectionAlignment,
    TextSectionLengthMismatch,
    EmptySymbolTable,
    NonCanonicalSymbolId,
    NonCanonicalSourceFunctionIndex,
    DuplicateMachine,
    DuplicateSymbolName,
    NonCanonicalSymbolName,
    ReservedProcessEntryName,
    SymbolIntervalOverflow,
    NonDenseSymbolInterval,
    SymbolOutsideTextSection,
    WrongSemanticEntryRole,
    MissingSemanticEntry,
    MultipleSemanticEntries,
    WrongSemanticEntrySymbol,
    RelocationsPresent,
    LengthOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeObjectDecodeError {
    WrongMagic,
    UnsupportedVersion(u32),
    Truncated,
    TrailingBytes,
    InvalidUtf8,
    InvalidVocabulary(u16),
    InvalidFuelSchedule,
    InvalidMachine,
    InvalidSymbolId,
    UnknownTargetArchitecture(u8),
    UnknownObjectFormat(u8),
    UnknownSymbolPolicy(u8),
    UnknownLinkage(u8),
    UnknownSymbolRole(u8),
    UnknownRelocationRequirements(u8),
    LengthOverflow,
    InvalidObject(RelocationFreeObjectError),
}

pub fn canonical_private_machine_symbol_name(machine: MachineId) -> String {
    format!("__omega_terminal_machine_{}", machine.get())
}

pub fn validate_relocation_free_object(
    object: &RelocationFreeObjectPlan,
) -> Result<(), RelocationFreeObjectError> {
    if object.recomputed_identity()? != object.identity {
        return Err(RelocationFreeObjectError::StaleObjectIdentity);
    }
    if !matches!(
        object.target,
        NativeTarget {
            architecture: Architecture::Aarch64,
            object_format: ObjectFormat::Elf | ObjectFormat::MachO,
            pointer_size: 8,
            pointer_alignment: 8,
        } | NativeTarget {
            architecture: Architecture::X86_64,
            object_format: ObjectFormat::Elf | ObjectFormat::Coff,
            pointer_size: 8,
            pointer_alignment: 8,
        }
    ) {
        return Err(RelocationFreeObjectError::NonCanonicalTarget);
    }
    if object.text_section.name != section_name(object.target, SectionKind::Text) {
        return Err(RelocationFreeObjectError::WrongTextSectionName);
    }
    let expected_alignment = match object.target.architecture {
        Architecture::Aarch64 => 4,
        Architecture::X86_64 => 1,
    };
    if object.text_section.alignment != expected_alignment {
        return Err(RelocationFreeObjectError::WrongTextSectionAlignment);
    }
    if u64::try_from(object.text_section.bytes.len())
        .map_err(|_| RelocationFreeObjectError::LengthOverflow)?
        != object.text_section.byte_count
    {
        return Err(RelocationFreeObjectError::TextSectionLengthMismatch);
    }
    if object.symbols.is_empty() {
        return Err(RelocationFreeObjectError::EmptySymbolTable);
    }
    if object.relocation_record_count != 0 {
        return Err(RelocationFreeObjectError::RelocationsPresent);
    }

    let mut machines = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut cursor = 0_u64;
    let mut entry_count = 0_u64;
    for (index, symbol) in object.symbols.iter().enumerate() {
        let ordinal = u64::try_from(index)
            .map_err(|_| RelocationFreeObjectError::LengthOverflow)?
            .checked_add(1)
            .ok_or(RelocationFreeObjectError::LengthOverflow)?;
        if symbol.symbol.get() != ordinal {
            return Err(RelocationFreeObjectError::NonCanonicalSymbolId);
        }
        if symbol.source_function_index != ordinal - 1 {
            return Err(RelocationFreeObjectError::NonCanonicalSourceFunctionIndex);
        }
        if !machines.insert(symbol.machine) {
            return Err(RelocationFreeObjectError::DuplicateMachine);
        }
        if !names.insert(symbol.name.as_str()) {
            return Err(RelocationFreeObjectError::DuplicateSymbolName);
        }
        if symbol.name != canonical_private_machine_symbol_name(symbol.machine) {
            return Err(RelocationFreeObjectError::NonCanonicalSymbolName);
        }
        if symbol.name == "main" || symbol.name == "_main" {
            return Err(RelocationFreeObjectError::ReservedProcessEntryName);
        }
        if symbol.section_offset != cursor {
            return Err(RelocationFreeObjectError::NonDenseSymbolInterval);
        }
        cursor = cursor
            .checked_add(symbol.byte_count)
            .ok_or(RelocationFreeObjectError::SymbolIntervalOverflow)?;
        if cursor > object.text_section.byte_count {
            return Err(RelocationFreeObjectError::SymbolOutsideTextSection);
        }
        let is_entry = symbol.machine == object.semantic_entry;
        if is_entry {
            entry_count = entry_count
                .checked_add(1)
                .ok_or(RelocationFreeObjectError::LengthOverflow)?;
        }
        let expected_role = if is_entry {
            RelocationFreeObjectSymbolRole::SemanticEntryV1
        } else {
            RelocationFreeObjectSymbolRole::PrivateFunctionV1
        };
        if symbol.role != expected_role {
            return Err(RelocationFreeObjectError::WrongSemanticEntryRole);
        }
        if is_entry && symbol.symbol != object.semantic_entry_symbol {
            return Err(RelocationFreeObjectError::WrongSemanticEntrySymbol);
        }
    }
    if cursor != object.text_section.byte_count {
        return Err(RelocationFreeObjectError::NonDenseSymbolInterval);
    }
    match entry_count {
        0 => Err(RelocationFreeObjectError::MissingSemanticEntry),
        1 => Ok(()),
        _ => Err(RelocationFreeObjectError::MultipleSemanticEntries),
    }
}

pub fn encode_relocation_free_object(
    object: &RelocationFreeObjectPlan,
) -> Result<RelocationFreeObjectContainer, RelocationFreeObjectError> {
    validate_relocation_free_object(object)?;
    let content = encode_plan_content(object)?;
    let mut bytes = Vec::with_capacity(44_usize.saturating_add(content.len()));
    bytes.extend_from_slice(CONTAINER_MAGIC);
    bytes.extend_from_slice(&CONTAINER_VERSION.to_le_bytes());
    bytes.extend_from_slice(&object.identity.bytes());
    bytes.extend_from_slice(&content);
    Ok(RelocationFreeObjectContainer {
        identity: RelocationFreeObjectContainerIdentity::from_canonical_bytes(&bytes),
        object: object.identity,
        bytes,
    })
}

pub fn decode_relocation_free_object(
    encoded: &[u8],
) -> Result<RelocationFreeObjectPlan, RelocationFreeObjectDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != CONTAINER_MAGIC {
        return Err(RelocationFreeObjectDecodeError::WrongMagic);
    }
    let version = u32::from_le_bytes(cursor.array()?);
    if version != CONTAINER_VERSION {
        return Err(RelocationFreeObjectDecodeError::UnsupportedVersion(version));
    }
    let identity = RelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
    let object = decode_plan_content(&mut cursor, identity)?;
    if cursor.remaining() != 0 {
        return Err(RelocationFreeObjectDecodeError::TrailingBytes);
    }
    validate_relocation_free_object(&object)
        .map_err(RelocationFreeObjectDecodeError::InvalidObject)?;
    Ok(object)
}

fn encode_plan_content(
    object: &RelocationFreeObjectPlan,
) -> Result<Vec<u8>, RelocationFreeObjectError> {
    let mut output = Vec::new();
    output.extend_from_slice(&object.source_text_section.bytes());
    output.extend_from_slice(&object.psi.vocabulary_marker.get().to_le_bytes());
    output.extend_from_slice(object.psi.program_fingerprint.as_bytes());
    output.extend_from_slice(&object.fuel_schedule.marker().to_le_bytes());
    output.extend_from_slice(&object.selected.bytes());
    output.extend_from_slice(&object.selections.bytes());
    encode_target(&mut output, object.target);
    encode_string(&mut output, &object.text_section.name)?;
    output.extend_from_slice(&object.text_section.alignment.to_le_bytes());
    output.extend_from_slice(&object.text_section.byte_count.to_le_bytes());
    encode_bytes(&mut output, &object.text_section.bytes)?;
    output.push(1);
    output.extend_from_slice(
        &u64::try_from(object.symbols.len())
            .map_err(|_| RelocationFreeObjectError::LengthOverflow)?
            .to_le_bytes(),
    );
    for symbol in &object.symbols {
        output.extend_from_slice(&symbol.symbol.get().to_le_bytes());
        output.extend_from_slice(&symbol.source_function_index.to_le_bytes());
        output.extend_from_slice(&symbol.machine.get().to_le_bytes());
        encode_string(&mut output, &symbol.name)?;
        output.extend_from_slice(&symbol.section_offset.to_le_bytes());
        output.extend_from_slice(&symbol.byte_count.to_le_bytes());
        output.push(1);
        output.push(match symbol.role {
            RelocationFreeObjectSymbolRole::SemanticEntryV1 => 1,
            RelocationFreeObjectSymbolRole::PrivateFunctionV1 => 2,
        });
    }
    output.extend_from_slice(&object.semantic_entry.get().to_le_bytes());
    output.extend_from_slice(&object.semantic_entry_symbol.get().to_le_bytes());
    output.extend_from_slice(&object.relocation_record_count.to_le_bytes());
    output.push(1);
    Ok(output)
}

fn decode_plan_content(
    cursor: &mut Cursor<'_>,
    identity: RelocationFreeObjectPlanIdentity,
) -> Result<RelocationFreeObjectPlan, RelocationFreeObjectDecodeError> {
    let source_text_section =
        TerminalRelocationFreeTextSectionIdentity::from_bytes(cursor.array()?);
    let marker = u16::from_le_bytes(cursor.array()?);
    let vocabulary_marker = VocabularyMarker::new(marker)
        .ok_or(RelocationFreeObjectDecodeError::InvalidVocabulary(marker))?;
    let psi = TerminalPsiIdentity {
        vocabulary_marker,
        program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
    };
    let fuel = u32::from_le_bytes(cursor.array()?);
    let fuel_schedule = FuelScheduleIdentity::new(fuel)
        .ok_or(RelocationFreeObjectDecodeError::InvalidFuelSchedule)?;
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let target = decode_target(cursor)?;
    let text_section = RelocationFreeObjectTextSection {
        name: cursor.string()?,
        alignment: u64::from_le_bytes(cursor.array()?),
        byte_count: u64::from_le_bytes(cursor.array()?),
        bytes: cursor.bytes()?,
    };
    let symbol_policy_tag = cursor.byte()?;
    if symbol_policy_tag != 1 {
        return Err(RelocationFreeObjectDecodeError::UnknownSymbolPolicy(
            symbol_policy_tag,
        ));
    }
    let symbol_count = cursor.length()?;
    let mut symbols = Vec::with_capacity(symbol_count);
    for _ in 0..symbol_count {
        let symbol = ObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
            .ok_or(RelocationFreeObjectDecodeError::InvalidSymbolId)?;
        let source_function_index = u64::from_le_bytes(cursor.array()?);
        let machine = MachineId::new(u64::from_le_bytes(cursor.array()?))
            .ok_or(RelocationFreeObjectDecodeError::InvalidMachine)?;
        let name = cursor.string()?;
        let section_offset = u64::from_le_bytes(cursor.array()?);
        let byte_count = u64::from_le_bytes(cursor.array()?);
        let linkage_tag = cursor.byte()?;
        if linkage_tag != 1 {
            return Err(RelocationFreeObjectDecodeError::UnknownLinkage(linkage_tag));
        }
        let role = match cursor.byte()? {
            1 => RelocationFreeObjectSymbolRole::SemanticEntryV1,
            2 => RelocationFreeObjectSymbolRole::PrivateFunctionV1,
            tag => {
                return Err(RelocationFreeObjectDecodeError::UnknownSymbolRole(tag));
            }
        };
        symbols.push(RelocationFreeFunctionSymbol {
            symbol,
            source_function_index,
            machine,
            name,
            section_offset,
            byte_count,
            linkage: RelocationFreeObjectSymbolLinkage::ObjectLocalV1,
            role,
        });
    }
    let semantic_entry = MachineId::new(u64::from_le_bytes(cursor.array()?))
        .ok_or(RelocationFreeObjectDecodeError::InvalidMachine)?;
    let semantic_entry_symbol = ObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
        .ok_or(RelocationFreeObjectDecodeError::InvalidSymbolId)?;
    let relocation_record_count = u64::from_le_bytes(cursor.array()?);
    let relocation_tag = cursor.byte()?;
    if relocation_tag != 1 {
        return Err(RelocationFreeObjectDecodeError::UnknownRelocationRequirements(relocation_tag));
    }
    Ok(RelocationFreeObjectPlan {
        identity,
        source_text_section,
        psi,
        fuel_schedule,
        selected,
        selections,
        target,
        text_section,
        symbol_policy: RelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
        symbols,
        semantic_entry,
        semantic_entry_symbol,
        relocation_record_count,
        relocation_requirements:
            RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    })
}

fn encode_target(output: &mut Vec<u8>, target: NativeTarget) {
    output.push(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    output.push(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    output.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    output.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}

fn decode_target(cursor: &mut Cursor<'_>) -> Result<NativeTarget, RelocationFreeObjectDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(RelocationFreeObjectDecodeError::UnknownTargetArchitecture(
                tag,
            ));
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => return Err(RelocationFreeObjectDecodeError::UnknownObjectFormat(tag)),
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| RelocationFreeObjectDecodeError::LengthOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| RelocationFreeObjectDecodeError::LengthOverflow)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

fn encode_string(output: &mut Vec<u8>, value: &str) -> Result<(), RelocationFreeObjectError> {
    encode_bytes(output, value.as_bytes())
}

fn encode_bytes(output: &mut Vec<u8>, value: &[u8]) -> Result<(), RelocationFreeObjectError> {
    output.extend_from_slice(
        &u64::try_from(value.len())
            .map_err(|_| RelocationFreeObjectError::LengthOverflow)?
            .to_le_bytes(),
    );
    output.extend_from_slice(value);
    Ok(())
}

struct Cursor<'a> {
    encoded: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    const fn new(encoded: &'a [u8]) -> Self {
        Self {
            encoded,
            position: 0,
        }
    }

    fn remaining(&self) -> usize {
        self.encoded.len().saturating_sub(self.position)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], RelocationFreeObjectDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(RelocationFreeObjectDecodeError::LengthOverflow)?;
        let value = self
            .encoded
            .get(self.position..end)
            .ok_or(RelocationFreeObjectDecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RelocationFreeObjectDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| RelocationFreeObjectDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, RelocationFreeObjectDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn length(&mut self) -> Result<usize, RelocationFreeObjectDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| RelocationFreeObjectDecodeError::LengthOverflow)
    }

    fn bytes(&mut self) -> Result<Vec<u8>, RelocationFreeObjectDecodeError> {
        let length = self.length()?;
        Ok(self.take(length)?.to_vec())
    }

    fn string(&mut self) -> Result<String, RelocationFreeObjectDecodeError> {
        String::from_utf8(self.bytes()?).map_err(|_| RelocationFreeObjectDecodeError::InvalidUtf8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> RelocationFreeObjectPlan {
        let machine = MachineId::new(7).unwrap();
        let mut plan = RelocationFreeObjectPlan {
            identity: RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"pending"),
            source_text_section: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                b"text",
            ),
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([4; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            selected: SelectedInstructionPlanIdentity::from_bytes([5; 32]),
            selections: OptimizationSelectionIdentity::from_bytes([6; 32]),
            target: NativeTarget::linux_arm64(),
            text_section: RelocationFreeObjectTextSection {
                name: ".text".to_owned(),
                alignment: 4,
                byte_count: 4,
                bytes: vec![0x20, 0, 0, 0xb5],
            },
            symbol_policy:
                RelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
            symbols: vec![RelocationFreeFunctionSymbol {
                symbol: ObjectLocalSymbolId::new(1).unwrap(),
                source_function_index: 0,
                machine,
                name: canonical_private_machine_symbol_name(machine),
                section_offset: 0,
                byte_count: 4,
                linkage: RelocationFreeObjectSymbolLinkage::ObjectLocalV1,
                role: RelocationFreeObjectSymbolRole::SemanticEntryV1,
            }],
            semantic_entry: machine,
            semantic_entry_symbol: ObjectLocalSymbolId::new(1).unwrap(),
            relocation_record_count: 0,
            relocation_requirements:
                RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
        };
        plan.identity = plan.recomputed_identity().unwrap();
        plan
    }

    #[test]
    fn clean_object_container_round_trips_and_binds_exact_bytes() {
        let object = plan();
        let container = encode_relocation_free_object(&object).unwrap();
        assert_eq!(
            decode_relocation_free_object(&container.bytes),
            Ok(object.clone())
        );
        assert_eq!(container.object, object.identity);
        assert_eq!(
            container.identity,
            RelocationFreeObjectContainerIdentity::from_canonical_bytes(&container.bytes)
        );
    }

    #[test]
    fn object_validation_rejects_process_name_relocation_and_interval_corruption() {
        let mut object = plan();
        object.symbols[0].name = "main".to_owned();
        object.identity = object.recomputed_identity().unwrap();
        assert_eq!(
            validate_relocation_free_object(&object),
            Err(RelocationFreeObjectError::NonCanonicalSymbolName)
        );

        let mut object = plan();
        object.relocation_record_count = 1;
        object.identity = object.recomputed_identity().unwrap();
        assert_eq!(
            validate_relocation_free_object(&object),
            Err(RelocationFreeObjectError::RelocationsPresent)
        );

        let mut object = plan();
        object.symbols[0].section_offset = 1;
        object.identity = object.recomputed_identity().unwrap();
        assert_eq!(
            validate_relocation_free_object(&object),
            Err(RelocationFreeObjectError::NonDenseSymbolInterval)
        );

        let mut object = plan();
        object.target.pointer_size = 4;
        object.identity = object.recomputed_identity().unwrap();
        assert_eq!(
            validate_relocation_free_object(&object),
            Err(RelocationFreeObjectError::NonCanonicalTarget)
        );
    }

    #[test]
    fn two_function_container_preserves_order_dense_intervals_and_nonzero_entry() {
        let mut object = plan();
        let first_machine = MachineId::new(7).unwrap();
        let entry_machine = MachineId::new(8).unwrap();
        object.text_section.byte_count = 8;
        object
            .text_section
            .bytes
            .extend_from_slice(&[0xc0, 0x03, 0x5f, 0xd6]);
        object.semantic_entry = entry_machine;
        object.semantic_entry_symbol = ObjectLocalSymbolId::new(2).unwrap();
        object.symbols = vec![
            RelocationFreeFunctionSymbol {
                symbol: ObjectLocalSymbolId::new(1).unwrap(),
                source_function_index: 0,
                machine: first_machine,
                name: canonical_private_machine_symbol_name(first_machine),
                section_offset: 0,
                byte_count: 4,
                linkage: RelocationFreeObjectSymbolLinkage::ObjectLocalV1,
                role: RelocationFreeObjectSymbolRole::PrivateFunctionV1,
            },
            RelocationFreeFunctionSymbol {
                symbol: ObjectLocalSymbolId::new(2).unwrap(),
                source_function_index: 1,
                machine: entry_machine,
                name: canonical_private_machine_symbol_name(entry_machine),
                section_offset: 4,
                byte_count: 4,
                linkage: RelocationFreeObjectSymbolLinkage::ObjectLocalV1,
                role: RelocationFreeObjectSymbolRole::SemanticEntryV1,
            },
        ];
        object.identity = object.recomputed_identity().unwrap();

        let container = encode_relocation_free_object(&object).unwrap();
        assert_eq!(
            decode_relocation_free_object(&container.bytes),
            Ok(object.clone())
        );
        assert_eq!(
            object
                .symbols
                .iter()
                .map(|symbol| (symbol.symbol.get(), symbol.source_function_index))
                .collect::<Vec<_>>(),
            vec![(1, 0), (2, 1)]
        );
        assert_eq!(object.symbols[0].section_offset, 0);
        assert_eq!(object.symbols[1].section_offset, 4);
        assert_ne!(object.symbols[0].name, object.symbols[1].name);
        assert_eq!(object.semantic_entry_symbol, object.symbols[1].symbol);

        object.symbols.swap(0, 1);
        object.identity = object.recomputed_identity().unwrap();
        assert_eq!(
            validate_relocation_free_object(&object),
            Err(RelocationFreeObjectError::NonCanonicalSymbolId)
        );
    }

    #[test]
    fn decoder_rejects_corrupt_envelope_and_stale_identity() {
        let container = encode_relocation_free_object(&plan()).unwrap();
        let mut wrong_magic = container.bytes.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            decode_relocation_free_object(&wrong_magic),
            Err(RelocationFreeObjectDecodeError::WrongMagic)
        );
        let mut trailing = container.bytes.clone();
        trailing.push(0);
        assert_eq!(
            decode_relocation_free_object(&trailing),
            Err(RelocationFreeObjectDecodeError::TrailingBytes)
        );
        let mut stale = container.bytes;
        stale[12] ^= 1;
        assert_eq!(
            decode_relocation_free_object(&stale),
            Err(RelocationFreeObjectDecodeError::InvalidObject(
                RelocationFreeObjectError::StaleObjectIdentity
            ))
        );
    }
}
