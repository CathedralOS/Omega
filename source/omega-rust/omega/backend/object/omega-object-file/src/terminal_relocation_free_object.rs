use std::collections::BTreeSet;

use crate::{SectionKind, section_name};
use omega_optimization_core::{
    OptimizationSelectionIdentity, TerminalRelocationFreeObjectContainerIdentity,
    TerminalRelocationFreeObjectPlanIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

const PLAN_SCHEMA: &[u8] = b"omega.terminal.relocation-free-object-plan.v1\0";
const CONTAINER_MAGIC: &[u8; 8] = b"OMGTRO\0\0";
const CONTAINER_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TerminalObjectLocalSymbolId(u64);

impl TerminalObjectLocalSymbolId {
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalRelocationFreeObjectSymbolPolicy {
    PrivateSemanticMachineSymbolsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalRelocationFreeObjectSymbolLinkage {
    ObjectLocalV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalRelocationFreeObjectSymbolRole {
    SemanticEntryV1,
    PrivateFunctionV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalRelocationFreeObjectRelocationRequirements {
    ProvenNoneForFullyResolvedInternalControlV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRelocationFreeObjectTextSection {
    pub name: String,
    pub alignment: u64,
    pub byte_count: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRelocationFreeFunctionSymbol {
    pub symbol: TerminalObjectLocalSymbolId,
    pub source_function_index: u64,
    pub machine: MachineId,
    pub name: String,
    pub section_offset: u64,
    pub byte_count: u64,
    pub linkage: TerminalRelocationFreeObjectSymbolLinkage,
    pub role: TerminalRelocationFreeObjectSymbolRole,
}

/// Clean object-owned representation of one fully resolved optimizer text section.
///
/// This value deliberately owns no native-image, installation, process-entry, export, or
/// publication authority. Its semantic entry remains an object-local function symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRelocationFreeObjectPlan {
    pub identity: TerminalRelocationFreeObjectPlanIdentity,
    pub source_text_section: TerminalRelocationFreeTextSectionIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub text_section: TerminalRelocationFreeObjectTextSection,
    pub symbol_policy: TerminalRelocationFreeObjectSymbolPolicy,
    pub symbols: Vec<TerminalRelocationFreeFunctionSymbol>,
    pub semantic_entry: MachineId,
    pub semantic_entry_symbol: TerminalObjectLocalSymbolId,
    pub relocation_record_count: u64,
    pub relocation_requirements: TerminalRelocationFreeObjectRelocationRequirements,
}

impl TerminalRelocationFreeObjectPlan {
    pub fn recomputed_identity(
        &self,
    ) -> Result<TerminalRelocationFreeObjectPlanIdentity, TerminalRelocationFreeObjectError> {
        let mut canonical = PLAN_SCHEMA.to_vec();
        canonical.extend_from_slice(&encode_plan_content(self)?);
        Ok(TerminalRelocationFreeObjectPlanIdentity::from_canonical_bytes(&canonical))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRelocationFreeObjectContainer {
    pub identity: TerminalRelocationFreeObjectContainerIdentity,
    pub object: TerminalRelocationFreeObjectPlanIdentity,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalRelocationFreeObjectError {
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
pub enum TerminalRelocationFreeObjectDecodeError {
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
    InvalidObject(TerminalRelocationFreeObjectError),
}

pub fn canonical_terminal_private_machine_symbol_name(machine: MachineId) -> String {
    format!("__omega_terminal_machine_{}", machine.get())
}

pub fn validate_terminal_relocation_free_object(
    object: &TerminalRelocationFreeObjectPlan,
) -> Result<(), TerminalRelocationFreeObjectError> {
    if object.recomputed_identity()? != object.identity {
        return Err(TerminalRelocationFreeObjectError::StaleObjectIdentity);
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
        return Err(TerminalRelocationFreeObjectError::NonCanonicalTarget);
    }
    if object.text_section.name != section_name(object.target, SectionKind::Text) {
        return Err(TerminalRelocationFreeObjectError::WrongTextSectionName);
    }
    let expected_alignment = match object.target.architecture {
        Architecture::Aarch64 => 4,
        Architecture::X86_64 => 1,
    };
    if object.text_section.alignment != expected_alignment {
        return Err(TerminalRelocationFreeObjectError::WrongTextSectionAlignment);
    }
    if u64::try_from(object.text_section.bytes.len())
        .map_err(|_| TerminalRelocationFreeObjectError::LengthOverflow)?
        != object.text_section.byte_count
    {
        return Err(TerminalRelocationFreeObjectError::TextSectionLengthMismatch);
    }
    if object.symbols.is_empty() {
        return Err(TerminalRelocationFreeObjectError::EmptySymbolTable);
    }
    if object.relocation_record_count != 0 {
        return Err(TerminalRelocationFreeObjectError::RelocationsPresent);
    }

    let mut machines = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut cursor = 0_u64;
    let mut entry_count = 0_u64;
    for (index, symbol) in object.symbols.iter().enumerate() {
        let ordinal = u64::try_from(index)
            .map_err(|_| TerminalRelocationFreeObjectError::LengthOverflow)?
            .checked_add(1)
            .ok_or(TerminalRelocationFreeObjectError::LengthOverflow)?;
        if symbol.symbol.get() != ordinal {
            return Err(TerminalRelocationFreeObjectError::NonCanonicalSymbolId);
        }
        if symbol.source_function_index != ordinal - 1 {
            return Err(TerminalRelocationFreeObjectError::NonCanonicalSourceFunctionIndex);
        }
        if !machines.insert(symbol.machine) {
            return Err(TerminalRelocationFreeObjectError::DuplicateMachine);
        }
        if !names.insert(symbol.name.as_str()) {
            return Err(TerminalRelocationFreeObjectError::DuplicateSymbolName);
        }
        if symbol.name != canonical_terminal_private_machine_symbol_name(symbol.machine) {
            return Err(TerminalRelocationFreeObjectError::NonCanonicalSymbolName);
        }
        if symbol.name == "main" || symbol.name == "_main" {
            return Err(TerminalRelocationFreeObjectError::ReservedProcessEntryName);
        }
        if symbol.section_offset != cursor {
            return Err(TerminalRelocationFreeObjectError::NonDenseSymbolInterval);
        }
        cursor = cursor
            .checked_add(symbol.byte_count)
            .ok_or(TerminalRelocationFreeObjectError::SymbolIntervalOverflow)?;
        if cursor > object.text_section.byte_count {
            return Err(TerminalRelocationFreeObjectError::SymbolOutsideTextSection);
        }
        let is_entry = symbol.machine == object.semantic_entry;
        if is_entry {
            entry_count = entry_count
                .checked_add(1)
                .ok_or(TerminalRelocationFreeObjectError::LengthOverflow)?;
        }
        let expected_role = if is_entry {
            TerminalRelocationFreeObjectSymbolRole::SemanticEntryV1
        } else {
            TerminalRelocationFreeObjectSymbolRole::PrivateFunctionV1
        };
        if symbol.role != expected_role {
            return Err(TerminalRelocationFreeObjectError::WrongSemanticEntryRole);
        }
        if is_entry && symbol.symbol != object.semantic_entry_symbol {
            return Err(TerminalRelocationFreeObjectError::WrongSemanticEntrySymbol);
        }
    }
    if cursor != object.text_section.byte_count {
        return Err(TerminalRelocationFreeObjectError::NonDenseSymbolInterval);
    }
    match entry_count {
        0 => Err(TerminalRelocationFreeObjectError::MissingSemanticEntry),
        1 => Ok(()),
        _ => Err(TerminalRelocationFreeObjectError::MultipleSemanticEntries),
    }
}

pub fn encode_terminal_relocation_free_object(
    object: &TerminalRelocationFreeObjectPlan,
) -> Result<TerminalRelocationFreeObjectContainer, TerminalRelocationFreeObjectError> {
    validate_terminal_relocation_free_object(object)?;
    let content = encode_plan_content(object)?;
    let mut bytes = Vec::with_capacity(44_usize.saturating_add(content.len()));
    bytes.extend_from_slice(CONTAINER_MAGIC);
    bytes.extend_from_slice(&CONTAINER_VERSION.to_le_bytes());
    bytes.extend_from_slice(&object.identity.bytes());
    bytes.extend_from_slice(&content);
    Ok(TerminalRelocationFreeObjectContainer {
        identity: TerminalRelocationFreeObjectContainerIdentity::from_canonical_bytes(&bytes),
        object: object.identity,
        bytes,
    })
}

pub fn decode_terminal_relocation_free_object(
    encoded: &[u8],
) -> Result<TerminalRelocationFreeObjectPlan, TerminalRelocationFreeObjectDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != CONTAINER_MAGIC {
        return Err(TerminalRelocationFreeObjectDecodeError::WrongMagic);
    }
    let version = u32::from_le_bytes(cursor.array()?);
    if version != CONTAINER_VERSION {
        return Err(TerminalRelocationFreeObjectDecodeError::UnsupportedVersion(
            version,
        ));
    }
    let identity = TerminalRelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
    let object = decode_plan_content(&mut cursor, identity)?;
    if cursor.remaining() != 0 {
        return Err(TerminalRelocationFreeObjectDecodeError::TrailingBytes);
    }
    validate_terminal_relocation_free_object(&object)
        .map_err(TerminalRelocationFreeObjectDecodeError::InvalidObject)?;
    Ok(object)
}

fn encode_plan_content(
    object: &TerminalRelocationFreeObjectPlan,
) -> Result<Vec<u8>, TerminalRelocationFreeObjectError> {
    let mut output = Vec::new();
    output.extend_from_slice(&object.source_text_section.bytes());
    output.extend_from_slice(&object.terminal_psi.vocabulary_marker.get().to_le_bytes());
    output.extend_from_slice(object.terminal_psi.program_fingerprint.as_bytes());
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
            .map_err(|_| TerminalRelocationFreeObjectError::LengthOverflow)?
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
            TerminalRelocationFreeObjectSymbolRole::SemanticEntryV1 => 1,
            TerminalRelocationFreeObjectSymbolRole::PrivateFunctionV1 => 2,
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
    identity: TerminalRelocationFreeObjectPlanIdentity,
) -> Result<TerminalRelocationFreeObjectPlan, TerminalRelocationFreeObjectDecodeError> {
    let source_text_section =
        TerminalRelocationFreeTextSectionIdentity::from_bytes(cursor.array()?);
    let marker = u16::from_le_bytes(cursor.array()?);
    let vocabulary_marker = VocabularyMarker::new(marker).ok_or(
        TerminalRelocationFreeObjectDecodeError::InvalidVocabulary(marker),
    )?;
    let terminal_psi = TerminalPsiIdentity {
        vocabulary_marker,
        program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
    };
    let fuel = u32::from_le_bytes(cursor.array()?);
    let fuel_schedule = FuelScheduleIdentity::new(fuel)
        .ok_or(TerminalRelocationFreeObjectDecodeError::InvalidFuelSchedule)?;
    let selected = TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let target = decode_target(cursor)?;
    let text_section = TerminalRelocationFreeObjectTextSection {
        name: cursor.string()?,
        alignment: u64::from_le_bytes(cursor.array()?),
        byte_count: u64::from_le_bytes(cursor.array()?),
        bytes: cursor.bytes()?,
    };
    let symbol_policy_tag = cursor.byte()?;
    if symbol_policy_tag != 1 {
        return Err(
            TerminalRelocationFreeObjectDecodeError::UnknownSymbolPolicy(symbol_policy_tag),
        );
    }
    let symbol_count = cursor.length()?;
    let mut symbols = Vec::with_capacity(symbol_count);
    for _ in 0..symbol_count {
        let symbol = TerminalObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
            .ok_or(TerminalRelocationFreeObjectDecodeError::InvalidSymbolId)?;
        let source_function_index = u64::from_le_bytes(cursor.array()?);
        let machine = MachineId::new(u64::from_le_bytes(cursor.array()?))
            .ok_or(TerminalRelocationFreeObjectDecodeError::InvalidMachine)?;
        let name = cursor.string()?;
        let section_offset = u64::from_le_bytes(cursor.array()?);
        let byte_count = u64::from_le_bytes(cursor.array()?);
        let linkage_tag = cursor.byte()?;
        if linkage_tag != 1 {
            return Err(TerminalRelocationFreeObjectDecodeError::UnknownLinkage(
                linkage_tag,
            ));
        }
        let role = match cursor.byte()? {
            1 => TerminalRelocationFreeObjectSymbolRole::SemanticEntryV1,
            2 => TerminalRelocationFreeObjectSymbolRole::PrivateFunctionV1,
            tag => {
                return Err(TerminalRelocationFreeObjectDecodeError::UnknownSymbolRole(
                    tag,
                ));
            }
        };
        symbols.push(TerminalRelocationFreeFunctionSymbol {
            symbol,
            source_function_index,
            machine,
            name,
            section_offset,
            byte_count,
            linkage: TerminalRelocationFreeObjectSymbolLinkage::ObjectLocalV1,
            role,
        });
    }
    let semantic_entry = MachineId::new(u64::from_le_bytes(cursor.array()?))
        .ok_or(TerminalRelocationFreeObjectDecodeError::InvalidMachine)?;
    let semantic_entry_symbol =
        TerminalObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
            .ok_or(TerminalRelocationFreeObjectDecodeError::InvalidSymbolId)?;
    let relocation_record_count = u64::from_le_bytes(cursor.array()?);
    let relocation_tag = cursor.byte()?;
    if relocation_tag != 1 {
        return Err(
            TerminalRelocationFreeObjectDecodeError::UnknownRelocationRequirements(relocation_tag),
        );
    }
    Ok(TerminalRelocationFreeObjectPlan {
        identity,
        source_text_section,
        terminal_psi,
        fuel_schedule,
        selected,
        selections,
        target,
        text_section,
        symbol_policy: TerminalRelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
        symbols,
        semantic_entry,
        semantic_entry_symbol,
        relocation_record_count,
        relocation_requirements:
            TerminalRelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
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

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, TerminalRelocationFreeObjectDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => return Err(TerminalRelocationFreeObjectDecodeError::UnknownTargetArchitecture(tag)),
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => return Err(TerminalRelocationFreeObjectDecodeError::UnknownObjectFormat(tag)),
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| TerminalRelocationFreeObjectDecodeError::LengthOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| TerminalRelocationFreeObjectDecodeError::LengthOverflow)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

fn encode_string(
    output: &mut Vec<u8>,
    value: &str,
) -> Result<(), TerminalRelocationFreeObjectError> {
    encode_bytes(output, value.as_bytes())
}

fn encode_bytes(
    output: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), TerminalRelocationFreeObjectError> {
    output.extend_from_slice(
        &u64::try_from(value.len())
            .map_err(|_| TerminalRelocationFreeObjectError::LengthOverflow)?
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

    fn take(&mut self, length: usize) -> Result<&'a [u8], TerminalRelocationFreeObjectDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(TerminalRelocationFreeObjectDecodeError::LengthOverflow)?;
        let value = self
            .encoded
            .get(self.position..end)
            .ok_or(TerminalRelocationFreeObjectDecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], TerminalRelocationFreeObjectDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| TerminalRelocationFreeObjectDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, TerminalRelocationFreeObjectDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn length(&mut self) -> Result<usize, TerminalRelocationFreeObjectDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| TerminalRelocationFreeObjectDecodeError::LengthOverflow)
    }

    fn bytes(&mut self) -> Result<Vec<u8>, TerminalRelocationFreeObjectDecodeError> {
        let length = self.length()?;
        Ok(self.take(length)?.to_vec())
    }

    fn string(&mut self) -> Result<String, TerminalRelocationFreeObjectDecodeError> {
        String::from_utf8(self.bytes()?)
            .map_err(|_| TerminalRelocationFreeObjectDecodeError::InvalidUtf8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> TerminalRelocationFreeObjectPlan {
        let machine = MachineId::new(7).unwrap();
        let mut plan = TerminalRelocationFreeObjectPlan {
            identity: TerminalRelocationFreeObjectPlanIdentity::from_canonical_bytes(b"pending"),
            source_text_section: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                b"text",
            ),
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([4; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            selected: TerminalSelectedInstructionPlanIdentity::from_bytes([5; 32]),
            selections: OptimizationSelectionIdentity::from_bytes([6; 32]),
            target: NativeTarget::linux_arm64(),
            text_section: TerminalRelocationFreeObjectTextSection {
                name: ".text".to_owned(),
                alignment: 4,
                byte_count: 4,
                bytes: vec![0x20, 0, 0, 0xb5],
            },
            symbol_policy:
                TerminalRelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
            symbols: vec![TerminalRelocationFreeFunctionSymbol {
                symbol: TerminalObjectLocalSymbolId::new(1).unwrap(),
                source_function_index: 0,
                machine,
                name: canonical_terminal_private_machine_symbol_name(machine),
                section_offset: 0,
                byte_count: 4,
                linkage: TerminalRelocationFreeObjectSymbolLinkage::ObjectLocalV1,
                role: TerminalRelocationFreeObjectSymbolRole::SemanticEntryV1,
            }],
            semantic_entry: machine,
            semantic_entry_symbol: TerminalObjectLocalSymbolId::new(1).unwrap(),
            relocation_record_count: 0,
            relocation_requirements:
                TerminalRelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
        };
        plan.identity = plan.recomputed_identity().unwrap();
        plan
    }

    #[test]
    fn clean_object_container_round_trips_and_binds_exact_bytes() {
        let object = plan();
        let container = encode_terminal_relocation_free_object(&object).unwrap();
        assert_eq!(
            decode_terminal_relocation_free_object(&container.bytes),
            Ok(object.clone())
        );
        assert_eq!(container.object, object.identity);
        assert_eq!(
            container.identity,
            TerminalRelocationFreeObjectContainerIdentity::from_canonical_bytes(&container.bytes)
        );
    }

    #[test]
    fn object_validation_rejects_process_name_relocation_and_interval_corruption() {
        let mut object = plan();
        object.symbols[0].name = "main".to_owned();
        object.identity = object.recomputed_identity().unwrap();
        assert_eq!(
            validate_terminal_relocation_free_object(&object),
            Err(TerminalRelocationFreeObjectError::NonCanonicalSymbolName)
        );

        let mut object = plan();
        object.relocation_record_count = 1;
        object.identity = object.recomputed_identity().unwrap();
        assert_eq!(
            validate_terminal_relocation_free_object(&object),
            Err(TerminalRelocationFreeObjectError::RelocationsPresent)
        );

        let mut object = plan();
        object.symbols[0].section_offset = 1;
        object.identity = object.recomputed_identity().unwrap();
        assert_eq!(
            validate_terminal_relocation_free_object(&object),
            Err(TerminalRelocationFreeObjectError::NonDenseSymbolInterval)
        );

        let mut object = plan();
        object.target.pointer_size = 4;
        object.identity = object.recomputed_identity().unwrap();
        assert_eq!(
            validate_terminal_relocation_free_object(&object),
            Err(TerminalRelocationFreeObjectError::NonCanonicalTarget)
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
        object.semantic_entry_symbol = TerminalObjectLocalSymbolId::new(2).unwrap();
        object.symbols = vec![
            TerminalRelocationFreeFunctionSymbol {
                symbol: TerminalObjectLocalSymbolId::new(1).unwrap(),
                source_function_index: 0,
                machine: first_machine,
                name: canonical_terminal_private_machine_symbol_name(first_machine),
                section_offset: 0,
                byte_count: 4,
                linkage: TerminalRelocationFreeObjectSymbolLinkage::ObjectLocalV1,
                role: TerminalRelocationFreeObjectSymbolRole::PrivateFunctionV1,
            },
            TerminalRelocationFreeFunctionSymbol {
                symbol: TerminalObjectLocalSymbolId::new(2).unwrap(),
                source_function_index: 1,
                machine: entry_machine,
                name: canonical_terminal_private_machine_symbol_name(entry_machine),
                section_offset: 4,
                byte_count: 4,
                linkage: TerminalRelocationFreeObjectSymbolLinkage::ObjectLocalV1,
                role: TerminalRelocationFreeObjectSymbolRole::SemanticEntryV1,
            },
        ];
        object.identity = object.recomputed_identity().unwrap();

        let container = encode_terminal_relocation_free_object(&object).unwrap();
        assert_eq!(
            decode_terminal_relocation_free_object(&container.bytes),
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
            validate_terminal_relocation_free_object(&object),
            Err(TerminalRelocationFreeObjectError::NonCanonicalSymbolId)
        );
    }

    #[test]
    fn decoder_rejects_corrupt_envelope_and_stale_identity() {
        let container = encode_terminal_relocation_free_object(&plan()).unwrap();
        let mut wrong_magic = container.bytes.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            decode_terminal_relocation_free_object(&wrong_magic),
            Err(TerminalRelocationFreeObjectDecodeError::WrongMagic)
        );
        let mut trailing = container.bytes.clone();
        trailing.push(0);
        assert_eq!(
            decode_terminal_relocation_free_object(&trailing),
            Err(TerminalRelocationFreeObjectDecodeError::TrailingBytes)
        );
        let mut stale = container.bytes;
        stale[12] ^= 1;
        assert_eq!(
            decode_terminal_relocation_free_object(&stale),
            Err(TerminalRelocationFreeObjectDecodeError::InvalidObject(
                TerminalRelocationFreeObjectError::StaleObjectIdentity
            ))
        );
    }
}
